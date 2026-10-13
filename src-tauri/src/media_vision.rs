//! Poster vision results: what the free Hugging Face vision models saw on a
//! title's poster (see src/services/posterVision.ts).
//!
//! The WebView runs the models (local CLIP, plus Qwen3-VL through the Hugging
//! Face router when a token is set) and sends the result here. Each title gets
//! one row in `media_vision`. Library fields are only ever filled when empty:
//! vision never overwrites a genre or year that came from TMDB, OMDb, an NFO
//! or the user.

use crate::db::{Database, MediaItem};
use crate::AppState;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

/// Most titles one pending query hands back.
const MAX_PENDING: i64 = 200;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VisionRecord {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub moods: Vec<String>,
    #[serde(default)]
    pub era: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub caption: Option<String>,
    #[serde(default)]
    pub read_title: Option<String>,
    #[serde(default)]
    pub read_year: Option<i32>,
    /// Models that produced the record, e.g. "Xenova/clip-vit-base-patch32".
    #[serde(default)]
    pub models: Vec<String>,
    /// Genre text to write into `media_items.genre` when it is empty.
    #[serde(default)]
    pub fill_genre: Option<String>,
    /// Year to write into `media_items.year` when it is empty.
    #[serde(default)]
    pub fill_year: Option<i32>,
    #[serde(default)]
    pub analyzed_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VisionSaveResult {
    pub genre_filled: bool,
    pub year_filled: bool,
}

pub fn ensure_table(db: &Database) -> Result<(), String> {
    db.conn
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS media_vision (
                media_id INTEGER PRIMARY KEY,
                kind TEXT,
                genres TEXT NOT NULL DEFAULT '[]',
                moods TEXT NOT NULL DEFAULT '[]',
                era TEXT,
                tags TEXT NOT NULL DEFAULT '[]',
                caption TEXT,
                read_title TEXT,
                read_year INTEGER,
                models TEXT NOT NULL DEFAULT '[]',
                analyzed_at TEXT NOT NULL
            );",
        )
        .map_err(|error| error.to_string())
}

fn clean_list(values: &[String], max: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for value in values {
        let trimmed: String = value.trim().chars().take(60).collect();
        if !trimmed.is_empty() && !out.iter().any(|seen| seen.eq_ignore_ascii_case(&trimmed)) {
            out.push(trimmed);
        }
        if out.len() >= max {
            break;
        }
    }
    out
}

fn clean_text(value: &Option<String>, max: usize) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(|text| text.chars().take(max).collect())
}

fn plausible_year(year: Option<i32>) -> Option<i32> {
    year.filter(|value| (1880..=2100).contains(value))
}

fn to_json(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".to_string())
}

fn from_json(text: String) -> Vec<String> {
    serde_json::from_str(&text).unwrap_or_default()
}

/// Titles with a poster and no vision row yet: newest first, adult titles last.
pub fn pending(db: &Database, limit: i64) -> Result<Vec<MediaItem>, String> {
    ensure_table(db)?;
    let mut stmt = db
        .conn
        .prepare(
            "SELECT * FROM media_items m
             WHERE m.poster_path IS NOT NULL AND TRIM(m.poster_path) <> ''
               AND m.media_type <> 'photo'
               AND NOT EXISTS (SELECT 1 FROM media_vision v WHERE v.media_id = m.id)
             ORDER BY (m.media_type = 'adult'), m.date_added DESC
             LIMIT ?1",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![limit.clamp(0, MAX_PENDING)], Database::row_to_media)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

/// Stores the record and fills the title's empty genre/year from it.
pub fn save(db: &Database, id: i64, record: &VisionRecord) -> Result<VisionSaveResult, String> {
    ensure_table(db)?;
    let exists: bool = db
        .conn
        .query_row(
            "SELECT 1 FROM media_items WHERE id = ?1",
            params![id],
            |_| Ok(()),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .is_some();
    if !exists {
        return Err(format!("No library item with id {id}"));
    }
    db.conn
        .execute(
            "INSERT OR REPLACE INTO media_vision
                (media_id, kind, genres, moods, era, tags, caption, read_title, read_year, models, analyzed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'))",
            params![
                id,
                clean_text(&record.kind, 40),
                to_json(&clean_list(&record.genres, 5)),
                to_json(&clean_list(&record.moods, 4)),
                clean_text(&record.era, 40),
                to_json(&clean_list(&record.tags, 12)),
                clean_text(&record.caption, 400),
                clean_text(&record.read_title, 200),
                plausible_year(record.read_year),
                to_json(&clean_list(&record.models, 4)),
            ],
        )
        .map_err(|error| error.to_string())?;

    let mut result = VisionSaveResult::default();
    if let Some(genre) = clean_text(&record.fill_genre, 120) {
        result.genre_filled = db
            .conn
            .execute(
                "UPDATE media_items SET genre = ?1 WHERE id = ?2 AND (genre IS NULL OR TRIM(genre) = '')",
                params![genre, id],
            )
            .map_err(|error| error.to_string())?
            > 0;
    }
    if let Some(year) = plausible_year(record.fill_year) {
        result.year_filled = db
            .conn
            .execute(
                "UPDATE media_items SET year = ?1 WHERE id = ?2 AND (year IS NULL OR year = 0)",
                params![year, id],
            )
            .map_err(|error| error.to_string())?
            > 0;
    }
    Ok(result)
}

pub fn get(db: &Database, id: i64) -> Result<Option<VisionRecord>, String> {
    ensure_table(db)?;
    db.conn
        .query_row(
            "SELECT kind, genres, moods, era, tags, caption, read_title, read_year, models, analyzed_at
             FROM media_vision WHERE media_id = ?1",
            params![id],
            |row| {
                Ok(VisionRecord {
                    kind: row.get(0)?,
                    genres: from_json(row.get(1)?),
                    moods: from_json(row.get(2)?),
                    era: row.get(3)?,
                    tags: from_json(row.get(4)?),
                    caption: row.get(5)?,
                    read_title: row.get(6)?,
                    read_year: row.get(7)?,
                    models: from_json(row.get(8)?),
                    fill_genre: None,
                    fill_year: None,
                    analyzed_at: row.get(9)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())
}

/// Drops a title's record so the next pass looks at its poster again.
pub fn forget(db: &Database, id: i64) -> Result<(), String> {
    ensure_table(db)?;
    db.conn
        .execute("DELETE FROM media_vision WHERE media_id = ?1", params![id])
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn media_vision_pending(
    state: State<AppState>,
    limit: Option<i64>,
) -> Result<Vec<MediaItem>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    pending(&db, limit.unwrap_or(40))
}

#[tauri::command]
pub fn media_vision_save(
    state: State<AppState>,
    id: i64,
    record: VisionRecord,
) -> Result<VisionSaveResult, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    save(&db, id, &record)
}

#[tauri::command]
pub fn media_vision_get(state: State<AppState>, id: i64) -> Result<Option<VisionRecord>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    get(&db, id)
}

#[tauri::command]
pub fn media_vision_forget(state: State<AppState>, id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    forget(&db, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(
        db: &Database,
        title: &str,
        poster: Option<&str>,
        genre: Option<&str>,
        media_type: &str,
    ) -> i64 {
        db.conn
            .execute(
                "INSERT INTO media_items (title, file_path, media_type, poster_path, genre, verified, watched, favorite, date_added)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, 0, datetime('now'))",
                params![title, format!("/m/{title}.mkv"), media_type, poster, genre],
            )
            .unwrap();
        db.conn.last_insert_rowid()
    }

    #[test]
    fn pending_lists_titles_with_posters_and_no_record_adult_last() {
        let db = Database::new(":memory:").unwrap();
        let adult = add(&db, "A", Some("/p/a.jpg"), None, "adult");
        let movie = add(&db, "B", Some("/p/b.jpg"), None, "movie");
        add(&db, "C", None, None, "movie");
        add(&db, "D", Some("  "), None, "movie");
        add(&db, "E", Some("/p/e.jpg"), None, "photo");
        let ids: Vec<i64> = pending(&db, 10)
            .unwrap()
            .into_iter()
            .filter_map(|item| item.id)
            .collect();
        assert_eq!(ids, vec![movie, adult]);

        save(&db, movie, &VisionRecord::default()).unwrap();
        let ids: Vec<i64> = pending(&db, 10)
            .unwrap()
            .into_iter()
            .filter_map(|item| item.id)
            .collect();
        assert_eq!(ids, vec![adult]);
        assert!(pending(&db, 0).unwrap().is_empty());
    }

    #[test]
    fn save_fills_only_empty_fields_and_round_trips() {
        let db = Database::new(":memory:").unwrap();
        let empty = add(&db, "Empty", Some("/p/1.jpg"), None, "movie");
        let known = add(&db, "Known", Some("/p/2.jpg"), Some("Drama"), "movie");
        let record = VisionRecord {
            kind: Some("movie".into()),
            genres: vec!["Horror".into(), "horror".into(), " Thriller ".into()],
            tags: vec!["monsters".into()],
            read_title: Some("Empty".into()),
            read_year: Some(1982),
            models: vec!["Xenova/clip-vit-base-patch32".into()],
            fill_genre: Some("Horror, Thriller".into()),
            fill_year: Some(1982),
            ..Default::default()
        };

        let first = save(&db, empty, &record).unwrap();
        assert_eq!(
            first,
            VisionSaveResult {
                genre_filled: true,
                year_filled: true
            }
        );
        let second = save(&db, known, &record).unwrap();
        assert!(!second.genre_filled);
        let genre: String = db
            .conn
            .query_row(
                "SELECT genre FROM media_items WHERE id = ?1",
                params![known],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(genre, "Drama");

        let stored = get(&db, empty).unwrap().expect("record saved");
        assert_eq!(
            stored.genres,
            vec!["Horror".to_string(), "Thriller".to_string()]
        );
        assert_eq!(stored.read_year, Some(1982));
        assert!(stored.analyzed_at.is_some());

        forget(&db, empty).unwrap();
        assert!(get(&db, empty).unwrap().is_none());
    }

    #[test]
    fn save_rejects_unknown_titles_and_implausible_years() {
        let db = Database::new(":memory:").unwrap();
        assert!(save(&db, 999, &VisionRecord::default()).is_err());
        let id = add(&db, "Odd", Some("/p/o.jpg"), None, "movie");
        let record = VisionRecord {
            read_year: Some(12),
            fill_year: Some(3000),
            ..Default::default()
        };
        assert!(!save(&db, id, &record).unwrap().year_filled);
        assert_eq!(get(&db, id).unwrap().unwrap().read_year, None);
    }
}
