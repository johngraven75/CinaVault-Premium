// CinaVault Premium — SQLite Database Layer (rusqlite) — Build 115
// Premium defaults: all features ON, full persistence support

use rusqlite::{Connection, OptionalExtension, params, Result as SqlResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use tauri::State;
use crate::AppState;
use crate::library_artifacts::{
    is_generated_chapter_image_path, is_internal_artwork_cache_path, is_sidecar_artwork_image,
};
#[cfg(test)]
use crate::library_artifacts::available_poster_path_for_media;

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemoteAccessUserProvision {
    pub id: i64,
    pub email: String,
    pub display_name: Option<String>,
    pub access_key: String,
    pub access_key_preview: String,
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemoteAccessUserSummary {
    pub id: i64,
    pub email: String,
    pub display_name: Option<String>,
    pub access_key_preview: String,
    pub enabled: bool,
    pub permissions: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_login: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemoteAccessPrincipal {
    pub id: i64,
    pub email: String,
    pub display_name: Option<String>,
    pub auth_method: String,
    pub session_token: String,
    pub expires_at: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemoteAccessKeyRotation {
    pub email: String,
    pub access_key: String,
    pub access_key_preview: String,
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

            CREATE TABLE IF NOT EXISTS remote_access_users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                email TEXT NOT NULL UNIQUE COLLATE NOCASE,
                display_name TEXT,
                password_salt TEXT NOT NULL,
                password_hash TEXT NOT NULL,
                access_key_salt TEXT NOT NULL,
                access_key_hash TEXT NOT NULL,
                access_key_preview TEXT NOT NULL,
                enabled INTEGER DEFAULT 1,
                permissions TEXT NOT NULL DEFAULT 'server:read,library:read,stream:play',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                last_login TEXT
            );

            CREATE TABLE IF NOT EXISTS remote_access_sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                token_salt TEXT NOT NULL,
                token_hash TEXT NOT NULL UNIQUE,
                auth_method TEXT NOT NULL,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                revoked INTEGER DEFAULT 0,
                FOREIGN KEY (user_id) REFERENCES remote_access_users(id)
            );

            CREATE INDEX IF NOT EXISTS idx_media_title ON media_items(title);
            CREATE INDEX IF NOT EXISTS idx_media_type ON media_items(media_type);
            CREATE INDEX IF NOT EXISTS idx_media_source ON media_items(source_id);
            CREATE INDEX IF NOT EXISTS idx_media_verified ON media_items(verified);
            CREATE INDEX IF NOT EXISTS idx_media_date ON media_items(date_added);
            CREATE INDEX IF NOT EXISTS idx_remote_access_users_email ON remote_access_users(email);
            CREATE INDEX IF NOT EXISTS idx_remote_access_sessions_user ON remote_access_sessions(user_id);
        ")?;
        self.ensure_column("plugins", "plugin_key", "TEXT")?;
        self.ensure_column("plugins", "platform", "TEXT")?;
        self.ensure_column("plugins", "install_path", "TEXT")?;
        self.ensure_column("plugins", "enabled", "INTEGER DEFAULT 1")?;
        self.ensure_column("plugins", "repo_url", "TEXT")?;
        self.conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_plugins_plugin_key ON plugins(plugin_key) WHERE plugin_key IS NOT NULL",
            [],
        )?;

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
            ("prefer_embedded_titles", "true"),
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
            ("ai_model", "katanemo/Arch-Router-1.5B:hf-inference"),
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
        self.conn.execute(
            "UPDATE settings SET value = 'true' WHERE key = 'prefer_embedded_titles' AND value = 'false'",
            [],
        )?;

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

        self.cleanup_non_library_photo_artifacts()?;

        Ok(())
    }

    fn cleanup_non_library_photo_artifacts(&self) -> SqlResult<()> {
        let rows = {
            let mut stmt = self
                .conn
                .prepare("SELECT id, file_path FROM media_items WHERE media_type = 'photo'")?;
            let iter = stmt.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?;
            iter.collect::<Result<Vec<_>, _>>()?
        };

        for (id, file_path) in rows {
            let path = Path::new(&file_path);
            if is_generated_chapter_image_path(path)
                || is_internal_artwork_cache_path(path)
                || is_sidecar_artwork_image(path)
            {
                self.conn
                    .execute("DELETE FROM media_items WHERE id = ?1", params![id])?;
            }
        }

        Ok(())
    }

    #[cfg(test)]
    fn sync_sidecar_artwork_for_video_rows(&self) -> SqlResult<()> {
        let rows = {
            let mut stmt = self.conn.prepare(
                "SELECT file_path
                 FROM media_items
                 WHERE media_type IN ('adult', 'movie', 'episode', 'video')
                   AND (poster_path IS NULL OR trim(poster_path) = '')",
            )?;
            let iter = stmt.query_map([], |row| row.get::<_, String>(0))?;
            iter.collect::<Result<Vec<_>, _>>()?
        };

        for file_path in rows {
            let Some(poster_path) = available_poster_path_for_media(Path::new(&file_path)) else {
                continue;
            };
            self.conn.execute(
                "UPDATE media_items
                 SET poster_path = ?1
                 WHERE file_path = ?2
                   AND (poster_path IS NULL OR trim(poster_path) = '')",
                params![poster_path, file_path],
            )?;
        }

        Ok(())
    }

    fn ensure_column(&self, table: &str, column: &str, definition: &str) -> SqlResult<()> {
        let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
        let mut exists = false;
        for existing in columns {
            if existing?.eq_ignore_ascii_case(column) {
                exists = true;
                break;
            }
        }

        if !exists {
            self.conn.execute(
                &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
                [],
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
        let off = offset.unwrap_or(0);
        match (media_type, limit) {
            (Some(mt), Some(lim)) => {
                let mut stmt = self.conn.prepare(
                    "SELECT * FROM media_items WHERE media_type = ?1 ORDER BY date_added DESC LIMIT ?2 OFFSET ?3"
                )?;
                let rows = stmt.query_map(params![mt, lim, off], Self::row_to_media)?;
                rows.collect()
            }
            (Some(mt), None) => {
                let mut stmt = self.conn.prepare(
                    "SELECT * FROM media_items WHERE media_type = ?1 ORDER BY date_added DESC"
                )?;
                let rows = stmt.query_map(params![mt], Self::row_to_media)?;
                rows.collect()
            }
            (None, Some(lim)) => {
                let mut stmt = self.conn.prepare(
                    "SELECT * FROM media_items ORDER BY date_added DESC LIMIT ?1 OFFSET ?2"
                )?;
                let rows = stmt.query_map(params![lim, off], Self::row_to_media)?;
                rows.collect()
            }
            (None, None) => {
                let mut stmt = self.conn.prepare(
                    "SELECT * FROM media_items ORDER BY date_added DESC"
                )?;
                let rows = stmt.query_map([], Self::row_to_media)?;
                rows.collect()
            }
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

    pub fn upsert_scanned_media_item_data(&self, item: &MediaItem) -> SqlResult<bool> {
        let existing_id = self.conn.query_row(
            "SELECT id FROM media_items WHERE file_path = ?1",
            params![&item.file_path],
            |row| row.get::<_, i64>(0),
        );

        match existing_id {
            Ok(id) => {
                self.conn.execute(
                    "UPDATE media_items
                     SET title = ?1,
                         media_type = ?2,
                         file_size = ?3,
                         source_id = ?4,
                         poster_path = CASE
                             WHEN (poster_path IS NULL OR trim(poster_path) = '')
                                  AND ?5 IS NOT NULL
                                  AND trim(?5) <> ''
                             THEN ?5
                             ELSE poster_path
                         END
                     WHERE id = ?6",
                    params![
                        item.title,
                        item.media_type,
                        item.file_size,
                        item.source_id,
                        item.poster_path,
                        id
                    ],
                )?;
                Ok(false)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                self.add_media_item_data(item)?;
                Ok(true)
            }
            Err(err) => Err(err),
        }
    }

    pub fn update_media_metadata_data(
        &self,
        file_path: &str,
        title: Option<&str>,
        overview: Option<&str>,
        poster_path: Option<&str>,
        year: Option<i32>,
        rating: Option<f64>,
        genre: Option<&str>,
        tmdb_id: Option<&str>,
        imdb_id: Option<&str>,
        media_type: Option<&str>,
    ) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE media_items
             SET title = COALESCE(?1, title),
                 overview = COALESCE(?2, overview),
                 poster_path = COALESCE(?3, poster_path),
                 year = COALESCE(?4, year),
                 rating = COALESCE(?5, rating),
                 genre = COALESCE(?6, genre),
                 tmdb_id = COALESCE(?7, tmdb_id),
                 imdb_id = COALESCE(?8, imdb_id),
                 media_type = COALESCE(?9, media_type)
             WHERE file_path = ?10",
            params![
                title,
                overview,
                poster_path,
                year,
                rating,
                genre,
                tmdb_id,
                imdb_id,
                media_type,
                file_path,
            ],
        )?;
        Ok(())
    }

    pub fn update_media_file_path_data(
        &self,
        old_file_path: &str,
        new_file_path: &str,
        new_title: &str,
    ) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE media_items
             SET file_path = ?1,
                 title = ?2
             WHERE file_path = ?3",
            params![new_file_path, new_title, old_file_path],
        )?;
        Ok(())
    }

    pub fn search_media_data(&self, query: &str) -> SqlResult<Vec<MediaItem>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            "SELECT * FROM media_items WHERE title LIKE ?1 OR genre LIKE ?1 OR overview LIKE ?1 ORDER BY title"
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
            "SELECT * FROM media_items WHERE verified = 0 ORDER BY date_added DESC"
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

    pub fn create_remote_access_user(
        &self,
        email: &str,
        password: &str,
        display_name: Option<&str>,
    ) -> Result<RemoteAccessUserProvision, String> {
        let email = normalize_remote_email(email)?;
        validate_remote_password(password)?;

        let now = chrono::Utc::now().to_rfc3339();
        let password_salt = new_secret_salt();
        let password_hash = hash_secret(&password_salt, password);
        let access_key = generate_remote_access_key();
        let access_key_salt = new_secret_salt();
        let access_key_hash = hash_secret(&access_key_salt, &access_key);
        let access_key_preview = preview_secret(&access_key);
        let display_name = display_name
            .and_then(|value| non_empty_trimmed(value))
            .or_else(|| Some(email.clone()));

        self.conn
            .execute(
                "INSERT INTO remote_access_users
                 (email, display_name, password_salt, password_hash, access_key_salt,
                  access_key_hash, access_key_preview, enabled, permissions, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, 'server:read,library:read,stream:play', ?8, ?8)
                 ON CONFLICT(email) DO UPDATE SET
                   display_name = excluded.display_name,
                   password_salt = excluded.password_salt,
                   password_hash = excluded.password_hash,
                   access_key_salt = excluded.access_key_salt,
                   access_key_hash = excluded.access_key_hash,
                   access_key_preview = excluded.access_key_preview,
                   enabled = 1,
                   updated_at = excluded.updated_at",
                params![
                    email,
                    display_name,
                    password_salt,
                    password_hash,
                    access_key_salt,
                    access_key_hash,
                    access_key_preview,
                    now,
                ],
            )
            .map_err(|err| err.to_string())?;

        let row = self
            .conn
            .query_row(
                "SELECT id, email, display_name, enabled, created_at
                 FROM remote_access_users
                 WHERE email = ?1",
                params![email],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, bool>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .map_err(|err| err.to_string())?;

        Ok(RemoteAccessUserProvision {
            id: row.0,
            email: row.1,
            display_name: row.2,
            access_key: access_key.clone(),
            access_key_preview: preview_secret(&access_key),
            enabled: row.3,
            created_at: row.4,
        })
    }

    pub fn authenticate_remote_password(
        &self,
        email: &str,
        password: &str,
    ) -> Result<Option<RemoteAccessPrincipal>, String> {
        let email = normalize_remote_email(email)?;
        let row = self
            .conn
            .query_row(
                "SELECT id, email, display_name, password_salt, password_hash, enabled, permissions
                 FROM remote_access_users
                 WHERE email = ?1",
                params![email],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, bool>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            )
            .optional()
            .map_err(|err| err.to_string())?;

        let Some((id, email, display_name, salt, expected_hash, enabled, permissions)) = row else {
            return Ok(None);
        };
        if !enabled {
            return Ok(None);
        }
        let actual_hash = hash_secret(&salt, password);
        if !constant_time_eq(&actual_hash, &expected_hash) {
            return Ok(None);
        }

        self.create_remote_access_session(id, email, display_name, "password", &permissions)
    }

    pub fn authenticate_remote_access_key(
        &self,
        access_key: &str,
    ) -> Result<Option<RemoteAccessPrincipal>, String> {
        let access_key = access_key.trim();
        if access_key.is_empty() {
            return Ok(None);
        }

        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, email, display_name, access_key_salt, access_key_hash, enabled, permissions
                 FROM remote_access_users",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, bool>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .map_err(|err| err.to_string())?;

        for row in rows {
            let (id, email, display_name, salt, expected_hash, enabled, permissions) =
                row.map_err(|err| err.to_string())?;
            if !enabled {
                continue;
            }
            let actual_hash = hash_secret(&salt, access_key);
            if constant_time_eq(&actual_hash, &expected_hash) {
                return self.create_remote_access_session(
                    id,
                    email,
                    display_name,
                    "access_key",
                    &permissions,
                );
            }
        }

        Ok(None)
    }

    pub fn rotate_remote_access_key(
        &self,
        email: &str,
    ) -> Result<Option<RemoteAccessKeyRotation>, String> {
        let email = normalize_remote_email(email)?;
        let access_key = generate_remote_access_key();
        let access_key_salt = new_secret_salt();
        let access_key_hash = hash_secret(&access_key_salt, &access_key);
        let access_key_preview = preview_secret(&access_key);
        let updated_at = chrono::Utc::now().to_rfc3339();

        let changed = self
            .conn
            .execute(
                "UPDATE remote_access_users
                 SET access_key_salt = ?1,
                     access_key_hash = ?2,
                     access_key_preview = ?3,
                     updated_at = ?4
                 WHERE email = ?5",
                params![access_key_salt, access_key_hash, access_key_preview, updated_at, email],
            )
            .map_err(|err| err.to_string())?;
        if changed == 0 {
            return Ok(None);
        }

        Ok(Some(RemoteAccessKeyRotation {
            email,
            access_key: access_key.clone(),
            access_key_preview: preview_secret(&access_key),
        }))
    }

    pub fn set_remote_access_user_enabled(
        &self,
        email: &str,
        enabled: bool,
    ) -> Result<(), String> {
        let email = normalize_remote_email(email)?;
        self.conn
            .execute(
                "UPDATE remote_access_users
                 SET enabled = ?1, updated_at = ?2
                 WHERE email = ?3",
                params![enabled, chrono::Utc::now().to_rfc3339(), email],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn list_remote_access_users(&self) -> Result<Vec<RemoteAccessUserSummary>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, email, display_name, access_key_preview, enabled, permissions,
                        created_at, updated_at, last_login
                 FROM remote_access_users
                 ORDER BY email",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                let permissions: String = row.get(5)?;
                Ok(RemoteAccessUserSummary {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    display_name: row.get(2)?,
                    access_key_preview: row.get(3)?,
                    enabled: row.get(4)?,
                    permissions: parse_permissions(&permissions),
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    last_login: row.get(8)?,
                })
            })
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())
    }

    fn create_remote_access_session(
        &self,
        user_id: i64,
        email: String,
        display_name: Option<String>,
        auth_method: &str,
        permissions: &str,
    ) -> Result<Option<RemoteAccessPrincipal>, String> {
        let session_token = generate_remote_session_token();
        let token_salt = new_secret_salt();
        let token_hash = hash_secret(&token_salt, &session_token);
        let created_at = chrono::Utc::now();
        let expires_at = created_at + chrono::Duration::hours(12);
        let created_at = created_at.to_rfc3339();
        let expires_at_string = expires_at.to_rfc3339();

        self.conn
            .execute(
                "INSERT INTO remote_access_sessions
                 (user_id, token_salt, token_hash, auth_method, created_at, expires_at, revoked)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
                params![user_id, token_salt, token_hash, auth_method, created_at, expires_at_string],
            )
            .map_err(|err| err.to_string())?;
        self.conn
            .execute(
                "UPDATE remote_access_users SET last_login = ?1 WHERE id = ?2",
                params![created_at, user_id],
            )
            .map_err(|err| err.to_string())?;

        Ok(Some(RemoteAccessPrincipal {
            id: user_id,
            email,
            display_name,
            auth_method: auth_method.to_string(),
            session_token,
            expires_at: expires_at_string,
            permissions: parse_permissions(permissions),
        }))
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

fn normalize_remote_email(email: &str) -> Result<String, String> {
    let email = email.trim().to_ascii_lowercase();
    if email.len() < 5 || !email.contains('@') {
        return Err("A valid email address is required.".to_string());
    }
    let Some((local, domain)) = email.split_once('@') else {
        return Err("A valid email address is required.".to_string());
    };
    if local.trim().is_empty() || !domain.contains('.') || domain.ends_with('.') {
        return Err("A valid email address is required.".to_string());
    }
    Ok(email)
}

fn validate_remote_password(password: &str) -> Result<(), String> {
    if password.len() < 8 {
        return Err("Remote access passwords must be at least 8 characters.".to_string());
    }
    Ok(())
}

fn non_empty_trimmed(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn new_secret_salt() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn generate_remote_access_key() -> String {
    format!(
        "cvra_{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn generate_remote_session_token() -> String {
    format!(
        "cvrs_{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn preview_secret(secret: &str) -> String {
    let chars = secret.chars().collect::<Vec<_>>();
    let start = chars.len().saturating_sub(8);
    chars[start..].iter().collect()
}

fn hash_secret(salt: &str, secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(b":cinavault-remote-access:");
    hasher.update(secret.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right.iter())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

fn parse_permissions(permissions: &str) -> Vec<String> {
    permissions
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
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
pub fn create_remote_access_user(
    state: State<AppState>,
    email: String,
    password: String,
    display_name: Option<String>,
) -> Result<RemoteAccessUserProvision, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.create_remote_access_user(&email, &password, display_name.as_deref())
}

#[tauri::command]
pub fn authenticate_remote_password(
    state: State<AppState>,
    email: String,
    password: String,
) -> Result<Option<RemoteAccessPrincipal>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.authenticate_remote_password(&email, &password)
}

#[tauri::command]
pub fn authenticate_remote_access_key(
    state: State<AppState>,
    access_key: String,
) -> Result<Option<RemoteAccessPrincipal>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.authenticate_remote_access_key(&access_key)
}

#[tauri::command]
pub fn rotate_remote_access_key(
    state: State<AppState>,
    email: String,
) -> Result<Option<RemoteAccessKeyRotation>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.rotate_remote_access_key(&email)
}

#[tauri::command]
pub fn set_remote_access_user_enabled(
    state: State<AppState>,
    email: String,
    enabled: bool,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.set_remote_access_user_enabled(&email, enabled)
}

#[tauri::command]
pub fn list_remote_access_users(state: State<AppState>) -> Result<Vec<RemoteAccessUserSummary>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.list_remote_access_users()
}

#[tauri::command]
pub fn get_remote_access_security_status(state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let user_count = db.list_remote_access_users()?.len();
    let remote_enabled = db
        .get_setting_data("remote_access_enabled")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "true".to_string());
    let secure_mode = db
        .get_setting_data("remote_secure_connections")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "preferred".to_string());
    let public_port = db
        .get_setting_data("remote_public_port")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "32400".to_string());

    Ok(serde_json::json!({
        "remote_enabled": remote_enabled != "false",
        "secure_mode": secure_mode,
        "public_port": public_port,
        "account_count": user_count,
        "password_auth": true,
        "access_key_auth": true,
        "session_hours": 12,
        "permissions": ["server:read", "library:read", "stream:play"],
    }))
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

#[cfg(test)]
mod tests {
    use super::{Database, MediaItem};
    use rusqlite::params;
    use std::fs;

    fn test_db_path(name: &str) -> String {
        let mut path = std::env::temp_dir();
        path.push(format!("cinavault-{name}-{}.db", uuid::Uuid::new_v4()));
        path.to_string_lossy().to_string()
    }

    fn sample_item(title: &str, file_path: &str) -> MediaItem {
        MediaItem {
            id: None,
            title: title.to_string(),
            file_path: file_path.to_string(),
            media_type: "movie".to_string(),
            year: None,
            rating: None,
            overview: None,
            poster_path: None,
            backdrop_path: None,
            genre: None,
            duration: None,
            file_size: Some(100),
            resolution: None,
            codec: None,
            verified: false,
            watched: false,
            favorite: false,
            date_added: "2026-05-06T00:00:00Z".to_string(),
            last_played: None,
            tmdb_id: None,
            imdb_id: None,
            source_id: None,
        }
    }

    #[test]
    fn scan_upsert_updates_existing_title_without_overwriting_user_flags() {
        let db_path = test_db_path("scan-upsert");
        let db = Database::new(&db_path).expect("db should open");

        let mut original = sample_item("File Name", r"C:\media\movie.mkv");
        db.add_media_item_data(&original).expect("initial insert should succeed");
        db.conn.execute(
            "UPDATE media_items SET watched = 1, favorite = 1 WHERE file_path = ?1",
            params![&original.file_path],
        ).expect("should update flags");

        original.title = "Embedded Title".to_string();
        original.file_size = Some(200);
        let inserted = db
            .upsert_scanned_media_item_data(&original)
            .expect("scan upsert should succeed");

        assert!(!inserted, "existing rows should be refreshed, not counted as new");

        let row = db
            .conn
            .query_row(
                "SELECT title, file_size, watched, favorite FROM media_items WHERE file_path = ?1",
                params![&original.file_path],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, bool>(2)?,
                        row.get::<_, bool>(3)?,
                    ))
                },
            )
            .expect("item should still exist");

        assert_eq!(row.0, "Embedded Title");
        assert_eq!(row.1, Some(200));
        assert!(row.2, "watched state should be preserved");
        assert!(row.3, "favorite state should be preserved");

        drop(db);
        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn scan_upsert_fills_missing_poster_for_existing_items() {
        let db_path = test_db_path("scan-upsert-poster");
        let db = Database::new(&db_path).expect("db should open");

        let mut original = sample_item("Movie", r"C:\media\movie.mkv");
        db.add_media_item_data(&original).expect("initial insert should succeed");

        original.poster_path = Some(r"C:\media\movie-poster.jpg".to_string());
        let inserted = db
            .upsert_scanned_media_item_data(&original)
            .expect("scan upsert should succeed");

        assert!(!inserted, "existing rows should be refreshed, not inserted");

        let poster_path: Option<String> = db
            .conn
            .query_row(
                "SELECT poster_path FROM media_items WHERE file_path = ?1",
                params![&original.file_path],
                |row| row.get(0),
            )
            .expect("item should still exist");
        assert_eq!(poster_path, Some(r"C:\media\movie-poster.jpg".to_string()));

        drop(db);
        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn get_media_items_without_limit_returns_all_rows() {
        let db_path = test_db_path("get-all-media");
        let db = Database::new(&db_path).expect("db should open");

        for idx in 0..250 {
            let title = format!("Item {idx}");
            let path = format!(r"C:\media\item-{idx}.mkv");
            db.add_media_item_data(&sample_item(&title, &path))
                .expect("insert should succeed");
        }

        let all_items = db
            .get_media_items_data(None, None, Some(0))
            .expect("query should succeed");
        assert_eq!(all_items.len(), 250);

        drop(db);
        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn prefer_embedded_titles_defaults_to_true() {
        let db_path = test_db_path("embedded-title-default");
        let db = Database::new(&db_path).expect("db should open");

        assert_eq!(
            db.get_setting_data("prefer_embedded_titles")
                .expect("setting should load"),
            Some("true".to_string())
        );

        drop(db);
        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn search_media_is_not_capped_at_200() {
        let db_path = test_db_path("search-not-capped");
        let db = Database::new(&db_path).expect("db should open");

        for idx in 0..230 {
            let title = format!("Match {idx}");
            let path = format!(r"C:\media\match-{idx}.mkv");
            db.add_media_item_data(&sample_item(&title, &path))
                .expect("insert should succeed");
        }

        let matches = db.search_media_data("Match").expect("search should succeed");
        assert_eq!(matches.len(), 230);

        drop(db);
        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn enrichment_update_preserves_user_flags() {
        let db_path = test_db_path("enrichment-update");
        let db = Database::new(&db_path).expect("db should open");

        let item = sample_item("Old Title", r"C:\media\old-title.mp4");
        db.add_media_item_data(&item).expect("insert should succeed");
        db.conn.execute(
            "UPDATE media_items SET watched = 1, favorite = 1 WHERE file_path = ?1",
            params![&item.file_path],
        ).expect("flag update should succeed");

        db.update_media_metadata_data(
            &item.file_path,
            Some("Better Title"),
            Some("Overview text"),
            Some("https://poster"),
            Some(2024),
            Some(8.1),
            Some("Drama"),
            Some("123"),
            Some("tt123"),
            Some("adult"),
        ).expect("metadata update should succeed");

        let row = db.conn.query_row(
            "SELECT title, overview, watched, favorite, media_type FROM media_items WHERE file_path = ?1",
            params![&item.file_path],
            |row| Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, bool>(2)?,
                row.get::<_, bool>(3)?,
                row.get::<_, String>(4)?,
            )),
        ).expect("row should exist");

        assert_eq!(row.0, "Better Title");
        assert_eq!(row.1.as_deref(), Some("Overview text"));
        assert!(row.2);
        assert!(row.3);
        assert_eq!(row.4, "adult");

        drop(db);
        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn cleanup_removes_sidecar_artwork_photo_rows_without_backfilling_video_posters() {
        let db_path = test_db_path("sidecar-artwork-cleanup");
        let media_dir =
            std::env::temp_dir().join(format!("cinavault-sidecar-db-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&media_dir).expect("media dir should be created");
        let video_path = media_dir.join("Movie.mp4");
        let poster_path = media_dir.join("Movie-poster.jpg");
        fs::write(&video_path, b"video").expect("video should exist");
        fs::write(&poster_path, b"poster").expect("poster should exist");

        let db = Database::new(&db_path).expect("db should open");

        let video = sample_item("Movie", &video_path.to_string_lossy());
        let mut poster = sample_item("poster", &poster_path.to_string_lossy());
        poster.media_type = "photo".to_string();
        let mut backdrop = sample_item("scene-poster", r"E:\Videos\Movie\scene-poster.webp");
        backdrop.media_type = "photo".to_string();
        let mut real_photo = sample_item("beach-day", r"E:\Photos\Vacation\beach-day.jpg");
        real_photo.media_type = "photo".to_string();

        db.add_media_item_data(&video)
            .expect("video row should insert");
        db.add_media_item_data(&poster)
            .expect("poster row should insert");
        db.add_media_item_data(&backdrop)
            .expect("poster suffix row should insert");
        db.add_media_item_data(&real_photo)
            .expect("real photo row should insert");

        db.cleanup_non_library_photo_artifacts()
            .expect("cleanup should succeed");

        let remaining = db
            .conn
            .query_row("SELECT COUNT(*) FROM media_items", [], |row| row.get::<_, i64>(0))
            .expect("count should load");
        assert_eq!(remaining, 2);

        let attached_poster = db
            .conn
            .query_row(
                "SELECT poster_path FROM media_items WHERE file_path = ?1",
                params![video.file_path],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("video row should load");
        assert_eq!(attached_poster, None);

        let real_photo_exists = db
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM media_items WHERE file_path = ?1)",
                params![real_photo.file_path],
                |row| row.get::<_, bool>(0),
            )
            .expect("real photo lookup should load");
        assert!(real_photo_exists);

        drop(db);
        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(media_dir);
    }

    #[test]
    fn cleanup_removes_internal_trash_and_generated_poster_photo_rows() {
        let db_path = test_db_path("internal-artwork-cleanup");
        let db = Database::new(&db_path).expect("db should open");

        let mut trash_artwork = sample_item(
            "PISS PUMPING CLOUDS-(720p)",
            r"E:\Personal Vids X\.cinavault-trash\PISS PUMPING CLOUDS-(720p).jpg",
        );
        trash_artwork.media_type = "photo".to_string();
        let mut generated_poster = sample_item(
            "poster cache",
            r"C:\Users\johng\AppData\Roaming\CinaVault\generated-posters\abc123.jpg",
        );
        generated_poster.media_type = "photo".to_string();
        let mut real_photo = sample_item("beach-day", r"E:\Photos\Vacation\beach-day.jpg");
        real_photo.media_type = "photo".to_string();

        db.add_media_item_data(&trash_artwork)
            .expect("trash artwork row should insert");
        db.add_media_item_data(&generated_poster)
            .expect("generated poster row should insert");
        db.add_media_item_data(&real_photo)
            .expect("real photo row should insert");

        db.cleanup_non_library_photo_artifacts()
            .expect("cleanup should succeed");

        let remaining = db
            .conn
            .query_row("SELECT COUNT(*) FROM media_items", [], |row| row.get::<_, i64>(0))
            .expect("count should load");
        assert_eq!(remaining, 1);

        let real_photo_exists = db
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM media_items WHERE file_path = ?1)",
                params![real_photo.file_path],
                |row| row.get::<_, bool>(0),
            )
            .expect("real photo lookup should load");
        assert!(real_photo_exists);

        drop(db);
        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn manual_sidecar_artwork_backfill_populates_video_posters() {
        let db_path = test_db_path("manual-sidecar-backfill");
        let media_dir =
            std::env::temp_dir().join(format!("cinavault-manual-backfill-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&media_dir).expect("media dir should be created");
        let video_path = media_dir.join("Movie.mp4");
        let poster_path = media_dir.join("Movie-poster.jpg");
        fs::write(&video_path, b"video").expect("video should exist");
        fs::write(&poster_path, b"poster").expect("poster should exist");

        let db = Database::new(&db_path).expect("db should open");
        let video = sample_item("Movie", &video_path.to_string_lossy());
        db.add_media_item_data(&video)
            .expect("video row should insert");

        db.sync_sidecar_artwork_for_video_rows()
            .expect("manual backfill should succeed");

        let attached_poster = db
            .conn
            .query_row(
                "SELECT poster_path FROM media_items WHERE file_path = ?1",
                params![video.file_path],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("video row should load");
        assert_eq!(attached_poster.as_deref(), Some(poster_path.to_string_lossy().as_ref()));

        drop(db);
        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(media_dir);
    }

    #[test]
    fn manual_available_poster_backfill_supports_same_stem_artwork() {
        let db_path = test_db_path("manual-same-stem-backfill");
        let media_dir =
            std::env::temp_dir().join(format!("cinavault-manual-same-stem-db-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&media_dir).expect("media dir should be created");
        let video_path = media_dir.join("Movie.mp4");
        let poster_path = media_dir.join("Movie.jpg");
        fs::write(&video_path, b"video").expect("video should exist");
        fs::write(&poster_path, b"poster").expect("poster should exist");

        let db = Database::new(&db_path).expect("db should open");
        let video = sample_item("Movie", &video_path.to_string_lossy());
        db.add_media_item_data(&video)
            .expect("video row should insert");

        db.sync_sidecar_artwork_for_video_rows()
            .expect("manual backfill should succeed");

        let attached_poster = db
            .conn
            .query_row(
                "SELECT poster_path FROM media_items WHERE file_path = ?1",
                params![video.file_path],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("video row should load");
        assert_eq!(attached_poster.as_deref(), Some(poster_path.to_string_lossy().as_ref()));

        drop(db);
        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(media_dir);
    }

    #[test]
    fn database_startup_does_not_backfill_video_posters_from_filesystem() {
        let db_path = test_db_path("startup-does-not-backfill-posters");
        let media_dir =
            std::env::temp_dir().join(format!("cinavault-startup-db-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&media_dir).expect("media dir should be created");
        let video_path = media_dir.join("Movie.mp4");
        let poster_path = media_dir.join("Movie-poster.jpg");
        fs::write(&video_path, b"video").expect("video should exist");
        fs::write(&poster_path, b"poster").expect("poster should exist");

        {
            let db = Database::new(&db_path).expect("db should open");
            let video = sample_item("Movie", &video_path.to_string_lossy());
            db.add_media_item_data(&video)
                .expect("video row should insert");
        }

        let db = Database::new(&db_path).expect("db should reopen quickly");
        let attached_poster = db
            .conn
            .query_row(
                "SELECT poster_path FROM media_items WHERE file_path = ?1",
                params![video_path.to_string_lossy().as_ref()],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("video row should load");
        assert_eq!(attached_poster, None);

        drop(db);
        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(media_dir);
    }

    #[test]
    fn rename_update_changes_file_path_only_after_success() {
        let db_path = test_db_path("rename-update");
        let db = Database::new(&db_path).expect("db should open");

        let item = sample_item("Old Title", r"C:\media\old-title.mp4");
        db.add_media_item_data(&item).expect("insert should succeed");

        db.update_media_file_path_data(&item.file_path, r"C:\media\New Title.mp4", "New Title")
            .expect("rename update should succeed");

        let row = db.conn.query_row(
            "SELECT title, file_path FROM media_items WHERE file_path = ?1",
            params![r"C:\media\New Title.mp4"],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        ).expect("renamed row should exist");

        assert_eq!(row.0, "New Title");
        assert_eq!(row.1, r"C:\media\New Title.mp4");

        drop(db);
        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn remote_access_user_authenticates_with_email_password_or_access_key() {
        let db_path = test_db_path("remote-access-auth");
        let db = Database::new(&db_path).expect("db should open");

        let created = db
            .create_remote_access_user(" Owner@Example.COM ", "CorrectHorse42!", Some("Owner"))
            .expect("remote user should be created");

        assert_eq!(created.email, "owner@example.com");
        assert_eq!(created.display_name.as_deref(), Some("Owner"));
        assert!(created.access_key.starts_with("cvra_"));
        assert_eq!(created.access_key_preview.len(), 8);

        let stored_secret = db
            .conn
            .query_row(
                "SELECT password_hash, access_key_hash FROM remote_access_users WHERE email = ?1",
                params!["owner@example.com"],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .expect("stored secrets should load");
        assert!(!stored_secret.0.contains("CorrectHorse42!"));
        assert!(!stored_secret.1.contains(&created.access_key));

        let password_auth = db
            .authenticate_remote_password("owner@example.com", "CorrectHorse42!")
            .expect("password auth should run")
            .expect("correct password should authenticate");
        assert_eq!(password_auth.email, "owner@example.com");
        assert_eq!(password_auth.auth_method, "password");

        assert!(db
            .authenticate_remote_password("owner@example.com", "wrong-password")
            .expect("wrong password auth should run")
            .is_none());

        let key_auth = db
            .authenticate_remote_access_key(&created.access_key)
            .expect("access-key auth should run")
            .expect("correct key should authenticate");
        assert_eq!(key_auth.email, "owner@example.com");
        assert_eq!(key_auth.auth_method, "access_key");

        drop(db);
        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn disabled_remote_access_user_cannot_authenticate() {
        let db_path = test_db_path("remote-access-disabled");
        let db = Database::new(&db_path).expect("db should open");

        let created = db
            .create_remote_access_user("viewer@example.com", "CorrectHorse42!", None)
            .expect("remote user should be created");
        db.conn
            .execute(
                "UPDATE remote_access_users SET enabled = 0 WHERE email = ?1",
                params!["viewer@example.com"],
            )
            .expect("disable should succeed");

        assert!(db
            .authenticate_remote_password("viewer@example.com", "CorrectHorse42!")
            .expect("password auth should run")
            .is_none());
        assert!(db
            .authenticate_remote_access_key(&created.access_key)
            .expect("access-key auth should run")
            .is_none());

        drop(db);
        let _ = fs::remove_file(db_path);
    }
}
