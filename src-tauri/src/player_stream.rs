//! Streaming for the built-in player.
//!
//! `player_prepare` resolves a library title, lets the WebView read that one
//! file through the asset protocol (direct play), probes it with ffprobe, and
//! hands back a transcode URL on a loopback-only HTTP server. That server binds
//! 127.0.0.1 on a random port, and every path carries a random per-launch
//! token, so other local users and web pages cannot use it. Each request runs
//! one ffmpeg job (see transcode.rs) and streams its fragmented MP4 output;
//! seeking requests a new URL with `?start=seconds`, and dropping the response
//! kills that ffmpeg process.
//!
//! The embedded media server reuses `transcode_response` for /api/transcode.

use crate::db::Database;
use crate::feature_flags;
use crate::transcode::{self, MediaProbe, TranscodeJob, TranscodeSettings};
use crate::AppState;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderValue, Response, StatusCode};
use axum::routing::get;
use axum::Router;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll};
use tauri::{AppHandle, Manager, State as TauriState};
use tokio::io::{AsyncRead, ReadBuf};
use tokio::process::{Child, ChildStdout};
use tokio::sync::OnceCell;
use tokio_util::io::ReaderStream;

/// Owns the ffmpeg process for as long as its output is being sent; dropping
/// it (client gone, seek, player closed) kills the process.
struct ChildReader {
    _child: Child,
    stdout: ChildStdout,
}

impl AsyncRead for ChildReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().stdout).poll_read(cx, buf)
    }
}

/// Source heights for Auto quality, so a seek does not re-run ffprobe.
fn cached_height(path: &str) -> Option<u32> {
    static HEIGHTS: OnceLock<Mutex<HashMap<String, Option<u32>>>> = OnceLock::new();
    let cache = HEIGHTS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(height) = cache.lock().ok().and_then(|map| map.get(path).copied()) {
        return height;
    }
    let height = transcode::probe_media(path)
        .ok()
        .and_then(|probe| probe.height);
    if let Ok(mut map) = cache.lock() {
        if map.len() >= 256 {
            map.clear();
        }
        map.insert(path.to_string(), height);
    }
    height
}

type HttpError = (StatusCode, String);

/// Starts ffmpeg for `path` from `start_secs` and streams its output.
pub(crate) async fn transcode_response(
    path: PathBuf,
    start_secs: f64,
    settings: TranscodeSettings,
) -> Result<Response<Body>, HttpError> {
    if !path.is_file() {
        return Err((StatusCode::NOT_FOUND, "Media file not found".into()));
    }
    let input = path.to_string_lossy().into_owned();
    let probe_input = input.clone();
    let (encoder, height) = tokio::task::spawn_blocking(move || {
        if !transcode::ffmpeg_available() {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "ffmpeg is not installed; open Advanced > Media Tools to repair it".to_string(),
            ));
        }
        let encoder =
            transcode::choose_encoder(settings.hardware, transcode::usable_hardware_encoders());
        Ok((encoder, cached_height(&probe_input)))
    })
    .await
    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))??;

    let job = TranscodeJob {
        input: &input,
        start_secs,
        encoder,
        profile: transcode::quality_profile(settings.quality, height),
    };
    let mut command = tokio::process::Command::new(transcode::ffmpeg_path());
    command
        .args(transcode::ffmpeg_args(&job))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(target_os = "windows")]
    command.creation_flags(0x0800_0000);
    let mut child = command.spawn().map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("ffmpeg could not start: {error}"),
        )
    })?;
    let stdout = child.stdout.take().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "ffmpeg output unavailable".to_string(),
    ))?;
    let reader = ChildReader {
        _child: child,
        stdout,
    };
    let stream = ReaderStream::with_capacity(reader, settings.buffer.chunk_bytes);

    let mut response = Response::new(Body::from_stream(stream));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("video/mp4"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::HeaderName::from_static("x-cinavault-encoder"),
        HeaderValue::from_static(encoder.ffmpeg_name()),
    );
    Ok(response)
}

// ── Loopback server ──────────────────────────────────────────────────────────

struct LoopbackState {
    database_path: String,
    token: String,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct StartQuery {
    pub start: Option<f64>,
}

/// Compares without stopping at the first differing byte.
fn same_token(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

async fn loopback_transcode(
    State(state): State<Arc<LoopbackState>>,
    Path((token, media_id)): Path<(String, i64)>,
    Query(query): Query<StartQuery>,
) -> Result<Response<Body>, HttpError> {
    if !same_token(&token, &state.token) {
        return Err((StatusCode::NOT_FOUND, "Not found".into()));
    }
    let (path, settings) = {
        let db = Database::new(&state.database_path)
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        if feature_flags::is_enabled(&db, "direct_play") {
            return Err((
                StatusCode::CONFLICT,
                "Force Direct Play is on, so titles are never transcoded".into(),
            ));
        }
        let path: Option<String> = db
            .conn
            .query_row(
                "SELECT file_path FROM media_items WHERE id = ?1",
                params![media_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        let path = path.ok_or((StatusCode::NOT_FOUND, "Media item not found".to_string()))?;
        (PathBuf::from(path), TranscodeSettings::load(&db))
    };
    let mut response = transcode_response(path, query.start.unwrap_or(0.0), settings).await?;
    // The page (tauri origin) reads this through Web Audio, which needs CORS.
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    Ok(response)
}

fn loopback_router(database_path: String, token: String) -> Router {
    Router::new()
        .route("/{token}/transcode/{media_id}", get(loopback_transcode))
        .with_state(Arc::new(LoopbackState {
            database_path,
            token,
        }))
}

struct LoopbackServer {
    port: u16,
    token: String,
}

fn random_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

async fn bind_loopback(database_path: String) -> Result<LoopbackServer, String> {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|error| format!("Player stream server could not start: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    let token = random_token();
    let router = loopback_router(database_path, token.clone());
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, router).await {
            log::warn!("Player stream server stopped: {error}");
        }
    });
    Ok(LoopbackServer { port, token })
}

async fn loopback(database_path: String) -> Result<&'static LoopbackServer, String> {
    static SERVER: OnceCell<LoopbackServer> = OnceCell::const_new();
    SERVER
        .get_or_try_init(|| bind_loopback(database_path))
        .await
}

// ── Commands ─────────────────────────────────────────────────────────────────

/// Finds the library row for a file: by path first (a specific copy), then by id.
/// Only library titles may be opened, which keeps the asset scope narrow.
pub(crate) fn resolve_library_file(
    db: &Database,
    media_id: Option<i64>,
    file_path: &str,
) -> Result<(i64, String), String> {
    let by_path = db
        .conn
        .query_row(
            "SELECT id, file_path FROM media_items WHERE file_path = ?1 LIMIT 1",
            params![file_path],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if let Some(found) = by_path {
        return Ok(found);
    }
    if let Some(id) = media_id {
        let by_id = db
            .conn
            .query_row(
                "SELECT id, file_path FROM media_items WHERE id = ?1",
                params![id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if let Some(found) = by_id {
            return Ok(found);
        }
    }
    Err("Only titles in the library can play in the built-in player".into())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSource {
    pub media_id: i64,
    pub file_path: String,
    pub probe: Option<MediaProbe>,
    pub probe_error: Option<String>,
    /// Loopback URL that transcodes this title; add `?start=seconds` to seek.
    /// None when ffmpeg is missing or Force Direct Play is on.
    pub transcode_url: Option<String>,
    pub force_direct_play: bool,
    pub ffmpeg_available: bool,
}

/// Gets one library title ready for the built-in player.
#[tauri::command]
pub async fn player_prepare(
    app: AppHandle,
    state: TauriState<'_, AppState>,
    media_id: Option<i64>,
    file_path: String,
) -> Result<PlayerSource, String> {
    let database_path = state
        .app_data_dir
        .join("cinavault.db")
        .to_string_lossy()
        .into_owned();
    let (resolved_id, path, force_direct_play) = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        let (id, path) = resolve_library_file(&db, media_id, &file_path)?;
        (id, path, feature_flags::is_enabled(&db, "direct_play"))
    };
    if !std::path::Path::new(&path).is_file() {
        return Err(format!("Media file not found: {path}"));
    }
    app.asset_protocol_scope()
        .allow_file(&path)
        .map_err(|error| format!("Could not open the file for playback: {error}"))?;

    let probe_path = path.clone();
    let (probe, ffmpeg_available) = tokio::task::spawn_blocking(move || {
        (
            transcode::probe_media(&probe_path),
            transcode::ffmpeg_available(),
        )
    })
    .await
    .map_err(|error| error.to_string())?;

    let transcode_url = if ffmpeg_available && !force_direct_play {
        let server = loopback(database_path).await?;
        Some(format!(
            "http://127.0.0.1:{}/{}/transcode/{}",
            server.port, server.token, resolved_id
        ))
    } else {
        None
    };
    let (probe, probe_error) = match probe {
        Ok(probe) => (Some(probe), None),
        Err(error) => (None, Some(error)),
    };
    Ok(PlayerSource {
        media_id: resolved_id,
        file_path: path,
        probe,
        probe_error,
        transcode_url,
        force_direct_play,
        ffmpeg_available,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscodeStatus {
    pub ffmpeg_available: bool,
    pub hardware_enabled: bool,
    pub usable_hardware: Vec<&'static str>,
    pub encoder: &'static str,
}

/// Which encoder transcodes would use right now (for the Hardware Transcoding panel).
#[tauri::command]
pub async fn player_transcode_status(
    state: TauriState<'_, AppState>,
) -> Result<TranscodeStatus, String> {
    let hardware_enabled = feature_flags::is_enabled_state(&state, "hw_transcode");
    tokio::task::spawn_blocking(move || {
        let ffmpeg_available = transcode::ffmpeg_available();
        let usable: &[transcode::Encoder] = if ffmpeg_available {
            transcode::usable_hardware_encoders()
        } else {
            &[]
        };
        TranscodeStatus {
            ffmpeg_available,
            hardware_enabled,
            usable_hardware: usable.iter().map(|e| e.label()).collect(),
            encoder: transcode::choose_encoder(hardware_enabled, usable).label(),
        }
    })
    .await
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::MediaItem;
    use serde_json::Value;

    fn temp_db() -> (Database, String) {
        let path = std::env::temp_dir()
            .join(format!("cinavault-player-{}.db", uuid::Uuid::new_v4()))
            .to_string_lossy()
            .into_owned();
        (Database::new(&path).unwrap(), path)
    }

    fn media(title: &str, path: &str) -> MediaItem {
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
            date_added: "2026-01-01T00:00:00Z".into(),
            last_played: None,
            tmdb_id: None,
            imdb_id: None,
            source_id: None,
        }
    }

    /// A short AVI (MPEG-4 Part 2 + MP2) that a WebView cannot play directly.
    fn sample_video() -> Option<PathBuf> {
        if !transcode::ffmpeg_available() {
            eprintln!("ffmpeg not installed; skipping live transcode check");
            return None;
        }
        let path =
            std::env::temp_dir().join(format!("cinavault-sample-{}.avi", uuid::Uuid::new_v4()));
        let ok = std::process::Command::new(transcode::ffmpeg_path())
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=d=4:s=320x240:r=25",
                "-f",
                "lavfi",
                "-i",
                "sine=d=4",
                "-c:v",
                "mpeg4",
                "-c:a",
                "mp2",
                "-y",
            ])
            .arg(&path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        ok.then_some(path)
    }

    async fn serve(router: Router) -> String {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        format!("http://{address}")
    }

    #[test]
    fn library_lookup_prefers_the_exact_copy_and_rejects_strangers() {
        let (db, path) = temp_db();
        let first = db
            .add_media_item_data(&media("Film", "/media/film-1080.mkv"))
            .unwrap();
        let copy = db
            .add_media_item_data(&media("Film", "/media/film-4k.mkv"))
            .unwrap();
        assert_eq!(
            resolve_library_file(&db, Some(first), "/media/film-4k.mkv").unwrap(),
            (copy, "/media/film-4k.mkv".to_string())
        );
        assert_eq!(
            resolve_library_file(&db, Some(first), "/elsewhere/renamed.mkv").unwrap(),
            (first, "/media/film-1080.mkv".to_string())
        );
        assert!(resolve_library_file(&db, None, "/etc/passwd").is_err());
        assert!(resolve_library_file(&db, Some(9_999), "/etc/passwd").is_err());
        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn tokens_compare_exactly() {
        let token = random_token();
        assert_eq!(token.len(), 64);
        assert_ne!(token, random_token());
        assert!(same_token(&token, &token.clone()));
        assert!(!same_token(&token, &token[..63]));
        assert!(!same_token("abc", "abd"));
    }

    #[tokio::test]
    async fn loopback_server_checks_token_and_direct_play_then_streams_fmp4() {
        let (db, db_path) = temp_db();
        let sample = sample_video();
        let file = sample
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/missing.avi".into());
        let id = db.add_media_item_data(&media("Sample", &file)).unwrap();
        let base = serve(loopback_router(db_path.clone(), "secret-token".into())).await;
        let client = reqwest::Client::new();

        let wrong = client
            .get(format!("{base}/guess/transcode/{id}"))
            .send()
            .await
            .unwrap();
        assert_eq!(wrong.status(), StatusCode::NOT_FOUND);

        db.set_feature_setting_data("direct_play", true, "{}")
            .unwrap();
        let blocked = client
            .get(format!("{base}/secret-token/transcode/{id}"))
            .send()
            .await
            .unwrap();
        assert_eq!(blocked.status(), StatusCode::CONFLICT);
        db.set_feature_setting_data("direct_play", false, "{}")
            .unwrap();

        if let Some(sample) = sample {
            db.set_setting_data("quality_control", "low").unwrap();
            let response = client
                .get(format!("{base}/secret-token/transcode/{id}?start=1.5"))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()["content-type"], "video/mp4");
            assert_eq!(response.headers()["access-control-allow-origin"], "*");
            let body = response.bytes().await.unwrap();
            assert!(
                body.len() > 1_000,
                "transcode produced {} bytes",
                body.len()
            );
            assert_eq!(&body[4..8], b"ftyp");
            assert!(
                body.windows(4).any(|w| w == b"moof"),
                "output is fragmented"
            );
            let _ = std::fs::remove_file(sample);
        }
        drop(db);
        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn embedded_server_transcodes_for_signed_in_clients_only() {
        let (db, db_path) = temp_db();
        let provision = db
            .create_remote_access_user(
                "viewer@example.com",
                &format!("Pw-{}!", uuid::Uuid::new_v4().simple()),
                Some("Viewer"),
            )
            .unwrap();
        let sample = sample_video();
        let file = sample
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/missing.avi".into());
        db.add_media_item_data(&media("Sample", &file)).unwrap();
        let base = serve(crate::embedded_server::router(db_path.clone())).await;
        let client = reqwest::Client::new();
        let login: Value = client
            .post(format!("{base}/api/auth/access-key"))
            .json(&serde_json::json!({ "accessKey": provision.access_key }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let token = login["session_token"].as_str().unwrap().to_string();
        let library: Vec<Value> = client
            .get(format!("{base}/api/library"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let key = library[0]["mediaKey"].as_str().unwrap().to_string();

        let anonymous = client
            .get(format!("{base}/api/transcode/{key}"))
            .send()
            .await
            .unwrap();
        assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

        db.set_feature_setting_data("direct_play", true, "{}")
            .unwrap();
        let blocked = client
            .get(format!("{base}/api/transcode/{key}?start=0"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        assert_eq!(blocked.status(), StatusCode::CONFLICT);
        db.set_feature_setting_data("direct_play", false, "{}")
            .unwrap();

        if let Some(sample) = sample {
            let response = client
                .get(format!("{base}/api/transcode/{key}?start=2"))
                .bearer_auth(&token)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()["content-type"], "video/mp4");
            let body = response.bytes().await.unwrap();
            assert_eq!(&body[4..8], b"ftyp");
            let _ = std::fs::remove_file(sample);
        }
        drop(db);
        let _ = std::fs::remove_file(db_path);
    }
}
