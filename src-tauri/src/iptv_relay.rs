//! Loopback relay for Live TV streams.
//!
//! The embedded player fetches streams with hls.js / mpegts.js from the
//! WebView, and most IPTV servers send no CORS headers, so those requests
//! would fail. This relay binds 127.0.0.1 on a random port with a random
//! per-launch token (the same pattern as player_stream.rs), fetches the stream
//! with reqwest and returns it with CORS headers.
//!
//! URLs keep their shape: `http://host:port/live/1.m3u8?x=1` becomes
//! `{base}/http/host:port/live/1.m3u8?x=1`. HLS playlists are rewritten so
//! every segment, key and variant URI is an absolute relay URL resolved
//! against the playlist's final (post-redirect) address. Everything else is
//! streamed through unchanged, which covers endless MPEG-TS live streams.
//!
//! Streams that need a User-Agent or Referer (M3U `#EXTVLCOPT`, a source's
//! settings) get a header set registered for this launch, and their relay URLs
//! carry it as `{base}/h/{id}/http/host/...`. Rewritten playlists keep the
//! prefix, so segments and keys go out with the same headers.

use axum::body::Body;
use axum::extract::{Path, RawQuery, State};
use axum::http::{header, HeaderMap, HeaderValue, Response, StatusCode, Uri};
use axum::routing::get;
use axum::Router;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::sync::OnceCell;

/// Playlists larger than this are not rewritten (real ones are a few KB).
const MAX_PLAYLIST_BYTES: usize = 8 * 1024 * 1024;

/// Reads a response body, stopping as soon as it grows past `max` bytes.
pub(crate) async fn read_body_capped(
    mut response: reqwest::Response,
    max: usize,
) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
        if body.len() + chunk.len() > max {
            return Err(format!("larger than {} MB", max / (1024 * 1024)));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

const PLAYLIST_TYPES: [&str; 4] = [
    "application/vnd.apple.mpegurl",
    "application/x-mpegurl",
    "audio/mpegurl",
    "audio/x-mpegurl",
];

/// Turns a stream URL into its relay form. Returns None for anything that is
/// not an http(s) URL with a host.
pub fn to_relay_url(relay_base: &str, url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url.trim()).ok()?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let host = parsed.host_str()?;
    // host_str already brackets IPv6 addresses.
    let authority = match parsed.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    };
    let mut relayed = format!(
        "{}/{scheme}/{authority}{}",
        relay_base.trim_end_matches('/'),
        parsed.path()
    );
    if let Some(query) = parsed.query() {
        relayed.push('?');
        relayed.push_str(query);
    }
    Some(relayed)
}

/// Rebuilds the upstream URL from the relay path (`{scheme}/{authority}/...`).
pub fn from_relay_path(rest: &str, query: Option<&str>) -> Option<String> {
    let (scheme, remainder) = rest.split_once('/')?;
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let (authority, path) = match remainder.split_once('/') {
        Some((authority, path)) => (authority, path),
        None => (remainder, ""),
    };
    if authority.is_empty() || authority.contains(['@', '\\', '%']) {
        return None;
    }
    let mut url = format!("{scheme}://{authority}/{path}");
    if let Some(query) = query.filter(|query| !query.is_empty()) {
        url.push('?');
        url.push_str(query);
    }
    // Round-trip through the parser so only well-formed URLs leave the relay.
    let parsed = reqwest::Url::parse(&url).ok()?;
    parsed.host_str()?;
    Some(parsed.to_string())
}

fn relay_uri(relay_base: &str, base: &reqwest::Url, uri: &str) -> String {
    match base.join(uri.trim()) {
        Ok(absolute) => {
            to_relay_url(relay_base, absolute.as_str()).unwrap_or_else(|| uri.to_string())
        }
        Err(_) => uri.to_string(),
    }
}

fn uri_attribute() -> &'static regex::Regex {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    PATTERN.get_or_init(|| regex::Regex::new(r#"URI="([^"]*)""#).expect("valid URI pattern"))
}

/// Points every URI in an HLS playlist at the relay.
pub fn rewrite_playlist(body: &str, base: &reqwest::Url, relay_base: &str) -> String {
    let mut out = String::with_capacity(body.len() + 256);
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            out.push_str(line);
        } else if trimmed.starts_with('#') {
            let rewritten = uri_attribute().replace_all(line, |caps: &regex::Captures| {
                format!("URI=\"{}\"", relay_uri(relay_base, base, &caps[1]))
            });
            out.push_str(&rewritten);
        } else {
            out.push_str(&relay_uri(relay_base, base, trimmed));
        }
        out.push('\n');
    }
    out
}

fn looks_like_playlist(content_type: Option<&str>, url: &reqwest::Url) -> bool {
    let by_type = content_type
        .map(|value| {
            value
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        })
        .is_some_and(|value| PLAYLIST_TYPES.contains(&value.as_str()));
    let path = url.path().to_ascii_lowercase();
    by_type || path.ends_with(".m3u8") || path.ends_with(".m3u")
}

/// Compares without stopping at the first differing byte.
fn same_token(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// Headers the relay sends upstream for one registered stream.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct StreamHeaders {
    pub user_agent: Option<String>,
    pub referrer: Option<String>,
}

impl StreamHeaders {
    pub fn is_empty(&self) -> bool {
        self.user_agent.is_none() && self.referrer.is_none()
    }
}

/// Header sets registered this launch, by id. Identical sets share an id.
#[derive(Default)]
pub struct HeaderRegistry {
    by_id: HashMap<String, StreamHeaders>,
    by_headers: HashMap<StreamHeaders, String>,
}

impl HeaderRegistry {
    pub fn register(&mut self, headers: StreamHeaders) -> String {
        if let Some(id) = self.by_headers.get(&headers) {
            return id.clone();
        }
        let id = (self.by_id.len() + 1).to_string();
        self.by_id.insert(id.clone(), headers.clone());
        self.by_headers.insert(headers, id.clone());
        id
    }

    pub fn get(&self, id: &str) -> Option<&StreamHeaders> {
        self.by_id.get(id)
    }
}

pub type SharedHeaders = Arc<Mutex<HeaderRegistry>>;

/// Splits an optional `h/{id}/` prefix off a relay path.
pub fn split_header_prefix(rest: &str) -> (Option<&str>, &str) {
    match rest
        .strip_prefix("h/")
        .and_then(|tail| tail.split_once('/'))
    {
        Some((id, tail)) => (Some(id), tail),
        None => (None, rest),
    }
}

struct RelayState {
    token: String,
    base: String,
    client: reqwest::Client,
    headers: SharedHeaders,
}

type HttpError = (StatusCode, String);

fn with_cors(mut response: Response<Body>) -> Response<Body> {
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("Content-Length, Content-Range, Content-Type, Accept-Ranges"),
    );
    response
}

async fn preflight() -> Response<Body> {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::NO_CONTENT;
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, HEAD, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Range, Content-Type"),
    );
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("600"),
    );
    with_cors(response)
}

async fn relay(
    State(state): State<Arc<RelayState>>,
    Path((token, _)): Path<(String, String)>,
    uri: Uri,
    RawQuery(query): RawQuery,
    request_headers: HeaderMap,
) -> Result<Response<Body>, HttpError> {
    if !same_token(&token, &state.token) {
        return Err((StatusCode::NOT_FOUND, "Not found".into()));
    }
    // Use the raw path: the Path extractor percent-decodes, which would turn an
    // encoded `/`, `?` or `#` inside Xtream credentials into real delimiters.
    let rest = uri
        .path()
        .strip_prefix(&format!("/{token}/"))
        .ok_or((StatusCode::NOT_FOUND, "Not found".to_string()))?;
    let (header_id, rest) = split_header_prefix(rest);
    let (stream_headers, playlist_base) = match header_id {
        Some(id) => {
            let headers = state
                .headers
                .lock()
                .map_err(|_| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Relay unavailable".to_string(),
                    )
                })?
                .get(id)
                .cloned()
                .ok_or((StatusCode::NOT_FOUND, "Not found".to_string()))?;
            (headers, format!("{}/h/{id}", state.base))
        }
        None => (StreamHeaders::default(), state.base.clone()),
    };
    let upstream = from_relay_path(rest, query.as_deref())
        .ok_or((StatusCode::BAD_REQUEST, "Not a stream address".to_string()))?;

    let mut request = state.client.get(&upstream);
    if let Some(range) = request_headers.get(header::RANGE) {
        request = request.header(header::RANGE, range.clone());
    }
    if let Some(agent) = stream_headers
        .user_agent
        .as_deref()
        .and_then(|value| HeaderValue::from_str(value).ok())
    {
        request = request.header(header::USER_AGENT, agent);
    }
    if let Some(referrer) = stream_headers
        .referrer
        .as_deref()
        .and_then(|value| HeaderValue::from_str(value).ok())
    {
        request = request.header(header::REFERER, referrer);
    }
    let upstream_response = request.send().await.map_err(|error| {
        (
            StatusCode::BAD_GATEWAY,
            format!("Stream unreachable: {error}"),
        )
    })?;

    let status = StatusCode::from_u16(upstream_response.status().as_u16())
        .unwrap_or(StatusCode::BAD_GATEWAY);
    let final_url = upstream_response.url().clone();
    let content_type = upstream_response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);

    if status.is_success() && looks_like_playlist(content_type.as_deref(), &final_url) {
        let bytes = read_body_capped(upstream_response, MAX_PLAYLIST_BYTES)
            .await
            .map_err(|error| {
                (
                    StatusCode::BAD_GATEWAY,
                    format!("Playlist read failed: {error}"),
                )
            })?;
        let text = String::from_utf8_lossy(&bytes);
        // Some servers label MPEG-TS as a playlist; only rewrite real ones.
        if text
            .trim_start_matches('\u{feff}')
            .trim_start()
            .starts_with("#EXTM3U")
        {
            let body = rewrite_playlist(&text, &final_url, &playlist_base);
            let mut response = Response::new(Body::from(body));
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/vnd.apple.mpegurl"),
            );
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            return Ok(with_cors(response));
        }
        let mut response = Response::new(Body::from(bytes));
        *response.status_mut() = status;
        if let Some(value) = content_type.and_then(|value| HeaderValue::from_str(&value).ok()) {
            response.headers_mut().insert(header::CONTENT_TYPE, value);
        }
        return Ok(with_cors(response));
    }

    let mut response = Response::builder().status(status);
    for name in [
        reqwest::header::CONTENT_TYPE,
        reqwest::header::CONTENT_LENGTH,
        reqwest::header::CONTENT_RANGE,
        reqwest::header::ACCEPT_RANGES,
    ] {
        if let Some(value) = upstream_response.headers().get(&name) {
            if let Ok(value) = HeaderValue::from_bytes(value.as_bytes()) {
                response = response.header(name.as_str(), value);
            }
        }
    }
    let body = Body::from_stream(upstream_response.bytes_stream());
    let response = response
        .body(body)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(with_cors(response))
}

fn relay_router(token: String, base: String, headers: SharedHeaders) -> Router {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        // Some providers set a session cookie on the playlist that segment and
        // key requests must send back.
        .cookie_store(true)
        .user_agent(concat!(
            "CinaVault-Premium/",
            env!("CARGO_PKG_VERSION"),
            " (Live TV)"
        ))
        .build()
        .unwrap_or_default();
    Router::new()
        .route("/{token}/{*rest}", get(relay).options(preflight))
        .with_state(Arc::new(RelayState {
            token,
            base,
            client,
            headers,
        }))
}

fn random_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

struct Relay {
    base: String,
    headers: SharedHeaders,
}

async fn bind_relay() -> Result<Relay, String> {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|error| format!("Live TV relay could not start: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    let token = random_token();
    let base = format!("http://127.0.0.1:{port}/{token}");
    let headers = SharedHeaders::default();
    let router = relay_router(token, base.clone(), headers.clone());
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, router).await {
            log::warn!("Live TV relay stopped: {error}");
        }
    });
    Ok(Relay { base, headers })
}

async fn running_relay() -> Result<&'static Relay, String> {
    static RELAY: OnceCell<Relay> = OnceCell::const_new();
    RELAY.get_or_try_init(bind_relay).await
}

/// The relay base for a header set: plain, or `{base}/h/{id}`.
fn base_for(
    base: &str,
    registry: &SharedHeaders,
    headers: StreamHeaders,
) -> Result<String, String> {
    if headers.is_empty() {
        return Ok(base.to_string());
    }
    let id = registry
        .lock()
        .map_err(|_| "The stream relay is unavailable".to_string())?
        .register(headers);
    Ok(format!("{base}/h/{id}"))
}

/// The URL the embedded player should load for a stream, with the
/// User-Agent / Referer the relay must send for it.
#[tauri::command]
pub async fn get_stream_relay_url(
    url: String,
    user_agent: Option<String>,
    referrer: Option<String>,
) -> Result<String, String> {
    let headers = StreamHeaders {
        user_agent: crate::iptv::clean_header_value("User-Agent", user_agent.as_deref())?,
        referrer: crate::iptv::clean_header_value("Referer", referrer.as_deref())?,
    };
    let relay = running_relay().await?;
    let base = base_for(&relay.base, &relay.headers, headers)?;
    to_relay_url(&base, &url)
        .ok_or_else(|| "Only http:// and https:// streams can be played".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "http://127.0.0.1:5000/tok";

    #[test]
    fn relay_urls_round_trip() {
        for url in [
            "http://provider.example:8080/live/user/pass/42.ts",
            "https://cdn.example/hls/master.m3u8?token=a%20b&x=1",
            "http://[::1]:9000/stream",
            "http://192.168.1.20/play/a.m3u8",
        ] {
            let relayed = to_relay_url(BASE, url).unwrap();
            let rest = relayed.strip_prefix(&format!("{BASE}/")).unwrap();
            let (path, query) = match rest.split_once('?') {
                Some((path, query)) => (path, Some(query)),
                None => (rest, None),
            };
            assert_eq!(
                from_relay_path(path, query).unwrap(),
                reqwest::Url::parse(url).unwrap().to_string()
            );
        }
        assert_eq!(
            to_relay_url(BASE, "http://provider.example:8080/live/1.ts").unwrap(),
            "http://127.0.0.1:5000/tok/http/provider.example:8080/live/1.ts"
        );
    }

    #[test]
    fn relay_refuses_other_schemes_and_userinfo() {
        assert!(to_relay_url(BASE, "file:///C:/Windows/win.ini").is_none());
        assert!(to_relay_url(BASE, "rtmp://example/live").is_none());
        assert!(from_relay_path("file/etc/passwd", None).is_none());
        assert!(from_relay_path("http/user@evil.example/x", None).is_none());
        assert!(from_relay_path("http/", None).is_none());
    }

    #[test]
    fn playlists_point_every_uri_at_the_relay() {
        let base = reqwest::Url::parse("http://cdn.example:8080/hls/chan/index.m3u8?t=1").unwrap();
        let body = "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\"\n#EXTINF:6.0,\nseg-1.ts?t=1\n\n#EXTINF:6.0,\nhttps://other.example/seg-2.ts\n#EXT-X-MAP:URI=\"/init.mp4\"\n";
        let out = rewrite_playlist(body, &base, BASE);
        assert!(
            out.contains(
                "URI=\"http://127.0.0.1:5000/tok/http/cdn.example:8080/hls/chan/key.bin\""
            ),
            "{out}"
        );
        assert!(
            out.contains(
                "\nhttp://127.0.0.1:5000/tok/http/cdn.example:8080/hls/chan/seg-1.ts?t=1\n"
            ),
            "{out}"
        );
        assert!(
            out.contains("\nhttp://127.0.0.1:5000/tok/https/other.example/seg-2.ts\n"),
            "{out}"
        );
        assert!(
            out.contains("URI=\"http://127.0.0.1:5000/tok/http/cdn.example:8080/init.mp4\""),
            "{out}"
        );
        assert!(out.starts_with("#EXTM3U\n#EXT-X-VERSION:3\n"));
    }

    #[test]
    fn detects_playlists_by_type_or_extension() {
        let ts = reqwest::Url::parse("http://a.example/live/1.ts").unwrap();
        let m3u8 = reqwest::Url::parse("http://a.example/live/1.m3u8").unwrap();
        assert!(looks_like_playlist(
            Some("application/vnd.apple.mpegurl; charset=utf-8"),
            &ts
        ));
        assert!(looks_like_playlist(None, &m3u8));
        assert!(!looks_like_playlist(Some("video/mp2t"), &ts));
    }

    async fn serve(router: Router) -> String {
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        format!("http://{address}")
    }

    #[tokio::test]
    async fn relay_checks_token_rewrites_playlists_and_adds_cors() {
        let upstream = serve(
            Router::new()
                .route(
                    "/hls/index.m3u8",
                    get(|| async {
                        (
                            [(header::CONTENT_TYPE, "application/x-mpegURL")],
                            "#EXTM3U\n#EXTINF:4,\nseg.ts\n",
                        )
                    }),
                )
                .route(
                    "/hls/seg.ts",
                    get(|| async { ([(header::CONTENT_TYPE, "video/mp2t")], "TSDATA") }),
                )
                .route(
                    "/live/{user}/1.ts",
                    get(|axum::extract::Path(user): axum::extract::Path<String>| async move {
                        user
                    }),
                ),
        )
        .await;

        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let base = format!("http://127.0.0.1:{port}/secret");
        let registry = SharedHeaders::default();
        let router = relay_router("secret".into(), base.clone(), registry.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });

        let client = reqwest::Client::new();
        let playlist_url = to_relay_url(&base, &format!("{upstream}/hls/index.m3u8")).unwrap();
        let response = client.get(&playlist_url).send().await.unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["access-control-allow-origin"], "*");
        let text = response.text().await.unwrap();
        let segment = text
            .lines()
            .find(|line| line.ends_with("seg.ts"))
            .unwrap()
            .to_string();
        assert!(segment.starts_with(&base), "{text}");

        let segment_response = client.get(&segment).send().await.unwrap();
        assert_eq!(segment_response.headers()["content-type"], "video/mp2t");
        assert_eq!(segment_response.text().await.unwrap(), "TSDATA");

        // Encoded delimiters in credentials reach the provider still encoded.
        let encoded = to_relay_url(&base, &format!("{upstream}/live/a%2Fb%3F/1.ts")).unwrap();
        let echoed = client.get(&encoded).send().await.unwrap();
        assert_eq!(echoed.status(), 200);
        assert_eq!(echoed.text().await.unwrap(), "a/b?");

        let wrong = playlist_url.replace("/secret/", "/wrong!/");
        assert_eq!(client.get(&wrong).send().await.unwrap().status(), 404);

        // Unregistered header ids are refused rather than played without them.
        let unknown =
            to_relay_url(&format!("{base}/h/99"), &format!("{upstream}/hls/seg.ts")).unwrap();
        assert_eq!(client.get(&unknown).send().await.unwrap().status(), 404);
        let _ = registry;
    }

    #[test]
    fn header_sets_are_shared_and_prefixes_split() {
        let registry = SharedHeaders::default();
        let set = StreamHeaders {
            user_agent: Some("VLC/3.0".into()),
            referrer: None,
        };
        let first = base_for(BASE, &registry, set.clone()).unwrap();
        assert_eq!(first, format!("{BASE}/h/1"));
        assert_eq!(base_for(BASE, &registry, set).unwrap(), first);
        assert_eq!(
            base_for(BASE, &registry, StreamHeaders::default()).unwrap(),
            BASE
        );
        assert_eq!(
            split_header_prefix("h/1/http/a.example/x.ts"),
            (Some("1"), "http/a.example/x.ts")
        );
        assert_eq!(
            split_header_prefix("http/a.example/x.ts"),
            (None, "http/a.example/x.ts")
        );
    }

    #[tokio::test]
    async fn relay_sends_registered_headers_to_playlists_and_segments() {
        async fn echo(headers: HeaderMap) -> String {
            let get = |name: &str| {
                headers
                    .get(name)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("-")
                    .to_string()
            };
            format!("{}|{}", get("user-agent"), get("referer"))
        }
        let upstream = serve(
            Router::new()
                .route(
                    "/hls/index.m3u8",
                    get(|headers: HeaderMap| async move {
                        // Only hand out the playlist to the right agent.
                        if headers
                            .get("user-agent")
                            .is_some_and(|v| v == "SmartIPTV/1")
                        {
                            Ok((
                                [(header::CONTENT_TYPE, "application/x-mpegURL")],
                                "#EXTM3U\n#EXTINF:4,\nseg.ts\n",
                            ))
                        } else {
                            Err(StatusCode::FORBIDDEN)
                        }
                    }),
                )
                .route("/hls/seg.ts", get(echo)),
        )
        .await;

        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let base = format!("http://127.0.0.1:{port}/secret");
        let registry = SharedHeaders::default();
        let router = relay_router("secret".into(), base.clone(), registry.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });

        let client = reqwest::Client::new();
        let plain = to_relay_url(&base, &format!("{upstream}/hls/index.m3u8")).unwrap();
        assert_eq!(client.get(&plain).send().await.unwrap().status(), 403);

        let with_headers = base_for(
            &base,
            &registry,
            StreamHeaders {
                user_agent: Some("SmartIPTV/1".into()),
                referrer: Some("https://portal.example/".into()),
            },
        )
        .unwrap();
        let playlist = to_relay_url(&with_headers, &format!("{upstream}/hls/index.m3u8")).unwrap();
        let text = client
            .get(&playlist)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        let segment = text.lines().find(|line| line.ends_with("seg.ts")).unwrap();
        assert!(segment.starts_with(&format!("{base}/h/")), "{text}");
        assert_eq!(
            client
                .get(segment)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap(),
            "SmartIPTV/1|https://portal.example/"
        );
    }
}
