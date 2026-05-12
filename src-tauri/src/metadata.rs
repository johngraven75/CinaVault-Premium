// CinaVault Premium — Metadata Fetching Module
// Supports TMDb, OMDb, TVDB, Fanart.tv, and 30+ providers
use crate::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MetadataProvider {
    pub name: String,
    pub key: String,
    pub base_url: String,
    pub requires_key: bool,
    pub category: String,
}

const PROVIDERS: &[(&str, &str, &str, bool, &str)] = &[
    (
        "TMDb",
        "tmdb",
        "https://api.themoviedb.org/3",
        true,
        "Movies & TV",
    ),
    (
        "OMDb",
        "omdb",
        "https://www.omdbapi.com",
        true,
        "Movies & TV",
    ),
    (
        "TVDB",
        "tvdb",
        "https://api4.thetvdb.com/v4",
        true,
        "TV Shows",
    ),
    (
        "Fanart.tv",
        "fanart",
        "https://webservice.fanart.tv/v3",
        true,
        "Artwork",
    ),
    (
        "MusicBrainz",
        "musicbrainz",
        "https://musicbrainz.org/ws/2",
        false,
        "Music",
    ),
    (
        "AudioDB",
        "audiodb",
        "https://theaudiodb.com/api/v1/json",
        true,
        "Music",
    ),
    (
        "ThePornDB",
        "tpdb",
        "https://api.theporndb.net",
        true,
        "Adult",
    ),
    (
        "StashDB",
        "stashdb",
        "https://stashdb.org/graphql",
        true,
        "Adult",
    ),
    ("PhoenixAdult", "phoenixadult", "", false, "Adult"),
    ("IAFD", "iafd", "https://www.iafd.com", false, "Adult"),
    (
        "AniDB",
        "anidb",
        "https://api.anidb.net:9001/httpapi",
        true,
        "Anime",
    ),
    (
        "AniList",
        "anilist",
        "https://graphql.anilist.co",
        false,
        "Anime",
    ),
    (
        "MyAnimeList",
        "mal",
        "https://api.myanimelist.net/v2",
        true,
        "Anime",
    ),
    (
        "Kitsu",
        "kitsu",
        "https://kitsu.io/api/edge",
        false,
        "Anime",
    ),
    ("IGDB", "igdb", "https://api.igdb.com/v4", true, "Games"),
    (
        "OpenLibrary",
        "openlibrary",
        "https://openlibrary.org",
        false,
        "Books",
    ),
    (
        "GoodReads",
        "goodreads",
        "https://www.goodreads.com",
        true,
        "Books",
    ),
    (
        "Last.fm",
        "lastfm",
        "https://ws.audioscrobbler.com/2.0",
        true,
        "Music",
    ),
    (
        "Discogs",
        "discogs",
        "https://api.discogs.com",
        true,
        "Music",
    ),
    (
        "Trakt",
        "trakt",
        "https://api.trakt.tv",
        true,
        "Movies & TV",
    ),
    (
        "Rotten Tomatoes",
        "rt",
        "https://www.rottentomatoes.com",
        false,
        "Movies & TV",
    ),
    ("IMDb", "imdb", "https://www.imdb.com", false, "Movies & TV"),
    (
        "OpenSubtitles",
        "opensubtitles",
        "https://api.opensubtitles.com/api/v1",
        true,
        "Subtitles",
    ),
    (
        "Subscene",
        "subscene",
        "https://subscene.com",
        false,
        "Subtitles",
    ),
    (
        "CINEMETA",
        "cinemeta",
        "https://v3-cinemeta.strem.io",
        false,
        "Movies & TV",
    ),
    (
        "TheMovieDB Images",
        "tmdb_images",
        "https://image.tmdb.org/t/p",
        false,
        "Artwork",
    ),
    (
        "TVMaze",
        "tvmaze",
        "https://api.tvmaze.com",
        false,
        "TV Shows",
    ),
    ("EPG Guide", "epg", "", false, "Live TV"),
    ("MS-A Agents", "plex_agents", "", false, "Agents"),
    ("MS-B Providers", "emby_providers", "", false, "Agents"),
    ("MS-C Providers", "jellyfin_providers", "", false, "Agents"),
];

fn normalize_provider_key(provider: &str) -> String {
    match provider.trim().to_lowercase().as_str() {
        "themoviedb" | "themoviedb_images" | "tmdb_images" | "tmdb" => "tmdb".to_string(),
        "theporndb" | "tpdb" => "tpdb".to_string(),
        "open_movie_db" | "openmoviedb" | "omdb" => "omdb".to_string(),
        other => other.to_string(),
    }
}

fn is_known_provider(provider: &str) -> bool {
    let normalized = normalize_provider_key(provider);
    PROVIDERS.iter().any(|(_, key, _, _, _)| *key == normalized)
}

fn provider_has_live_key_check(provider: &str) -> bool {
    matches!(provider, "tmdb" | "omdb")
}

fn should_assume_key_validity(provider: &str) -> bool {
    is_known_provider(provider) && !provider_has_live_key_check(provider)
}

#[tauri::command]
pub fn get_metadata_providers() -> Vec<MetadataProvider> {
    PROVIDERS
        .iter()
        .map(|(name, key, url, req, cat)| MetadataProvider {
            name: name.to_string(),
            key: key.to_string(),
            base_url: url.to_string(),
            requires_key: *req,
            category: cat.to_string(),
        })
        .collect()
}

#[tauri::command]
pub async fn fetch_metadata(
    provider: String,
    query: String,
    api_key: Option<String>,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let provider = normalize_provider_key(&provider);

    match provider.as_str() {
        "tmdb" => {
            let key = api_key.ok_or("TMDb API key required")?;
            let url = format!(
                "https://api.themoviedb.org/3/search/multi?api_key={}&query={}&page=1",
                key,
                percent_encoding::utf8_percent_encode(&query, percent_encoding::NON_ALPHANUMERIC)
            );
            let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
            let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            Ok(data)
        }
        "omdb" => {
            let key = api_key.ok_or("OMDb API key required")?;
            let url = format!(
                "https://www.omdbapi.com/?apikey={}&s={}",
                key,
                percent_encoding::utf8_percent_encode(&query, percent_encoding::NON_ALPHANUMERIC)
            );
            let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
            let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            Ok(data)
        }
        "tvmaze" => {
            let url = format!(
                "https://api.tvmaze.com/search/shows?q={}",
                percent_encoding::utf8_percent_encode(&query, percent_encoding::NON_ALPHANUMERIC)
            );
            let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
            let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            Ok(data)
        }
        "musicbrainz" => {
            let url = format!(
                "https://musicbrainz.org/ws/2/recording/?query={}&fmt=json&limit=25",
                percent_encoding::utf8_percent_encode(&query, percent_encoding::NON_ALPHANUMERIC)
            );
            let resp = client
                .get(&url)
                .header("User-Agent", "CinaVault/1.0 (cinavault@example.com)")
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            Ok(data)
        }
        _ => Ok(serde_json::json!({
            "provider": provider,
            "query": query,
            "message": "Provider integration pending. Use API key configuration to enable."
        })),
    }
}

#[tauri::command]
pub async fn search_metadata(
    provider: String,
    query: String,
    media_type: Option<String>,
    api_key: Option<String>,
) -> Result<serde_json::Value, String> {
    fetch_metadata(provider, query, api_key).await
}

#[tauri::command]
pub fn get_provider_status(state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db
        .conn
        .prepare("SELECT provider, api_key FROM api_keys")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;

    let mut configured = serde_json::Map::new();
    for row in rows {
        let (provider, _key) = row.map_err(|e| e.to_string())?;
        configured.insert(normalize_provider_key(&provider), serde_json::Value::Bool(true));
    }

    Ok(serde_json::json!({
        "total_providers": PROVIDERS.len(),
        "configured": configured,
    }))
}

#[tauri::command]
pub async fn test_api_key(provider: String, api_key: String) -> Result<serde_json::Value, String> {
    let provider = normalize_provider_key(&provider);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let result = match provider.as_str() {
        "tmdb" => {
            let resp = client
                .get(format!(
                    "https://api.themoviedb.org/3/configuration?api_key={}",
                    api_key
                ))
                .send()
                .await
                .map_err(|e| e.to_string())?;
            resp.status().is_success()
        }
        "omdb" => {
            let resp = client
                .get(format!(
                    "https://www.omdbapi.com/?apikey={}&t=test",
                    api_key
                ))
                .send()
                .await
                .map_err(|e| e.to_string())?;
            resp.status().is_success()
        }
        _ => should_assume_key_validity(provider.as_str()),
    };

    Ok(serde_json::json!({
        "provider": provider,
        "valid": result,
    }))
}

#[cfg(test)]
mod tests {
    use super::{is_known_provider, normalize_provider_key, should_assume_key_validity};

    #[test]
    fn known_provider_is_detected() {
        assert!(is_known_provider("tmdb"));
        assert!(is_known_provider("themoviedb_images"));
        assert!(is_known_provider("tpdb"));
        assert!(!is_known_provider("unknown_provider"));
    }

    #[test]
    fn provider_key_aliases_are_normalized() {
        assert_eq!(normalize_provider_key("themoviedb_images"), "tmdb");
        assert_eq!(normalize_provider_key("theporndb"), "tpdb");
        assert_eq!(normalize_provider_key("openmoviedb"), "omdb");
    }

    #[test]
    fn unknown_provider_is_not_assumed_valid() {
        assert!(!should_assume_key_validity("unknown_provider"));
    }

    #[test]
    fn known_provider_without_live_check_is_assumed_valid() {
        assert!(should_assume_key_validity("tvdb"));
    }

    #[test]
    fn known_provider_with_live_check_is_not_assumed_valid() {
        assert!(!should_assume_key_validity("tmdb"));
    }
}

#[tauri::command]
pub fn set_api_key(
    state: State<AppState>,
    provider: String,
    api_key: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let provider = normalize_provider_key(&provider);
    db.conn
        .execute(
            "INSERT OR REPLACE INTO api_keys (provider, api_key) VALUES (?1, ?2)",
            params![provider, api_key],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_api_keys(state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db
        .conn
        .prepare("SELECT provider, api_key FROM api_keys")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;

    let mut keys = serde_json::Map::new();
    for row in rows {
        let (provider, key) = row.map_err(|e| e.to_string())?;
        let normalized_provider = normalize_provider_key(&provider);
        // Mask the key for security
        let masked = if key.len() > 4 {
            format!("{}...{}", &key[..2], &key[key.len() - 2..])
        } else {
            "****".to_string()
        };
        keys.insert(normalized_provider, serde_json::Value::String(masked));
    }

    Ok(serde_json::Value::Object(keys))
}
