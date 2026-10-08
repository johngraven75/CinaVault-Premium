//! Automatic collections for the "collection_auto" switch (Settings > Library
//! "Smart Collections" is the same switch).
//!
//! Three kinds, rebuilt after every scan and on demand:
//! - series: episodes of the same show (NFO show title, else the cleaned name
//!   in front of the SxxEyy marker);
//! - franchise: movies in the same set (NFO <set>, or TMDB
//!   belongs_to_collection when a TMDB key and id are known);
//! - genre: the most common movie genres.
//!
//! Adult titles never appear in collections.

use crate::db::{Database, MediaItem};
use crate::media_extras::{self, MediaExtras};
use crate::title_clean;
use crate::AppState;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::Duration;
use tauri::State;

/// TMDB lookups per rebuild, so a large first run stays short.
const TMDB_LOOKUPS_PER_RUN: usize = 40;

pub fn ensure_tables(db: &Database) -> Result<(), String> {
    media_extras::ensure_table(db)?;
    db.conn
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS collections (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                collection_key TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                item_count INTEGER NOT NULL DEFAULT 0,
                poster_path TEXT,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS collection_items (
                collection_id INTEGER NOT NULL,
                media_id INTEGER NOT NULL,
                position INTEGER NOT NULL,
                PRIMARY KEY (collection_id, media_id)
            );
            CREATE INDEX IF NOT EXISTS idx_collection_items_media ON collection_items(media_id);",
        )
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectionSettings {
    pub top_genres: usize,
    pub min_items: usize,
}

pub fn settings(db: &Database) -> CollectionSettings {
    let config = crate::feature_flags::config(db, "collection_auto");
    let number = |key: &str, fallback: usize, max: usize| {
        config
            .get(key)
            .and_then(Value::as_u64)
            .map(|value| (value as usize).clamp(1, max))
            .unwrap_or(fallback)
    };
    CollectionSettings {
        top_genres: number("topGenres", 8, 30),
        min_items: number("minItems", 2, 50).max(2),
    }
}

#[derive(Debug, Clone, Default)]
pub struct CollectionInput {
    pub id: i64,
    pub title: String,
    pub file_path: String,
    pub media_type: String,
    pub year: Option<i32>,
    pub rating: Option<f64>,
    pub genre: Option<String>,
    pub poster_path: Option<String>,
    pub extras: MediaExtras,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub key: String,
    pub name: String,
    pub kind: &'static str,
    pub item_ids: Vec<i64>,
    pub poster_path: Option<String>,
}

fn normalize_key(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

fn split_genres(genre: Option<&str>) -> Vec<String> {
    genre
        .unwrap_or_default()
        .split([',', '/', '|', ';'])
        .map(str::trim)
        .filter(|genre| !genre.is_empty())
        .map(str::to_string)
        .collect()
}

fn first_poster(items: &[&CollectionInput]) -> Option<String> {
    items
        .iter()
        .filter_map(|item| item.poster_path.clone())
        .find(|poster| !poster.trim().is_empty())
}

fn series_of(item: &CollectionInput) -> Option<(String, Option<u32>, Option<u32>)> {
    let parsed = title_clean::parse_path(Path::new(&item.file_path));
    let from_title = title_clean::parse_release_name(&item.title);
    let season = item.extras.season.or(parsed.season).or(from_title.season);
    let episode = item
        .extras
        .episode
        .or(parsed.episode)
        .or(from_title.episode);
    if let Some(show) = item
        .extras
        .series_title
        .clone()
        .filter(|s| !s.trim().is_empty())
    {
        return Some((show, season, episode));
    }
    if parsed.is_episode() && !parsed.title.is_empty() {
        return Some((parsed.title, season, episode));
    }
    if from_title.is_episode() && !from_title.title.is_empty() {
        return Some((from_title.title, season, episode));
    }
    None
}

/// (season, episode, item) for ordering a show.
type EpisodeRef<'a> = (Option<u32>, Option<u32>, &'a CollectionInput);

/// Groups the library. Pure: the database is not touched.
pub fn build_groups(items: &[CollectionInput], settings: CollectionSettings) -> Vec<Group> {
    let eligible: Vec<&CollectionInput> = items
        .iter()
        .filter(|item| {
            let kind = item.media_type.to_ascii_lowercase();
            kind != "adult" && kind != "music"
        })
        .collect();
    let mut groups = Vec::new();

    // Series.
    let mut series: BTreeMap<String, (String, Vec<EpisodeRef>)> = BTreeMap::new();
    let mut in_series = std::collections::HashSet::new();
    for item in &eligible {
        if let Some((show, season, episode)) = series_of(item) {
            let entry = series
                .entry(normalize_key(&show))
                .or_insert_with(|| (show.clone(), Vec::new()));
            entry.1.push((season, episode, item));
            in_series.insert(item.id);
        }
    }
    for (key, (name, mut members)) in series {
        if members.len() < settings.min_items || key.is_empty() {
            continue;
        }
        members.sort_by(|a, b| {
            (a.0.unwrap_or(0), a.1.unwrap_or(0), &a.2.title).cmp(&(
                b.0.unwrap_or(0),
                b.1.unwrap_or(0),
                &b.2.title,
            ))
        });
        let ordered: Vec<&CollectionInput> = members.iter().map(|m| m.2).collect();
        groups.push(Group {
            key: format!("series:{key}"),
            name,
            kind: "series",
            item_ids: ordered.iter().map(|item| item.id).collect(),
            poster_path: first_poster(&ordered),
        });
    }

    // Franchises.
    let mut franchises: BTreeMap<String, (String, Vec<&CollectionInput>)> = BTreeMap::new();
    for item in eligible.iter().filter(|item| !in_series.contains(&item.id)) {
        if let Some(name) = item
            .extras
            .collection_name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            franchises
                .entry(normalize_key(name))
                .or_insert_with(|| (name.to_string(), Vec::new()))
                .1
                .push(item);
        }
    }
    for (key, (name, mut members)) in franchises {
        if members.len() < settings.min_items {
            continue;
        }
        members.sort_by(|a, b| {
            (a.year.unwrap_or(9999), &a.title).cmp(&(b.year.unwrap_or(9999), &b.title))
        });
        groups.push(Group {
            key: format!("franchise:{key}"),
            name,
            kind: "franchise",
            item_ids: members.iter().map(|item| item.id).collect(),
            poster_path: first_poster(&members),
        });
    }

    // Top genres, movies only (episodes are already grouped by show).
    let mut by_genre: HashMap<String, (String, Vec<&CollectionInput>)> = HashMap::new();
    for item in eligible.iter().filter(|item| !in_series.contains(&item.id)) {
        let mut seen = Vec::new();
        for genre in split_genres(item.genre.as_deref()) {
            let key = normalize_key(&genre);
            if key.is_empty() || seen.contains(&key) {
                continue;
            }
            seen.push(key.clone());
            by_genre
                .entry(key)
                .or_insert_with(|| (genre.clone(), Vec::new()))
                .1
                .push(item);
        }
    }
    let mut genres: Vec<(String, String, Vec<&CollectionInput>)> = by_genre
        .into_iter()
        .filter(|(_, (_, members))| members.len() >= settings.min_items.max(3))
        .map(|(key, (name, members))| (key, name, members))
        .collect();
    genres.sort_by(|a, b| b.2.len().cmp(&a.2.len()).then_with(|| a.0.cmp(&b.0)));
    for (key, name, mut members) in genres.into_iter().take(settings.top_genres) {
        members.sort_by(|a, b| {
            b.rating
                .unwrap_or(0.0)
                .total_cmp(&a.rating.unwrap_or(0.0))
                .then_with(|| b.year.cmp(&a.year))
                .then_with(|| a.title.cmp(&b.title))
        });
        groups.push(Group {
            key: format!("genre:{key}"),
            name,
            kind: "genre",
            item_ids: members.iter().map(|item| item.id).collect(),
            poster_path: first_poster(&members),
        });
    }
    groups
}

fn load_inputs(db: &Database) -> Result<Vec<CollectionInput>, String> {
    media_extras::ensure_table(db)?;
    let mut statement = db
        .conn
        .prepare(
            "SELECT m.id, m.title, m.file_path, m.media_type, m.year, m.rating, m.genre, m.poster_path,
                    e.content_rating, e.collection_name, e.collection_tmdb_id, e.series_title,
                    e.season, e.episode, e.collection_checked_at
             FROM media_items m LEFT JOIN media_item_extras e ON e.media_id = m.id",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(CollectionInput {
                id: row.get(0)?,
                title: row.get(1)?,
                file_path: row.get(2)?,
                media_type: row.get(3)?,
                year: row.get(4)?,
                rating: row.get(5)?,
                genre: row.get(6)?,
                poster_path: row.get(7)?,
                extras: MediaExtras {
                    content_rating: row.get(8)?,
                    collection_name: row.get(9)?,
                    collection_tmdb_id: row.get(10)?,
                    series_title: row.get(11)?,
                    season: row.get(12)?,
                    episode: row.get(13)?,
                    collection_checked_at: row.get(14)?,
                },
            })
        })
        .map_err(|error| error.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RebuildReport {
    pub series: usize,
    pub franchises: usize,
    pub genres: usize,
    pub items_grouped: usize,
    pub franchise_lookups: usize,
    pub errors: Vec<String>,
}

/// Replaces the stored collections with a fresh grouping. Collection ids
/// stay stable for groups that still exist.
pub fn rebuild(db: &Database) -> Result<RebuildReport, String> {
    ensure_tables(db)?;
    let groups = build_groups(&load_inputs(db)?, settings(db));
    let now = chrono::Utc::now().to_rfc3339();
    let tx = db
        .conn
        .unchecked_transaction()
        .map_err(|error| error.to_string())?;
    tx.execute("DELETE FROM collection_items", [])
        .map_err(|error| error.to_string())?;
    let mut report = RebuildReport::default();
    let mut kept = Vec::new();
    for group in &groups {
        tx.execute(
            "INSERT INTO collections (collection_key, name, kind, item_count, poster_path, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(collection_key) DO UPDATE SET name = excluded.name, kind = excluded.kind,
                item_count = excluded.item_count, poster_path = excluded.poster_path,
                updated_at = excluded.updated_at",
            params![group.key, group.name, group.kind, group.item_ids.len() as i64, group.poster_path, now],
        )
        .map_err(|error| error.to_string())?;
        let id: i64 = tx
            .query_row(
                "SELECT id FROM collections WHERE collection_key = ?1",
                params![group.key],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        for (position, media_id) in group.item_ids.iter().enumerate() {
            tx.execute(
                "INSERT OR IGNORE INTO collection_items (collection_id, media_id, position) VALUES (?1, ?2, ?3)",
                params![id, media_id, position as i64],
            )
            .map_err(|error| error.to_string())?;
        }
        kept.push(group.key.clone());
        report.items_grouped += group.item_ids.len();
        match group.kind {
            "series" => report.series += 1,
            "franchise" => report.franchises += 1,
            _ => report.genres += 1,
        }
    }
    let stale: Vec<String> = {
        let mut statement = tx
            .prepare("SELECT collection_key FROM collections")
            .map_err(|error| error.to_string())?;
        let keys = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?
            .filter_map(Result::ok)
            .filter(|key| !kept.contains(key))
            .collect();
        keys
    };
    for key in stale {
        tx.execute(
            "DELETE FROM collections WHERE collection_key = ?1",
            params![key],
        )
        .map_err(|error| error.to_string())?;
    }
    tx.commit().map_err(|error| error.to_string())?;
    Ok(report)
}

fn tmdb_key(db: &Database) -> Option<String> {
    let stored = db
        .conn
        .query_row(
            "SELECT api_key FROM api_keys WHERE provider = 'tmdb'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten()?;
    crate::metadata_ext::resolve_stored_provider_key("tmdb", &stored)
        .ok()
        .flatten()
        .filter(|key| !key.trim().is_empty())
}

/// Reads `belongs_to_collection` from a TMDB movie response.
pub fn franchise_from_tmdb(body: &Value) -> Option<(String, String)> {
    let collection = body.get("belongs_to_collection")?;
    let name = collection.get("name")?.as_str()?.trim();
    let id = collection.get("id")?.as_i64()?;
    (!name.is_empty()).then(|| (name.to_string(), id.to_string()))
}

/// Asks TMDB which collection each not-yet-checked movie belongs to.
pub async fn refresh_franchises(state: &AppState) -> (usize, Vec<String>) {
    let (key, pending) = match state.db.lock() {
        Ok(db) => {
            let key = tmdb_key(&db);
            let pending: Vec<(i64, String)> =
                if key.is_some() && media_extras::ensure_table(&db).is_ok() {
                    db.conn
                        .prepare(
                            "SELECT m.id, m.tmdb_id FROM media_items m
                         LEFT JOIN media_item_extras e ON e.media_id = m.id
                         WHERE m.media_type = 'movie' AND trim(COALESCE(m.tmdb_id, '')) <> ''
                           AND e.collection_checked_at IS NULL
                         ORDER BY m.id DESC LIMIT ?1",
                        )
                        .and_then(|mut statement| {
                            statement
                                .query_map(params![TMDB_LOOKUPS_PER_RUN as i64], |row| {
                                    Ok((row.get(0)?, row.get(1)?))
                                })
                                .map(|rows| rows.filter_map(Result::ok).collect())
                        })
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
            (key, pending)
        }
        Err(error) => return (0, vec![error.to_string()]),
    };
    let Some(key) = key else {
        return (0, Vec::new());
    };
    let client = match reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(4))
        .timeout(Duration::from_secs(12))
        .build()
    {
        Ok(client) => client,
        Err(error) => return (0, vec![error.to_string()]),
    };
    let mut done = 0;
    let mut errors = Vec::new();
    for (media_id, tmdb_id) in pending {
        if crate::task_progress::stop_requested() {
            break;
        }
        let tmdb_id = tmdb_id.trim().to_string();
        if !tmdb_id.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        // v4 read-access tokens go in a header, v3 keys in the query.
        let mut request = client.get(format!("https://api.themoviedb.org/3/movie/{tmdb_id}"));
        request = if key.starts_with("eyJ") {
            request.bearer_auth(&key)
        } else {
            request.query(&[("api_key", key.as_str())])
        };
        let body = match request.send().await {
            Ok(response) if response.status().is_success() => response.json::<Value>().await.ok(),
            Ok(response) => {
                errors.push(format!("TMDB movie {tmdb_id}: HTTP {}", response.status()));
                if response.status().as_u16() == 401 {
                    break;
                }
                continue;
            }
            Err(error) => {
                errors.push(format!("TMDB movie {tmdb_id}: {error}"));
                continue;
            }
        };
        let Some(body) = body else { continue };
        let franchise = franchise_from_tmdb(&body);
        if let Ok(db) = state.db.lock() {
            let existing = media_extras::get(&db, media_id).unwrap_or_default();
            let extras = MediaExtras {
                // An NFO <set> stays; TMDB only fills the gap.
                collection_name: existing
                    .collection_name
                    .is_none()
                    .then(|| franchise.as_ref().map(|f| f.0.clone()))
                    .flatten(),
                collection_tmdb_id: franchise.map(|f| f.1),
                collection_checked_at: Some(chrono::Utc::now().to_rfc3339()),
                ..MediaExtras::default()
            };
            if media_extras::merge(&db, media_id, &extras).is_ok() {
                done += 1;
            }
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    (done, errors)
}

/// TMDB franchise lookups, then a rebuild. Used after scans and by the command.
pub async fn refresh_and_rebuild(state: &AppState) -> Result<RebuildReport, String> {
    let (lookups, errors) = refresh_franchises(state).await;
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let mut report = rebuild(&db)?;
    report.franchise_lookups = lookups;
    report.errors = errors;
    Ok(report)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionSummary {
    pub id: i64,
    pub key: String,
    pub name: String,
    pub kind: String,
    pub item_count: i64,
    pub poster_path: Option<String>,
    pub updated_at: String,
}

pub fn list(db: &Database) -> Result<Vec<CollectionSummary>, String> {
    ensure_tables(db)?;
    let mut statement = db
        .conn
        .prepare(
            "SELECT id, collection_key, name, kind, item_count, poster_path, updated_at FROM collections
             ORDER BY CASE kind WHEN 'franchise' THEN 0 WHEN 'series' THEN 1 ELSE 2 END,
                      item_count DESC, name",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(CollectionSummary {
                id: row.get(0)?,
                key: row.get(1)?,
                name: row.get(2)?,
                kind: row.get(3)?,
                item_count: row.get(4)?,
                poster_path: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(|error| error.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn items(db: &Database, collection_id: i64) -> Result<Vec<MediaItem>, String> {
    ensure_tables(db)?;
    let mut statement = db
        .conn
        .prepare(
            "SELECT m.* FROM collection_items c JOIN media_items m ON m.id = c.media_id
             WHERE c.collection_id = ?1 ORDER BY c.position",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![collection_id], Database::row_to_media)
        .map_err(|error| error.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

/// Collections for the library views; empty while the switch is off.
#[tauri::command]
pub fn collections_list(state: State<AppState>) -> Result<Vec<CollectionSummary>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    if !crate::feature_flags::is_enabled(&db, "collection_auto") {
        return Ok(Vec::new());
    }
    list(&db)
}

#[tauri::command]
pub fn collection_items(
    state: State<AppState>,
    collection_id: i64,
) -> Result<Vec<MediaItem>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    items(&db, collection_id)
}

#[tauri::command]
pub async fn collections_rebuild(state: State<'_, AppState>) -> Result<RebuildReport, String> {
    if !crate::feature_flags::is_enabled_state(state.inner(), "collection_auto") {
        return Err("Auto Collections is off".to_string());
    }
    refresh_and_rebuild(state.inner()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(id: i64, title: &str, file: &str, genre: Option<&str>) -> CollectionInput {
        CollectionInput {
            id,
            title: title.into(),
            file_path: file.into(),
            media_type: "movie".into(),
            genre: genre.map(str::to_string),
            ..Default::default()
        }
    }

    fn settings() -> CollectionSettings {
        CollectionSettings {
            top_genres: 2,
            min_items: 2,
        }
    }

    #[test]
    fn groups_series_franchises_and_top_genres() {
        let mut items = vec![
            input(
                1,
                "Breaking Bad - S01E02",
                "/tv/Breaking.Bad.S01E02.mkv",
                Some("Drama"),
            ),
            input(
                2,
                "Pilot",
                "/tv/Breaking.Bad.S01E01.720p.mkv",
                Some("Drama"),
            ),
            input(3, "Lonely Show", "/tv/Lonely.S01E01.mkv", None),
            input(4, "Alien", "/m/Alien.1979.mkv", Some("Horror, Sci-Fi")),
            input(5, "Aliens", "/m/Aliens.1986.mkv", Some("Action / Sci-Fi")),
            input(6, "Heat", "/m/Heat.mkv", Some("Crime, Drama")),
            input(7, "Arrival", "/m/Arrival.mkv", Some("Sci-Fi, Drama")),
            input(8, "Se7en", "/m/Se7en.mkv", Some("Crime, Drama")),
            input(9, "Adult", "/x/Adult.S01E01.mkv", Some("Drama")),
        ];
        items[3].year = Some(1979);
        items[4].year = Some(1986);
        items[3].extras.collection_name = Some("Alien Collection".into());
        items[4].extras.collection_name = Some("Alien Collection".into());
        items[5].rating = Some(8.3);
        items[8].media_type = "adult".into();
        items[6].poster_path = Some("/art/arrival.jpg".into());

        let groups = build_groups(&items, settings());
        let find = |key: &str| groups.iter().find(|g| g.key == key).cloned();

        let series = find("series:breaking-bad").expect("series group");
        assert_eq!(series.name, "Breaking Bad");
        assert_eq!(series.item_ids, vec![2, 1], "ordered by episode");
        assert!(
            find("series:lonely").is_none(),
            "one episode is not a collection"
        );

        let franchise = find("franchise:alien-collection").expect("franchise group");
        assert_eq!(franchise.item_ids, vec![4, 5]);

        // Movies only: Sci-Fi (4,5,7) and Drama (6,7,8); Crime (6,8) is below 3.
        let genres: Vec<_> = groups.iter().filter(|g| g.kind == "genre").collect();
        assert_eq!(genres.len(), 2);
        assert_eq!(genres[0].key, "genre:drama");
        assert_eq!(genres[0].item_ids[0], 6, "highest rated first");
        assert_eq!(genres[1].key, "genre:sci-fi");
        assert_eq!(genres[1].poster_path.as_deref(), Some("/art/arrival.jpg"));
        assert!(
            groups.iter().all(|g| !g.item_ids.contains(&9)),
            "adult titles are excluded"
        );
    }

    #[test]
    fn rebuild_keeps_ids_and_drops_stale_collections() {
        let db = Database::new(":memory:").unwrap();
        for (title, file) in [
            ("S1", "/tv/Show.S01E01.mkv"),
            ("S2", "/tv/Show.S01E02.mkv"),
            ("Other", "/tv/Other.S01E01.mkv"),
        ] {
            db.conn
                .execute(
                    "INSERT INTO media_items (title, file_path, media_type, date_added) VALUES (?1, ?2, 'episode', 'now')",
                    params![title, file],
                )
                .unwrap();
        }
        let report = rebuild(&db).unwrap();
        assert_eq!((report.series, report.items_grouped), (1, 2));
        let first = list(&db).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].name, "Show");
        let members = items(&db, first[0].id).unwrap();
        assert_eq!(
            members.iter().map(|m| m.title.as_str()).collect::<Vec<_>>(),
            vec!["S1", "S2"]
        );

        db.conn
            .execute(
                "INSERT INTO media_items (title, file_path, media_type, date_added) VALUES ('O2', '/tv/Other.S01E02.mkv', 'episode', 'now')",
                [],
            )
            .unwrap();
        rebuild(&db).unwrap();
        let second = list(&db).unwrap();
        assert_eq!(second.len(), 2);
        assert_eq!(
            second.iter().find(|c| c.key == "series:show").unwrap().id,
            first[0].id
        );

        db.conn
            .execute("DELETE FROM media_items WHERE file_path LIKE '%Show%'", [])
            .unwrap();
        rebuild(&db).unwrap();
        assert!(list(&db).unwrap().iter().all(|c| c.key != "series:show"));
    }

    #[test]
    fn reads_tmdb_collection_and_settings() {
        let body = serde_json::json!({ "belongs_to_collection": { "id": 8091, "name": "Alien Collection" } });
        assert_eq!(
            franchise_from_tmdb(&body),
            Some(("Alien Collection".to_string(), "8091".to_string()))
        );
        assert_eq!(
            franchise_from_tmdb(&serde_json::json!({ "belongs_to_collection": null })),
            None
        );
        let db = Database::new(":memory:").unwrap();
        assert_eq!(
            super::settings(&db),
            CollectionSettings {
                top_genres: 8,
                min_items: 2
            }
        );
        db.set_feature_setting_data("collection_auto", true, r#"{"topGenres":3,"minItems":4}"#)
            .unwrap();
        assert_eq!(
            super::settings(&db),
            CollectionSettings {
                top_genres: 3,
                min_items: 4
            }
        );
    }
}
