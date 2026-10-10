use crate::api_keys_server;
use crate::build_identity;
use crate::db::{Database, MediaItem, RemoteAccessPrincipal};
use crate::metadata_provider_config;
use crate::server_access::{self, ClientOrigin, ServerSwitches};
use crate::server_cache::{self, ArtworkLru, CachedArtwork};
use crate::shared_contracts::{
    validate_metadata_provider_contract, MetadataProviderRegistryContract,
    MetadataProviderRegistryInterface,
};
use crate::throttle;
use crate::user_data;
use axum::body::Body;
use axum::extract::{ConnectInfo, Extension, Path, Query, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Response, StatusCode};
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path as FilePath, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::RwLock;
use tokio_util::io::ReaderStream;
use tower_http::cors::{Any, CorsLayer};

const MAX_ARTWORK_BYTES: usize = 25 * 1024 * 1024;
// This domain is intentionally stable so existing opaque remote media keys do not change on upgrade.
const REMOTE_MEDIA_KEY_DOMAIN: &[u8] = b"cinavault-build-170-remote-media-v1";

/// How long a snapshot of the Feature Matrix switches is reused.
const SWITCH_SNAPSHOT_TTL: Duration = Duration::from_secs(2);

struct HttpState {
    database_path: String,
    sessions: Arc<RwLock<HashMap<String, RemoteAccessPrincipal>>>,
    switches: Mutex<Option<(Instant, ServerSwitches)>>,
    artwork_cache: Mutex<ArtworkLru>,
}

impl HttpState {
    fn new(database_path: String) -> Self {
        Self {
            database_path,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            switches: Mutex::new(None),
            artwork_cache: Mutex::new(ArtworkLru::new(0)),
        }
    }

    /// Current switches, re-read from the database at most every couple of
    /// seconds. Unreadable settings fall back to the safest choices.
    fn switches(&self) -> ServerSwitches {
        let mut guard = match self.switches.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some((read_at, switches)) = guard.as_ref() {
            if read_at.elapsed() < SWITCH_SNAPSHOT_TTL {
                return switches.clone();
            }
        }
        let switches = rusqlite::Connection::open(&self.database_path)
            .map(|conn| server_access::read_switches(&Database { conn }))
            .unwrap_or(ServerSwitches {
                remote_access: false,
                api_keys: false,
                stream_cap: Some(throttle::bytes_per_second(throttle::DEFAULT_MBPS)),
                cache: None,
                library_filter: Some("0 = 1".into()),
            });
        *guard = Some((Instant::now(), switches.clone()));
        switches
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PasswordLogin {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccessKeyLogin {
    access_key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerInfo {
    name: String,
    product: &'static str,
    version: String,
    build: String,
    display_name: String,
    release_tag: String,
    account_email: String,
    permissions: Vec<String>,
    remote_transport: &'static str,
    media_identifiers: &'static str,
    local_paths_exposed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryCount {
    total_items: i64,
    count_policy: &'static str,
    capped: bool,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct LibraryQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteMediaItem {
    media_key: String,
    title: String,
    media_type: String,
    year: Option<i32>,
    rating: Option<f64>,
    overview: Option<String>,
    genre: Option<String>,
    duration: Option<i64>,
    file_size: Option<i64>,
    resolution: Option<String>,
    codec: Option<String>,
    verified: bool,
    watched: bool,
    favorite: bool,
    date_added: String,
    last_played: Option<String>,
    tmdb_id: Option<String>,
    imdb_id: Option<String>,
    artwork_url: Option<String>,
    stream_url: String,
}

fn open_database(path: &str) -> Result<Database, (StatusCode, String)> {
    Database::new(path).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database unavailable: {error}"),
        )
    })
}

fn media_key(item: &MediaItem) -> Option<String> {
    let id = item.id?;
    let mut hasher = Sha256::new();
    hasher.update(REMOTE_MEDIA_KEY_DOMAIN);
    hasher.update(id.to_le_bytes());
    hasher.update(item.file_path.as_bytes());
    Some(
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

/// The item behind an opaque key, if the parental filter lets the active
/// profile see it.
fn find_item_by_key(
    database: &Database,
    key: &str,
    filter: Option<&str>,
) -> Result<Option<MediaItem>, String> {
    database
        .get_media_items_visible(None, None, None, filter)
        .map_err(|error| error.to_string())
        .map(|items| {
            items
                .into_iter()
                .find(|item| media_key(item).as_deref() == Some(key))
        })
}

fn preferred_artwork(item: &MediaItem) -> Option<(&'static str, String)> {
    item.poster_path
        .as_ref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| ("poster", value.clone()))
        .or_else(|| {
            item.backdrop_path
                .as_ref()
                .filter(|value| !value.trim().is_empty())
                .map(|value| ("backdrop", value.clone()))
        })
}

fn remote_media_item(item: MediaItem) -> Option<RemoteMediaItem> {
    let key = media_key(&item)?;
    let artwork_url =
        preferred_artwork(&item).map(|(kind, _)| format!("/api/artwork/{key}/{kind}"));
    Some(RemoteMediaItem {
        media_key: key.clone(),
        title: item.title,
        media_type: item.media_type,
        year: item.year,
        rating: item.rating,
        overview: item.overview,
        genre: item.genre,
        duration: item.duration,
        file_size: item.file_size,
        resolution: item.resolution,
        codec: item.codec,
        verified: item.verified,
        watched: item.watched,
        favorite: item.favorite,
        date_added: item.date_added,
        last_played: item.last_played,
        tmdb_id: item.tmdb_id,
        imdb_id: item.imdb_id,
        artwork_url,
        stream_url: format!("/api/stream/{key}"),
    })
}

async fn register_session(
    state: &HttpState,
    principal: RemoteAccessPrincipal,
) -> Json<RemoteAccessPrincipal> {
    state
        .sessions
        .write()
        .await
        .insert(principal.session_token.clone(), principal.clone());
    Json(principal)
}

/// Records a sign-in attempt in the activity log / webhooks.
fn record_login(
    database: &Database,
    outcome: Option<&RemoteAccessPrincipal>,
    method: &str,
    origin: ClientOrigin,
    attempted: &str,
) {
    if user_data::ensure_tables(database).is_err() {
        return;
    }
    let detail = serde_json::json!({ "method": method, "origin": origin.as_str() });
    match outcome {
        Some(principal) => user_data::emit(database, "remote.login", &principal.email, detail),
        None => user_data::emit(database, "remote.login_failed", attempted, detail),
    }
}

/// Sign-in responses carry a session token and are never cached.
fn auth_response(principal: Json<RemoteAccessPrincipal>) -> Response<Body> {
    let mut response = principal.into_response();
    hardened_response_headers(&mut response);
    response
}

async fn login_password(
    State(state): State<Arc<HttpState>>,
    Extension(origin): Extension<ClientOrigin>,
    Json(payload): Json<PasswordLogin>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let database = open_database(&state.database_path)?;
    let principal = database
        .authenticate_remote_password(&payload.email, &payload.password)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?;
    record_login(
        &database,
        principal.as_ref(),
        "password",
        origin,
        &payload.email,
    );
    match principal {
        Some(principal) => Ok(auth_response(register_session(&state, principal).await)),
        None => Err((
            StatusCode::UNAUTHORIZED,
            "Invalid account credentials".into(),
        )),
    }
}

async fn login_access_key(
    State(state): State<Arc<HttpState>>,
    Extension(origin): Extension<ClientOrigin>,
    Json(payload): Json<AccessKeyLogin>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let database = open_database(&state.database_path)?;
    let principal = database
        .authenticate_remote_access_key(&payload.access_key)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?;
    record_login(
        &database,
        principal.as_ref(),
        "access_key",
        origin,
        "access key",
    );
    match principal {
        Some(principal) => Ok(auth_response(register_session(&state, principal).await)),
        None => Err((
            StatusCode::UNAUTHORIZED,
            "Invalid account access key".into(),
        )),
    }
}

/// An app's API key as a principal ("api_keys" switch). Keys are refused
/// outright while the switch is off.
fn api_key_principal(
    state: &HttpState,
    key: &str,
) -> Result<RemoteAccessPrincipal, (StatusCode, String)> {
    if !state.switches().api_keys {
        return Err((
            StatusCode::UNAUTHORIZED,
            "API keys are turned off on this server".into(),
        ));
    }
    let database = open_database(&state.database_path)?;
    let summary = api_keys_server::authenticate(&database, key)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "API key is invalid or revoked".into(),
        ))?;
    Ok(RemoteAccessPrincipal {
        id: summary.id,
        email: format!("api-key:{}", summary.name),
        display_name: Some(summary.name.clone()),
        auth_method: "api_key".into(),
        session_token: String::new(),
        expires_at: String::new(),
        permissions: api_keys_server::server_permissions(&summary),
    })
}

async fn authenticated_principal(
    state: &HttpState,
    headers: &HeaderMap,
    permission: &str,
) -> Result<RemoteAccessPrincipal, (StatusCode, String)> {
    if let Some(key) = api_keys_server::key_from_headers(headers) {
        let principal = api_key_principal(state, &key)?;
        return if principal
            .permissions
            .iter()
            .any(|value| value == permission)
        {
            Ok(principal)
        } else {
            Err((
                StatusCode::FORBIDDEN,
                "API key lacks required permission".into(),
            ))
        };
    }
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or((StatusCode::UNAUTHORIZED, "Bearer token required".into()))?;

    let principal = state.sessions.read().await.get(token).cloned().ok_or((
        StatusCode::UNAUTHORIZED,
        "Session is invalid or expired".into(),
    ))?;

    if !principal
        .permissions
        .iter()
        .any(|value| value == permission)
    {
        return Err((
            StatusCode::FORBIDDEN,
            "Account lacks required permission".into(),
        ));
    }
    Ok(principal)
}

fn hardened_response_headers(response: &mut Response<Body>) {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store, max-age=0"),
    );
    response
        .headers_mut()
        .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    security_headers(response);
}

fn security_headers(response: &mut Response<Body>) {
    response.headers_mut().insert(
        header::HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        header::HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );
}

async fn health(State(state): State<Arc<HttpState>>) -> impl IntoResponse {
    let build = build_identity::current();
    let database_health = open_database(&state.database_path)
        .map(|_| (true, None))
        .unwrap_or_else(|(_, error)| (false, Some(error)));
    let status = if database_health.0 {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(serde_json::json!({
            "status": if database_health.0 { "ok" } else { "unhealthy" },
            "databaseHealthy": database_health.0,
            "error": database_health.1,
            "product": "CinaVault Embedded Media Server",
            "version": build.semantic_version,
            "build": build.display_build,
            "displayName": build.display_name,
            "releaseTag": build.release_tag,
            "remoteTransport": "HTTPS relay required by default",
            "localPathsExposed": false
        })),
    )
}

async fn server_info(
    State(state): State<Arc<HttpState>>,
    headers: HeaderMap,
) -> Result<Json<ServerInfo>, (StatusCode, String)> {
    let principal = authenticated_principal(&state, &headers, "server:read").await?;
    let build = build_identity::current();
    Ok(Json(ServerInfo {
        name: build.product_name.clone(),
        product: "CinaVault Embedded Media Server",
        version: build.semantic_version.clone(),
        build: build.display_build.clone(),
        display_name: build.display_name.clone(),
        release_tag: build.release_tag.clone(),
        account_email: principal.email,
        permissions: principal.permissions,
        remote_transport: "HTTPS relay",
        media_identifiers: "opaque SHA-256 media keys",
        local_paths_exposed: false,
    }))
}

async fn metadata_providers(
    State(state): State<Arc<HttpState>>,
    headers: HeaderMap,
) -> Result<Json<MetadataProviderRegistryContract>, (StatusCode, String)> {
    authenticated_principal(&state, &headers, "server:read").await?;
    let registry = metadata_provider_config::public_registry()
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?;
    let contract = registry.metadata_provider_contract();
    validate_metadata_provider_contract(&contract)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?;
    Ok(Json(contract))
}

/// JSON for library endpoints. With "cdn_cache" on it gets a short private
/// cache lifetime and an ETag (a matching If-None-Match gets 304); otherwise
/// it is never stored.
fn library_json<T: Serialize>(
    value: &T,
    request_headers: &HeaderMap,
    cache: Option<&server_access::CacheSettings>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    let Some(cache) = cache else {
        let mut response = Response::new(Body::from(bytes));
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        hardened_response_headers(&mut response);
        return Ok(response);
    };
    let etag = server_cache::etag_for(&bytes);
    let not_modified = server_cache::if_none_match(
        request_headers
            .get(header::IF_NONE_MATCH)
            .and_then(|value| value.to_str().ok()),
        &etag,
    );
    let mut response = if not_modified {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::NOT_MODIFIED;
        response
    } else {
        let mut response = Response::new(Body::from(bytes));
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        response
    };
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_str(&format!("private, max-age={}", cache.library_max_age))
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?,
    );
    headers.insert(
        header::ETAG,
        HeaderValue::from_str(&etag)
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?,
    );
    headers.insert(
        header::VARY,
        HeaderValue::from_static("Authorization, X-Api-Key"),
    );
    security_headers(&mut response);
    Ok(response)
}

async fn library_count(
    State(state): State<Arc<HttpState>>,
    headers: HeaderMap,
) -> Result<Response<Body>, (StatusCode, String)> {
    authenticated_principal(&state, &headers, "library:read").await?;
    let switches = state.switches();
    let database = open_database(&state.database_path)?;
    let visible = switches.library_filter.as_deref().unwrap_or("1 = 1");
    let total_items = database
        .conn
        .query_row(
            &format!("SELECT COUNT(*) FROM media_items WHERE media_type <> 'photo' AND {visible}"),
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    library_json(
        &LibraryCount {
            total_items,
            count_policy: "all indexed non-artwork media rows",
            capped: false,
        },
        &headers,
        switches.cache.as_ref(),
    )
}

async fn library(
    State(state): State<Arc<HttpState>>,
    headers: HeaderMap,
    Query(query): Query<LibraryQuery>,
) -> Result<Response<Body>, (StatusCode, String)> {
    authenticated_principal(&state, &headers, "library:read").await?;
    let switches = state.switches();
    let database = open_database(&state.database_path)?;
    let limit = query.limit.unwrap_or(100).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);
    let items = database
        .get_media_items_visible(
            None,
            Some(limit),
            Some(offset),
            switches.library_filter.as_deref(),
        )
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    let items: Vec<RemoteMediaItem> = items.into_iter().filter_map(remote_media_item).collect();
    library_json(&items, &headers, switches.cache.as_ref())
}

async fn library_item(
    State(state): State<Arc<HttpState>>,
    headers: HeaderMap,
    Path(media_key): Path<String>,
) -> Result<Response<Body>, (StatusCode, String)> {
    authenticated_principal(&state, &headers, "library:read").await?;
    let switches = state.switches();
    let database = open_database(&state.database_path)?;
    let item = find_item_by_key(&database, &media_key, switches.library_filter.as_deref())
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?
        .and_then(remote_media_item)
        .ok_or((StatusCode::NOT_FOUND, "Media item not found".into()))?;
    library_json(&item, &headers, switches.cache.as_ref())
}

fn content_type(path: &FilePath) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "mp4" | "m4v" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "avi" => "video/x-msvideo",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "flac" => "audio/flac",
        "wav" => "audio/wav",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    }
}

fn requested_range(headers: &HeaderMap, size: u64) -> Option<(u64, u64)> {
    let value = headers.get(header::RANGE)?.to_str().ok()?;
    let range = value.strip_prefix("bytes=")?.split(',').next()?;
    let (start, end) = range.split_once('-')?;
    let start = start.parse::<u64>().ok()?;
    let end = if end.trim().is_empty() {
        size.saturating_sub(1)
    } else {
        end.parse::<u64>().ok()?.min(size.saturating_sub(1))
    };
    (start <= end && end < size).then_some((start, end))
}

fn selected_artwork(item: &MediaItem, requested_kind: Option<&str>) -> Option<(String, String)> {
    match requested_kind {
        Some("poster") => item
            .poster_path
            .clone()
            .filter(|value| !value.trim().is_empty())
            .map(|value| (value, "poster".to_string())),
        Some("backdrop") => item
            .backdrop_path
            .clone()
            .filter(|value| !value.trim().is_empty())
            .map(|value| (value, "backdrop".to_string())),
        Some(_) => None,
        None => preferred_artwork(item).map(|(kind, value)| (value, kind.to_string())),
    }
}

async fn read_artwork_bytes(artwork: &str) -> Result<(Vec<u8>, String), (StatusCode, String)> {
    let (bytes, mime) = if artwork.starts_with("https://") {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(3))
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        let response = client
            .get(artwork)
            .send()
            .await
            .map_err(|error| (StatusCode::BAD_GATEWAY, error.to_string()))?
            .error_for_status()
            .map_err(|error| (StatusCode::BAD_GATEWAY, error.to_string()))?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_ARTWORK_BYTES as u64)
        {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                "Artwork exceeds 25 MiB".into(),
            ));
        }
        let mime = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("application/octet-stream")
            .split(';')
            .next()
            .unwrap_or("application/octet-stream")
            .trim()
            .to_string();
        let bytes = response
            .bytes()
            .await
            .map_err(|error| (StatusCode::BAD_GATEWAY, error.to_string()))?;
        (bytes.to_vec(), mime)
    } else {
        let path = PathBuf::from(artwork);
        let metadata = tokio::fs::metadata(&path)
            .await
            .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?;
        if metadata.len() > MAX_ARTWORK_BYTES as u64 {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                "Artwork exceeds 25 MiB".into(),
            ));
        }
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?;
        (bytes, content_type(&path).to_string())
    };

    if bytes.is_empty() {
        return Err((StatusCode::NOT_FOUND, "Artwork is empty".into()));
    }
    if bytes.len() > MAX_ARTWORK_BYTES {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            "Artwork exceeds 25 MiB".into(),
        ));
    }
    if !mime.starts_with("image/") {
        return Err((
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Artwork response is not an image".into(),
        ));
    }
    Ok((bytes, mime))
}

async fn artwork_response(
    state: Arc<HttpState>,
    headers: HeaderMap,
    media_key: String,
    requested_kind: Option<String>,
) -> Result<Response<Body>, (StatusCode, String)> {
    authenticated_principal(&state, &headers, "library:read").await?;
    let switches = state.switches();
    let item = {
        let database = open_database(&state.database_path)?;
        find_item_by_key(&database, &media_key, switches.library_filter.as_deref())
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?
            .ok_or((StatusCode::NOT_FOUND, "Media item not found".into()))?
    };
    let (artwork, _) = selected_artwork(&item, requested_kind.as_deref())
        .ok_or((StatusCode::NOT_FOUND, "Artwork not available".into()))?;

    let Some(cache) = switches.cache else {
        // "cdn_cache" off: nothing is kept in memory or by clients.
        if let Ok(mut lru) = state.artwork_cache.lock() {
            lru.set_capacity(0);
        }
        let (bytes, mime) = read_artwork_bytes(&artwork).await?;
        let mut response = Response::new(Body::from(bytes));
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(&mime)
                .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?,
        );
        hardened_response_headers(&mut response);
        return Ok(response);
    };

    let cached = state.artwork_cache.lock().ok().and_then(|mut lru| {
        lru.set_capacity(cache.artwork_bytes);
        lru.get(&artwork)
    });
    let cached = match cached {
        Some(cached) => cached,
        None => {
            let (bytes, mime) = read_artwork_bytes(&artwork).await?;
            let entry = CachedArtwork {
                etag: server_cache::etag_for(&bytes),
                bytes: axum::body::Bytes::from(bytes),
                mime,
            };
            if let Ok(mut lru) = state.artwork_cache.lock() {
                lru.insert(artwork.clone(), entry.clone());
            }
            entry
        }
    };
    let not_modified = server_cache::if_none_match(
        headers
            .get(header::IF_NONE_MATCH)
            .and_then(|value| value.to_str().ok()),
        &cached.etag,
    );
    let mut response = if not_modified {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::NOT_MODIFIED;
        response
    } else {
        let mut response = Response::new(Body::from(cached.bytes.clone()));
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(&cached.mime)
                .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?,
        );
        response
    };
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(server_cache::ARTWORK_CACHE_CONTROL),
    );
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&cached.etag)
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?,
    );
    security_headers(&mut response);
    Ok(response)
}

async fn artwork_media(
    State(state): State<Arc<HttpState>>,
    headers: HeaderMap,
    Path(media_key): Path<String>,
) -> Result<Response<Body>, (StatusCode, String)> {
    artwork_response(state, headers, media_key, None).await
}

async fn artwork_media_kind(
    State(state): State<Arc<HttpState>>,
    headers: HeaderMap,
    Path((media_key, kind)): Path<(String, String)>,
) -> Result<Response<Body>, (StatusCode, String)> {
    artwork_response(state, headers, media_key, Some(kind)).await
}

async fn stream_media(
    State(state): State<Arc<HttpState>>,
    headers: HeaderMap,
    Path(media_key): Path<String>,
) -> Result<Response<Body>, (StatusCode, String)> {
    authenticated_principal(&state, &headers, "stream:play").await?;
    let filter = state.switches().library_filter;
    let database = open_database(&state.database_path)?;
    let item = find_item_by_key(&database, &media_key, filter.as_deref())
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?
        .ok_or((StatusCode::NOT_FOUND, "Media item not found".into()))?;

    let path = PathBuf::from(item.file_path);
    let mut file = tokio::fs::File::open(&path)
        .await
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?;
    let size = file
        .metadata()
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .len();

    if size == 0 {
        return Err((
            StatusCode::RANGE_NOT_SATISFIABLE,
            "Media file is empty".into(),
        ));
    }

    let (status, start, end) = requested_range(&headers, size)
        .map(|(start, end)| (StatusCode::PARTIAL_CONTENT, start, end))
        .unwrap_or((StatusCode::OK, 0, size.saturating_sub(1)));
    let length = end.saturating_sub(start).saturating_add(1);
    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    let stream = ReaderStream::new(file.take(length));
    let mut response = Response::new(Body::from_stream(stream));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type(&path)),
    );
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&length.to_string())
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?,
    );
    response
        .headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    if status == StatusCode::PARTIAL_CONTENT {
        response.headers_mut().insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{end}/{size}"))
                .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?,
        );
    }
    hardened_response_headers(&mut response);
    Ok(response)
}

/// Transcoded H.264/AAC fragmented MP4 for clients that cannot play the
/// original; `?start=seconds` seeks. Refused while Force Direct Play is on.
async fn transcode_media(
    State(state): State<Arc<HttpState>>,
    headers: HeaderMap,
    Path(media_key): Path<String>,
    Query(query): Query<crate::player_stream::StartQuery>,
) -> Result<Response<Body>, (StatusCode, String)> {
    authenticated_principal(&state, &headers, "stream:play").await?;
    let filter = state.switches().library_filter;
    let (path, settings) = {
        let database = open_database(&state.database_path)?;
        if crate::feature_flags::is_enabled(&database, "direct_play") {
            return Err((
                StatusCode::CONFLICT,
                "Force Direct Play is on; use /api/stream for the original file".into(),
            ));
        }
        let item = find_item_by_key(&database, &media_key, filter.as_deref())
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?
            .ok_or((StatusCode::NOT_FOUND, "Media item not found".into()))?;
        (
            PathBuf::from(item.file_path),
            crate::transcode::TranscodeSettings::load(&database),
        )
    };
    let mut response =
        crate::player_stream::transcode_response(path, query.start.unwrap_or(0.0), settings)
            .await?;
    hardened_response_headers(&mut response);
    Ok(response)
}

/// Runs in front of every route. Refuses clients from outside the LAN while
/// "remote_access" is off, tags the request with its origin, and paces media
/// responses to non-local clients while "bandwidth_limit" is on.
async fn access_layer(
    State(state): State<Arc<HttpState>>,
    mut request: Request,
    next: Next,
) -> Response<Body> {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip());
    let origin = server_access::classify(peer, request.headers());
    let switches = state.switches();
    if origin == ClientOrigin::Remote && !switches.remote_access {
        let mut response = (
            StatusCode::FORBIDDEN,
            "Remote access is turned off on this server",
        )
            .into_response();
        hardened_response_headers(&mut response);
        return response;
    }
    let paced = origin != ClientOrigin::Loopback && is_media_path(request.uri().path());
    request.extensions_mut().insert(origin);
    let response = next.run(request).await;
    match switches.stream_cap {
        Some(cap) if paced && response.status().is_success() => {
            let (parts, body) = response.into_parts();
            Response::from_parts(parts, throttle::limit_body(body, cap))
        }
        _ => response,
    }
}

/// Routes whose bodies are media streams (subject to the bandwidth cap).
fn is_media_path(path: &str) -> bool {
    path.starts_with("/api/stream/")
        || path == "/api/transcode"
        || path.starts_with("/api/transcode/")
}

/// The router as a service that knows each client's address, which the
/// access rules need. Serve this, not `router` directly.
pub(crate) fn service(
    database_path: String,
) -> axum::extract::connect_info::IntoMakeServiceWithConnectInfo<Router, SocketAddr> {
    router(database_path).into_make_service_with_connect_info::<SocketAddr>()
}

pub(crate) fn router(database_path: String) -> Router {
    let state = Arc::new(HttpState::new(database_path));
    let routes = Router::new()
        .route("/health", get(health))
        .route("/api/auth/password", post(login_password))
        .route("/api/auth/access-key", post(login_access_key))
        .route("/api/server/info", get(server_info))
        .route("/api/metadata/providers", get(metadata_providers))
        .route("/api/library", get(library))
        .route("/api/library/count", get(library_count))
        .route("/api/library/{media_key}", get(library_item))
        .route("/api/artwork/{media_key}", get(artwork_media))
        .route("/api/artwork/{media_key}/{kind}", get(artwork_media_kind))
        .route("/api/transcode/{media_key}", get(transcode_media))
        .route("/api/stream/{media_key}", get(stream_media));
    with_access_rules(routes, state)
}

/// Puts every route behind the access layer and CORS. Add routes in `router`;
/// anything chained after this would skip the remote-access and bandwidth rules.
fn with_access_rules(routes: Router<Arc<HttpState>>, state: Arc<HttpState>) -> Router {
    routes
        .layer(middleware::from_fn_with_state(state.clone(), access_layer))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_headers([
                    header::AUTHORIZATION,
                    header::CONTENT_TYPE,
                    header::RANGE,
                    header::IF_NONE_MATCH,
                    header::HeaderName::from_static("x-api-key"),
                ])
                .expose_headers([header::ETAG, header::CONTENT_RANGE])
                .allow_methods(Any),
        )
        .with_state(state)
}
#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::db::MediaItem;
    use serde_json::Value;
    use tokio::sync::oneshot;

    async fn spawn_test_server(database_path: String) -> (String, oneshot::Sender<()>) {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind ephemeral loopback listener");
        let address = listener.local_addr().expect("read listener address");
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        tokio::spawn(async move {
            axum::serve(listener, service(database_path))
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .expect("serve test API");
        });
        (format!("http://{address}"), shutdown_tx)
    }

    fn media(title: &str, path: &str, date_added: &str) -> MediaItem {
        MediaItem {
            id: None,
            title: title.into(),
            file_path: path.into(),
            media_type: "movie".into(),
            year: None,
            rating: None,
            overview: None,
            poster_path: None,
            backdrop_path: None,
            genre: None,
            duration: None,
            file_size: None,
            resolution: None,
            codec: None,
            verified: true,
            watched: false,
            favorite: false,
            date_added: date_added.into(),
            last_played: None,
            tmdb_id: None,
            imdb_id: None,
            source_id: None,
        }
    }

    #[tokio::test]
    async fn remote_client_authenticates_and_reads_paginated_library_across_restart() {
        let database_path = std::env::temp_dir().join(format!(
            "cinavault-embedded-http-{}.db",
            uuid::Uuid::new_v4()
        ));
        let database_path_text = database_path.to_string_lossy().into_owned();
        let database = Database::new(&database_path_text).expect("create temporary database");
        let provision = database
            .create_remote_access_user(
                "viewer@example.com",
                &format!("Pw-{}!", uuid::Uuid::new_v4().simple()),
                Some("Viewer"),
            )
            .expect("provision remote user");
        database
            .add_media_item_data(&media(
                "Older",
                "C:/media/older.mp4",
                "2026-01-01T00:00:00Z",
            ))
            .expect("insert older media");
        database
            .add_media_item_data(&media(
                "Newest",
                "C:/media/newest.mp4",
                "2026-02-01T00:00:00Z",
            ))
            .expect("insert newest media");
        drop(database);

        let client = reqwest::Client::new();
        let (base_url, shutdown) = spawn_test_server(database_path_text.clone()).await;
        let invalid = client
            .post(format!("{base_url}/api/auth/access-key"))
            .json(&serde_json::json!({ "accessKey": "cvra_invalid" }))
            .send()
            .await
            .expect("send invalid-key request");
        assert_eq!(invalid.status(), StatusCode::UNAUTHORIZED);

        let login = client
            .post(format!("{base_url}/api/auth/access-key"))
            .json(&serde_json::json!({ "accessKey": provision.access_key }))
            .send()
            .await
            .expect("send access-key login");
        assert_eq!(login.status(), StatusCode::OK);
        let principal: Value = login.json().await.expect("decode login response");
        let token = principal["session_token"].as_str().expect("session token");

        let info: Value = client
            .get(format!("{base_url}/api/server/info"))
            .bearer_auth(token)
            .send()
            .await
            .expect("request server info")
            .error_for_status()
            .expect("authorized server info")
            .json()
            .await
            .expect("decode server info");
        assert_eq!(info["accountEmail"], "viewer@example.com");

        let library: Vec<Value> = client
            .get(format!("{base_url}/api/library?limit=1&offset=0"))
            .bearer_auth(token)
            .send()
            .await
            .expect("request library page")
            .error_for_status()
            .expect("authorized library page")
            .json()
            .await
            .expect("decode library page");
        assert_eq!(library.len(), 1);
        assert_eq!(library[0]["title"], "Newest");

        shutdown.send(()).expect("stop first server");
        let (restarted_url, restarted_shutdown) = spawn_test_server(database_path_text).await;
        let stale_session = client
            .get(format!("{restarted_url}/api/server/info"))
            .bearer_auth(token)
            .send()
            .await
            .expect("request with stale session");
        assert_eq!(stale_session.status(), StatusCode::UNAUTHORIZED);
        restarted_shutdown.send(()).expect("stop restarted server");
        let _ = std::fs::remove_file(database_path);
    }

    /// A database with one remote user, one API key and a few titles.
    struct Fixture {
        path: std::path::PathBuf,
        dir: std::path::PathBuf,
        access_key: String,
        api_key: String,
        stream_only_key: String,
    }

    fn fixture() -> Fixture {
        let dir = std::env::temp_dir().join(format!("cinavault-switches-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("server.db");
        let database = Database::new(path.to_str().unwrap()).unwrap();
        user_data::ensure_tables(&database).unwrap();
        let access_key = database
            .create_remote_access_user(
                "family@example.com",
                &format!("Pw-{}!", uuid::Uuid::new_v4().simple()),
                None,
            )
            .unwrap()
            .access_key;
        let api_key = api_keys_server::issue(
            &database,
            "Test app",
            &["library:read".into(), "stream:play".into()],
        )
        .unwrap()
        .key;
        let stream_only_key = api_keys_server::issue(&database, "Player", &["stream:play".into()])
            .unwrap()
            .key;
        let video = dir.join("Family Film.mp4");
        std::fs::write(&video, vec![42u8; 250_000]).unwrap();
        let mut family = media(
            "Family Film",
            video.to_str().unwrap(),
            "2026-03-01T00:00:00Z",
        );
        family.genre = Some("Animation".into());
        database.add_media_item_data(&family).unwrap();
        database
            .add_media_item_data(&media(
                "Night Terror",
                "C:/media/terror.mp4",
                "2026-02-01T00:00:00Z",
            ))
            .unwrap();
        database
            .conn
            .execute_batch(
                "UPDATE media_items SET content_rating = 'G' WHERE title = 'Family Film';
                 UPDATE media_items SET content_rating = 'R', genre = 'Horror' WHERE title = 'Night Terror';",
            )
            .unwrap();
        Fixture {
            path,
            dir,
            access_key,
            api_key,
            stream_only_key,
        }
    }

    fn set_switch(fixture: &Fixture, key: &str, enabled: bool, config: &str) {
        Database::new(fixture.path.to_str().unwrap())
            .unwrap()
            .set_feature_setting_data(key, enabled, config)
            .unwrap();
    }

    /// The server re-reads switches every couple of seconds; tests restart it instead.
    async fn server(fixture: &Fixture) -> (String, oneshot::Sender<()>) {
        spawn_test_server(fixture.path.to_string_lossy().into_owned()).await
    }

    #[tokio::test]
    async fn remote_access_switch_refuses_relayed_clients_but_not_local_ones() {
        let fixture = fixture();
        let client = reqwest::Client::new();
        set_switch(&fixture, "remote_access", false, "{}");
        let (base, stop) = server(&fixture).await;
        let relayed = client
            .get(format!("{base}/health"))
            .header("cf-connecting-ip", "203.0.113.9")
            .send()
            .await
            .unwrap();
        assert_eq!(relayed.status(), StatusCode::FORBIDDEN);
        let relayed_login = client
            .post(format!("{base}/api/auth/access-key"))
            .header("x-forwarded-for", "203.0.113.9")
            .json(&serde_json::json!({ "accessKey": fixture.access_key }))
            .send()
            .await
            .unwrap();
        assert_eq!(relayed_login.status(), StatusCode::FORBIDDEN);
        // Accounts keep working for local clients with the switch off.
        let local_login = client
            .post(format!("{base}/api/auth/access-key"))
            .json(&serde_json::json!({ "accessKey": fixture.access_key }))
            .send()
            .await
            .unwrap();
        assert_eq!(local_login.status(), StatusCode::OK);
        assert_eq!(
            local_login.headers()[header::CACHE_CONTROL],
            "private, no-store, max-age=0",
            "sign-in responses are never cacheable"
        );
        stop.send(()).unwrap();

        set_switch(&fixture, "remote_access", true, "{}");
        let (base, stop) = server(&fixture).await;
        let relayed = client
            .get(format!("{base}/health"))
            .header("cf-connecting-ip", "203.0.113.9")
            .send()
            .await
            .unwrap();
        assert_eq!(relayed.status(), StatusCode::OK);
        stop.send(()).unwrap();

        let db = Database::new(fixture.path.to_str().unwrap()).unwrap();
        let kinds: Vec<String> = user_data::activity(&db, 10)
            .unwrap()
            .into_iter()
            .map(|entry| entry.kind)
            .collect();
        assert!(kinds.contains(&"remote.login".to_string()));
        std::fs::remove_dir_all(&fixture.dir).ok();
    }

    #[tokio::test]
    async fn api_keys_authenticate_with_permissions_and_follow_their_switch() {
        let fixture = fixture();
        let client = reqwest::Client::new();
        let (base, stop) = server(&fixture).await;
        let library: Vec<Value> = client
            .get(format!("{base}/api/library"))
            .header("x-api-key", &fixture.api_key)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(library.len(), 2);
        let info: Value = client
            .get(format!("{base}/api/server/info"))
            .header("authorization", format!("ApiKey {}", fixture.api_key))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(info["accountEmail"], "api-key:Test app");
        let forbidden = client
            .get(format!("{base}/api/library"))
            .header("x-api-key", &fixture.stream_only_key)
            .send()
            .await
            .unwrap();
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
        let bogus = client
            .get(format!("{base}/api/library"))
            .header(
                "x-api-key",
                "cvk_00000000_000000000000000000000000000000000000000000000000",
            )
            .send()
            .await
            .unwrap();
        assert_eq!(bogus.status(), StatusCode::UNAUTHORIZED);
        stop.send(()).unwrap();

        set_switch(&fixture, "api_keys", false, "{}");
        let (base, stop) = server(&fixture).await;
        let off = client
            .get(format!("{base}/api/library"))
            .header("x-api-key", &fixture.api_key)
            .send()
            .await
            .unwrap();
        assert_eq!(off.status(), StatusCode::UNAUTHORIZED);
        stop.send(()).unwrap();
        std::fs::remove_dir_all(&fixture.dir).ok();
    }

    #[tokio::test]
    async fn cache_switch_controls_library_etags_and_headers() {
        let fixture = fixture();
        let client = reqwest::Client::new();
        let (base, stop) = server(&fixture).await;
        let first = client
            .get(format!("{base}/api/library"))
            .header("x-api-key", &fixture.api_key)
            .send()
            .await
            .unwrap();
        assert_eq!(
            first.headers()[header::CACHE_CONTROL],
            "private, max-age=30"
        );
        let etag = first.headers()[header::ETAG].to_str().unwrap().to_string();
        let again = client
            .get(format!("{base}/api/library"))
            .header("x-api-key", &fixture.api_key)
            .header(header::IF_NONE_MATCH, &etag)
            .send()
            .await
            .unwrap();
        assert_eq!(again.status(), StatusCode::NOT_MODIFIED);
        stop.send(()).unwrap();

        set_switch(&fixture, "cdn_cache", false, "{}");
        let (base, stop) = server(&fixture).await;
        let uncached = client
            .get(format!("{base}/api/library"))
            .header("x-api-key", &fixture.api_key)
            .header(header::IF_NONE_MATCH, &etag)
            .send()
            .await
            .unwrap();
        assert_eq!(uncached.status(), StatusCode::OK);
        assert_eq!(
            uncached.headers()[header::CACHE_CONTROL],
            "private, no-store, max-age=0"
        );
        assert!(uncached.headers().get(header::ETAG).is_none());
        stop.send(()).unwrap();
        std::fs::remove_dir_all(&fixture.dir).ok();
    }

    #[tokio::test]
    async fn bandwidth_cap_paces_remote_streams_and_exempts_loopback() {
        let fixture = fixture();
        set_switch(&fixture, "bandwidth_limit", true, r#"{"mbps":1}"#);
        let client = reqwest::Client::new();
        let (base, stop) = server(&fixture).await;
        let library: Vec<Value> = client
            .get(format!("{base}/api/library"))
            .header("x-api-key", &fixture.api_key)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let stream_url = library
            .iter()
            .find(|item| item["title"] == "Family Film")
            .and_then(|item| item["streamUrl"].as_str())
            .unwrap()
            .to_string();

        let started = Instant::now();
        let local = client
            .get(format!("{base}{stream_url}"))
            .header("x-api-key", &fixture.api_key)
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        assert_eq!(local.len(), 250_000);
        assert!(
            started.elapsed() < Duration::from_millis(900),
            "loopback is exempt"
        );

        // 1 Mbps = 125 KB/s with a 62.5 KB burst: 250 KB takes about 1.5 s.
        let started = Instant::now();
        let relayed = client
            .get(format!("{base}{stream_url}"))
            .header("x-api-key", &fixture.api_key)
            .header("cf-connecting-ip", "203.0.113.9")
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        assert_eq!(relayed.len(), 250_000);
        assert!(
            started.elapsed() >= Duration::from_millis(1_400),
            "took {:?}",
            started.elapsed()
        );
        stop.send(()).unwrap();
        std::fs::remove_dir_all(&fixture.dir).ok();
    }

    #[tokio::test]
    async fn restricted_profiles_only_see_allowed_titles_on_the_server() {
        let fixture = fixture();
        {
            let db = Database::new(fixture.path.to_str().unwrap()).unwrap();
            let kid = user_data::create_profile(&db, "Kid", "#f00", true).unwrap();
            db.set_feature_setting_data("user_profiles", true, "{}")
                .unwrap();
            db.set_feature_setting_data("parental_ctrl", true, r#"{"maxRating":"PG"}"#)
                .unwrap();
            user_data::set_active_profile(&db, kid.id).unwrap();
        }
        let client = reqwest::Client::new();
        let (base, stop) = server(&fixture).await;
        let library: Vec<Value> = client
            .get(format!("{base}/api/library"))
            .header("x-api-key", &fixture.api_key)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let titles: Vec<&str> = library
            .iter()
            .filter_map(|item| item["title"].as_str())
            .collect();
        assert_eq!(titles, vec!["Family Film"]);
        let count: Value = client
            .get(format!("{base}/api/library/count"))
            .header("x-api-key", &fixture.api_key)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(count["totalItems"], 1);
        stop.send(()).unwrap();
        std::fs::remove_dir_all(&fixture.dir).ok();
    }
}
