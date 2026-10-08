//! Per-item facts that media_items has no column for: the content rating
//! (NFO <mpaa>), the franchise a movie belongs to (NFO <set> or TMDB
//! belongs_to_collection) and the series an episode belongs to. Filled by NFO
//! import and the TMDB franchise lookup; read by collections and parental
//! controls (`media_item_extras_get`).

use crate::db::Database;
use crate::AppState;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaExtras {
    pub content_rating: Option<String>,
    pub collection_name: Option<String>,
    pub collection_tmdb_id: Option<String>,
    pub series_title: Option<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    /// When TMDB was last asked for this movie's collection.
    pub collection_checked_at: Option<String>,
}

pub fn ensure_table(db: &Database) -> Result<(), String> {
    db.conn
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS media_item_extras (
                media_id INTEGER PRIMARY KEY,
                content_rating TEXT,
                collection_name TEXT,
                collection_tmdb_id TEXT,
                series_title TEXT,
                season INTEGER,
                episode INTEGER,
                collection_checked_at TEXT,
                updated_at TEXT NOT NULL
            );",
        )
        .map_err(|error| error.to_string())
}

/// Stores the non-empty fields of `extras`, keeping existing values elsewhere.
pub fn merge(db: &Database, media_id: i64, extras: &MediaExtras) -> Result<(), String> {
    if *extras == MediaExtras::default() {
        return Ok(());
    }
    ensure_table(db)?;
    db.conn
        .execute(
            "INSERT INTO media_item_extras (media_id, content_rating, collection_name,
                collection_tmdb_id, series_title, season, episode, collection_checked_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(media_id) DO UPDATE SET
                content_rating = COALESCE(excluded.content_rating, content_rating),
                collection_name = COALESCE(excluded.collection_name, collection_name),
                collection_tmdb_id = COALESCE(excluded.collection_tmdb_id, collection_tmdb_id),
                series_title = COALESCE(excluded.series_title, series_title),
                season = COALESCE(excluded.season, season),
                episode = COALESCE(excluded.episode, episode),
                collection_checked_at = COALESCE(excluded.collection_checked_at, collection_checked_at),
                updated_at = excluded.updated_at",
            params![
                media_id,
                extras.content_rating,
                extras.collection_name,
                extras.collection_tmdb_id,
                extras.series_title,
                extras.season,
                extras.episode,
                extras.collection_checked_at,
                chrono::Utc::now().to_rfc3339(),
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn get(db: &Database, media_id: i64) -> Result<MediaExtras, String> {
    ensure_table(db)?;
    db.conn
        .query_row(
            "SELECT content_rating, collection_name, collection_tmdb_id, series_title, season,
                    episode, collection_checked_at
             FROM media_item_extras WHERE media_id = ?1",
            params![media_id],
            |row| {
                Ok(MediaExtras {
                    content_rating: row.get(0)?,
                    collection_name: row.get(1)?,
                    collection_tmdb_id: row.get(2)?,
                    series_title: row.get(3)?,
                    season: row.get(4)?,
                    episode: row.get(5)?,
                    collection_checked_at: row.get(6)?,
                })
            },
        )
        .optional()
        .map(Option::unwrap_or_default)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn media_item_extras_get(state: State<AppState>, id: i64) -> Result<MediaExtras, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    get(&db, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_keeps_existing_values() {
        let db = Database::new(":memory:").unwrap();
        merge(
            &db,
            7,
            &MediaExtras {
                content_rating: Some("PG-13".into()),
                collection_name: Some("Alien Collection".into()),
                ..Default::default()
            },
        )
        .unwrap();
        merge(
            &db,
            7,
            &MediaExtras {
                collection_tmdb_id: Some("8091".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let extras = get(&db, 7).unwrap();
        assert_eq!(extras.content_rating.as_deref(), Some("PG-13"));
        assert_eq!(extras.collection_name.as_deref(), Some("Alien Collection"));
        assert_eq!(extras.collection_tmdb_id.as_deref(), Some("8091"));
        assert_eq!(get(&db, 8).unwrap(), MediaExtras::default());
    }
}
