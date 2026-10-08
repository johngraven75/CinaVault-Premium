//! Fills `media_items.content_rating` for parental controls from what the
//! library already has: the `<mpaa>` / `<certification>` tag of a sidecar NFO,
//! then TMDB's certification (movie release_dates or TV content_ratings) for
//! titles with a TMDB id and a configured TMDB key.
//!
//! Stored values are normalised labels (see `parental::normalize_certification`).
//! An empty string means "looked, found nothing", so a refresh without `force`
//! skips those rows.

use crate::db::Database;
use crate::feature_flags;
use crate::parental::{self, normalize_certification};
use crate::AppState;
use rusqlite::params;
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::State;

const REFRESH_LIMIT: i64 = 2000;

/// Certification from an NFO document: `<mpaa>` first, then `<certification>`.
/// Multi-country values ("US:PG-13 / GB:12A") prefer the `region` entry.
pub fn nfo_certification(xml: &str, region: &str) -> Option<String> {
    ["mpaa", "certification"]
        .iter()
        .filter_map(|tag| tag_text(xml, tag))
        .find_map(|value| pick_region(&value, region))
}

fn tag_text(xml: &str, tag: &str) -> Option<String> {
    let lower = xml.to_ascii_lowercase();
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = lower.find(&open)? + open.len();
    let end = start + lower[start..].find(&close)?;
    let text = xml[start..end].trim();
    let text = text
        .strip_prefix("<![CDATA[")
        .and_then(|rest| rest.strip_suffix("]]>"))
        .unwrap_or(text)
        .trim();
    (!text.is_empty()).then(|| text.to_string())
}

fn pick_region(value: &str, region: &str) -> Option<String> {
    let parts: Vec<&str> = value.split(['/', ',', ';']).map(str::trim).collect();
    let wanted = format!("{}:", region.to_ascii_uppercase());
    parts
        .iter()
        .find(|part| part.to_ascii_uppercase().starts_with(&wanted))
        .and_then(|part| normalize_certification(part))
        .or_else(|| parts.iter().find_map(|part| normalize_certification(part)))
}

/// Certification from TMDB `/movie/{id}/release_dates`; theatrical (type 3)
/// wins over other release types.
pub fn tmdb_movie_certification(body: &Value, region: &str) -> Option<String> {
    let releases = body
        .get("results")?
        .as_array()?
        .iter()
        .find(|entry| entry.get("iso_3166_1").and_then(Value::as_str) == Some(region))?
        .get("release_dates")?
        .as_array()?;
    let certification = |release: &&Value| {
        release
            .get("certification")
            .and_then(Value::as_str)
            .and_then(normalize_certification)
    };
    releases
        .iter()
        .filter(|release| release.get("type").and_then(Value::as_i64) == Some(3))
        .find_map(|release| certification(&release))
        .or_else(|| releases.iter().find_map(|release| certification(&release)))
}

/// Certification from TMDB `/tv/{id}/content_ratings`.
pub fn tmdb_tv_certification(body: &Value, region: &str) -> Option<String> {
    body.get("results")?
        .as_array()?
        .iter()
        .find(|entry| entry.get("iso_3166_1").and_then(Value::as_str) == Some(region))?
        .get("rating")
        .and_then(Value::as_str)
        .and_then(normalize_certification)
}

/// NFO files that may describe `file_path`: `<stem>.nfo`, then `movie.nfo`,
/// then `tvshow.nfo` beside it or one folder up (season folders).
pub fn nfo_candidates(file_path: &str) -> Vec<PathBuf> {
    let path = Path::new(file_path);
    let Some(parent) = path.parent() else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
        candidates.push(parent.join(format!("{stem}.nfo")));
    }
    candidates.push(parent.join("movie.nfo"));
    candidates.push(parent.join("tvshow.nfo"));
    if let Some(grandparent) = parent.parent() {
        candidates.push(grandparent.join("tvshow.nfo"));
    }
    candidates
}

fn is_tv(media_type: &str) -> bool {
    matches!(
        media_type.trim().to_ascii_lowercase().as_str(),
        "tv" | "series" | "show" | "episode" | "tvshow" | "tv_show"
    )
}

#[derive(Debug, Clone)]
struct Candidate {
    id: i64,
    file_path: String,
    media_type: String,
    tmdb_id: Option<String>,
}

#[derive(Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RatingRefresh {
    pub checked: usize,
    pub from_nfo: usize,
    pub from_tmdb: usize,
    pub unrated: usize,
}

fn candidates(db: &Database, force: bool) -> rusqlite::Result<Vec<Candidate>> {
    let filter = if force {
        "1 = 1"
    } else {
        "content_rating IS NULL"
    };
    let mut stmt = db.conn.prepare(&format!(
        "SELECT id, file_path, media_type, tmdb_id FROM media_items
         WHERE media_type <> 'photo' AND {filter} ORDER BY id LIMIT ?1"
    ))?;
    let rows = stmt.query_map(params![REFRESH_LIMIT], |row| {
        Ok(Candidate {
            id: row.get(0)?,
            file_path: row.get(1)?,
            media_type: row.get(2)?,
            tmdb_id: row.get(3)?,
        })
    })?;
    rows.collect()
}

fn tmdb_key(db: &Database) -> Option<String> {
    let mut stmt = db
        .conn
        .prepare("SELECT provider, api_key FROM api_keys WHERE lower(provider) IN ('tmdb', 'themoviedb')")
        .ok()?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .ok()?
        .filter_map(Result::ok)
        .collect();
    rows.into_iter().find_map(|(provider, stored)| {
        crate::metadata_ext::resolve_stored_provider_key(&provider, &stored)
            .ok()
            .flatten()
    })
}

fn region(db: &Database) -> String {
    feature_flags::config(db, parental::SWITCH)
        .get("region")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_ascii_uppercase())
        .filter(|value| value.len() == 2)
        .unwrap_or_else(|| "US".into())
}

async fn fetch_tmdb(
    client: &reqwest::Client,
    key: &str,
    tmdb_id: &str,
    tv: bool,
    region: &str,
) -> Option<String> {
    let id: String = tmdb_id.chars().filter(char::is_ascii_digit).collect();
    if id.is_empty() {
        return None;
    }
    let url = if tv {
        format!("https://api.themoviedb.org/3/tv/{id}/content_ratings?api_key={key}")
    } else {
        format!("https://api.themoviedb.org/3/movie/{id}/release_dates?api_key={key}")
    };
    let body = client
        .get(url)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<Value>()
        .await
        .ok()?;
    if tv {
        tmdb_tv_certification(&body, region)
    } else {
        tmdb_movie_certification(&body, region)
    }
}

/// Looks up certifications for rows without one (all rows with `force`).
/// The database lock is only held while reading candidates and writing results.
pub async fn refresh(
    db: &std::sync::Mutex<Database>,
    force: bool,
) -> Result<RatingRefresh, String> {
    let (rows, key, region) = {
        let db = db.lock().map_err(|error| error.to_string())?;
        (
            candidates(&db, force).map_err(|error| error.to_string())?,
            tmdb_key(&db),
            region(&db),
        )
    };
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| error.to_string())?;
    let mut summary = RatingRefresh::default();
    let mut results = Vec::with_capacity(rows.len());
    for row in rows {
        summary.checked += 1;
        let from_nfo = nfo_candidates(&row.file_path)
            .into_iter()
            .filter(|path| path.is_file())
            .find_map(|path| std::fs::read_to_string(path).ok())
            .and_then(|xml| nfo_certification(&xml, &region));
        let rating = if let Some(rating) = from_nfo {
            summary.from_nfo += 1;
            Some(rating)
        } else if let (Some(key), Some(tmdb_id)) = (key.as_deref(), row.tmdb_id.as_deref()) {
            let tv = is_tv(&row.media_type);
            let found = match fetch_tmdb(&client, key, tmdb_id, tv, &region).await {
                Some(found) => Some(found),
                None => fetch_tmdb(&client, key, tmdb_id, !tv, &region).await,
            };
            if found.is_some() {
                summary.from_tmdb += 1;
            }
            found
        } else {
            None
        };
        if rating.is_none() {
            summary.unrated += 1;
        }
        results.push((row.id, rating.unwrap_or_default()));
    }
    let db = db.lock().map_err(|error| error.to_string())?;
    for (id, rating) in results {
        db.conn
            .execute(
                "UPDATE media_items SET content_rating = ?2 WHERE id = ?1",
                params![id, rating],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(summary)
}

/// Fills missing ratings in the background when parental controls are on.
pub fn spawn_startup_refresh(app: tauri::AppHandle) {
    use tauri::Manager;
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if !feature_flags::is_enabled_state(&state, parental::SWITCH) {
            return;
        }
        match refresh(&state.db, false).await {
            Ok(summary) => log::info!("Content ratings refreshed at startup: {summary:?}"),
            Err(error) => log::warn!("Content rating refresh failed: {error}"),
        }
    });
}

#[tauri::command]
pub async fn parental_refresh_ratings(
    state: State<'_, AppState>,
    force: Option<bool>,
) -> Result<RatingRefresh, String> {
    refresh(&state.db, force.unwrap_or(false)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn nfo_mpaa_and_certification_tags_are_read() {
        let movie = "<?xml version=\"1.0\"?><movie><title>X</title><MPAA>Rated PG-13 for peril</MPAA></movie>";
        assert_eq!(nfo_certification(movie, "US").as_deref(), Some("PG-13"));
        let multi = "<movie><mpaa>GB:15 / US:R</mpaa></movie>";
        assert_eq!(nfo_certification(multi, "US").as_deref(), Some("R"));
        assert_eq!(nfo_certification(multi, "GB").as_deref(), Some("15"));
        let cdata = "<tvshow><certification><![CDATA[TV-14]]></certification></tvshow>";
        assert_eq!(nfo_certification(cdata, "US").as_deref(), Some("TV-14"));
        let unknown = "<movie><mpaa>Not Rated</mpaa></movie>";
        assert_eq!(nfo_certification(unknown, "US"), None);
        assert_eq!(nfo_certification("<movie></movie>", "US"), None);
    }

    #[test]
    fn tmdb_movie_prefers_regional_theatrical_certification() {
        let body = json!({ "id": 1, "results": [
            { "iso_3166_1": "GB", "release_dates": [{ "certification": "12A", "type": 3 }] },
            { "iso_3166_1": "US", "release_dates": [
                { "certification": "", "type": 1 },
                { "certification": "NR", "type": 4 },
                { "certification": "PG-13", "type": 3 }
            ]}
        ]});
        assert_eq!(
            tmdb_movie_certification(&body, "US").as_deref(),
            Some("PG-13")
        );
        assert_eq!(
            tmdb_movie_certification(&body, "GB").as_deref(),
            Some("12A")
        );
        assert_eq!(tmdb_movie_certification(&body, "FR"), None);
        let digital_only = json!({ "results": [
            { "iso_3166_1": "US", "release_dates": [{ "certification": "R", "type": 4 }] }
        ]});
        assert_eq!(
            tmdb_movie_certification(&digital_only, "US").as_deref(),
            Some("R")
        );
    }

    #[test]
    fn tmdb_tv_rating_is_read() {
        let body = json!({ "results": [
            { "iso_3166_1": "DE", "rating": "12" },
            { "iso_3166_1": "US", "rating": "TV-MA" }
        ]});
        assert_eq!(tmdb_tv_certification(&body, "US").as_deref(), Some("TV-MA"));
        assert_eq!(tmdb_tv_certification(&body, "DE").as_deref(), Some("12"));
    }

    #[test]
    fn sidecar_nfo_fills_ratings_and_marks_misses() {
        let dir = std::env::temp_dir().join(format!("cinavault-nfo-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let rated = dir.join("Rated Film.mkv");
        let bare = dir.join("Bare Film.mkv");
        std::fs::write(
            dir.join("Rated Film.nfo"),
            "<movie><mpaa>US:PG</mpaa></movie>",
        )
        .unwrap();
        let db_path = dir.join("lib.db");
        let db = Database::new(db_path.to_str().unwrap()).unwrap();
        for path in [&rated, &bare] {
            db.conn
                .execute(
                    "INSERT INTO media_items (title, file_path, media_type, date_added) VALUES ('t', ?1, 'movie', 'now')",
                    params![path.to_string_lossy()],
                )
                .unwrap();
        }
        let lock = std::sync::Mutex::new(db);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let summary = runtime.block_on(refresh(&lock, false)).unwrap();
        assert_eq!(
            summary,
            RatingRefresh {
                checked: 2,
                from_nfo: 1,
                from_tmdb: 0,
                unrated: 1
            }
        );
        let db = lock.into_inner().unwrap();
        let ratings: Vec<String> = db
            .conn
            .prepare("SELECT content_rating FROM media_items ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(ratings, vec!["PG".to_string(), String::new()]);
        // Already-checked rows are skipped unless forced.
        let again = runtime
            .block_on(refresh(&std::sync::Mutex::new(db), false))
            .unwrap();
        assert_eq!(again.checked, 0);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn nfo_candidates_cover_movie_and_season_layouts() {
        let found = nfo_candidates("/lib/Show/Season 1/Show S01E01.mkv");
        assert_eq!(
            found[0],
            PathBuf::from("/lib/Show/Season 1/Show S01E01.nfo")
        );
        assert!(found.contains(&PathBuf::from("/lib/Show/tvshow.nfo")));
        assert!(found.contains(&PathBuf::from("/lib/Show/Season 1/movie.nfo")));
    }
}
