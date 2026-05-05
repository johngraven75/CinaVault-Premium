// CinaVault Premium — SQLite Database Layer (rusqlite) — Build 111
// Premium defaults: all features ON, full persistence support

use rusqlite::{Connection, params, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::State;
use crate::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaItem {
    pub id: Option<i64>,
    pub title: String,
    pub file_path: String,
    pub media_type: String,
    pub year: Option<i32>,
    pub rating: Option<f64>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub genre: Option<String>,
    pub duration: Option<i64>,
    pub file_size: Option<i64>,
    pub resolution: Option<String>,
    pub codec: Option<String>,
    pub verified: bool,
    pub watched: bool,
    pub favorite: bool,
    pub date_added: String,
    pub last_played: Option<String>,
    pub tmdb_id: Option<String>,
    pub imdb_id: Option<String>,
    pub source_id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaSource {
    pub id: Option<i64>,
    pub path: String,
    pub source_type: String,
    pub name: String,
    pub enabled: bool,
    pub last_scanned: Option<String>,
    pub item_count: i64,
}

pub struct Database {
    pub conn: Connection,
}

impl Database {
    pub fn new(path: &str) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        let db = Database { conn };
        db.initialize_tables()?;
        Ok(db)
    }

    fn initialize_tables(&self) -> SqlResult<()> {
        self.conn.execute_batch("
            CREATE TABLE IF NOT EXISTS media_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                file_path TEXT NOT NULL UNIQUE,
                media_type TEXT NOT NULL DEFAULT 'movie',
                year INTEGER,
                rating REAL,
                overview TEXT,
                poster_path TEXT,
                backdrop_path TEXT,
                genre TEXT,
                duration INTEGER,
                file_size INTEGER,
                resolution TEXT,
                codec TEXT,
                verified INTEGER DEFAULT 0,
                watched INTEGER DEFAULT 0,
                favorite INTEGER DEFAULT 0,
                date_added TEXT NOT NULL,
                last_played TEXT,
                tmdb_id TEXT,
                imdb_id TEXT,
                source_id INTEGER,
                FOREIGN KEY (source_id) REFERENCES media_sources(id)
            );

            CREATE TABLE IF NOT EXISTS media_sources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                source_type TEXT NOT NULL DEFAULT 'folder',
                name TEXT NOT NULL,
                enabled INTEGER DEFAULT 1,
                last_scanned TEXT,
                item_count INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS feature_settings (
                feature_key TEXT PRIMARY KEY,
                enabled INTEGER DEFAULT 0,
                config_json TEXT DEFAULT '{}'
            );

            CREATE TABLE IF NOT EXISTS xtream_profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                server_url TEXT NOT NULL,
                username TEXT NOT NULL,
                password TEXT NOT NULL,
                enabled INTEGER DEFAULT 1,
                last_synced TEXT
            );

            CREATE TABLE IF NOT EXISTS live_channels (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                profile_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                stream_url TEXT NOT NULL,
                logo_url TEXT,
                group_name TEXT,
                epg_id TEXT,
                FOREIGN KEY (profile_id) REFERENCES xtream_profiles(id)
            );

            CREATE TABLE IF NOT EXISTS plugin_repos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                url TEXT NOT NULL UNIQUE,
                enabled INTEGER DEFAULT 1,
                last_synced TEXT
            );

            CREATE TABLE IF NOT EXISTS plugins (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                version TEXT,
                description TEXT,
                author TEXT,
                repo_id INTEGER,
                installed INTEGER DEFAULT 0,
                config_json TEXT DEFAULT '{}',
                FOREIGN KEY (repo_id) REFERENCES plugin_repos(id)
            );

            CREATE TABLE IF NOT EXISTS api_keys (
                provider TEXT PRIMARY KEY,
                api_key TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS download_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL,
                title TEXT,
                status TEXT DEFAULT 'pending',
                file_path TEXT,
                file_size INTEGER,
                started_at TEXT,
                completed_at TEXT,
                error TEXT
            );

            CREATE TABLE IF NOT EXISTS duplicate_groups (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                group_hash TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS duplicate_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                group_id INTEGER NOT NULL,
                media_id INTEGER NOT NULL,
                file_path TEXT NOT NULL,
                file_size INTEGER,
                FOREIGN KEY (group_id) REFERENCES duplicate_groups(id),
                FOREIGN KEY (media_id) REFERENCES media_items(id)
            );

            CREATE INDEX IF NOT EXISTS idx_media_title ON media_items(title);
            CREATE INDEX IF NOT EXISTS idx_media_type ON media_items(media_type);
            CREATE INDEX IF NOT EXISTS idx_media_source ON media_items(source_id);
            CREATE INDEX IF NOT EXISTS idx_media_verified ON media_items(verified);
            CREATE INDEX IF NOT EXISTS idx_media_date ON media_items(date_added);
        ")?;

        // ── Premium defaults: ALL features ON ──
        let defaults = vec![
            ("theme", "vidhub_flagship"),
            ("window_width", "1400"),
            ("window_height", "900"),
            ("window_opacity", "100"),
            ("splash_enabled", "true"),
            ("sidebar_collapsed", "false"),
            ("motion_enabled", "true"),
            ("skip_intro", "true"),
            ("skip_outro", "true"),
            ("auto_next", "true"),
            ("auto_subtitles", "true"),
            ("chapter_thumbs_enabled", "true"),
            ("default_player", "system"),
            ("smart_collections", "true"),
            ("poster_sync", "true"),
            ("unified_library", "true"),
            ("watchlist_enabled", "true"),
            ("hw_transcoding", "true"),
            ("quality_control", "auto"),
            ("particle_effects", "true"),
            ("ai_visualizer", "true"),
            ("glassmorphism", "true"),
            ("starfield_header", "true"),
            ("offline_mode", "false"),
            ("ai_model", "facebook/bart-large-cnn"),
            ("hf_token", ""),
            // Scheduled task defaults
            ("_scheduledTasks", r#"{"thumbnails":"on_scan","chapter_images":"on_scan","metadata_check":"daily","match_unmatch":"on_import"}"#),
        ];
        for (key, value) in defaults {
            self.conn.execute(
                "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
                params![key, value],
            )?;
        }

        // ── Premium feature defaults: ALL enabled ──
        let features = vec![
            "smart_collections", "poster_sync", "unified_library", "watchlist",
            "skip_intro", "skip_outro", "auto_next", "auto_subtitles",
            "chapter_thumbs", "hw_transcoding", "motion_effects", "splash_screen",
            "particle_effects", "ai_visualizer", "glassmorphism", "starfield_header",
            "animated_sidebar", "emby_sdk", "vpn_integration", "ai_diagnostics",
            "duplicate_finder", "iptv_support", "plugin_system",
        ];
        for feature in features {
            self.conn.execute(
                "INSERT OR IGNORE INTO feature_settings (feature_key, enabled, config_json) VALUES (?1, 1, '{}')",
                params![feature],
            )?;
        }

        Ok(())
    }

    // ── Settings ──
    pub fn get_all_settings_data(&self) -> SqlResult<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect()
    }

    pub fn get_setting_data(&self, key: &str) -> SqlResult<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get::<_, String>(0))?;
        match rows.next() {
            Some(Ok(val)) => Ok(Some(val)),
            _ => Ok(None),
        }
    }

    pub fn set_setting_data(&self, key: &str, value: &str) -> SqlResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    // ── Feature settings ──
    pub fn get_feature_settings_data(&self) -> SqlResult<Vec<serde_json::Value>> {
        let mut stmt = self.conn.prepare(
            "SELECT feature_key, enabled, config_json FROM feature_settings"
        )?;
        let rows = stmt.query_map([], |row| {
            let key: String = row.get(0)?;
            let enabled: bool = row.get(1)?;
            let config: String = row.get(2)?;
            Ok(serde_json::json!({
                "key": key,
                "enabled": enabled,
                "config": serde_json::from_str::<serde_json::Value>(&config).unwrap_or_default()
            }))
        })?;
        rows.collect()
    }

    pub fn set_feature_setting_data(&self, key: &str, enabled: bool, config: &str) -> SqlResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO feature_settings (feature_key, enabled, config_json) VALUES (?1, ?2, ?3)",
            params![key, enabled, config],
        )?;
        Ok(())
    }

    // ── Media items ──
    pub fn get_media_items_data(&self, media_type: Option<&str>, limit: Option<i64>, offset: Option<i64>) -> SqlResult<Vec<MediaItem>> {
        let sql = match media_type {
            Some(_) => "SELECT * FROM media_items WHERE media_type = ?1 ORDER BY date_added DESC LIMIT ?2 OFFSET ?3",
            None => "SELECT * FROM media_items ORDER BY date_added DESC LIMIT ?1 OFFSET ?2",
        };
        let mut stmt = self.conn.prepare(sql)?;
        let lim = limit.unwrap_or(100);
        let off = offset.unwrap_or(0);
        if let Some(mt) = media_type {
            let rows = stmt.query_map(params![mt, lim, off], Self::row_to_media)?;
            rows.collect()
        } else {
            let rows = stmt.query_map(params![lim, off], Self::row_to_media)?;
            rows.collect()
        }
    }

    pub fn add_media_item_data(&self, item: &MediaItem) -> SqlResult<i64> {
        self.conn.execute(
            "INSERT OR IGNORE INTO media_items (title, file_path, media_type, year, rating, overview, poster_path, backdrop_path, genre, duration, file_size, resolution, codec, verified, watched, favorite, date_added, tmdb_id, imdb_id, source_id) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)",
            params![
                item.title, item.file_path, item.media_type, item.year, item.rating,
                item.overview, item.poster_path, item.backdrop_path, item.genre,
                item.duration, item.file_size, item.resolution, item.codec,
                item.verified, item.watched, item.favorite, item.date_added,
                item.tmdb_id, item.imdb_id, item.source_id
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn search_media_data(&self, query: &str) -> SqlResult<Vec<MediaItem>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            "SELECT * FROM media_items WHERE title LIKE ?1 OR genre LIKE ?1 OR overview LIKE ?1 ORDER BY title LIMIT 200"
        )?;
        let rows = stmt.query_map(params![pattern], |row| Self::row_to_media(row))?;
        rows.collect()
    }

    pub fn get_recent_media_data(&self, limit: i64) -> SqlResult<Vec<MediaItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM media_items ORDER BY date_added DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit], |row| Self::row_to_media(row))?;
        rows.collect()
    }

    pub fn get_unverified_media_data(&self) -> SqlResult<Vec<MediaItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM media_items WHERE verified = 0 ORDER BY date_added DESC LIMIT 200"
        )?;
        let rows = stmt.query_map([], |row| Self::row_to_media(row))?;
        rows.collect()
    }

    // ── Sources ──
    pub fn get_sources_data(&self) -> SqlResult<Vec<MediaSource>> {
        let mut stmt = self.conn.prepare("SELECT * FROM media_sources ORDER BY name")?;
        let rows = stmt.query_map([], |row| {
            Ok(MediaSource {
                id: Some(row.get(0)?),
                path: row.get(1)?,
                source_type: row.get(2)?,
                name: row.get(3)?,
                enabled: row.get(4)?,
                last_scanned: row.get(5)?,
                item_count: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    pub fn add_source_data(&self, source: &MediaSource) -> SqlResult<i64> {
        self.conn.execute(
            "INSERT OR IGNORE INTO media_sources (path, source_type, name, enabled, item_count) VALUES (?1,?2,?3,?4,?5)",
            params![source.path, source.source_type, source.name, source.enabled, source.item_count],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn remove_source_data(&self, id: i64) -> SqlResult<()> {
        self.conn.execute("DELETE FROM media_items WHERE source_id = ?1", params![id])?;
        self.conn.execute("DELETE FROM media_sources WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn row_to_media(row: &rusqlite::Row) -> rusqlite::Result<MediaItem> {
        Ok(MediaItem {
            id: Some(row.get(0)?),
            title: row.get(1)?,
            file_path: row.get(2)?,
            media_type: row.get(3)?,
            year: row.get(4)?,
            rating: row.get(5)?,
            overview: row.get(6)?,
            poster_path: row.get(7)?,
            backdrop_path: row.get(8)?,
            genre: row.get(9)?,
            duration: row.get(10)?,
            file_size: row.get(11)?,
            resolution: row.get(12)?,
            codec: row.get(13)?,
            verified: row.get(14)?,
            watched: row.get(15)?,
            favorite: row.get(16)?,
            date_added: row.get(17)?,
            last_played: row.get(18)?,
            tmdb_id: row.get(19)?,
            imdb_id: row.get(20)?,
            source_id: row.get(21)?,
        })
    }
}

// ════════════════════════════════════════════════════════════
//  Tauri Commands
// ════════════════════════════════════════════════════════════

#[tauri::command]
pub fn get_all_settings(state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let settings = db.get_all_settings_data().map_err(|e| e.to_string())?;
    let mut map = serde_json::Map::new();
    for (k, v) in settings {
        map.insert(k, serde_json::Value::String(v));
    }
    Ok(serde_json::Value::Object(map))
}

#[tauri::command]
pub fn get_setting(state: State<AppState>, key: String) -> Result<Option<String>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_setting_data(&key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_setting(state: State<AppState>, key: String, value: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.set_setting_data(&key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_feature_settings(state: State<AppState>) -> Result<Vec<serde_json::Value>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_feature_settings_data().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_feature_setting(state: State<AppState>, key: String, enabled: bool, config: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.set_feature_setting_data(&key, enabled, &config).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_media_items(state: State<AppState>, media_type: Option<String>, limit: Option<i64>, offset: Option<i64>) -> Result<Vec<MediaItem>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_media_items_data(media_type.as_deref(), limit, offset).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_media_item(state: State<AppState>, id: i64) -> Result<Option<MediaItem>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let items = db.get_media_items_data(None, Some(1), None).map_err(|e| e.to_string())?;
    Ok(items.into_iter().find(|i| i.id == Some(id)))
}

#[tauri::command]
pub fn add_media_item(state: State<AppState>, item: MediaItem) -> Result<i64, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.add_media_item_data(&item).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_media_item(state: State<AppState>, id: i64, title: Option<String>, verified: Option<bool>, watched: Option<bool>, favorite: Option<bool>) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    if let Some(t) = title {
        db.conn.execute("UPDATE media_items SET title = ?1 WHERE id = ?2", params![t, id]).map_err(|e| e.to_string())?;
    }
    if let Some(v) = verified {
        db.conn.execute("UPDATE media_items SET verified = ?1 WHERE id = ?2", params![v, id]).map_err(|e| e.to_string())?;
    }
    if let Some(w) = watched {
        db.conn.execute("UPDATE media_items SET watched = ?1 WHERE id = ?2", params![w, id]).map_err(|e| e.to_string())?;
    }
    if let Some(f) = favorite {
        db.conn.execute("UPDATE media_items SET favorite = ?1 WHERE id = ?2", params![f, id]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn delete_media_item(state: State<AppState>, id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn.execute("DELETE FROM media_items WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn search_media(state: State<AppState>, query: String) -> Result<Vec<MediaItem>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.search_media_data(&query).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_recent_media(state: State<AppState>, limit: Option<i64>) -> Result<Vec<MediaItem>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_recent_media_data(limit.unwrap_or(20)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_unverified_media(state: State<AppState>) -> Result<Vec<MediaItem>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_unverified_media_data().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn verify_media_item(state: State<AppState>, id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn.execute("UPDATE media_items SET verified = 1 WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_sources(state: State<AppState>) -> Result<Vec<MediaSource>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_sources_data().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_source(state: State<AppState>, path: String, source_type: String, name: String) -> Result<i64, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let source = MediaSource {
        id: None, path, source_type, name,
        enabled: true, last_scanned: None, item_count: 0,
    };
    db.add_source_data(&source).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_source(state: State<AppState>, id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.remove_source_data(id).map_err(|e| e.to_string())
}
