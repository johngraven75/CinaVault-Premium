use crate::metadata::MetadataProvider;
use crate::{metadata_ext, AppState};
use rusqlite::params;
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::time::{Duration, Instant};
use tauri::State;

const IMPLEMENTED_PROVIDERS: &[&str] = &[
    "tmdb",
    "omdb",
    "tvmaze",
    "musicbrainz",
    "tpdb",
    "stashdb",
    "phoenixadult",
    "pgma",
    "porn_site_nuxt",
];

#[derive(Debug, Clone)]
struct ItemRecord {
    id: i64,
    title: String,
    file_path: String,
    media_type: String,
    overview: Option<String>,
    poster_path: Option<String>,
    year: Option<i32>,
    rating: Option<f64>,
    genre: Option<String>,
    tmdb_id: Option<String>,
    imdb_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct ProviderMatch {
    provider: &'static str,
    title: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    year: Option<i32>,
    rating: Option<f64>,
    genre: Option<String>,
    tmdb_id: Option<String>,
    imdb_id: Option<String>,
    media_type: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct MetadataUpdate {
    title: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    year: Option<i32>,
    rating: Option<f64>,
    genre: Option<String>,
    tmdb_id: Option<String>,
    imdb_id: Option<String>,
    media_type: Option<String>,
}

#[derive(Debug, Serialize)]
struct UpdatedItem {
    id: i64,
    title: String,
    file_path: String,
    media_type: String,
    overview: Option<String>,
    poster_path: Option<String>,
    year: Option<i32>,
    rating: Option<f64>,
    genre: Option<String>,
    tmdb_id: Option<String>,
    imdb_id: Option<String>,
}

fn normalize_provider_key(provider: &str) -> String {
    match provider.trim().to_ascii_lowercase().as_str() {
        "themoviedb" | "themoviedb_images" | "tmdb_images" | "tmdb" => "tmdb".to_string(),
        "theporndb" | "tpdb" => "tpdb".to_string(),
        "open_movie_db" | "openmoviedb" | "omdb" => "omdb".to_string(),
        "pgma-modernized" | "pgma_modernized" | "pgma modernized" | "plex pgma" => {
            "pgma".to_string()
        }
        "irenehub" | "porn-site-nuxt" | "porn_site_nuxt" => "porn_site_nuxt".to_string(),
        other => other.to_string(),
    }
}

fn ensure_implemented(provider: &str) -> Result<String, String> {
    let normalized = normalize_provider_key(provider);
    if IMPLEMENTED_PROVIDERS.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(format!(
            "Metadata provider '{provider}' is not implemented in this build"
        ))
    }
}

fn clean(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("N/A"))
        .map(str::to_string)
}

fn parse_year(value: Option<&str>) -> Option<i32> {
    let value = value?.trim();
    if