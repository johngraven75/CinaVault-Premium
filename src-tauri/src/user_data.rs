//! Per-viewer state behind the feature switches: profiles, playback progress,
//! the watchlist and the activity log, plus the event hook that feeds the
//! activity log and outgoing webhooks. Everything is keyed by profile, and with
//! "Multiple User Profiles" off every viewer is the built-in profile 1.

use crate::db::{Database, MediaItem};
use crate::feature_flags;
use crate::AppState;
use rusqlite::{params, OptionalExtension, Result as SqlResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;

pub const DEFAULT_PROFILE_ID: i64 = 1;
const ACTIVE_PROFILE_SETTING: &str = "active_profile_id";
/// A title counts as finished once this much of it has played.
pub const FINISHED_FRACTION: f64 = 0.92;
const ACTIVITY_LOG_KEEP: i64 = 5000;

/// media_items columns in the order Database::row_to_media reads them.
pub const MEDIA_COLUMNS: &str = "m.id, m.title, m.file_path, m.media_type, m.year, m.rating, m.overview, \
    m.poster_path, m.backdrop_path, m.genre, m.duration, m.file_size, m.resolution, m.codec, m.verified, \
    m.watched, m.favorite, m.date_added, m.last_played, m.tmdb_id, m.imdb_id, m.source_id";

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub color: String,
    /// Parental controls apply to this profile.
    pub restricted: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Progress {
    pub media_id: i64,
    pub position: f64,
    pub duration: f64,
    pub finished: bool,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ProgressItem {
    pub item: MediaItem,
    pub progress: Progress,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ActivityEntry {
    pub id: i64,
    pub at: String,
    pub profile_id: i64,
    pub kind: String,
    pub title: String,
    pub detail: Value,
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn ensure_tables(db: &Database) -> SqlResult<()> {
    db.conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS profiles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            color TEXT NOT NULL DEFAULT '#38bdf8',
            restricted INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS playback_progress (
            profile_id INTEGER NOT NULL,
            media_id INTEGER NOT NULL,
            position REAL NOT NULL DEFAULT 0,
            duration REAL NOT NULL DEFAULT 0,
            finished INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (profile_id, media_id)
        );
        CREATE TABLE IF NOT EXISTS watchlist (
            profile_id INTEGER NOT NULL,
            media_id INTEGER NOT NULL,
            added_at TEXT NOT NULL,
            PRIMARY KEY (profile_id, media_id)
        );
        CREATE TABLE IF NOT EXISTS activity_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            at TEXT NOT NULL,
            profile_id INTEGER NOT NULL,
            kind TEXT NOT NULL,
            title TEXT NOT NULL,
            detail_json TEXT NOT NULL DEFAULT '{}'
        );
        CREATE INDEX IF NOT EXISTS idx_progress_updated ON playback_progress(profile_id, updated_at);
        CREATE INDEX IF NOT EXISTS idx_activity_at ON activity_log(at);",
    )?;
    db.conn.execute(
        "INSERT OR IGNORE INTO profiles (id, name, color, restricted, created_at) VALUES (?1, 'Owner', '#38bdf8', 0, ?2)",
        params![DEFAULT_PROFILE_ID, now()],
    )?;
    Ok(())
}

// ------------------------------------------------------------- profiles ----

fn row_to_profile(row: &rusqlite::Row) -> rusqlite::Result<Profile> {
    Ok(Profile {
        id: row.get(0)?,
        name: row.get(1)?,
        color: row.get(2)?,
        restricted: row.get(3)?,
        created_at: row.get(4)?,
    })
}

pub fn list_profiles(db: &Database) -> SqlResult<Vec<Profile>> {
    let mut stmt = db
        .conn
        .prepare("SELECT id, name, color, restricted, created_at FROM profiles ORDER BY id")?;
    let rows = stmt.query_map([], row_to_profile)?;
    rows.collect()
}

pub fn get_profile(db: &Database, id: i64) -> SqlResult<Option<Profile>> {
    db.conn
        .query_row(
            "SELECT id, name, color, restricted, created_at FROM profiles WHERE id = ?1",
            params![id],
            row_to_profile,
        )
        .optional()
}

/// The profile everything is recorded against: the chosen one while
/// "Multiple User Profiles" is on, otherwise the built-in profile.
pub fn active_profile_id(db: &Database) -> i64 {
    if !feature_flags::is_enabled(db, "user_profiles") {
        return DEFAULT_PROFILE_ID;
    }
    db.get_setting_data(ACTIVE_PROFILE_SETTING)
        .ok()
        .flatten()
        .and_then(|raw| raw.parse::<i64>().ok())
        .filter(|id| get_profile(db, *id).ok().flatten().is_some())
        .unwrap_or(DEFAULT_PROFILE_ID)
}

pub fn active_profile(db: &Database) -> SqlResult<Profile> {
    let id = active_profile_id(db);
    Ok(get_profile(db, id)?.unwrap_or(Profile {
        id: DEFAULT_PROFILE_ID,
        name: "Owner".into(),
        color: "#38bdf8".into(),
        restricted: false,
        created_at: now(),
    }))
}

pub fn create_profile(
    db: &Database,
    name: &str,
    color: &str,
    restricted: bool,
) -> Result<Profile, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err("A profile name needs 1 to 40 characters".into());
    }
    let color = if color.trim().is_empty() {
        "#38bdf8"
    } else {
        color.trim()
    };
    db.conn
        .execute(
            "INSERT INTO profiles (name, color, restricted, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![name, color, restricted, now()],
        )
        .map_err(|e| e.to_string())?;
    let id = db.conn.last_insert_rowid();
    get_profile(db, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Profile vanished".into())
}

pub fn update_profile(
    db: &Database,
    id: i64,
    name: &str,
    color: &str,
    restricted: bool,
) -> Result<Profile, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err("A profile name needs 1 to 40 characters".into());
    }
    let changed = db
        .conn
        .execute(
            "UPDATE profiles SET name = ?2, color = ?3, restricted = ?4 WHERE id = ?1",
            params![id, name, color, restricted],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err(format!("No profile {id}"));
    }
    get_profile(db, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Profile vanished".into())
}

pub fn delete_profile(db: &Database, id: i64) -> Result<(), String> {
    if id == DEFAULT_PROFILE_ID {
        return Err("The Owner profile cannot be deleted".into());
    }
    for sql in [
        "DELETE FROM playback_progress WHERE profile_id = ?1",
        "DELETE FROM watchlist WHERE profile_id = ?1",
        "DELETE FROM profiles WHERE id = ?1",
    ] {
        db.conn
            .execute(sql, params![id])
            .map_err(|e| e.to_string())?;
    }
    if db
        .get_setting_data(ACTIVE_PROFILE_SETTING)
        .ok()
        .flatten()
        .as_deref()
        == Some(&id.to_string())
    {
        db.set_setting_data(ACTIVE_PROFILE_SETTING, &DEFAULT_PROFILE_ID.to_string())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn set_active_profile(db: &Database, id: i64) -> Result<Profile, String> {
    let profile = get_profile(db, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("No profile {id}"))?;
    db.set_setting_data(ACTIVE_PROFILE_SETTING, &id.to_string())
        .map_err(|e| e.to_string())?;
    Ok(profile)
}

// ------------------------------------------------------------- progress ----

fn row_to_progress(row: &rusqlite::Row, offset: usize) -> rusqlite::Result<Progress> {
    Ok(Progress {
        media_id: row.get(offset)?,
        position: row.get(offset + 1)?,
        duration: row.get(offset + 2)?,
        finished: row.get(offset + 3)?,
        updated_at: row.get(offset + 4)?,
    })
}

/// Whether a position counts as having watched the whole title.
pub fn is_finished(position: f64, duration: f64) -> bool {
    duration > 0.0 && (position / duration >= FINISHED_FRACTION || duration - position <= 30.0)
}

pub fn save_progress(
    db: &Database,
    profile_id: i64,
    media_id: i64,
    position: f64,
    duration: f64,
) -> SqlResult<Progress> {
    let position = if position.is_finite() {
        position.max(0.0)
    } else {
        0.0
    };
    let duration = if duration.is_finite() {
        duration.max(0.0)
    } else {
        0.0
    };
    let finished = is_finished(position, duration);
    let at = now();
    db.conn.execute(
        "INSERT INTO playback_progress (profile_id, media_id, position, duration, finished, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(profile_id, media_id) DO UPDATE SET
           position = excluded.position, duration = excluded.duration,
           finished = excluded.finished, updated_at = excluded.updated_at",
        params![profile_id, media_id, position, duration, finished, at],
    )?;
    db.conn.execute(
        "UPDATE media_items SET last_played = ?2, watched = CASE WHEN ?3 THEN 1 ELSE watched END WHERE id = ?1",
        params![media_id, at, finished],
    )?;
    Ok(Progress {
        media_id,
        position,
        duration,
        finished,
        updated_at: at,
    })
}

pub fn get_progress(db: &Database, profile_id: i64, media_id: i64) -> SqlResult<Option<Progress>> {
    db.conn
        .query_row(
            "SELECT media_id, position, duration, finished, updated_at FROM playback_progress
             WHERE profile_id = ?1 AND media_id = ?2",
            params![profile_id, media_id],
            |row| row_to_progress(row, 0),
        )
        .optional()
}

/// Titles started but not finished, most recent first.
pub fn continue_watching(
    db: &Database,
    profile_id: i64,
    limit: i64,
) -> SqlResult<Vec<ProgressItem>> {
    let sql = format!(
        "SELECT {MEDIA_COLUMNS}, p.media_id, p.position, p.duration, p.finished, p.updated_at
         FROM playback_progress p JOIN media_items m ON m.id = p.media_id
         WHERE p.profile_id = ?1 AND p.finished = 0 AND p.position >= 15
         ORDER BY p.updated_at DESC LIMIT ?2"
    );
    let mut stmt = db.conn.prepare(&sql)?;
    let rows = stmt.query_map(params![profile_id, limit.clamp(1, 200)], |row| {
        Ok(ProgressItem {
            item: Database::row_to_media(row)?,
            progress: row_to_progress(row, 22)?,
        })
    })?;
    rows.collect()
}

pub fn clear_progress(db: &Database, profile_id: i64, media_id: i64) -> SqlResult<()> {
    db.conn.execute(
        "DELETE FROM playback_progress WHERE profile_id = ?1 AND media_id = ?2",
        params![profile_id, media_id],
    )?;
    Ok(())
}

// ------------------------------------------------------------ watchlist ----

/// Adds or removes a title; returns whether it is now on the watchlist.
pub fn toggle_watchlist(db: &Database, profile_id: i64, media_id: i64) -> SqlResult<bool> {
    let removed = db.conn.execute(
        "DELETE FROM watchlist WHERE profile_id = ?1 AND media_id = ?2",
        params![profile_id, media_id],
    )?;
    if removed > 0 {
        return Ok(false);
    }
    db.conn.execute(
        "INSERT INTO watchlist (profile_id, media_id, added_at) VALUES (?1, ?2, ?3)",
        params![profile_id, media_id, now()],
    )?;
    Ok(true)
}

pub fn watchlist(db: &Database, profile_id: i64) -> SqlResult<Vec<MediaItem>> {
    let sql = format!(
        "SELECT {MEDIA_COLUMNS} FROM watchlist w JOIN media_items m ON m.id = w.media_id
         WHERE w.profile_id = ?1 ORDER BY w.added_at DESC"
    );
    let mut stmt = db.conn.prepare(&sql)?;
    let rows = stmt.query_map(params![profile_id], Database::row_to_media)?;
    rows.collect()
}

// ------------------------------------------------------- events + log ----

/// Records something that happened. It lands in the activity log while
/// "Activity Logging" is on, and goes to every webhook URL while "Webhook
/// Notifications" is on (when its config lists no events, or lists this kind).
pub fn emit(db: &Database, kind: &str, title: &str, detail: Value) {
    let profile_id = active_profile_id(db);
    let at = now();
    if feature_flags::is_enabled(db, "activity_log") {
        let result = db.conn.execute(
            "INSERT INTO activity_log (at, profile_id, kind, title, detail_json) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![at, profile_id, kind, title, detail.to_string()],
        );
        if let Err(error) = result {
            log::warn!("Activity log write failed: {error}");
        }
        db.conn
            .execute(
                "DELETE FROM activity_log WHERE id <= (SELECT MAX(id) FROM activity_log) - ?1",
                params![ACTIVITY_LOG_KEEP],
            )
            .ok();
    }
    if feature_flags::is_enabled(db, "webhook") {
        let config = feature_flags::config(db, "webhook");
        let targets = webhook_targets(&config, kind);
        if !targets.is_empty() {
            let payload = json!({
                "event": kind,
                "title": title,
                "detail": detail,
                "at": at,
                "profileId": profile_id,
                "source": "CinaVault Premium",
            });
            for url in targets {
                send_webhook(url, payload.clone());
            }
        }
    }
}

/// URLs to notify for `kind`, from the webhook config `{urls: [...], events: [...]}`.
pub fn webhook_targets(config: &serde_json::Map<String, Value>, kind: &str) -> Vec<String> {
    let wanted: Vec<&str> = config
        .get("events")
        .and_then(Value::as_array)
        .map(|events| events.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if !wanted.is_empty()
        && !wanted
            .iter()
            .any(|event| *event == kind || kind.starts_with(&format!("{event}.")))
    {
        return Vec::new();
    }
    config
        .get("urls")
        .and_then(Value::as_array)
        .map(|urls| {
            urls.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|url| url.starts_with("https://") || url.starts_with("http://"))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

fn send_webhook(url: String, payload: Value) {
    #[cfg(test)]
    {
        let _ = (url, payload);
    }
    #[cfg(not(test))]
    tauri::async_runtime::spawn(async move {
        let _ = post_webhook(&url, &payload).await;
    });
}

async fn post_webhook(url: &str, payload: &Value) -> Result<u16, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .post(url)
        .header("User-Agent", "CinaVault-Premium-Webhook")
        .json(payload)
        .send()
        .await
        .map_err(|e| {
            log::warn!("Webhook to {url} failed: {e}");
            e.to_string()
        })?;
    Ok(response.status().as_u16())
}

pub fn activity(db: &Database, limit: i64) -> SqlResult<Vec<ActivityEntry>> {
    let mut stmt = db.conn.prepare(
        "SELECT id, at, profile_id, kind, title, detail_json FROM activity_log ORDER BY id DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit.clamp(1, 1000)], |row| {
        let raw: String = row.get(5)?;
        Ok(ActivityEntry {
            id: row.get(0)?,
            at: row.get(1)?,
            profile_id: row.get(2)?,
            kind: row.get(3)?,
            title: row.get(4)?,
            detail: serde_json::from_str(&raw).unwrap_or(Value::Null),
        })
    })?;
    rows.collect()
}

// ------------------------------------------------------------- commands ----

fn with_db<T>(
    state: &State<AppState>,
    f: impl FnOnce(&Database) -> Result<T, String>,
) -> Result<T, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    ensure_tables(&db).map_err(|e| e.to_string())?;
    f(&db)
}

#[tauri::command]
pub fn profiles_list(state: State<AppState>) -> Result<Vec<Profile>, String> {
    with_db(&state, |db| list_profiles(db).map_err(|e| e.to_string()))
}

#[tauri::command]
pub fn profile_active(state: State<AppState>) -> Result<Profile, String> {
    with_db(&state, |db| active_profile(db).map_err(|e| e.to_string()))
}

#[tauri::command]
pub fn profile_create(
    state: State<AppState>,
    name: String,
    color: Option<String>,
    restricted: Option<bool>,
) -> Result<Profile, String> {
    with_db(&state, |db| {
        let profile = create_profile(
            db,
            &name,
            color.as_deref().unwrap_or(""),
            restricted.unwrap_or(false),
        )?;
        emit(
            db,
            "profile.created",
            &profile.name,
            json!({ "profileId": profile.id }),
        );
        Ok(profile)
    })
}

#[tauri::command]
pub fn profile_update(
    state: State<AppState>,
    id: i64,
    name: String,
    color: String,
    restricted: bool,
) -> Result<Profile, String> {
    with_db(&state, |db| {
        update_profile(db, id, &name, &color, restricted)
    })
}

#[tauri::command]
pub fn profile_delete(state: State<AppState>, id: i64) -> Result<(), String> {
    with_db(&state, |db| delete_profile(db, id))
}

#[tauri::command]
pub fn profile_switch(state: State<AppState>, id: i64) -> Result<Profile, String> {
    with_db(&state, |db| {
        let profile = set_active_profile(db, id)?;
        emit(
            db,
            "profile.switched",
            &profile.name,
            json!({ "profileId": profile.id }),
        );
        Ok(profile)
    })
}

#[tauri::command]
pub fn playback_progress_save(
    state: State<AppState>,
    media_id: i64,
    position: f64,
    duration: f64,
) -> Result<Progress, String> {
    with_db(&state, |db| {
        let profile = active_profile_id(db);
        let before = get_progress(db, profile, media_id).map_err(|e| e.to_string())?;
        let progress =
            save_progress(db, profile, media_id, position, duration).map_err(|e| e.to_string())?;
        if progress.finished && !before.map(|p| p.finished).unwrap_or(false) {
            let title = db
                .conn
                .query_row(
                    "SELECT title FROM media_items WHERE id = ?1",
                    params![media_id],
                    |row| row.get::<_, String>(0),
                )
                .unwrap_or_default();
            emit(
                db,
                "playback.finished",
                &title,
                json!({ "mediaId": media_id }),
            );
        }
        Ok(progress)
    })
}

#[tauri::command]
pub fn playback_progress_get(
    state: State<AppState>,
    media_id: i64,
) -> Result<Option<Progress>, String> {
    with_db(&state, |db| {
        get_progress(db, active_profile_id(db), media_id).map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn playback_progress_clear(state: State<AppState>, media_id: i64) -> Result<(), String> {
    with_db(&state, |db| {
        clear_progress(db, active_profile_id(db), media_id).map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn continue_watching_list(
    state: State<AppState>,
    limit: Option<i64>,
) -> Result<Vec<ProgressItem>, String> {
    with_db(&state, |db| {
        continue_watching(db, active_profile_id(db), limit.unwrap_or(24)).map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn watchlist_toggle(state: State<AppState>, media_id: i64) -> Result<bool, String> {
    with_db(&state, |db| {
        let on =
            toggle_watchlist(db, active_profile_id(db), media_id).map_err(|e| e.to_string())?;
        emit(
            db,
            if on {
                "watchlist.added"
            } else {
                "watchlist.removed"
            },
            "",
            json!({ "mediaId": media_id }),
        );
        Ok(on)
    })
}

#[tauri::command]
pub fn watchlist_list(state: State<AppState>) -> Result<Vec<MediaItem>, String> {
    with_db(&state, |db| {
        watchlist(db, active_profile_id(db)).map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn activity_log_list(
    state: State<AppState>,
    limit: Option<i64>,
) -> Result<Vec<ActivityEntry>, String> {
    with_db(&state, |db| {
        activity(db, limit.unwrap_or(200)).map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn activity_log_clear(state: State<AppState>) -> Result<(), String> {
    with_db(&state, |db| {
        db.conn
            .execute("DELETE FROM activity_log", [])
            .map(|_| ())
            .map_err(|e| e.to_string())
    })
}

/// Lets the UI report things only it sees, such as playback starting.
#[tauri::command]
pub fn activity_record(
    state: State<AppState>,
    kind: String,
    title: String,
    detail: Option<Value>,
) -> Result<(), String> {
    let kind = kind.trim();
    if kind.is_empty() || kind.len() > 64 {
        return Err("Event kind needs 1 to 64 characters".into());
    }
    with_db(&state, |db| {
        emit(db, kind, &title, detail.unwrap_or(Value::Null));
        Ok(())
    })
}

/// Sends a test event to one URL and reports the HTTP status.
#[tauri::command]
pub async fn webhook_test(url: String) -> Result<u16, String> {
    let url = url.trim().to_string();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Webhook URLs start with https:// or http://".into());
    }
    post_webhook(
        &url,
        &json!({ "event": "webhook.test", "title": "CinaVault webhook test", "at": now(), "source": "CinaVault Premium" }),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> (Database, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("cinavault-user-{}.db", uuid::Uuid::new_v4()));
        let db = Database::new(path.to_str().unwrap()).unwrap();
        ensure_tables(&db).unwrap();
        (db, path)
    }

    fn add_item(db: &Database, title: &str) -> i64 {
        db.conn
            .execute(
                "INSERT INTO media_items (title, file_path, media_type, date_added) VALUES (?1, ?2, 'movie', ?3)",
                params![title, format!("C:/m/{title}.mkv"), now()],
            )
            .unwrap();
        db.conn.last_insert_rowid()
    }

    #[test]
    fn profiles_fall_back_to_owner_while_the_switch_is_off() {
        let (db, path) = temp_db();
        let kid = create_profile(&db, "Kid", "#f00", true).unwrap();
        set_active_profile(&db, kid.id).unwrap();
        assert_eq!(active_profile_id(&db), DEFAULT_PROFILE_ID);
        db.set_feature_setting_data("user_profiles", true, "{}")
            .unwrap();
        assert_eq!(active_profile_id(&db), kid.id);
        delete_profile(&db, kid.id).unwrap();
        assert_eq!(active_profile_id(&db), DEFAULT_PROFILE_ID);
        assert!(delete_profile(&db, DEFAULT_PROFILE_ID).is_err());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn progress_drives_continue_watching_and_watched() {
        let (db, path) = temp_db();
        let a = add_item(&db, "Alpha");
        let b = add_item(&db, "Beta");
        save_progress(&db, 1, a, 600.0, 6000.0).unwrap();
        save_progress(&db, 1, b, 5990.0, 6000.0).unwrap();
        let list = continue_watching(&db, 1, 10).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].item.title, "Alpha");
        assert_eq!(list[0].progress.position, 600.0);
        let watched: bool = db
            .conn
            .query_row(
                "SELECT watched FROM media_items WHERE id = ?1",
                params![b],
                |r| r.get(0),
            )
            .unwrap();
        assert!(watched);
        assert!(continue_watching(&db, 2, 10).unwrap().is_empty());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn watchlist_toggles_per_profile() {
        let (db, path) = temp_db();
        let a = add_item(&db, "Alpha");
        assert!(toggle_watchlist(&db, 1, a).unwrap());
        assert_eq!(watchlist(&db, 1).unwrap().len(), 1);
        assert!(watchlist(&db, 2).unwrap().is_empty());
        assert!(!toggle_watchlist(&db, 1, a).unwrap());
        assert!(watchlist(&db, 1).unwrap().is_empty());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn activity_log_follows_its_switch() {
        let (db, path) = temp_db();
        emit(&db, "scan.finished", "Library", json!({ "added": 3 }));
        assert_eq!(
            activity(&db, 10).unwrap().len(),
            1,
            "activity_log defaults on"
        );
        db.set_feature_setting_data("activity_log", false, "{}")
            .unwrap();
        emit(&db, "scan.finished", "Library", json!({}));
        assert_eq!(activity(&db, 10).unwrap().len(), 1);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn webhook_targets_respect_event_filter() {
        let config: serde_json::Map<String, Value> = serde_json::from_value(json!({
            "urls": ["https://example.com/a", "ftp://nope", " http://lan/b "],
            "events": ["playback"]
        }))
        .unwrap();
        assert_eq!(
            webhook_targets(&config, "playback.finished"),
            vec!["https://example.com/a", "http://lan/b"]
        );
        assert!(webhook_targets(&config, "scan.finished").is_empty());
        let open: serde_json::Map<String, Value> =
            serde_json::from_value(json!({ "urls": ["https://x.test"] })).unwrap();
        assert_eq!(webhook_targets(&open, "anything"), vec!["https://x.test"]);
    }
}
