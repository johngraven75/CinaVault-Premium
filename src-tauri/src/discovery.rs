//! Library home discovery: the Trending, Recommended, More Like This, New
//! Releases and Genre Radio shelves. Everything works on "works" (the unified
//! library's one-entry-per-title groups) so a film with three copies is one
//! card, and everything respects the active profile from user_data.
//!
//! Trending uses TMDB's weekly trending list when a TMDB key is configured,
//! matched against the library by TMDB id or cleaned title + year and cached
//! for six hours. Without a key, or when TMDB cannot be reached and nothing is
//! cached, it ranks the library by recent plays on this PC instead.

use crate::db::{Database, MediaItem};
use crate::feature_flags;
use crate::library_unify::{self, NormalizedTitle, UnifiedEntry};
use crate::user_data;
use crate::AppState;
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use rusqlite::{params, OptionalExtension, Result as SqlResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use tauri::State;

/// How long a fetched TMDB trending list is reused.
pub const TRENDING_TTL_HOURS: i64 = 6;
/// Plays older than this do not count towards "Popular on this PC".
pub const POPULAR_WINDOW_DAYS: i64 = 30;
const TRENDING_CACHE_KEY: &str = "tmdb_trending_week";
const TMDB_PAGES: u32 = 3;
const NEW_RELEASES_SETTING_PREFIX: &str = "new_releases_seen_";

/// One shelf card: a unified work plus why it is on the shelf.
#[derive(Debug, Serialize, Clone)]
pub struct DiscoveryEntry {
    #[serde(flatten)]
    pub entry: UnifiedEntry,
    pub reason: String,
    pub score: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct TrendingShelf {
    /// "tmdb" or "local".
    pub source: String,
    pub title: String,
    /// Why the shelf fell back, or what it is based on.
    pub note: Option<String>,
    pub fetched_at: Option<String>,
    pub items: Vec<DiscoveryEntry>,
}

#[derive(Debug, Serialize, Clone)]
pub struct NewReleases {
    /// The last visit the count is measured from; None on a profile's first visit.
    pub since: Option<String>,
    pub checked_at: String,
    pub count: usize,
    pub items: Vec<DiscoveryEntry>,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct GenreCount {
    pub name: String,
    pub count: usize,
}

/// A TMDB trending title, as cached.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TrendingTitle {
    pub tmdb_id: i64,
    /// "movie" or "tv".
    pub media_type: String,
    pub title: String,
    pub year: Option<i32>,
    pub rank: usize,
}

pub fn ensure_tables(db: &Database) -> SqlResult<()> {
    db.conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS discovery_cache (
            cache_key TEXT PRIMARY KEY,
            fetched_at TEXT NOT NULL,
            payload TEXT NOT NULL
        );",
    )
}

// ------------------------------------------------------------ helpers ----

/// Parses the timestamp formats media_items and the user tables use.
pub fn parse_timestamp(raw: &str) -> Option<DateTime<Utc>> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(parsed) = DateTime::parse_from_rfc3339(raw) {
        return Some(parsed.with_timezone(&Utc));
    }
    for format in ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S%.f"] {
        if let Ok(parsed) = NaiveDateTime::parse_from_str(raw, format) {
            return Some(parsed.and_utc());
        }
    }
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .ok()
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|date| date.and_utc())
}

/// Splits a stored genre field ("Action, Drama / Sci-Fi") into distinct names.
pub fn split_genres(raw: Option<&str>) -> Vec<String> {
    let mut seen = HashSet::new();
    raw.unwrap_or_default()
        .split([',', '/', '|', ';'])
        .map(str::trim)
        .filter(|name| !name.is_empty() && name.len() <= 40)
        .filter(|name| seen.insert(name.to_lowercase()))
        .map(String::from)
        .collect()
}

/// What the similarity function compares.
#[derive(Debug, Clone, PartialEq)]
pub struct Features {
    pub genres: BTreeSet<String>,
    pub year: Option<i32>,
    pub rating: Option<f64>,
    pub group: String,
}

impl Features {
    pub fn of(item: &MediaItem) -> Self {
        let normalized = library_unify::normalize_title(&item.title);
        Features {
            genres: split_genres(item.genre.as_deref())
                .into_iter()
                .map(|genre| genre.to_lowercase())
                .collect(),
            year: item.year.or(normalized.year),
            rating: item
                .rating
                .filter(|rating| rating.is_finite() && *rating > 0.0),
            group: library_unify::media_type_group(&item.media_type),
        }
    }
}

/// How alike two titles are, 0..=1: genre overlap carries most of the weight,
/// then release year proximity, rating closeness and media type.
pub fn similarity(a: &Features, b: &Features) -> f64 {
    let shared = a.genres.intersection(&b.genres).count() as f64;
    let union = a.genres.union(&b.genres).count() as f64;
    let genre = if union > 0.0 { shared / union } else { 0.0 };
    let year = match (a.year, b.year) {
        (Some(x), Some(y)) => (-(f64::from((x - y).abs())) / 10.0).exp(),
        _ => 0.0,
    };
    let rating = match (a.rating, b.rating) {
        (Some(x), Some(y)) => (1.0 - (x - y).abs() / 10.0).clamp(0.0, 1.0),
        _ => 0.0,
    };
    let kind = if a.group == b.group { 1.0 } else { 0.0 };
    0.55 * genre + 0.2 * year + 0.15 * rating + 0.1 * kind
}

fn shared_genres(a: &MediaItem, b: &MediaItem) -> Vec<String> {
    let theirs: HashSet<String> = split_genres(b.genre.as_deref())
        .into_iter()
        .map(|genre| genre.to_lowercase())
        .collect();
    split_genres(a.genre.as_deref())
        .into_iter()
        .filter(|genre| theirs.contains(&genre.to_lowercase()))
        .collect()
}

/// Every media id of a work, once each (the primary is also listed as a copy).
fn entry_ids(entry: &UnifiedEntry) -> impl Iterator<Item = i64> {
    let mut ids: Vec<i64> = entry
        .primary
        .id
        .into_iter()
        .chain(entry.copies.iter().map(|copy| copy.id))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids.into_iter()
}

/// Episodes of one show collapse to a single card: the earliest episode.
fn collapse_episodes(entries: Vec<UnifiedEntry>) -> Vec<UnifiedEntry> {
    let mut shows: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<UnifiedEntry> = Vec::with_capacity(entries.len());
    for entry in entries {
        let normalized: NormalizedTitle = library_unify::normalize_title(&entry.primary.title);
        let is_episode = library_unify::media_type_group(&entry.primary.media_type) == "episode"
            || normalized.episode.is_some();
        if !is_episode || normalized.title.is_empty() {
            out.push(entry);
            continue;
        }
        let episode = normalized.episode.unwrap_or((u32::MAX, u32::MAX));
        match shows.get(&normalized.title) {
            Some(&index) => {
                let current = library_unify::normalize_title(&out[index].primary.title)
                    .episode
                    .unwrap_or((u32::MAX, u32::MAX));
                if episode < current {
                    out[index] = entry;
                }
            }
            None => {
                shows.insert(normalized.title, out.len());
                out.push(entry);
            }
        }
    }
    out
}

// ---------------------------------------------------- recommendations ----

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SeedKind {
    Watched,
    Favorite,
    Watchlist,
}

/// A title the profile has shown interest in, with how strongly (0..=1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Seed {
    pub media_id: i64,
    pub weight: f64,
    pub kind: SeedKind,
}

/// What the ranking needs to know about the profile.
#[derive(Debug, Clone, Default)]
pub struct ProfileSignals {
    pub seeds: Vec<Seed>,
    /// Finished or currently-in-progress titles: never recommended.
    pub exclude: HashSet<i64>,
}

fn reason_for(kind: SeedKind, title: &str) -> String {
    match kind {
        SeedKind::Watched => format!("Because you watched {title}"),
        SeedKind::Favorite => format!("Because you liked {title}"),
        SeedKind::Watchlist => format!("Because {title} is on your watchlist"),
    }
}

/// Ranks works the profile has not watched by how much they resemble the
/// titles it watched, liked or saved. Without any signal, falls back to the
/// best rated unwatched titles.
pub fn rank_recommendations(
    works: &[UnifiedEntry],
    signals: &ProfileSignals,
    limit: usize,
) -> Vec<DiscoveryEntry> {
    let mut seed_by_id: HashMap<i64, Seed> = HashMap::new();
    for seed in &signals.seeds {
        let keep = seed_by_id
            .get(&seed.media_id)
            .map(|existing| existing.weight < seed.weight)
            .unwrap_or(true);
        if keep {
            seed_by_id.insert(seed.media_id, *seed);
        }
    }
    // Seeds at work level: the strongest signal on any copy.
    let mut seed_works: Vec<(usize, Seed, Features)> = Vec::new();
    let mut seeded: HashSet<usize> = HashSet::new();
    for (index, work) in works.iter().enumerate() {
        let best = entry_ids(work)
            .filter_map(|id| seed_by_id.get(&id).copied())
            .max_by(|a, b| a.weight.total_cmp(&b.weight));
        if let Some(seed) = best {
            seed_works.push((index, seed, Features::of(&work.primary)));
            seeded.insert(index);
        }
    }

    let candidates = works.iter().enumerate().filter(|(index, work)| {
        !seeded.contains(index) && !entry_ids(work).any(|id| signals.exclude.contains(&id))
    });

    let mut ranked: Vec<DiscoveryEntry> = if seed_works.is_empty() {
        candidates
            .filter_map(|(_, work)| {
                let rating = work.primary.rating.filter(|rating| *rating >= 6.0)?;
                Some(DiscoveryEntry {
                    entry: work.clone(),
                    reason: "Top rated in your library".into(),
                    score: rating / 10.0,
                })
            })
            .collect()
    } else {
        candidates
            .filter_map(|(_, work)| {
                let features = Features::of(&work.primary);
                let (score, seed_index) = seed_works
                    .iter()
                    .enumerate()
                    .map(|(i, (_, seed, seed_features))| {
                        (seed.weight * similarity(&features, seed_features), i)
                    })
                    .max_by(|a, b| a.0.total_cmp(&b.0))?;
                let (seed_work, seed, seed_features) = &seed_works[seed_index];
                // Without a shared genre a match is only a coincidence of year and type.
                if features.genres.is_disjoint(&seed_features.genres) {
                    return None;
                }
                let quality = work.primary.rating.unwrap_or(0.0).clamp(0.0, 10.0) / 10.0;
                Some(DiscoveryEntry {
                    entry: work.clone(),
                    reason: reason_for(seed.kind, &works[*seed_work].primary.title),
                    score: score + 0.08 * quality,
                })
            })
            .filter(|entry| entry.score >= 0.25)
            .collect()
    };
    ranked.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.entry.primary.title.cmp(&b.entry.primary.title))
    });
    ranked.truncate(limit);
    ranked
}

/// Works most like `target` (excluding the target's own work).
pub fn rank_similar(
    works: &[UnifiedEntry],
    target: &MediaItem,
    limit: usize,
) -> Vec<DiscoveryEntry> {
    let target_features = Features::of(target);
    let target_ids: HashSet<i64> = target.id.into_iter().collect();
    let target_title = library_unify::normalize_title(&target.title).title;
    let mut ranked: Vec<DiscoveryEntry> = works
        .iter()
        .filter(|work| !entry_ids(work).any(|id| target_ids.contains(&id)))
        .filter(|work| {
            // Other episodes of the same show are not "more like this".
            target_title.is_empty()
                || library_unify::normalize_title(&work.primary.title).title != target_title
        })
        .filter_map(|work| {
            let features = Features::of(&work.primary);
            if features.genres.is_disjoint(&target_features.genres) {
                return None;
            }
            let score = similarity(&target_features, &features);
            let shared = shared_genres(&work.primary, target);
            Some(DiscoveryEntry {
                entry: work.clone(),
                reason: format!("Also {}", shared.join(", ")),
                score,
            })
        })
        .filter(|entry| entry.score >= 0.3)
        .collect();
    ranked.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.entry.primary.title.cmp(&b.entry.primary.title))
    });
    ranked.truncate(limit);
    ranked
}

// ------------------------------------------------------------ trending ----

/// Reads TMDB's /trending/all/week results (one or more pages, in order).
pub fn parse_tmdb_trending(pages: &[Value]) -> Vec<TrendingTitle> {
    let mut out = Vec::new();
    for page in pages {
        let Some(results) = page.get("results").and_then(Value::as_array) else {
            continue;
        };
        for result in results {
            let media_type = result
                .get("media_type")
                .and_then(Value::as_str)
                .unwrap_or("movie");
            if media_type != "movie" && media_type != "tv" {
                continue;
            }
            let Some(tmdb_id) = result.get("id").and_then(Value::as_i64) else {
                continue;
            };
            let title = result
                .get("title")
                .or_else(|| result.get("name"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_string();
            if title.is_empty() {
                continue;
            }
            let year = result
                .get("release_date")
                .or_else(|| result.get("first_air_date"))
                .and_then(Value::as_str)
                .and_then(|date| date.get(..4))
                .and_then(|year| year.parse().ok());
            out.push(TrendingTitle {
                tmdb_id,
                media_type: media_type.into(),
                title,
                year,
                rank: out.len() + 1,
            });
        }
    }
    out
}

fn type_compatible(trending_type: &str, group: &str) -> bool {
    match trending_type {
        "tv" => group == "episode",
        _ => group == "movie" || group == "video",
    }
}

/// Library works on TMDB's trending list, in trending order. A work matches by
/// TMDB id, or by cleaned title with a year within one (or no year on either side).
pub fn match_trending(trending: &[TrendingTitle], works: &[UnifiedEntry]) -> Vec<DiscoveryEntry> {
    struct Candidate<'a> {
        work: &'a UnifiedEntry,
        group: String,
        normalized: NormalizedTitle,
        tmdb_ids: HashSet<String>,
    }
    let candidates: Vec<Candidate> = works
        .iter()
        .map(|work| {
            let normalized = library_unify::normalize_title(&work.primary.title);
            let mut group = library_unify::media_type_group(&work.primary.media_type);
            if normalized.episode.is_some() {
                group = "episode".into();
            }
            let tmdb_ids = work
                .primary
                .tmdb_id
                .iter()
                .map(|id| id.trim().to_string())
                .filter(|id| !id.is_empty())
                .collect();
            Candidate {
                work,
                group,
                normalized,
                tmdb_ids,
            }
        })
        .collect();

    let mut used: HashSet<&str> = HashSet::new();
    let mut out = Vec::new();
    for title in trending {
        let id = title.tmdb_id.to_string();
        let wanted = library_unify::normalize_title(&title.title).title;
        let mut matches: Vec<&Candidate> = candidates
            .iter()
            .filter(|candidate| type_compatible(&title.media_type, &candidate.group))
            .filter(|candidate| !used.contains(candidate.work.work_key.as_str()))
            .filter(|candidate| {
                if candidate.tmdb_ids.contains(&id) {
                    return true;
                }
                if wanted.is_empty() || candidate.normalized.title != wanted {
                    return false;
                }
                let year = candidate.work.primary.year.or(candidate.normalized.year);
                match (year, title.year) {
                    (Some(a), Some(b)) => (a - b).abs() <= 1,
                    _ => true,
                }
            })
            .collect();
        if matches.is_empty() {
            continue;
        }
        // For a show, the earliest episode stands for it.
        matches.sort_by_key(|candidate| candidate.normalized.episode.unwrap_or((u32::MAX, 0)));
        let best = matches[0];
        if title.media_type == "tv" {
            for candidate in &matches {
                used.insert(candidate.work.work_key.as_str());
            }
        } else {
            used.insert(best.work.work_key.as_str());
        }
        out.push(DiscoveryEntry {
            entry: best.work.clone(),
            reason: format!("#{} trending this week", title.rank),
            score: 1.0 / title.rank as f64,
        });
    }
    out
}

/// A play of `media_id` at `at`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayEvent {
    pub media_id: i64,
    pub at: DateTime<Utc>,
}

/// Per-title play counts within the window and a recency-weighted score.
pub fn popularity(
    events: &[PlayEvent],
    now: DateTime<Utc>,
    window_days: i64,
) -> HashMap<i64, (usize, f64)> {
    let window = chrono::Duration::days(window_days);
    let mut scores: HashMap<i64, (usize, f64)> = HashMap::new();
    for event in events {
        let age = now - event.at;
        if age < chrono::Duration::zero() - chrono::Duration::hours(1) || age > window {
            continue;
        }
        let freshness = 1.0 - age.num_minutes().max(0) as f64 / window.num_minutes() as f64;
        let slot = scores.entry(event.media_id).or_insert((0, 0.0));
        slot.0 += 1;
        slot.1 += 0.5 + 0.5 * freshness;
    }
    scores
}

/// Works ranked by recent plays on this PC (all profiles).
pub fn rank_popular(
    works: &[UnifiedEntry],
    events: &[PlayEvent],
    now: DateTime<Utc>,
    limit: usize,
) -> Vec<DiscoveryEntry> {
    let scores = popularity(events, now, POPULAR_WINDOW_DAYS);
    let mut ranked: Vec<DiscoveryEntry> = works
        .iter()
        .filter_map(|work| {
            let (plays, score) = entry_ids(work)
                .filter_map(|id| scores.get(&id))
                .fold((0usize, 0.0f64), |acc, (plays, score)| {
                    (acc.0 + plays, acc.1 + score)
                });
            (plays > 0).then(|| DiscoveryEntry {
                entry: work.clone(),
                reason: if plays == 1 {
                    "Played once this month".into()
                } else {
                    format!("Played {plays} times this month")
                },
                score,
            })
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.entry.primary.title.cmp(&b.entry.primary.title))
    });
    ranked.truncate(limit);
    ranked
}

// ------------------------------------------------------- new releases ----

/// Works added after `since`, newest first.
pub fn added_since(works: &[UnifiedEntry], since: DateTime<Utc>) -> Vec<DiscoveryEntry> {
    let mut found: Vec<(DateTime<Utc>, &UnifiedEntry)> = works
        .iter()
        .filter_map(|work| {
            let added = parse_timestamp(&work.primary.date_added)?;
            (added > since).then_some((added, work))
        })
        .collect();
    found.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    found
        .into_iter()
        .map(|(added, work)| DiscoveryEntry {
            entry: work.clone(),
            reason: format!("Added {}", added.format("%Y-%m-%d")),
            score: added.timestamp() as f64,
        })
        .collect()
}

// ------------------------------------------------------------ genres ----

pub fn genre_counts(works: &[UnifiedEntry]) -> Vec<GenreCount> {
    let mut counts: HashMap<String, (String, usize)> = HashMap::new();
    for work in works {
        for genre in split_genres(work.primary.genre.as_deref()) {
            let slot = counts
                .entry(genre.to_lowercase())
                .or_insert_with(|| (genre.clone(), 0));
            slot.1 += 1;
        }
    }
    let mut out: Vec<GenreCount> = counts
        .into_values()
        .map(|(name, count)| GenreCount { name, count })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    out
}

pub fn works_in_genre(works: &[UnifiedEntry], genre: &str) -> Vec<UnifiedEntry> {
    let wanted = genre.trim().to_lowercase();
    works
        .iter()
        .filter(|work| {
            split_genres(work.primary.genre.as_deref())
                .iter()
                .any(|name| name.to_lowercase() == wanted)
        })
        .cloned()
        .collect()
}

// --------------------------------------------------------- data access ----

/// The active profile's library as works: store-safe builds and restricted
/// profiles never see adult titles.
fn load_works(db: &Database, collapse_shows: bool) -> Result<Vec<UnifiedEntry>, String> {
    let restricted = user_data::active_profile(db)
        .map(|profile| profile.restricted)
        .unwrap_or(false);
    let items: Vec<MediaItem> = db
        .get_media_items_data(None, None, None)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|item| {
            let adult = library_unify::media_type_group(&item.media_type) == "adult";
            !(adult && (restricted || crate::edition::STORE_SAFE))
        })
        .collect();
    let works = library_unify::group_items(items);
    Ok(if collapse_shows {
        collapse_episodes(works)
    } else {
        works
    })
}

fn load_signals(db: &Database, profile_id: i64) -> Result<ProfileSignals, String> {
    let mut signals = ProfileSignals::default();
    {
        let mut stmt = db
            .conn
            .prepare(
                "SELECT media_id, position, duration, finished FROM playback_progress WHERE profile_id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![profile_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, f64>(1)?,
                    row.get::<_, f64>(2)?,
                    row.get::<_, bool>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (media_id, position, duration, finished) = row.map_err(|e| e.to_string())?;
            signals.exclude.insert(media_id);
            let fraction = if duration > 0.0 {
                (position / duration).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let weight = if finished { 1.0 } else { fraction };
            if weight >= 0.25 {
                signals.seeds.push(Seed {
                    media_id,
                    weight,
                    kind: SeedKind::Watched,
                });
            }
        }
    }
    // The library-wide watched/favorite flags belong to the owner profile.
    if profile_id == user_data::DEFAULT_PROFILE_ID {
        let mut stmt = db
            .conn
            .prepare(
                "SELECT id, watched, favorite FROM media_items WHERE watched = 1 OR favorite = 1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, bool>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (media_id, watched, favorite) = row.map_err(|e| e.to_string())?;
            if watched {
                signals.exclude.insert(media_id);
                signals.seeds.push(Seed {
                    media_id,
                    weight: 0.9,
                    kind: SeedKind::Watched,
                });
            }
            if favorite {
                signals.seeds.push(Seed {
                    media_id,
                    weight: 0.8,
                    kind: SeedKind::Favorite,
                });
            }
        }
    }
    for item in user_data::watchlist(db, profile_id).map_err(|e| e.to_string())? {
        if let Some(media_id) = item.id {
            signals.exclude.insert(media_id);
            signals.seeds.push(Seed {
                media_id,
                weight: 0.6,
                kind: SeedKind::Watchlist,
            });
        }
    }
    Ok(signals)
}

fn load_play_events(db: &Database) -> Result<Vec<PlayEvent>, String> {
    let mut events = Vec::new();
    let mut push = |media_id: i64, raw: &str| {
        if let Some(at) = parse_timestamp(raw) {
            events.push(PlayEvent { media_id, at });
        }
    };
    {
        let mut stmt = db
            .conn
            .prepare("SELECT media_id, updated_at, finished FROM playback_progress")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, bool>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (media_id, at, finished) = row.map_err(|e| e.to_string())?;
            push(media_id, &at);
            if finished {
                push(media_id, &at);
            }
        }
    }
    {
        let mut stmt = db
            .conn
            .prepare("SELECT detail_json, at FROM activity_log WHERE kind = 'playback.started'")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (detail, at) = row.map_err(|e| e.to_string())?;
            let media_id = serde_json::from_str::<Value>(&detail)
                .ok()
                .and_then(|detail| detail.get("mediaId").and_then(Value::as_i64));
            if let Some(media_id) = media_id {
                push(media_id, &at);
            }
        }
    }
    {
        let mut stmt = db
            .conn
            .prepare("SELECT id, last_played FROM media_items WHERE last_played IS NOT NULL AND last_played <> ''")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (media_id, at) = row.map_err(|e| e.to_string())?;
            push(media_id, &at);
        }
    }
    Ok(events)
}

/// The configured TMDB key: the saved provider key (secure store or plain),
/// else the CINAVAULT_TMDB_API_KEY / TMDB_API_KEY environment variable.
fn tmdb_key(db: &Database) -> Option<String> {
    let saved = db
        .conn
        .prepare(
            "SELECT provider, api_key FROM api_keys
             WHERE lower(provider) IN ('tmdb', 'tmdb_images', 'themoviedb', 'themoviedb_images')
               AND trim(api_key) <> ''",
        )
        .and_then(|mut stmt| {
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect::<SqlResult<Vec<_>>>()
        })
        .unwrap_or_default();
    saved
        .into_iter()
        .filter(|(_, stored)| !stored.starts_with("http"))
        .find_map(|(provider, stored)| {
            crate::metadata_ext::resolve_stored_provider_key(&provider, &stored)
                .ok()
                .flatten()
        })
        .or_else(|| {
            ["CINAVAULT_TMDB_API_KEY", "TMDB_API_KEY"]
                .iter()
                .find_map(|name| std::env::var(name).ok())
        })
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

fn read_cache(db: &Database) -> Option<(DateTime<Utc>, Vec<TrendingTitle>)> {
    let (fetched_at, payload): (String, String) = db
        .conn
        .query_row(
            "SELECT fetched_at, payload FROM discovery_cache WHERE cache_key = ?1",
            params![TRENDING_CACHE_KEY],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .ok()
        .flatten()?;
    Some((
        parse_timestamp(&fetched_at)?,
        serde_json::from_str(&payload).ok()?,
    ))
}

fn write_cache(db: &Database, titles: &[TrendingTitle], at: DateTime<Utc>) -> Result<(), String> {
    let payload = serde_json::to_string(titles).map_err(|e| e.to_string())?;
    db.conn
        .execute(
            "INSERT OR REPLACE INTO discovery_cache (cache_key, fetched_at, payload) VALUES (?1, ?2, ?3)",
            params![TRENDING_CACHE_KEY, at.to_rfc3339(), payload],
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Whether a cached list fetched at `fetched_at` is still fresh at `now`.
pub fn cache_is_fresh(fetched_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    let age = now - fetched_at;
    age >= chrono::Duration::zero() && age < chrono::Duration::hours(TRENDING_TTL_HOURS)
}

async fn fetch_tmdb_trending(key: &str) -> Result<Vec<TrendingTitle>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|e| e.to_string())?;
    // A v4 read token is a JWT; a v3 key goes in the query string.
    let bearer = key.starts_with("eyJ");
    let mut pages = Vec::new();
    for page in 1..=TMDB_PAGES {
        let mut request = client
            .get("https://api.themoviedb.org/3/trending/all/week")
            .query(&[("page", page.to_string())])
            .header("Accept", "application/json");
        request = if bearer {
            request.bearer_auth(key)
        } else {
            request.query(&[("api_key", key)])
        };
        let response = request.send().await.map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            return Err(format!("TMDB answered HTTP {}", response.status().as_u16()));
        }
        pages.push(response.json::<Value>().await.map_err(|e| e.to_string())?);
    }
    Ok(parse_tmdb_trending(&pages))
}

fn local_trending(
    db: &Database,
    works: &[UnifiedEntry],
    limit: usize,
    note: Option<String>,
) -> Result<TrendingShelf, String> {
    let events = load_play_events(db)?;
    Ok(TrendingShelf {
        source: "local".into(),
        title: "Popular on this PC".into(),
        note,
        fetched_at: None,
        items: rank_popular(works, &events, Utc::now(), limit),
    })
}

// ------------------------------------------------------------ commands ----

fn with_db<T>(
    state: &State<AppState>,
    f: impl FnOnce(&Database) -> Result<T, String>,
) -> Result<T, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    user_data::ensure_tables(&db).map_err(|e| e.to_string())?;
    ensure_tables(&db).map_err(|e| e.to_string())?;
    f(&db)
}

#[tauri::command]
pub fn discovery_recommendations(
    state: State<AppState>,
    limit: Option<usize>,
) -> Result<Vec<DiscoveryEntry>, String> {
    with_db(&state, |db| {
        if !feature_flags::is_enabled(db, "recommendations") {
            return Ok(Vec::new());
        }
        let works = load_works(db, true)?;
        let signals = load_signals(db, user_data::active_profile_id(db))?;
        Ok(rank_recommendations(
            &works,
            &signals,
            limit.unwrap_or(24).clamp(1, 100),
        ))
    })
}

#[tauri::command]
pub fn discovery_similar(
    state: State<AppState>,
    media_id: i64,
    limit: Option<usize>,
) -> Result<Vec<DiscoveryEntry>, String> {
    with_db(&state, |db| {
        if !feature_flags::is_enabled(db, "similar_titles") {
            return Ok(Vec::new());
        }
        let Some(target) = db
            .get_media_items_data(None, None, None)
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|item| item.id == Some(media_id))
        else {
            return Ok(Vec::new());
        };
        let works = load_works(db, true)?;
        Ok(rank_similar(
            &works,
            &target,
            limit.unwrap_or(12).clamp(1, 50),
        ))
    })
}

#[tauri::command]
pub async fn discovery_trending(
    state: State<'_, AppState>,
    limit: Option<usize>,
    local_only: Option<bool>,
) -> Result<TrendingShelf, String> {
    let limit = limit.unwrap_or(20).clamp(1, 60);
    let setup = with_db(&state, |db| {
        Ok(feature_flags::is_enabled(db, "trending").then(|| (tmdb_key(db), read_cache(db))))
    })?;
    let Some((key, cached)) = setup else {
        return Ok(TrendingShelf {
            source: "off".into(),
            title: String::new(),
            note: None,
            fetched_at: None,
            items: Vec::new(),
        });
    };
    if local_only.unwrap_or(false) {
        return with_db(&state, |db| {
            local_trending(db, &load_works(db, true)?, limit, None)
        });
    }
    let Some(key) = key else {
        return with_db(&state, |db| {
            let works = load_works(db, true)?;
            local_trending(
                db,
                &works,
                limit,
                Some("Add a TMDB key in Settings to see what is trending worldwide.".into()),
            )
        });
    };

    let now = Utc::now();
    let (titles, fetched_at, note) = match cached {
        Some((at, titles)) if cache_is_fresh(at, now) => (Some(titles), Some(at), None),
        stale => match fetch_tmdb_trending(&key).await {
            Ok(titles) => {
                with_db(&state, |db| write_cache(db, &titles, now))?;
                (Some(titles), Some(now), None)
            }
            Err(error) => {
                log::warn!("TMDB trending fetch failed: {error}");
                match stale {
                    Some((at, titles)) => (
                        Some(titles),
                        Some(at),
                        Some(format!(
                            "TMDB unreachable; showing the list from {}",
                            at.format("%Y-%m-%d %H:%M UTC")
                        )),
                    ),
                    None => (
                        None,
                        None,
                        Some(format!(
                            "TMDB unreachable ({error}); showing plays on this PC."
                        )),
                    ),
                }
            }
        },
    };

    with_db(&state, |db| {
        let works = load_works(db, false)?;
        let Some(titles) = titles else {
            return local_trending(db, &collapse_episodes(works), limit, note);
        };
        let mut items = match_trending(&titles, &works);
        if items.is_empty() {
            return local_trending(
                db,
                &collapse_episodes(works),
                limit,
                Some("None of this week's TMDB trending titles are in your library.".into()),
            );
        }
        items.truncate(limit);
        Ok(TrendingShelf {
            source: "tmdb".into(),
            title: "Trending in your library".into(),
            note,
            fetched_at: fetched_at.map(|at| at.to_rfc3339()),
            items,
        })
    })
}

/// Titles added since the active profile's last visit. `since` overrides the
/// stored last visit (the UI passes the value it got at startup so refreshes
/// keep counting from the same point); `acknowledge` records now as the new
/// last visit.
#[tauri::command]
pub fn discovery_new_releases(
    state: State<AppState>,
    since: Option<String>,
    acknowledge: Option<bool>,
) -> Result<NewReleases, String> {
    with_db(&state, |db| {
        let checked_at = Utc::now();
        if !feature_flags::is_enabled(db, "new_releases") {
            return Ok(NewReleases {
                since: None,
                checked_at: checked_at.to_rfc3339(),
                count: 0,
                items: Vec::new(),
            });
        }
        let setting = format!(
            "{NEW_RELEASES_SETTING_PREFIX}{}",
            user_data::active_profile_id(db)
        );
        let since = since
            .filter(|raw| parse_timestamp(raw).is_some())
            .or_else(|| db.get_setting_data(&setting).ok().flatten());
        let items = match since.as_deref().and_then(parse_timestamp) {
            Some(at) => added_since(&load_works(db, false)?, at),
            None => Vec::new(),
        };
        if acknowledge.unwrap_or(false) {
            db.set_setting_data(&setting, &checked_at.to_rfc3339())
                .map_err(|e| e.to_string())?;
            if !items.is_empty() {
                user_data::emit(
                    db,
                    "library.new_releases",
                    &format!("{} new since your last visit", items.len()),
                    json!({ "count": items.len(), "since": since }),
                );
            }
        }
        Ok(NewReleases {
            since,
            checked_at: checked_at.to_rfc3339(),
            count: items.len(),
            items: items.into_iter().take(60).collect(),
        })
    })
}

#[tauri::command]
pub fn discovery_genres(state: State<AppState>) -> Result<Vec<GenreCount>, String> {
    with_db(&state, |db| {
        if !feature_flags::is_enabled(db, "genre_radio") {
            return Ok(Vec::new());
        }
        Ok(genre_counts(&load_works(db, false)?))
    })
}

#[tauri::command]
pub fn discovery_genre_queue(
    state: State<AppState>,
    genre: String,
    limit: Option<usize>,
) -> Result<Vec<UnifiedEntry>, String> {
    with_db(&state, |db| {
        if !feature_flags::is_enabled(db, "genre_radio") {
            return Err("Genre Radio is switched off".into());
        }
        let mut works = works_in_genre(&load_works(db, false)?, &genre);
        works.truncate(limit.unwrap_or(300).clamp(1, 1000));
        user_data::emit(
            db,
            "genre_radio.started",
            &genre,
            json!({ "genre": genre, "titles": works.len() }),
        );
        Ok(works)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library_unify::test_item;
    use chrono::TimeZone;

    fn item(id: i64, title: &str, genre: &str, year: i32, rating: f64) -> MediaItem {
        let mut item = test_item(id, title, &format!("C:/m/{id}.mkv"), "movie");
        item.genre = Some(genre.into());
        item.year = Some(year);
        item.rating = Some(rating);
        item
    }

    fn works(items: Vec<MediaItem>) -> Vec<UnifiedEntry> {
        library_unify::group_items(items)
    }

    fn titles(entries: &[DiscoveryEntry]) -> Vec<String> {
        entries
            .iter()
            .map(|e| e.entry.primary.title.clone())
            .collect()
    }

    #[test]
    fn genres_split_on_common_separators_once_each() {
        assert_eq!(
            split_genres(Some("Action, Drama / Sci-Fi|action ; ")),
            vec!["Action", "Drama", "Sci-Fi"]
        );
        assert!(split_genres(None).is_empty());
    }

    #[test]
    fn timestamps_parse_in_every_stored_format() {
        let expected = Utc.with_ymd_and_hms(2026, 3, 4, 5, 6, 7).unwrap();
        assert_eq!(parse_timestamp("2026-03-04T05:06:07Z"), Some(expected));
        assert_eq!(parse_timestamp("2026-03-04T07:06:07+02:00"), Some(expected));
        assert_eq!(parse_timestamp("2026-03-04 05:06:07"), Some(expected));
        assert_eq!(
            parse_timestamp("2026-03-04"),
            Some(Utc.with_ymd_and_hms(2026, 3, 4, 0, 0, 0).unwrap())
        );
        assert_eq!(parse_timestamp("yesterday"), None);
    }

    #[test]
    fn similarity_weights_genre_then_year_rating_and_type() {
        let a = Features::of(&item(1, "A", "Action, Sci-Fi", 2010, 8.0));
        let same = Features::of(&item(2, "B", "Action, Sci-Fi", 2010, 8.0));
        let half = Features::of(&item(3, "C", "Action, Drama", 2010, 8.0));
        let none = Features::of(&item(4, "D", "Romance", 2010, 8.0));
        assert!((similarity(&a, &same) - 1.0).abs() < 1e-9);
        // Jaccard 1/3: 0.55/3 + 0.2 + 0.15 + 0.1
        assert!((similarity(&a, &half) - (0.55 / 3.0 + 0.45)).abs() < 1e-9);
        assert!((similarity(&a, &none) - 0.45).abs() < 1e-9);
    }

    #[test]
    fn recommendations_follow_what_the_profile_watched() {
        let library = works(vec![
            item(1, "Alien", "Horror, Sci-Fi", 1979, 8.5),
            item(2, "Aliens", "Action, Horror, Sci-Fi", 1986, 8.4),
            item(3, "Event Horizon", "Horror, Sci-Fi", 1997, 6.7),
            item(4, "Notting Hill", "Comedy, Romance", 1999, 7.2),
            item(5, "Prometheus", "Sci-Fi", 2012, 7.0),
        ]);
        let signals = ProfileSignals {
            seeds: vec![Seed {
                media_id: 1,
                weight: 1.0,
                kind: SeedKind::Watched,
            }],
            exclude: [1, 5].into_iter().collect(),
        };
        let ranked = rank_recommendations(&library, &signals, 10);
        assert_eq!(titles(&ranked), vec!["Event Horizon", "Aliens"]);
        assert_eq!(ranked[0].reason, "Because you watched Alien");
    }

    #[test]
    fn recommendations_without_history_fall_back_to_top_rated() {
        let library = works(vec![
            item(1, "Low", "Drama", 2000, 4.0),
            item(2, "High", "Drama", 2000, 9.0),
            item(3, "Mid", "Drama", 2000, 7.0),
        ]);
        let ranked = rank_recommendations(&library, &ProfileSignals::default(), 10);
        assert_eq!(titles(&ranked), vec!["High", "Mid"]);
        assert_eq!(ranked[0].reason, "Top rated in your library");
    }

    #[test]
    fn similar_titles_skip_the_target_and_unrelated_genres() {
        let target = item(1, "Heat", "Crime, Thriller", 1995, 8.3);
        let library = works(vec![
            target.clone(),
            item(2, "Collateral", "Crime, Thriller", 2004, 7.5),
            item(3, "Ronin", "Action, Thriller", 1998, 7.2),
            item(4, "Frozen", "Animation", 2013, 7.4),
        ]);
        let ranked = rank_similar(&library, &target, 5);
        assert_eq!(titles(&ranked), vec!["Collateral", "Ronin"]);
        assert_eq!(ranked[0].reason, "Also Crime, Thriller");
    }

    #[test]
    fn tmdb_trending_parses_movies_and_shows_in_rank_order() {
        let page = json!({ "results": [
            { "id": 11, "media_type": "movie", "title": "Dune", "release_date": "2021-09-15" },
            { "id": 99, "media_type": "person", "name": "Someone" },
            { "id": 22, "media_type": "tv", "name": "Severance", "first_air_date": "2022-02-18" }
        ]});
        let parsed = parse_tmdb_trending(&[page]);
        assert_eq!(parsed.len(), 2);
        assert_eq!(
            parsed[0],
            TrendingTitle {
                tmdb_id: 11,
                media_type: "movie".into(),
                title: "Dune".into(),
                year: Some(2021),
                rank: 1
            }
        );
        assert_eq!(parsed[1].title, "Severance");
        assert_eq!(parsed[1].rank, 2);
    }

    #[test]
    fn trending_matches_by_tmdb_id_or_clean_title_and_year() {
        let mut by_id = item(1, "Some Odd Local Name", "Sci-Fi", 2021, 8.0);
        by_id.tmdb_id = Some("11".into());
        let by_title = item(2, "The.Matrix.1999.1080p.BluRay", "Action", 1999, 8.7);
        let wrong_year = item(3, "Matrix", "Action", 2021, 5.0);
        let mut episode = test_item(4, "Severance S01E02", "C:/tv/sev2.mkv", "episode");
        episode.genre = Some("Drama".into());
        let first_episode = test_item(5, "Severance S01E01", "C:/tv/sev1.mkv", "episode");
        let library = works(vec![by_id, by_title, wrong_year, episode, first_episode]);
        let trending = vec![
            TrendingTitle {
                tmdb_id: 22,
                media_type: "tv".into(),
                title: "Severance".into(),
                year: Some(2022),
                rank: 1,
            },
            TrendingTitle {
                tmdb_id: 11,
                media_type: "movie".into(),
                title: "Dune".into(),
                year: Some(2021),
                rank: 2,
            },
            TrendingTitle {
                tmdb_id: 603,
                media_type: "movie".into(),
                title: "The Matrix".into(),
                year: Some(1999),
                rank: 3,
            },
            TrendingTitle {
                tmdb_id: 7,
                media_type: "movie".into(),
                title: "Not Here".into(),
                year: None,
                rank: 4,
            },
        ];
        let matched = match_trending(&trending, &library);
        assert_eq!(
            titles(&matched),
            vec![
                "Severance S01E01",
                "Some Odd Local Name",
                "The.Matrix.1999.1080p.BluRay"
            ]
        );
        assert_eq!(matched[2].reason, "#3 trending this week");
    }

    #[test]
    fn popularity_counts_recent_plays_only() {
        let now = Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap();
        let library = works(vec![
            item(1, "Often", "Drama", 2000, 7.0),
            item(2, "Once", "Drama", 2000, 7.0),
            item(3, "Long Ago", "Drama", 2000, 7.0),
        ]);
        let events = vec![
            PlayEvent {
                media_id: 1,
                at: now - chrono::Duration::days(1),
            },
            PlayEvent {
                media_id: 1,
                at: now - chrono::Duration::days(3),
            },
            PlayEvent {
                media_id: 2,
                at: now - chrono::Duration::hours(2),
            },
            PlayEvent {
                media_id: 3,
                at: now - chrono::Duration::days(90),
            },
        ];
        let ranked = rank_popular(&library, &events, now, 10);
        assert_eq!(titles(&ranked), vec!["Often", "Once"]);
        assert_eq!(ranked[0].reason, "Played 2 times this month");
        assert_eq!(ranked[1].reason, "Played once this month");
    }

    #[test]
    fn new_releases_list_works_added_after_the_last_visit() {
        let mut old = item(1, "Old", "Drama", 2000, 7.0);
        old.date_added = "2026-01-01T00:00:00Z".into();
        let mut newer = item(2, "Newer", "Drama", 2000, 7.0);
        newer.date_added = "2026-05-02 10:00:00".into();
        let mut newest = item(3, "Newest", "Drama", 2000, 7.0);
        newest.date_added = "2026-06-01T00:00:00+00:00".into();
        let since = parse_timestamp("2026-05-01T00:00:00Z").unwrap();
        let found = added_since(&works(vec![old, newer, newest]), since);
        assert_eq!(titles(&found), vec!["Newest", "Newer"]);
        assert_eq!(found[1].reason, "Added 2026-05-02");
    }

    #[test]
    fn genre_counts_and_queue_ignore_case() {
        let library = works(vec![
            item(1, "A", "Comedy, Drama", 2000, 7.0),
            item(2, "B", "comedy", 2001, 7.0),
            item(3, "C", "Horror", 2002, 7.0),
        ]);
        assert_eq!(
            genre_counts(&library),
            vec![
                GenreCount {
                    name: "Comedy".into(),
                    count: 2
                },
                GenreCount {
                    name: "Drama".into(),
                    count: 1
                },
                GenreCount {
                    name: "Horror".into(),
                    count: 1
                },
            ]
        );
        let queue = works_in_genre(&library, "COMEDY");
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn trending_cache_expires_after_six_hours() {
        let now = Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap();
        assert!(cache_is_fresh(now - chrono::Duration::hours(5), now));
        assert!(!cache_is_fresh(now - chrono::Duration::hours(6), now));
        assert!(!cache_is_fresh(now + chrono::Duration::hours(1), now));
    }

    #[test]
    fn signals_and_cache_round_trip_through_the_database() {
        let path =
            std::env::temp_dir().join(format!("cinavault-discovery-{}.db", uuid::Uuid::new_v4()));
        let db = Database::new(path.to_str().unwrap()).unwrap();
        user_data::ensure_tables(&db).unwrap();
        ensure_tables(&db).unwrap();
        for (title, genre) in [("Alpha", "Drama"), ("Beta", "Drama"), ("Gamma", "Comedy")] {
            db.conn
                .execute(
                    "INSERT INTO media_items (title, file_path, media_type, genre, date_added) VALUES (?1, ?2, 'movie', ?3, ?4)",
                    params![title, format!("C:/m/{title}.mkv"), genre, Utc::now().to_rfc3339()],
                )
                .unwrap();
        }
        user_data::save_progress(&db, 1, 1, 5990.0, 6000.0).unwrap();
        user_data::toggle_watchlist(&db, 1, 3).unwrap();
        let signals = load_signals(&db, 1).unwrap();
        assert!(signals.exclude.contains(&1) && signals.exclude.contains(&3));
        let ranked = rank_recommendations(&load_works(&db, true).unwrap(), &signals, 10);
        assert_eq!(titles(&ranked), vec!["Beta"]);
        assert_eq!(ranked[0].reason, "Because you watched Alpha");
        let events = load_play_events(&db).unwrap();
        assert_eq!(events.iter().filter(|e| e.media_id == 1).count(), 3);

        let list = vec![TrendingTitle {
            tmdb_id: 1,
            media_type: "movie".into(),
            title: "Alpha".into(),
            year: None,
            rank: 1,
        }];
        let at = Utc.with_ymd_and_hms(2026, 10, 1, 0, 0, 0).unwrap();
        write_cache(&db, &list, at).unwrap();
        assert_eq!(read_cache(&db), Some((at, list)));
        drop(db);
        std::fs::remove_file(path).ok();
    }
}
