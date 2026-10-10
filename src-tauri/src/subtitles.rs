//! OpenSubtitles downloads for the "subtitle_fetch" switch.
//!
//! Uses the OpenSubtitles REST API v1, which needs an API key. The key comes
//! from the provider key store (Settings > metadata providers, provider
//! "opensubtitles", kept in the OS credential store) or the OpenSubtitles
//! plugin config. Without a key nothing is requested and the run is reported
//! as skipped. Files are saved next to the video as `<name>.<lang>.srt`; a
//! language that already has a sidecar is skipped.

use crate::db::Database;
use crate::AppState;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::State;

const API_BASE: &str = "https://api.opensubtitles.com/api/v1";
const USER_AGENT: &str = concat!("CinaVault Premium v", env!("CARGO_PKG_VERSION"));
const LAST_RUN_SETTING: &str = "subtitle_fetch_last_run";
const SUBTITLE_EXTS: &[&str] = &["srt", "ass", "ssa", "vtt", "sub"];
const MAX_SUBTITLE_BYTES: usize = 5 * 1024 * 1024;
/// Gap between API calls; OpenSubtitles allows a few requests per second.
const REQUEST_GAP: Duration = Duration::from_millis(350);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleSettings {
    pub languages: Vec<String>,
    pub hearing_impaired: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleItemReport {
    pub item_id: i64,
    pub title: String,
    pub downloaded: Vec<DownloadedSubtitle>,
    pub already_present: Vec<String>,
    pub not_found: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadedSubtitle {
    pub language: String,
    pub path: String,
}

/// Lower-case language codes, deduplicated, in the order given ("en", "pt-br").
pub fn normalize_languages(raw: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for code in raw {
        let code = code.trim().to_ascii_lowercase().replace('_', "-");
        let valid = !code.is_empty()
            && code.len() <= 7
            && code.chars().all(|c| c.is_ascii_alphabetic() || c == '-');
        if valid && !out.contains(&code) {
            out.push(code);
        }
    }
    out
}

fn plugin_config(db: &Database) -> serde_json::Map<String, Value> {
    db.conn
        .query_row(
            "SELECT config_json FROM plugins WHERE plugin_key = 'opensubtitles'",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .ok()
        .flatten()
        .flatten()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

pub fn settings(db: &Database) -> SubtitleSettings {
    let feature = crate::feature_flags::config(db, "subtitle_fetch");
    let plugin = plugin_config(db);
    let strings = |value: Option<&Value>| -> Vec<String> {
        value
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut languages = normalize_languages(&strings(feature.get("languages")));
    if languages.is_empty() {
        languages = normalize_languages(&strings(plugin.get("languages")));
    }
    if languages.is_empty() {
        languages = vec!["en".to_string()];
    }
    let hearing_impaired = feature
        .get("hearingImpaired")
        .or_else(|| plugin.get("hearing_impaired"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    SubtitleSettings {
        languages,
        hearing_impaired,
    }
}

/// The OpenSubtitles API key, if the user stored one.
pub fn api_key(db: &Database) -> Option<String> {
    let stored = db
        .conn
        .query_row(
            "SELECT api_key FROM api_keys WHERE provider = 'opensubtitles'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten();
    let from_store = stored.and_then(|stored| {
        crate::metadata_ext::resolve_stored_provider_key("opensubtitles", &stored)
            .map_err(|error| log::warn!("OpenSubtitles key could not be read: {error}"))
            .ok()
            .flatten()
    });
    from_store
        .or_else(|| {
            plugin_config(db)
                .get("api_key")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

/// The OpenSubtitles file hash: size plus the 64-bit little-endian word sums
/// of the first and last 64 KiB.
pub fn movie_hash(path: &Path) -> std::io::Result<String> {
    const CHUNK: u64 = 64 * 1024;
    let mut file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();
    let mut hash = size;
    let mut add_chunk = |file: &mut std::fs::File, offset: u64| -> std::io::Result<()> {
        file.seek(SeekFrom::Start(offset))?;
        let mut buffer = vec![0u8; CHUNK.min(size) as usize];
        let read = file.read(&mut buffer)?;
        buffer.truncate(read - read % 8);
        for word in buffer.chunks_exact(8) {
            hash = hash.wrapping_add(u64::from_le_bytes(word.try_into().expect("8 bytes")));
        }
        Ok(())
    };
    add_chunk(&mut file, 0)?;
    add_chunk(&mut file, size.saturating_sub(CHUNK))?;
    Ok(format!("{hash:016x}"))
}

/// Whether a subtitle for `language` already sits next to `video`. A plain
/// `<name>.srt` counts for the first (preferred) language.
pub fn has_sidecar(video: &Path, language: &str, is_primary: bool) -> bool {
    let (Some(parent), Some(stem)) = (video.parent(), video.file_stem()) else {
        return false;
    };
    let stem = stem.to_string_lossy().to_ascii_lowercase();
    let short = language.split('-').next().unwrap_or(language);
    let Ok(entries) = std::fs::read_dir(parent) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        let Some((base, ext)) = name.rsplit_once('.') else {
            return false;
        };
        if !SUBTITLE_EXTS.contains(&ext) {
            return false;
        }
        if base == stem {
            return is_primary;
        }
        let Some(tags) = base.strip_prefix(&format!("{stem}.")) else {
            return false;
        };
        tags.split('.')
            .any(|tag| tag == language || tag == short || (short == "en" && tag == "eng"))
    })
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone)]
pub struct SearchTarget {
    pub query: String,
    pub year: Option<i32>,
    pub imdb_numeric: Option<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub moviehash: Option<String>,
}

/// Query parameters in the alphabetical, lower-case form the API asks for.
pub fn search_params(target: &SearchTarget, language: &str) -> Vec<(String, String)> {
    let mut params: Vec<(String, String)> = Vec::new();
    if let Some(episode) = target.episode {
        params.push(("episode_number".into(), episode.to_string()));
    }
    let episodic = target.episode.is_some();
    if let Some(imdb) = target.imdb_numeric.as_ref().filter(|_| !episodic) {
        params.push(("imdb_id".into(), imdb.clone()));
    }
    params.push(("languages".into(), language.to_string()));
    if let Some(hash) = &target.moviehash {
        params.push(("moviehash".into(), hash.clone()));
    }
    if target.imdb_numeric.is_none() || episodic {
        params.push(("query".into(), target.query.to_ascii_lowercase()));
    }
    if let Some(season) = target.season.filter(|_| episodic) {
        params.push(("season_number".into(), season.to_string()));
    }
    if let Some(year) = target
        .year
        .filter(|_| !episodic && target.imdb_numeric.is_none())
    {
        params.push(("year".into(), year.to_string()));
    }
    params.sort();
    params
}

/// The best file in a search response: a hash match first, then the most
/// downloaded, skipping hearing-impaired files unless asked for.
pub fn pick_file(response: &Value, hearing_impaired: bool) -> Option<(i64, String)> {
    let entries = response.get("data")?.as_array()?;
    let mut best: Option<(bool, bool, i64, i64, String)> = None;
    for entry in entries {
        let attributes = entry.get("attributes")?;
        let hi = attributes
            .get("hearing_impaired")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let hash_match = attributes
            .get("moviehash_match")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let downloads = attributes
            .get("download_count")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let Some(file) = attributes
            .get("files")
            .and_then(Value::as_array)
            .and_then(|files| files.first())
        else {
            continue;
        };
        let Some(file_id) = file.get("file_id").and_then(Value::as_i64) else {
            continue;
        };
        let name = file
            .get("file_name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let preferred_hi = hi == hearing_impaired;
        let candidate = (hash_match, preferred_hi, downloads, file_id, name);
        let better = best.as_ref().is_none_or(|current| {
            (candidate.0, candidate.1, candidate.2) > (current.0, current.1, current.2)
        });
        if better {
            best = Some(candidate);
        }
    }
    best.map(|(_, _, _, id, name)| (id, name))
}

async fn search(
    client: &reqwest::Client,
    key: &str,
    target: &SearchTarget,
    language: &str,
) -> Result<Value, String> {
    let response = client
        .get(format!("{API_BASE}/subtitles"))
        .header("Api-Key", key)
        .header("Accept", "application/json")
        .query(&search_params(target, language))
        .send()
        .await
        .map_err(|error| format!("search request failed: {error}"))?;
    let status = response.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err("OpenSubtitles rejected the API key".to_string());
    }
    if !status.is_success() {
        return Err(format!("search returned HTTP {status}"));
    }
    response
        .json::<Value>()
        .await
        .map_err(|error| format!("search response unreadable: {error}"))
}

async fn download(client: &reqwest::Client, key: &str, file_id: i64) -> Result<Vec<u8>, String> {
    let response = client
        .post(format!("{API_BASE}/download"))
        .header("Api-Key", key)
        .header("Accept", "application/json")
        .json(&json!({ "file_id": file_id, "sub_format": "srt" }))
        .send()
        .await
        .map_err(|error| format!("download request failed: {error}"))?;
    let status = response.status();
    let body = response
        .json::<Value>()
        .await
        .map_err(|error| format!("download response unreadable: {error}"))?;
    if !status.is_success() {
        let message = body
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("no message");
        return Err(format!("download refused (HTTP {status}): {message}"));
    }
    let link = body
        .get("link")
        .and_then(Value::as_str)
        .filter(|link| link.starts_with("https://"))
        .ok_or("download response had no link")?;
    let bytes = client
        .get(link)
        .send()
        .await
        .map_err(|error| format!("subtitle fetch failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("subtitle fetch failed: {error}"))?
        .bytes()
        .await
        .map_err(|error| format!("subtitle read failed: {error}"))?;
    if bytes.is_empty() || bytes.len() > MAX_SUBTITLE_BYTES {
        return Err("subtitle file was empty or too large".to_string());
    }
    Ok(bytes.to_vec())
}

pub fn sidecar_path(video: &Path, language: &str) -> Option<PathBuf> {
    let stem = video.file_stem()?.to_string_lossy().to_string();
    Some(video.parent()?.join(format!("{stem}.{language}.srt")))
}

struct ItemRow {
    title: String,
    file_path: String,
    year: Option<i32>,
    imdb_id: Option<String>,
    media_type: String,
}

fn load_item(db: &Database, id: i64) -> Result<ItemRow, String> {
    db.conn
        .query_row(
            "SELECT title, file_path, year, imdb_id, media_type FROM media_items WHERE id = ?1",
            params![id],
            |row| {
                Ok(ItemRow {
                    title: row.get(0)?,
                    file_path: row.get(1)?,
                    year: row.get(2)?,
                    imdb_id: row.get(3)?,
                    media_type: row.get(4)?,
                })
            },
        )
        .map_err(|error| format!("media item {id}: {error}"))
}

fn target_for(row: &ItemRow) -> SearchTarget {
    let path = Path::new(&row.file_path);
    let parsed = crate::title_clean::parse_path(path);
    let from_title = crate::title_clean::parse_release_name(&row.title);
    let episode = from_title.episode.or(parsed.episode);
    let query = if episode.is_some() {
        crate::title_clean::search_query(&row.title, &row.file_path)
    } else if from_title.title.is_empty() {
        parsed.title.clone()
    } else {
        from_title.title.clone()
    };
    SearchTarget {
        query,
        year: row.year.or(parsed.year),
        imdb_numeric: row
            .imdb_id
            .as_deref()
            .map(|id| id.trim().trim_start_matches("tt").to_string())
            .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit())),
        season: from_title.season.or(parsed.season),
        episode,
        moviehash: movie_hash(path).ok(),
    }
}

/// Finds and saves subtitles for one library item.
pub async fn fetch_for_item(
    state: &AppState,
    id: i64,
    key: &str,
    settings: &SubtitleSettings,
) -> SubtitleItemReport {
    let row = {
        let db = match state.db.lock() {
            Ok(db) => db,
            Err(error) => {
                return SubtitleItemReport {
                    item_id: id,
                    errors: vec![error.to_string()],
                    ..Default::default()
                }
            }
        };
        load_item(&db, id)
    };
    let mut report = SubtitleItemReport {
        item_id: id,
        ..Default::default()
    };
    let row = match row {
        Ok(row) => row,
        Err(error) => {
            report.errors.push(error);
            return report;
        }
    };
    report.title = row.title.clone();
    let video = Path::new(&row.file_path);
    if row.media_type == "music" || !video.is_file() {
        report
            .errors
            .push("not a video file on an available drive".to_string());
        return report;
    }
    let target = target_for(&row);
    let client = match client() {
        Ok(client) => client,
        Err(error) => {
            report.errors.push(error);
            return report;
        }
    };

    for (index, language) in settings.languages.iter().enumerate() {
        if has_sidecar(video, language, index == 0) {
            report.already_present.push(language.clone());
            continue;
        }
        let result = async {
            let found = search(&client, key, &target, language).await?;
            tokio::time::sleep(REQUEST_GAP).await;
            let Some((file_id, _)) = pick_file(&found, settings.hearing_impaired) else {
                return Ok(None);
            };
            let bytes = download(&client, key, file_id).await?;
            tokio::time::sleep(REQUEST_GAP).await;
            let destination = sidecar_path(video, language).ok_or("video has no file name")?;
            crate::atomic_file::write_verified_atomic(&destination, &bytes)?;
            Ok::<_, String>(Some(destination))
        }
        .await;
        match result {
            Ok(Some(path)) => report.downloaded.push(DownloadedSubtitle {
                language: language.clone(),
                path: path.to_string_lossy().to_string(),
            }),
            Ok(None) => report.not_found.push(language.clone()),
            Err(error) => {
                let fatal = error.contains("API key") || error.contains("HTTP 406");
                report.errors.push(format!("{language}: {error}"));
                if fatal {
                    break;
                }
            }
        }
    }
    report
}

/// Downloads subtitles for `ids` (the items a scan added). Records the run
/// for the settings panel and reports downloads to the activity log.
pub async fn fetch_for_items(state: &AppState, ids: &[i64]) -> Value {
    let (key, settings) = match state.db.lock() {
        Ok(db) => (api_key(&db), settings(&db)),
        Err(error) => return json!({ "status": "failed", "error": error.to_string() }),
    };
    let started = chrono::Utc::now().to_rfc3339();
    let summary = match key {
        None => json!({
            "status": "skipped_no_key",
            "at": started,
            "items": ids.len(),
            "message": "No OpenSubtitles API key is stored, so no subtitles were requested.",
        }),
        Some(key) => {
            let mut reports = Vec::new();
            for id in ids {
                if crate::task_progress::stop_requested() {
                    break;
                }
                reports.push(fetch_for_item(state, *id, &key, &settings).await);
            }
            let downloaded: usize = reports.iter().map(|r| r.downloaded.len()).sum();
            let errors: Vec<String> = reports
                .iter()
                .flat_map(|r| r.errors.iter().map(move |e| format!("{}: {e}", r.title)))
                .take(20)
                .collect();
            json!({
                "status": if errors.is_empty() { "success" } else { "partial" },
                "at": started,
                "items": reports.len(),
                "downloaded": downloaded,
                "alreadyPresent": reports.iter().map(|r| r.already_present.len()).sum::<usize>(),
                "notFound": reports.iter().map(|r| r.not_found.len()).sum::<usize>(),
                "errors": errors,
                "files": reports.iter().flat_map(|r| r.downloaded.iter().map(|d| d.path.clone())).take(50).collect::<Vec<_>>(),
            })
        }
    };
    if let Ok(db) = state.db.lock() {
        db.set_setting_data(LAST_RUN_SETTING, &summary.to_string())
            .ok();
        let downloaded = summary
            .get("downloaded")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if downloaded > 0 {
            crate::user_data::emit(
                &db,
                "subtitles.downloaded",
                &format!("Downloaded {downloaded} subtitle file(s)"),
                summary.clone(),
            );
        }
    }
    summary
}

#[tauri::command]
pub fn subtitles_status(state: State<AppState>) -> Result<Value, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let settings = settings(&db);
    let last_run = db
        .get_setting_data(LAST_RUN_SETTING)
        .map_err(|error| error.to_string())?
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok());
    Ok(json!({
        "keyConfigured": api_key(&db).is_some(),
        "languages": settings.languages,
        "hearingImpaired": settings.hearing_impaired,
        "lastRun": last_run,
    }))
}

/// "Find subtitles" for one item, whatever the switch says.
#[tauri::command]
pub async fn subtitles_find(
    state: State<'_, AppState>,
    id: i64,
) -> Result<SubtitleItemReport, String> {
    let (key, settings) = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        (api_key(&db), settings(&db))
    };
    let key = key.ok_or(
        "No OpenSubtitles API key is stored. Add one under Auto Subtitle Download in Advanced > Feature Matrix.",
    )?;
    let report = fetch_for_item(state.inner(), id, &key, &settings).await;
    if !report.downloaded.is_empty() {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        crate::user_data::emit(
            &db,
            "subtitles.downloaded",
            &format!("Subtitles for {}", report.title),
            serde_json::to_value(&report).unwrap_or(Value::Null),
        );
    }
    Ok(report)
}

/// A text subtitle next to a video that the built-in player can show.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSubtitle {
    /// BCP 47-style tag from the file name (`movie.pt-br.srt` -> "pt-br"),
    /// or "und" for a plain `movie.srt`.
    pub language: String,
    pub label: String,
    /// The subtitle as WebVTT, ready for a `<track>`.
    pub vtt: String,
}

/// SubRip and WebVTT sidecars for `video`, converted to WebVTT. Other formats
/// (ASS, SSA, VobSub) are left to external players. Unreadable, oversized or
/// empty files are skipped.
pub fn player_subtitles(video: &Path) -> Vec<PlayerSubtitle> {
    let (Some(parent), Some(stem)) = (video.parent(), video.file_stem()) else {
        return Vec::new();
    };
    let stem = stem.to_string_lossy().to_ascii_lowercase();
    let Ok(entries) = std::fs::read_dir(parent) else {
        return Vec::new();
    };
    let mut found: Vec<(String, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            let (base, ext) = name.rsplit_once('.')?;
            if ext != "srt" && ext != "vtt" {
                return None;
            }
            let language = if base == stem {
                "und".to_string()
            } else {
                let tags = base.strip_prefix(&format!("{stem}."))?;
                tags.split('.')
                    .find(|tag| is_language_tag(tag))?
                    .to_string()
            };
            Some((language, entry.path()))
        })
        .collect();
    found.sort();
    found
        .into_iter()
        .filter_map(|(language, path)| {
            let bytes = std::fs::read(&path).ok()?;
            if bytes.is_empty() || bytes.len() > MAX_SUBTITLE_BYTES {
                return None;
            }
            let vtt = to_webvtt(&String::from_utf8_lossy(&bytes));
            let label = if language == "und" {
                "Subtitles".to_string()
            } else {
                language.to_uppercase()
            };
            Some(PlayerSubtitle {
                language,
                label,
                vtt,
            })
        })
        .collect()
}

/// "en", "eng", "pt-br", "zh-hans": letters with at most one hyphen part.
fn is_language_tag(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let primary = parts.next().unwrap_or("");
    let rest: Vec<&str> = parts.collect();
    (2..=3).contains(&primary.len())
        && primary.bytes().all(|b| b.is_ascii_lowercase())
        && rest.len() <= 1
        && rest.iter().all(|part| {
            (2..=4).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_alphanumeric())
        })
}

/// Converts SubRip text to WebVTT (WebVTT input passes through): drops a
/// byte-order mark, normalises line endings, and switches the millisecond
/// separator on cue timing lines from "," to ".".
pub fn to_webvtt(text: &str) -> String {
    let text = text
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    if text.trim_start().starts_with("WEBVTT") {
        return text;
    }
    let mut out = String::with_capacity(text.len() + 8);
    out.push_str("WEBVTT\n\n");
    for line in text.lines() {
        if line.contains("-->") {
            out.push_str(&line.replace(',', "."));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cinavault-subs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn movie_hash_matches_the_reference_algorithm() {
        let dir = temp_dir();
        let path = dir.join("sample.bin");
        // 200 KiB of bytes 0..=255 repeating: both chunks hold the same words.
        let bytes: Vec<u8> = (0..200 * 1024).map(|i| (i % 256) as u8).collect();
        std::fs::write(&path, &bytes).unwrap();
        let mut expected = bytes.len() as u64;
        for chunk in [&bytes[..65536], &bytes[bytes.len() - 65536..]] {
            for word in chunk.chunks_exact(8) {
                expected = expected.wrapping_add(u64::from_le_bytes(word.try_into().unwrap()));
            }
        }
        assert_eq!(movie_hash(&path).unwrap(), format!("{expected:016x}"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn existing_sidecars_are_detected_per_language() {
        let dir = temp_dir();
        let video = dir.join("Heat (1995).mkv");
        std::fs::write(&video, b"x").unwrap();
        assert!(!has_sidecar(&video, "en", true));
        std::fs::write(dir.join("Heat (1995).srt"), b"1").unwrap();
        assert!(has_sidecar(&video, "en", true));
        assert!(!has_sidecar(&video, "fr", false));
        std::fs::write(dir.join("Heat (1995).fr.forced.srt"), b"1").unwrap();
        assert!(has_sidecar(&video, "fr", false));
        std::fs::write(dir.join("Heat (1995).pt.ass"), b"1").unwrap();
        assert!(has_sidecar(&video, "pt-br", false));
        assert_eq!(
            sidecar_path(&video, "de").unwrap(),
            dir.join("Heat (1995).de.srt")
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn search_params_are_sorted_and_episode_aware() {
        let movie = SearchTarget {
            query: "Heat".into(),
            year: Some(1995),
            imdb_numeric: Some("113277".into()),
            season: None,
            episode: None,
            moviehash: Some("8e245d9679d31e12".into()),
        };
        assert_eq!(
            search_params(&movie, "en"),
            vec![
                ("imdb_id".to_string(), "113277".to_string()),
                ("languages".to_string(), "en".to_string()),
                ("moviehash".to_string(), "8e245d9679d31e12".to_string()),
            ]
        );
        let episode = SearchTarget {
            query: "Breaking Bad".into(),
            year: None,
            imdb_numeric: Some("1".into()),
            season: Some(1),
            episode: Some(2),
            moviehash: None,
        };
        assert_eq!(
            search_params(&episode, "pt-br"),
            vec![
                ("episode_number".to_string(), "2".to_string()),
                ("languages".to_string(), "pt-br".to_string()),
                ("query".to_string(), "breaking bad".to_string()),
                ("season_number".to_string(), "1".to_string()),
            ]
        );
    }

    #[test]
    fn picks_hash_matches_then_popular_files() {
        let response = json!({ "data": [
            { "attributes": { "download_count": 900, "hearing_impaired": false, "moviehash_match": false,
                "files": [{ "file_id": 1, "file_name": "popular.srt" }] } },
            { "attributes": { "download_count": 10, "hearing_impaired": false, "moviehash_match": true,
                "files": [{ "file_id": 2, "file_name": "exact.srt" }] } },
            { "attributes": { "download_count": 5000, "hearing_impaired": true, "moviehash_match": false,
                "files": [{ "file_id": 3, "file_name": "sdh.srt" }] } }
        ]});
        assert_eq!(
            pick_file(&response, false),
            Some((2, "exact.srt".to_string()))
        );
        let no_hash = json!({ "data": [
            { "attributes": { "download_count": 900, "hearing_impaired": false, "files": [{ "file_id": 1 }] } },
            { "attributes": { "download_count": 5000, "hearing_impaired": true, "files": [{ "file_id": 3 }] } }
        ]});
        assert_eq!(pick_file(&no_hash, false).map(|f| f.0), Some(1));
        assert_eq!(pick_file(&no_hash, true).map(|f| f.0), Some(3));
        assert_eq!(pick_file(&json!({ "data": [] }), false), None);
    }

    #[test]
    fn languages_are_normalized() {
        assert_eq!(
            normalize_languages(&[
                "EN".into(),
                " pt_BR ".into(),
                "en".into(),
                "bad code!".into()
            ]),
            vec!["en", "pt-br"]
        );
    }

    #[test]
    fn settings_and_key_fall_back_cleanly() {
        let db = Database::new(":memory:").unwrap();
        let settings = settings(&db);
        assert_eq!(settings.languages, vec!["en"]);
        db.set_feature_setting_data(
            "subtitle_fetch",
            true,
            r#"{"languages":["fr","de"],"hearingImpaired":true}"#,
        )
        .unwrap();
        let settings = super::settings(&db);
        assert_eq!(settings.languages, vec!["fr", "de"]);
        assert!(settings.hearing_impaired);
        // The seeded plugin config has an empty key: that is "no key".
        db.conn
            .execute("DELETE FROM api_keys WHERE provider = 'opensubtitles'", [])
            .ok();
        assert_eq!(api_key(&db), None);
    }

    #[test]
    fn srt_becomes_webvtt() {
        let srt = "\u{feff}1\r\n00:00:01,500 --> 00:00:03,250\r\nHello, world\r\n\r\n";
        assert_eq!(
            to_webvtt(srt),
            "WEBVTT\n\n1\n00:00:01.500 --> 00:00:03.250\nHello, world\n\n"
        );
        let vtt = "WEBVTT\n\n00:01.000 --> 00:02.000\nHi\n";
        assert_eq!(to_webvtt(vtt), vtt);
    }

    #[test]
    fn player_subtitles_lists_srt_and_vtt_sidecars_only() {
        let dir = temp_dir();
        let video = dir.join("Movie.mkv");
        std::fs::write(&video, b"x").unwrap();
        std::fs::write(
            dir.join("Movie.en.srt"),
            "1\n00:00:01,000 --> 00:00:02,000\nHi\n",
        )
        .unwrap();
        std::fs::write(dir.join("movie.pt-br.forced.vtt"), "WEBVTT\n\n").unwrap();
        std::fs::write(
            dir.join("Movie.srt"),
            "1\n00:00:01,000 --> 00:00:02,000\nPlain\n",
        )
        .unwrap();
        std::fs::write(dir.join("Movie.fr.ass"), "[Script Info]").unwrap();
        std::fs::write(dir.join("Other.en.srt"), "x").unwrap();
        std::fs::write(dir.join("Movie.de.srt"), "").unwrap();

        let subs = player_subtitles(&video);
        let languages: Vec<&str> = subs.iter().map(|s| s.language.as_str()).collect();
        assert_eq!(languages, ["en", "pt-br", "und"]);
        assert!(subs[0].vtt.contains("00:00:01.000 --> 00:00:02.000"));
        assert_eq!(subs[0].label, "EN");
        assert_eq!(subs[2].label, "Subtitles");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn language_tags() {
        for ok in ["en", "eng", "pt-br", "zh-hans"] {
            assert!(is_language_tag(ok), "{ok}");
        }
        for bad in ["forced", "e", "en-us-x", "1080p"] {
            assert!(!is_language_tag(bad), "{bad}");
        }
    }
}
