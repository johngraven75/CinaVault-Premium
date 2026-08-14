use crate::build_identity;
use crate::db::{Database, MediaItem, RemoteAccessPrincipal};
use crate::metadata_provider_config;
use crate::shared_contracts::{
    validate_metadata_provider_contract, MetadataProviderRegistryContract,
    MetadataProviderRegistryInterface,
};
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, Response, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path as FilePath, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::RwLock;
use tokio_util::io::ReaderStream;
use tower_http::cors::{Any, CorsLayer};

const MAX_ARTWORK_BYTES: usize = 25 * 1024 * 1024;
// This domain is intentionally stable so existing opaque remote media keys do not change on upgrade.
const REMOTE_MEDIA_KEY_DOMAIN: &[u8] = b"cinavault-build-170-remote-media-v1";

#[derive(Clone)]
struct HttpState {
    database_path: String,
    sessions: Arc<RwLock<HashMap<String, RemoteAccessPrincipal>>>,
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

fn find_item_by_key(database: &Database, key: &str) -> Result<Option<MediaItem>, String> {
    database
        .get_media_items_data(None, None, None)
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

async fn login_password(
    State(state): State<Arc<HttpState>>,
    Json(payload): Json<PasswordLogin>,
) -> Result<Json<RemoteAccessPrincipal>, (StatusCode, String)> {
    let database = open_database(&state.database_path)?;
    match database
        .authenticate_remote_password(&payload.email, &payload.password)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?
    {
        Some(principal) => Ok(register_session(&state, principal).await),
        None => Err((
            StatusCode::UNAUTHORIZED,
            "Invalid account credentials".into(),
        )),
    }
}

async fn login_access_key(
    State(state): State<Arc<HttpState>>,
    Json(payload): Json<AccessKeyLogin>,
) -> Result<Json<RemoteAccessPrincipal>, (StatusCode, String)> {
    let database = open_database(&state.database_path)?;
    match database
        .authenticate_remote_access_key(&payload.access_key)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?
    {
        Some(principal) => Ok(register_session(&state, principal).await),
        None => Err((
            StatusCode::UNAUTHORIZED,
            "Invalid account access key".into(),
        )),
    }
}

async fn authenticated_principal(
    state: &HttpState,
    headers: &HeaderMap,
    permission: &str,
) -> Result<RemoteAccessPrincipal, (StatusCode, String)> {
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

async fn library_count(
    State(state): State<Arc<HttpState>>,
    headers: HeaderMap,
) -> Result<Json<LibraryCount>, (StatusCode, String)> {
    authenticated_principal(&state, &headers, "library:read").await?;
    let database = open_database(&state.database_path)?;
    let total_items = database
        .conn
        .query_row(
            "SELECT COUNT(*) FROM media_items WHERE media_type <> 'photo'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(LibraryCount {
        total_items,
        count_policy: "all indexed non-artwork media rows",
        capped: false,
    }))
}

async fn library(
    State(state): State<Arc<HttpState>>,
    headers: HeaderMap,
    Query(query): Query<LibraryQuery>,
) -> Result<Json<Vec<RemoteMediaItem>>, (StatusCode, String)> {
    authenticated_principal(&state, &headers, "library:read").await?;
    let database = open_database(&state.database_path)?;
    let limit = query.limit.unwrap_or(100).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);
    let items = database
        .get_media_items_data(None, Some(limit), Some(offset))
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(
        items.into_iter().filter_map(remote_media_item).collect(),
    ))
}

async fn library_item(
    State(state): State<Arc<HttpState>>,
    headers: HeaderMap,
    Path(media_key): Path<String>,
) -> Result<Json<RemoteMediaItem>, (StatusCode, String)> {
    authenticated_principal(&state, &headers, "library:read").await?;
    let database = open_database(&state.database_path)?;
    find_item_by_key(&database, &media_key)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?
        .and_then(remote_media_item)
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "Media item not found".into()))
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
    let database = open_database(&state.database_path)?;
    let item = find_item_by_key(&database, &media_key)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?
        .ok_or((StatusCode::NOT_FOUND, "Media item not found".into()))?;
    let (artwork, _) = selected_artwork(&item, requested_kind.as_deref())
        .ok_or((StatusCode::NOT_FOUND, "Artwork not available".into()))?;
    let (bytes, mime) = read_artwork_bytes(&artwork).await?;

    let mut response = Response::new(Body::from(bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&mime)
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?,
    );
    hardened_response_headers(&mut response);
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
    let database = open_database(&state.database_path)?;
    let item = find_item_by_key(&database, &media_key)
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

pub(crate) fn router(database_path: String) -> Router {
    let state = Arc::new(HttpState {
        database_path,
        sessions: Arc::new(RwLock::new(HashMap::new())),
    });
    Router::new()
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
        .route("/api/stream/{media_key}", get(stream_media))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::RANGE])
                .allow_methods(Any),
        )
        .with_state(state)
}
#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::db::MediaItem;
    use serde_json::Value;
    use std::sync::Arc;
    use tokio::sync::oneshot;

    async fn spawn_test_server(database_path: String) -> (String, oneshot::Sender<()>) {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind ephemeral loopback listener");
        let address = listener.local_addr().expect("read listener address");
        let state = Arc::new(HttpState {
            database_path,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        });
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        tokio::spawn(async move {
            axum::serve(listener, router(state))
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
            .create_remote_access_user("viewer@example.com", "CorrectHorse42!", Some("Viewer"))
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
}
