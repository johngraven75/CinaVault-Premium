//! Per-item actions for a library card: unmatch and delete.
//!
//! These back the hover controls on every poster card in the Media Library.
//! "Check metadata" reuses `metadata_guard::check_media_item_metadata`; this
//! module adds the two actions that change library state rather than fetch it.
//!
//! Deleting a file is never a hard delete: the file is sent to the operating
//! system's recycle bin / trash via the `trash` crate, so a mistaken click is
//! recoverable. Unmatch never touches the file on disk at all.

use crate::db::Database;
use crate::AppState;
use rusqlite::{params, OptionalExtension};
use std::path::Path;
use tauri::State;

/// The file's base name without its extension, used as the fallback title
/// after an unmatch so the card shows the filename again.
///
/// Splits on both `/` and `\` regardless of host OS: CinaVault runs on
/// Windows so stored paths use backslashes, but `std::path::Path` only treats
/// `\` as a separator on Windows, so a `Path`-based stem would keep the whole
/// directory on Linux (where CI runs the tests).
pub(crate) fn filename_title(file_path: &str) -> String {
    let name = file_path
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or(file_path);
    // Strip the extension at the last dot, but keep a leading-dot name intact.
    let stem = match name.rfind('.') {
        Some(dot) if dot > 0 => &name[..dot],
        _ => name,
    };
    let trimmed = stem.trim();
    if trimmed.is_empty() {
        file_path.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Clears the metadata match on one item: resets the title to the filename and
/// nulls every fetched field and external id. Returns the fallback title.
/// Does not touch the file on disk.
pub(crate) fn unmatch_item_in_db(db: &Database, id: i64) -> Result<String, String> {
    let file_path: String = db
        .conn
        .query_row(
            "SELECT file_path FROM media_items WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let fallback_title = filename_title(&file_path);
    db.conn
        .execute(
            "UPDATE media_items SET title = ?1, overview = NULL, poster_path = NULL, \
             backdrop_path = NULL, year = NULL, rating = NULL, genre = NULL, \
             tmdb_id = NULL, imdb_id = NULL WHERE id = ?2",
            params![fallback_title, id],
        )
        .map_err(|error| error.to_string())?;
    Ok(fallback_title)
}

/// Removes one item's library row and returns its file path if it had one.
/// The file itself is left on disk; the caller decides whether to trash it.
pub(crate) fn delete_item_row(db: &Database, id: i64) -> Result<Option<String>, String> {
    let file_path: Option<String> = db
        .conn
        .query_row(
            "SELECT file_path FROM media_items WHERE id = ?1",
            params![id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    db.conn
        .execute("DELETE FROM media_items WHERE id = ?1", params![id])
        .map_err(|error| error.to_string())?;
    Ok(file_path)
}

/// Clears the current metadata match so the item falls back to its filename and
/// the next scan or a Check metadata run can match it fresh.
#[tauri::command]
pub fn metadata_unmatch_item(state: State<AppState>, id: i64) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let fallback_title = unmatch_item_in_db(&db, id)?;
    Ok(serde_json::json!({
        "type": "metadata_unmatch",
        "status": "success",
        "item_id": id,
        "fallback_title": fallback_title,
        "file_removed": false,
    }))
}

/// Deletes one library item. Always removes the library row. When `remove_file`
/// is true, the file is also sent to the OS trash (recoverable), never deleted
/// outright.
#[tauri::command]
pub fn library_delete_item(
    state: State<AppState>,
    id: i64,
    remove_file: bool,
) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;

    // Look up the path first, then trash the file BEFORE removing the row, so a
    // trash failure leaves the library row intact and the command's error
    // matches on-disk reality (the card stays, the file stays).
    let file_path: Option<String> = db
        .conn
        .query_row(
            "SELECT file_path FROM media_items WHERE id = ?1",
            params![id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    let mut file_removed = false;
    if remove_file {
        if let Some(path) = file_path.as_deref() {
            if Path::new(path).exists() {
                trash::delete(path)
                    .map_err(|error| format!("file was not sent to trash: {error}"))?;
                file_removed = true;
            }
        }
    }

    delete_item_row(&db, id)?;

    Ok(serde_json::json!({
        "type": "library_delete",
        "status": "success",
        "item_id": id,
        "file_removed": file_removed,
        "removed_path": file_path,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_item(db: &Database, title: &str, path: &str) -> i64 {
        db.conn
            .execute(
                "INSERT INTO media_items (title, file_path, media_type, year, rating, overview, \
                 poster_path, genre, tmdb_id, imdb_id, date_added) \
                 VALUES (?1, ?2, 'adult', 2021, 7.5, 'synopsis', '/art/p.jpg', 'Drama', 'tt1', 'im1', '2026-01-01')",
                params![title, path],
            )
            .expect("insert item");
        db.conn.last_insert_rowid()
    }

    #[test]
    fn filename_title_uses_the_stem() {
        assert_eq!(filename_title(r"E:\Vids\Scene.One.2021.1080p.mp4"), "Scene.One.2021.1080p");
        assert_eq!(filename_title("/media/clip.mkv"), "clip");
    }

    #[test]
    fn unmatch_clears_match_and_keeps_the_row() {
        let db = Database::new(":memory:").expect("db");
        let id = seed_item(&db, "Matched Title", r"E:\Vids\raw_file_name.mp4");

        let title = unmatch_item_in_db(&db, id).expect("unmatch");
        assert_eq!(title, "raw_file_name");

        // The row still exists, now reset to the filename with no fetched fields.
        let (title, tmdb, imdb, poster, overview): (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = db
            .conn
            .query_row(
                "SELECT title, tmdb_id, imdb_id, poster_path, overview FROM media_items WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .expect("row still present");
        assert_eq!(title, "raw_file_name");
        assert!(tmdb.is_none() && imdb.is_none() && poster.is_none() && overview.is_none());
    }

    #[test]
    fn delete_row_removes_item_and_returns_path() {
        let db = Database::new(":memory:").expect("db");
        let id = seed_item(&db, "Gone", r"E:\Vids\gone.mp4");

        let path = delete_item_row(&db, id).expect("delete");
        assert_eq!(path.as_deref(), Some(r"E:\Vids\gone.mp4"));

        let remaining: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM media_items WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .expect("count");
        assert_eq!(remaining, 0, "the library row is gone");
    }
}
