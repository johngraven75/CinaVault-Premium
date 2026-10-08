//! Shared artwork folder for the "poster_sync" switch.
//!
//! Export copies each item's poster and backdrop (local cache, sidecar or a
//! downloaded https URL) into `<folder>/<key>/poster.<ext>` and
//! `backdrop.<ext>`, where key is `tmdb-<id>`, `imdb-<id>` or `file-<hash of
//! the file name>`. Import fills items that have no artwork from that folder.
//! Point two PCs at the same synced folder (OneDrive, Dropbox, a NAS share)
//! and they share artwork without downloading it twice.

use crate::db::Database;
use crate::AppState;
use rusqlite::params;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tauri::State;

const LAST_RUN_SETTING: &str = "poster_sync_last_run";
const IMAGE_EXTS: &[&str] = &["jpg", "png", "webp", "gif"];
const MAX_ART_BYTES: usize = 25 * 1024 * 1024;
const LINK_FILE: &str = "key.txt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncSettings {
    pub folder: Option<PathBuf>,
    pub include_adult: bool,
}

pub fn settings(db: &Database) -> SyncSettings {
    let config = crate::feature_flags::config(db, "poster_sync");
    SyncSettings {
        folder: config
            .get("folder")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|folder| !folder.is_empty())
            .map(PathBuf::from),
        include_adult: config
            .get("includeAdult")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

fn file_key(file_path: &str) -> String {
    let name = file_path
        .rsplit(['/', '\\'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(file_path)
        .to_lowercase();
    let digest = Sha256::digest(name.as_bytes());
    let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    format!("file-{hex}")
}

fn safe_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric()))
        .map(str::to_ascii_lowercase)
}

/// Folder keys for an item, best first: TMDB id, IMDb id, file name hash.
pub fn sync_keys(tmdb_id: Option<&str>, imdb_id: Option<&str>, file_path: &str) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(id) = safe_id(tmdb_id) {
        keys.push(format!("tmdb-{id}"));
    }
    if let Some(id) = safe_id(imdb_id) {
        keys.push(format!("imdb-{id}"));
    }
    keys.push(file_key(file_path));
    keys
}

fn find_art(dir: &Path, kind: &str) -> Option<PathBuf> {
    IMAGE_EXTS
        .iter()
        .map(|ext| dir.join(format!("{kind}.{ext}")))
        .find(|path| path.is_file())
}

/// The shared image for `kind` under any of `keys`, following `key.txt` links.
pub fn find_shared(folder: &Path, keys: &[String], kind: &str) -> Option<PathBuf> {
    for key in keys {
        let dir = folder.join(key);
        if let Some(found) = find_art(&dir, kind) {
            return Some(found);
        }
        let linked = std::fs::read_to_string(dir.join(LINK_FILE)).ok();
        if let Some(target) = linked
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        {
            if let Some(found) = find_art(&folder.join(target), kind) {
                return Some(found);
            }
        }
    }
    None
}

struct ArtRow {
    id: i64,
    title: String,
    file_path: String,
    year: Option<i32>,
    tmdb_id: Option<String>,
    imdb_id: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    media_type: String,
}

fn load_rows(db: &Database, ids: Option<&[i64]>) -> Result<Vec<ArtRow>, String> {
    let mut statement = db
        .conn
        .prepare(
            "SELECT id, title, file_path, year, tmdb_id, imdb_id, poster_path, backdrop_path, media_type
             FROM media_items WHERE media_type <> 'music' ORDER BY id",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(ArtRow {
                id: row.get(0)?,
                title: row.get(1)?,
                file_path: row.get(2)?,
                year: row.get(3)?,
                tmdb_id: row.get(4)?,
                imdb_id: row.get(5)?,
                poster_path: row.get(6)?,
                backdrop_path: row.get(7)?,
                media_type: row.get(8)?,
            })
        })
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .filter(|row| ids.is_none_or(|ids| ids.contains(&row.id)))
        .collect();
    Ok(rows)
}

fn usable_local(path: Option<&str>) -> Option<PathBuf> {
    let path = path?.trim();
    if path.is_empty() || path.starts_with("http://") || path.starts_with("https://") {
        return None;
    }
    let path = PathBuf::from(path);
    path.is_file().then_some(path)
}

fn extension_of(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg")
        .to_ascii_lowercase();
    if ext == "jpeg" {
        "jpg".into()
    } else {
        ext
    }
}

/// Writes `bytes` as `<dir>/<kind>.<ext>` unless an identical file is there,
/// removing the same image saved under another extension. True when written.
fn store(dir: &Path, kind: &str, ext: &str, bytes: &[u8]) -> Result<bool, String> {
    std::fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let destination = dir.join(format!("{kind}.{ext}"));
    if std::fs::read(&destination).is_ok_and(|existing| existing == bytes) {
        return Ok(false);
    }
    crate::atomic_file::write_verified_atomic(&destination, bytes)?;
    for other in IMAGE_EXTS.iter().filter(|other| **other != ext) {
        std::fs::remove_file(dir.join(format!("{kind}.{other}"))).ok();
    }
    Ok(true)
}

async fn artwork_bytes(
    client: &reqwest::Client,
    reference: Option<&str>,
) -> Result<Option<(Vec<u8>, String)>, String> {
    if let Some(local) = usable_local(reference) {
        let bytes = std::fs::read(&local).map_err(|error| error.to_string())?;
        if bytes.is_empty() || bytes.len() > MAX_ART_BYTES {
            return Ok(None);
        }
        return Ok(Some((bytes, extension_of(&local))));
    }
    let Some(url) = reference
        .map(str::trim)
        .filter(|r| r.starts_with("https://"))
    else {
        return Ok(None);
    };
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("artwork download failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("artwork download failed: {error}"))?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("artwork download failed: {error}"))?;
    if bytes.is_empty() || bytes.len() > MAX_ART_BYTES {
        return Ok(None);
    }
    let Some((ext, _)) = crate::metadata_keyless::detect_image_type(&content_type, &bytes) else {
        return Err(format!("{url} is not an image"));
    };
    Ok(Some((bytes.to_vec(), ext.to_string())))
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SyncCounts {
    pub exported: usize,
    pub imported: usize,
    pub unchanged: usize,
    pub errors: Vec<String>,
}

fn include(row: &ArtRow, settings: &SyncSettings) -> bool {
    settings.include_adult || !row.media_type.eq_ignore_ascii_case("adult")
}

/// Copies the artwork of `ids` (all items when None) into the shared folder.
pub async fn export(state: &AppState, ids: Option<&[i64]>) -> Result<SyncCounts, String> {
    let (rows, settings) = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        (load_rows(&db, ids)?, settings(&db))
    };
    let folder = settings
        .folder
        .clone()
        .ok_or("No poster sync folder is set")?;
    let client = crate::metadata_keyless::http_client()?;
    let mut counts = SyncCounts::default();
    for row in rows.iter().filter(|row| include(row, &settings)) {
        if crate::task_progress::stop_requested() {
            break;
        }
        let keys = sync_keys(
            row.tmdb_id.as_deref(),
            row.imdb_id.as_deref(),
            &row.file_path,
        );
        let dir = folder.join(&keys[0]);
        let mut wrote_any = false;
        for (kind, reference) in [
            ("poster", row.poster_path.as_deref()),
            ("backdrop", row.backdrop_path.as_deref()),
        ] {
            match artwork_bytes(&client, reference).await {
                Ok(Some((bytes, ext))) => match store(&dir, kind, &ext, &bytes) {
                    Ok(true) => {
                        counts.exported += 1;
                        wrote_any = true;
                    }
                    Ok(false) => counts.unchanged += 1,
                    Err(error) => counts.errors.push(format!("{}: {error}", row.title)),
                },
                Ok(None) => {}
                Err(error) => counts.errors.push(format!("{}: {error}", row.title)),
            }
        }
        if wrote_any {
            let info = json!({
                "title": row.title,
                "year": row.year,
                "tmdbId": row.tmdb_id,
                "imdbId": row.imdb_id,
                "updatedAt": chrono::Utc::now().to_rfc3339(),
            });
            std::fs::write(dir.join("info.json"), info.to_string()).ok();
            // Let a PC that only knows the file name find the id folder.
            if let Some(file_key) = keys.last().filter(|key| *key != &keys[0]) {
                let link_dir = folder.join(file_key);
                if std::fs::create_dir_all(&link_dir).is_ok() {
                    std::fs::write(link_dir.join(LINK_FILE), &keys[0]).ok();
                }
            }
        }
    }
    Ok(counts)
}

fn missing(path: Option<&str>) -> bool {
    match path.map(str::trim) {
        None | Some("") => true,
        Some(value) if value.starts_with("https://") => false,
        Some(value) => !Path::new(value).is_file(),
    }
}

/// Fills items without artwork (of `ids`, or all) from the shared folder.
/// Images are copied into the app's artwork cache so an offline cloud
/// folder never breaks the library.
pub fn import(
    db: &Database,
    app_data_dir: &Path,
    ids: Option<&[i64]>,
) -> Result<SyncCounts, String> {
    let settings = settings(db);
    let folder = settings
        .folder
        .clone()
        .ok_or("No poster sync folder is set")?;
    let mut counts = SyncCounts::default();
    if !folder.is_dir() {
        return Err(format!(
            "Poster sync folder {} is not available",
            folder.display()
        ));
    }
    for row in load_rows(db, ids)?
        .iter()
        .filter(|row| include(row, &settings))
    {
        let keys = sync_keys(
            row.tmdb_id.as_deref(),
            row.imdb_id.as_deref(),
            &row.file_path,
        );
        for (kind, column, current) in [
            ("poster", "poster_path", row.poster_path.as_deref()),
            ("backdrop", "backdrop_path", row.backdrop_path.as_deref()),
        ] {
            if !missing(current) {
                continue;
            }
            let Some(shared) = find_shared(&folder, &keys, kind) else {
                continue;
            };
            let result = (|| {
                let bytes = std::fs::read(&shared).map_err(|error| error.to_string())?;
                if bytes.is_empty() || bytes.len() > MAX_ART_BYTES {
                    return Err("shared image is empty or too large".to_string());
                }
                if crate::metadata_keyless::detect_image_type("", &bytes).is_none() {
                    return Err("shared file is not an image".to_string());
                }
                let cache = app_data_dir.join("artwork").join(row.id.to_string());
                std::fs::create_dir_all(&cache).map_err(|error| error.to_string())?;
                let local = cache.join(format!("{kind}-sync.{}", extension_of(&shared)));
                crate::atomic_file::write_verified_atomic(&local, &bytes)?;
                db.conn
                    .execute(
                        &format!("UPDATE media_items SET {column} = ?1 WHERE id = ?2"),
                        params![local.to_string_lossy(), row.id],
                    )
                    .map_err(|error| error.to_string())?;
                Ok(())
            })();
            match result {
                Ok(()) => counts.imported += 1,
                Err(error) => counts.errors.push(format!("{}: {error}", row.title)),
            }
        }
    }
    Ok(counts)
}

fn record(db: &Database, summary: &Value) {
    db.set_setting_data(LAST_RUN_SETTING, &summary.to_string())
        .ok();
}

fn summary(kind: &str, export: Option<&SyncCounts>, import: Option<&SyncCounts>) -> Value {
    let errors: Vec<&String> = export
        .into_iter()
        .chain(import)
        .flat_map(|counts| counts.errors.iter())
        .take(20)
        .collect();
    json!({
        "trigger": kind,
        "at": chrono::Utc::now().to_rfc3339(),
        "exported": export.map(|c| c.exported).unwrap_or(0),
        "unchanged": export.map(|c| c.unchanged).unwrap_or(0),
        "imported": import.map(|c| c.imported).unwrap_or(0),
        "errors": errors,
    })
}

/// After a scan: import artwork for new items, export what they have.
pub async fn after_scan(state: &AppState, ids: &[i64]) -> Value {
    let imported = match state.db.lock() {
        Ok(db) => import(&db, &state.app_data_dir, Some(ids)),
        Err(error) => Err(error.to_string()),
    };
    let exported = export(state, Some(ids)).await;
    let mut errors = Vec::new();
    let import_counts = imported.map_err(|e| errors.push(e)).ok();
    let export_counts = exported.map_err(|e| errors.push(e)).ok();
    let mut value = summary("scan", export_counts.as_ref(), import_counts.as_ref());
    if !errors.is_empty() {
        value["errors"] = json!(errors);
    }
    if let Ok(db) = state.db.lock() {
        record(&db, &value);
    }
    value
}

#[tauri::command]
pub fn poster_sync_status(state: State<AppState>) -> Result<Value, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let settings = settings(&db);
    let last_run = db
        .get_setting_data(LAST_RUN_SETTING)
        .map_err(|error| error.to_string())?
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok());
    Ok(json!({
        "folder": settings.folder.as_ref().map(|f| f.to_string_lossy().to_string()),
        "folderAvailable": settings.folder.as_ref().is_some_and(|f| f.is_dir()),
        "includeAdult": settings.include_adult,
        "lastRun": last_run,
    }))
}

/// "Sync now": import missing artwork from the folder, then export all.
#[tauri::command]
pub async fn poster_sync_now(state: State<'_, AppState>) -> Result<Value, String> {
    let imported = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        if !crate::feature_flags::is_enabled(&db, "poster_sync") {
            return Err("Cloud Poster Sync is off".to_string());
        }
        import(&db, &state.app_data_dir, None)?
    };
    let exported = export(state.inner(), None).await?;
    let value = summary("manual", Some(&exported), Some(&imported));
    let db = state.db.lock().map_err(|error| error.to_string())?;
    record(&db, &value);
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cinavault-{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn keys_prefer_ids_and_are_path_independent() {
        let a = sync_keys(Some("949"), Some("tt0113277"), "C:\\Movies\\Heat.1995.mkv");
        assert_eq!(a[0], "tmdb-949");
        assert_eq!(a[1], "imdb-tt0113277");
        let b = sync_keys(None, None, "/mnt/nas/films/heat.1995.MKV");
        assert_eq!(
            a[2], b[0],
            "the same file name on another PC gives the same key"
        );
        assert!(b[0].starts_with("file-") && b[0].len() == 21);
        assert_eq!(sync_keys(Some("../x"), None, "a.mkv").len(), 1);
    }

    #[test]
    fn two_pcs_share_artwork_through_the_folder() {
        let shared = temp_dir("shared");
        let pc_a = temp_dir("pc-a");
        let pc_b = temp_dir("pc-b");
        let poster = pc_a.join("Heat-poster.jpg");
        std::fs::write(&poster, [0xFF, 0xD8, 0xFF, 1, 2, 3]).unwrap();
        let config = json!({ "folder": shared.to_string_lossy() }).to_string();

        // PC A: an enriched item with a poster exports it.
        let db_a = Database::new(":memory:").unwrap();
        db_a.set_feature_setting_data("poster_sync", true, &config)
            .unwrap();
        db_a.conn
            .execute(
                "INSERT INTO media_items (title, file_path, media_type, date_added, tmdb_id, poster_path)
                 VALUES ('Heat', 'D:/Films/Heat.1995.mkv', 'movie', 'now', '949', ?1)",
                params![poster.to_string_lossy()],
            )
            .unwrap();
        let state_a = AppState {
            db: std::sync::Mutex::new(db_a),
            app_data_dir: pc_a.clone(),
        };
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let counts = runtime.block_on(export(&state_a, None)).unwrap();
        assert_eq!(counts.exported, 1);
        assert!(shared.join("tmdb-949").join("poster.jpg").is_file());
        assert_eq!(
            runtime.block_on(export(&state_a, None)).unwrap().unchanged,
            1,
            "an unchanged poster is not copied again"
        );

        // PC B: the same file, not enriched yet, picks the poster up.
        let db_b = Database::new(":memory:").unwrap();
        db_b.set_feature_setting_data("poster_sync", true, &config)
            .unwrap();
        db_b.conn
            .execute(
                "INSERT INTO media_items (title, file_path, media_type, date_added)
                 VALUES ('Heat 1995', '/home/b/heat.1995.mkv', 'movie', 'now')",
                [],
            )
            .unwrap();
        let counts = import(&db_b, &pc_b, None).unwrap();
        assert_eq!(counts.imported, 1);
        let poster_b: String = db_b
            .conn
            .query_row("SELECT poster_path FROM media_items", [], |r| r.get(0))
            .unwrap();
        assert!(poster_b.starts_with(&*pc_b.to_string_lossy()));
        assert_eq!(
            std::fs::read(&poster_b).unwrap(),
            std::fs::read(&poster).unwrap()
        );

        for dir in [shared, pc_a, pc_b] {
            std::fs::remove_dir_all(dir).ok();
        }
    }
}
