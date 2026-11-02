//! Per-item actions for a library card: unmatch, delete, and apply a Lookup match.
//!
//! These back the hover controls and the corner menu on every poster card in
//! the Media Library. "Check metadata" reuses
//! `metadata_enrichment_runtime::check_media_item_metadata`; this module adds
//! the actions that change library state rather than fetch it. Applying a
//! Lookup match reuses `db::update_media_metadata_data` (the enrichment path's
//! writer, keyed by file_path with COALESCE semantics: a field sent as None
//! keeps its current value), plus a direct backdrop update, because that
//! helper does not cover `backdrop_path`.
//!
//! Deleting a file is never a hard delete: the file is sent to the operating
//! system's recycle bin / trash via the `trash` crate, so a mistaken click is
//! recoverable. Unmatch never touches the file on disk at all.

use crate::db::{Database, MediaItem};
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

/// Fields a Lookup candidate may apply to a library item. Every field is
/// optional: `None` (or a blank string) preserves the current value, it never
/// clears it. This mirrors `db::update_media_metadata_data`'s COALESCE
/// semantics. Search candidates carry title/overview/poster/year/tmdb/imdb;
/// backdrop/genre/rating/media_type are accepted for completeness but the
/// Lookup dialog sends them as null, so they are preserved, not overwritten.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ApplyMatchFields {
    pub title: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub year: Option<i32>,
    pub rating: Option<f64>,
    pub genre: Option<String>,
    pub tmdb_id: Option<String>,
    pub imdb_id: Option<String>,
    pub media_type: Option<String>,
}

/// Blank strings are treated as absent: a candidate field that is empty must
/// not overwrite a real value with whitespace.
fn clean_field(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

/// Applies one user-chosen Lookup candidate to a library item and returns the
/// refreshed row. The caller (the `metadata_apply_match_item` command) owns
/// the edition gate; this helper is deliberately gate-free so the unit tests
/// below run identically in both editions, exactly like `unmatch_item_in_db`.
pub(crate) fn apply_match_in_db(
    db: &Database,
    id: i64,
    fields: &ApplyMatchFields,
) -> Result<MediaItem, String> {
    let file_path: String = db
        .conn
        .query_row(
            "SELECT file_path FROM media_items WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Library item not found".to_string())?;

    let title = clean_field(&fields.title);
    let overview = clean_field(&fields.overview);
    let poster_path = clean_field(&fields.poster_path);
    let backdrop_path = clean_field(&fields.backdrop_path);
    let genre = clean_field(&fields.genre);
    let tmdb_id = clean_field(&fields.tmdb_id);
    let imdb_id = clean_field(&fields.imdb_id);
    let media_type = clean_field(&fields.media_type);
    let rating = fields.rating.filter(|value| value.is_finite());

    if title.is_none()
        && overview.is_none()
        && poster_path.is_none()
        && backdrop_path.is_none()
        && fields.year.is_none()
        && rating.is_none()
        && genre.is_none()
        && tmdb_id.is_none()
        && imdb_id.is_none()
        && media_type.is_none()
    {
        return Err("No metadata fields to apply".to_string());
    }

    db.update_media_metadata_data(
        &file_path,
        title.as_deref(),
        overview.as_deref(),
        poster_path.as_deref(),
        fields.year,
        rating,
        genre.as_deref(),
        tmdb_id.as_deref(),
        imdb_id.as_deref(),
        media_type.as_deref(),
    )
    .map_err(|error| error.to_string())?;

    // `update_media_metadata_data` does not cover backdrop_path, so a supplied
    // backdrop is written directly; an absent one leaves the current value.
    if let Some(backdrop) = backdrop_path.as_deref() {
        db.conn
            .execute(
                "UPDATE media_items SET backdrop_path = ?1 WHERE id = ?2",
                params![backdrop, id],
            )
            .map_err(|error| error.to_string())?;
    }

    db.conn
        .query_row(
            "SELECT * FROM media_items WHERE id = ?1",
            params![id],
            Database::row_to_media,
        )
        .map_err(|error| error.to_string())
}

/// Applies the candidate the user picked in the Lookup dialog. Returns the
/// updated row as `updated_item`, the same envelope shape Check metadata
/// returns, so the frontend refreshes through `applyUpdatedMediaItem`.
///
/// Edition gate, same shape as the sibling commands: there is deliberately no
/// blanket `ensure_adult_allowed` here — like unmatch and delete, this command
/// is media-type agnostic, and a blanket adult gate would refuse legitimate
/// movie/TV matches in the store-safe edition. Provider-level gating already
/// happened at search time (`search_metadata` runs
/// `library_metadata_policy::ensure_provider_allowed`, and the dialog only
/// offers providers that policy marks in use). The one case this command must
/// still refuse is an adult item in the store-safe build: those rows are
/// filtered from the UI there, so such a call could only be forged. That check
/// uses the edition module's own `source_type_blocked` helper.
#[tauri::command]
pub fn metadata_apply_match_item(
    state: State<AppState>,
    id: i64,
    candidate: ApplyMatchFields,
) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let media_type: String = db
        .conn
        .query_row(
            "SELECT media_type FROM media_items WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Library item not found".to_string())?;
    if crate::edition::source_type_blocked(&media_type) {
        return Err(crate::edition::STORE_SAFE_REFUSAL.into());
    }
    let updated = apply_match_in_db(&db, id, &candidate)?;
    Ok(serde_json::json!({
        "type": "metadata_apply_match",
        "status": "success",
        "item_id": id,
        "updated_item": updated,
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

    fn read_match_row(
        db: &Database,
        id: i64,
    ) -> (String, Option<String>, Option<String>, Option<String>, Option<i32>, Option<f64>, Option<String>, Option<String>, Option<String>) {
        db.conn
            .query_row(
                "SELECT title, overview, poster_path, backdrop_path, year, rating, genre, tmdb_id, imdb_id FROM media_items WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                    ))
                },
            )
            .expect("row still present")
    }

    #[test]
    fn apply_match_updates_supplied_fields_and_preserves_the_rest() {
        let db = Database::new(":memory:").expect("db");
        let id = seed_item(&db, "Raw File Name", r"E:\Vids\raw_file_name.mp4");

        let updated = apply_match_in_db(
            &db,
            id,
            &ApplyMatchFields {
                title: Some("Matched Title".to_string()),
                overview: Some("New synopsis".to_string()),
                poster_path: Some("/art/new.jpg".to_string()),
                backdrop_path: Some("/art/backdrop.jpg".to_string()),
                year: Some(2020),
                rating: None,
                genre: None,
                tmdb_id: Some("12345".to_string()),
                imdb_id: None,
                media_type: None,
            },
        )
        .expect("apply");

        // The command returns the refreshed row, not just an acknowledgement.
        assert_eq!(updated.id, Some(id));
        assert_eq!(updated.title, "Matched Title");
        assert_eq!(updated.backdrop_path.as_deref(), Some("/art/backdrop.jpg"));

        // Supplied fields changed; absent fields (rating, genre, imdb) kept
        // the seeded values instead of being cleared.
        let (title, overview, poster, backdrop, year, rating, genre, tmdb, imdb) =
            read_match_row(&db, id);
        assert_eq!(title, "Matched Title");
        assert_eq!(overview.as_deref(), Some("New synopsis"));
        assert_eq!(poster.as_deref(), Some("/art/new.jpg"));
        assert_eq!(backdrop.as_deref(), Some("/art/backdrop.jpg"));
        assert_eq!(year, Some(2020));
        assert_eq!(rating, Some(7.5));
        assert_eq!(genre.as_deref(), Some("Drama"));
        assert_eq!(tmdb.as_deref(), Some("12345"));
        assert_eq!(imdb.as_deref(), Some("im1"));
    }

    #[test]
    fn apply_match_rejects_an_empty_candidate_and_a_missing_item() {
        let db = Database::new(":memory:").expect("db");
        let id = seed_item(&db, "Something", r"E:\Vids\something.mp4");

        let empty = apply_match_in_db(&db, id, &ApplyMatchFields::default());
        assert!(empty.is_err(), "an all-None candidate must not write");

        let blank = apply_match_in_db(
            &db,
            id,
            &ApplyMatchFields {
                title: Some("   ".to_string()),
                ..Default::default()
            },
        );
        assert!(blank.is_err(), "a blank-only candidate must not write");

        let missing = apply_match_in_db(
            &db,
            id + 1000,
            &ApplyMatchFields {
                title: Some("Ghost".to_string()),
                ..Default::default()
            },
        );
        assert!(missing.is_err(), "a missing item must error, not insert");
    }
}
