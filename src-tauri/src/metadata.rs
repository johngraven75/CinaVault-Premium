// CinaVault Premium - Metadata Fetching Module
// Supports provider lookups, batch metadata search, and correction workflows.
use serde::{Deserialize, Serialize};
use tauri::State;
use crate::AppState;
use rusqlite::params;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MetadataProvider {
    pub name: String,
    pub key: String,
    pub base_url: String,
    pub requires_key: bool,
    pub category: String,
    pub implemented: bool,
}

const IMPLEMENTED_PROVIDERS: &[&str] = &[
    "tmdb", "omdb", "tvmaze", "musicbrainz", "tvdb", "fanart",
    "audiodb", "anilist", "kitsu", "openlibrary", "lastfm",
    "discogs", "trakt", "opensubtitles", "cinemeta",
    "tpdb", "stashdb",
];

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MetadataReplacement {
    pub media_id: i64,
    pub title: Option<String>,
    pub year: Option<i32>,
    pub rating: Option<f64>,
    pub overview: Option<String>,
    pub genre: Option<String>,
    pub tmdb_id: Option<String>,
    pub imdb_id: Option<String>,
    pub verified: Option<bool>,
}

#[derive(Debug, Clone)]
struct NormalizedMetadata {
    title: Option<String>,
    year: Option<i32>,
    rating: Option<f64>,
    overview: Option<String>,
    genre: Option<String>,
    tmdb_id: Option<String>,
    imdb_id: Option<String>,
}

const PROVIDERS: &[(&str, &str, &str, bool, &str)] = &[
    ("TMDb", "tmdb", "https://api.themoviedb.org/3", true, "Movies & TV"),
    ("OMDb", "omdb", "https://www.omdbapi.com", true, "Movies & TV"),
    ("TVDB", "tvdb", "https://api4.thetvdb.com/v4", true, "TV Shows"),
    ("Fanart.tv", "fanart", "https://webservice.fanart.tv/v3", true, "Artwork"),
    ("MusicBrainz", "musicbrainz", "https://musicbrainz.org/ws/2", false, "Music"),
    ("AudioDB", "audiodb", "https://theaudiodb.com/api/v1/json", true, "Music"),
    ("ThePornDB", "tpdb", "https://api.theporndb.net", true, "Adult"),
    ("StashDB", "stashdb", "https://stashdb.org/graphql", true, "Adult"),
    ("PhoenixAdult", "phoenixadult", "https://metadata.phoenixadult.com", false, "Adult"),
    ("IAFD", "iafd", "https://www.iafd.com", false, "Adult"),
    ("AniDB", "anidb", "https://api.anidb.net:9001/httpapi", true, "Anime"),
    ("AniList", "anilist", "https://graphql.anilist.co", false, "Anime"),
    ("MyAnimeList", "mal", "https://api.myanimelist.net/v2", true, "Anime"),
    ("Kitsu", "kitsu", "https://kitsu.io/api/edge", false, "Anime"),
    ("IGDB", "igdb", "https://api.igdb.com/v4", true, "Games"),
    ("OpenLibrary", "openlibrary", "https://openlibrary.org", false, "Books"),
    ("GoodReads", "goodreads", "https://www.goodreads.com", true, "Books"),
    ("Last.fm", "lastfm", "https://ws.audioscrobbler.com/2.0", true, "Music"),
    ("Discogs", "discogs", "https://api.discogs.com", true, "Music"),
    ("Trakt", "trakt", "https://api.trakt.tv", true, "Movies & TV"),
    ("Rotten Tomatoes", "rt", "https://www.rottentomatoes.com", false, "Movies & TV"),
    ("IMDb", "imdb", "https://www.imdb.com", false, "Movies & TV"),
    ("OpenSubtitles", "opensubtitles", "https://api.opensubtitles.com/api/v1", true, "Subtitles"),
    ("Subscene", "subscene", "https://subscene.com", false, "Subtitles"),
    ("CINEMETA", "cinemeta", "https://v3-cinemeta.strem.io", false, "Movies & TV"),
    ("TheMovieDB Images", "tmdb_images", "https://image.tmdb.org/t/p", false, "Artwork"),
    ("TVMaze", "tvmaze", "https://api.tvmaze.com", false, "TV Shows"),
    ("EPG Guide", "epg", "", false, "Live TV"),
    ("Plex Agents", "plex_agents", "", false, "Agents"),
    ("Emby Providers", "emby_providers", "", false, "Agents"),
    ("Jellyfin Providers", "jellyfin_providers", "", false, "Agents"),
];

fn provider_requires_key(provider: &str) -> bool {
    PROVIDERS
        .iter()
        .find(|(_, key, _, _, _)| *key == provider)
        .map(|(_, _, _, requires_key, _)| *requires_key)
        .unwrap_or(false)
}

async fn run_provider_search(provider: &str, query: &str, api_key: Option<String>) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let encoded = percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC);

    match provider {
        "tmdb" => {
            let key = api_key.ok_or("TMDb API key required. Get one at https://www.themoviedb.org/settings/api")?;
            let url = format!("https://api.themoviedb.org/3/search/multi?api_key={}&query={}&page=1", key, encoded);
            let resp = client.get(&url).send().await.map_err(|e| format!("TMDb request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await.map_err(|e| format!("TMDb parse failed: {}", e))?;
            Ok(data)
        }
        "omdb" => {
            let key = api_key.ok_or("OMDb API key required. Get one at https://www.omdbapi.com/apikey.aspx")?;
            let url = format!("https://www.omdbapi.com/?apikey={}&s={}", key, encoded);
            let resp = client.get(&url).send().await.map_err(|e| format!("OMDb request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await.map_err(|e| format!("OMDb parse failed: {}", e))?;
            Ok(data)
        }
        "tvmaze" => {
            let url = format!("https://api.tvmaze.com/search/shows?q={}", encoded);
            let resp = client.get(&url).send().await.map_err(|e| format!("TVMaze request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await.map_err(|e| format!("TVMaze parse failed: {}", e))?;
            Ok(data)
        }
        "musicbrainz" => {
            let url = format!("https://musicbrainz.org/ws/2/recording/?query={}&fmt=json&limit=25", encoded);
            let resp = client.get(&url).header("User-Agent", "CinaVault/1.0 (cinavault@example.com)").send().await.map_err(|e| format!("MusicBrainz request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await.map_err(|e| format!("MusicBrainz parse failed: {}", e))?;
            Ok(data)
        }
        "tvdb" => {
            let key = api_key.ok_or("TVDB API key required. Get one at https://thetvdb.com/api-information")?;
            let token = tvdb_get_token(&key).await?;
            let url = format!("https://api4.thetvdb.com/v4/search?query={}", encoded);
            let resp = client.get(&url).header("Authorization", format!("Bearer {}", token)).send().await.map_err(|e| format!("TVDB request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await.map_err(|e| format!("TVDB parse failed: {}", e))?;
            Ok(data)
        }
        "fanart" => {
            let key = api_key.ok_or("Fanart.tv API key required. Get one at https://fanart.tv/get-an-api-key")?;
            let url = format!("https://webservice.fanart.tv/v3/search?api_key={}&q={}", key, encoded);
            let resp = client.get(&url).send().await.map_err(|e| format!("Fanart request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await.map_err(|e| format!("Fanart parse failed: {}", e))?;
            Ok(data)
        }
        "audiodb" => {
            let key = api_key.ok_or("AudioDB API key required. Get one at https://theaudiodb.com/api_apply.php")?;
            let url = format!("https://theaudiodb.com/api/v1/json/{}/searchalbum.php?s={}", key, encoded);
            let resp = client.get(&url).send().await.map_err(|e| format!("AudioDB request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await.map_err(|e| format!("AudioDB parse failed: {}", e))?;
            Ok(data)
        }
        "anilist" => {
            let query_json = serde_json::json!({
                "query": "query ($search: String) { Page(page: 1, perPage: 10) { media(search: $search, type: ANIME) { id title { romaji english native } startDate { year } averageScore description genres } } }",
                "variables": { "search": query }
            });
            let resp = client.post("https://graphql.anilist.co").json(&query_json).send().await.map_err(|e| format!("AniList request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await.map_err(|e| format!("AniList parse failed: {}", e))?;
            Ok(data)
        }
        "kitsu" => {
            let url = format!("https://kitsu.io/api/edge/anime?filter[text]={}", encoded);
            let resp = client.get(&url).send().await.map_err(|e| format!("Kitsu request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await.map_err(|e| format!("Kitsu parse failed: {}", e))?;
            Ok(data)
        }
        "openlibrary" => {
            let url = format!("https://openlibrary.org/search.json?q={}&limit=10", encoded);
            let resp = client.get(&url).send().await.map_err(|e| format!("OpenLibrary request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await.map_err(|e| format!("OpenLibrary parse failed: {}", e))?;
            Ok(data)
        }
        "lastfm" => {
            let key = api_key.ok_or("Last.fm API key required. Get one at https://www.last.fm/api/accounts")?;
            let url = format!("https://ws.audioscrobbler.com/2.0/?method=album.search&album={}&api_key={}&format=json&limit=10", encoded, key);
            let resp = client.get(&url).send().await.map_err(|e| format!("Last.fm request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await.map_err(|e| format!("Last.fm parse failed: {}", e))?;
            Ok(data)
        }
        "discogs" => {
            let key = api_key.ok_or("Discogs API key required. Get one at https://www.discogs.com/settings/developers")?;
            let url = format!("https://api.discogs.com/database/search?q={}&type=release&per_page=10", encoded);
            let resp = client.get(&url).header("Authorization", format!("Discogs token={}", key)).send().await.map_err(|e| format!("Discogs request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await.map_err(|e| format!("Discogs parse failed: {}", e))?;
            Ok(data)
        }
        "trakt" => {
            let key = api_key.ok_or("Trakt API key required. Get one at https://trakt.tv/oauth/applications")?;
            let url = format!("https://api.trakt.tv/search/movie,show?query={}", encoded);
            let resp = client.get(&url).header("Content-Type", "application/json").header("trakt-api-version", "2").header("trakt-api-key", &key).send().await.map_err(|e| format!("Trakt request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await.map_err(|e| format!("Trakt parse failed: {}", e))?;
            Ok(data)
        }
        "opensubtitles" => {
            let key = api_key.ok_or("OpenSubtitles API key required. Get one at https://www.opensubtitles.com/en/consumers")?;
            let url = format!("https://api.opensubtitles.com/api/v1/subtitles?query={}&languages=en", encoded);
            let resp = client.get(&url).header("Api-Key", &key).send().await.map_err(|e| format!("OpenSubtitles request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await.map_err(|e| format!("OpenSubtitles parse failed: {}", e))?;
            Ok(data)
        }
        "cinemeta" => {
            let url = format!("https://v3-cinemeta.strem.io/catalog/movie/top/search={}.json", encoded);
            let resp = client.get(&url).send().await.map_err(|e| format!("CINEMETA request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await.map_err(|e| format!("CINEMETA parse failed: {}", e))?;
            Ok(data)
        }
        "tpdb" => {
            let key = api_key.ok_or("ThePornDB API key required. Get one at https://theporndb.net/account")?;
            let url = format!("https://api.theporndb.net/v1/scenes?q={}&page=1&per_page=20", encoded);
            let resp = client.get(&url).header("ApiKey", &key).send().await.map_err(|e| format!("ThePornDB request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await.map_err(|e| format!("ThePornDB parse failed: {}", e))?;
            Ok(data)
        }
        "stashdb" => {
            let key = api_key.ok_or("StashDB API key required. Get one at https://stashdb.org/settings")?;
            let query_json = serde_json::json!({
                "query": r#"query SearchScenes($input: SceneSearchQuery!) { searchScenes(input: $input) { scenes { id title details date { year month day } rating studio { name } urls { url } images { id url } } } }"#,
                "variables": { "input": { "query": query, "page": 1, "per_page": 20 } }
            });
            let resp = client.post("https://stashdb.org/graphql")
                .header("Content-Type", "application/json")
                .header("ApiKey", &key)
                .json(&query_json)
                .send().await.map_err(|e| format!("StashDB request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await.map_err(|e| format!("StashDB parse failed: {}", e))?;
            Ok(data)
        }
        _ => Ok(serde_json::json!({
            "provider": provider,
            "query": query,
            "status": "unimplemented",
            "message": format!("Provider '{}' integration is not yet implemented. Check Settings > Plugins for provider status.", provider)
        })),
    }
}

async fn tvdb_get_token(api_key: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client.post("https://api4.thetvdb.com/v4/login")
        .json(&serde_json::json!({ "apiKey": api_key }))
        .send().await.map_err(|e| format!("TVDB login failed: {}", e))?;
    let data: serde_json::Value = resp.json().await.map_err(|e| format!("TVDB login parse failed: {}", e))?;
    data.get("data").and_then(|d| d.get("token")).and_then(|t| t.as_str()).map(|s| s.to_string())
        .ok_or_else(|| "Failed to extract TVDB token from response".to_string())
}

fn normalize_metadata(provider: &str, raw: &serde_json::Value) -> Option<NormalizedMetadata> {
    match provider {
        "tmdb" => {
            let first = raw.get("results")?.as_array()?.first()?;
            let title = first.get("title").or_else(|| first.get("name")).and_then(|v| v.as_str()).map(str::to_string);
            let year = first.get("release_date").or_else(|| first.get("first_air_date")).and_then(|v| v.as_str()).and_then(|d| d.split('-').next()).and_then(|y| y.parse::<i32>().ok());
            let rating = first.get("vote_average").and_then(|v| v.as_f64());
            let overview = first.get("overview").and_then(|v| v.as_str()).map(str::to_string);
            let genre = first.get("genre_ids").and_then(|ids| ids.as_array()).and_then(|arr| arr.first()).and_then(|g| g.as_i64()).map(|id| format!("genre:{}", id));
            let tmdb_id = first.get("id").and_then(|v| v.as_i64()).map(|id| id.to_string());
            Some(NormalizedMetadata { title, year, rating, overview, genre, tmdb_id, imdb_id: None })
        }
        "omdb" => {
            let first = raw.get("Search")?.as_array()?.first()?;
            let title = first.get("Title").and_then(|v| v.as_str()).map(str::to_string);
            let year = first.get("Year").and_then(|v| v.as_str()).and_then(|y| y.split('-').next()).and_then(|y| y.parse::<i32>().ok());
            let imdb_id = first.get("imdbID").and_then(|v| v.as_str()).map(str::to_string);
            Some(NormalizedMetadata { title, year, rating: None, overview: None, genre: None, tmdb_id: None, imdb_id })
        }
        "tvmaze" => {
            let first = raw.as_array()?.first()?;
            let show = first.get("show")?;
            let title = show.get("name").and_then(|v| v.as_str()).map(str::to_string);
            let year = show.get("premiered").and_then(|v| v.as_str()).and_then(|d| d.split('-').next()).and_then(|y| y.parse::<i32>().ok());
            let rating = show.get("rating")?.get("average").and_then(|v| v.as_f64());
            let overview = show.get("summary").and_then(|v| v.as_str()).map(|s| s.replace("<p>", "").replace("</p>", "").replace("<b>", "").replace("</b>", ""));
            let genres = show.get("genres").and_then(|g| g.as_array()).and_then(|arr| arr.first()).and_then(|v| v.as_str());
            let genre = genres.map(|g| format!("genre:{}", g));
            Some(NormalizedMetadata { title, year, rating, overview, genre, tmdb_id: None, imdb_id: None })
        }
        "tvdb" => {
            let data_raw = raw.get("data");
            let results = if let Some(d) = data_raw { d.as_array() } else { raw.get("results")?.as_array() };
            let first = results?.first()?;
            let title = first.get("name").or_else(|| first.get("title")).and_then(|v| v.as_str()).map(str::to_string);
            let year = first.get("year").or_else(|| first.get("first_air_time")).and_then(|v| v.as_str()).and_then(|y| y.split('-').next()).and_then(|y| y.parse::<i32>().ok()).or_else(|| first.get("year").and_then(|v| v.as_i64()).map(|y| y as i32));
            let rating = first.get("score").and_then(|v| v.as_f64()).or_else(|| first.get("siteRating").and_then(|v| v.as_f64()));
            let overview = first.get("overview").and_then(|v| v.as_str()).map(str::to_string);
            let tmdb_id = first.get("tmdbId").and_then(|v| v.as_i64()).map(|id| id.to_string()).or_else(|| first.get("id").and_then(|v| v.as_i64()).map(|id| id.to_string()));
            let imdb_id = first.get("imdbId").and_then(|v| v.as_str()).map(str::to_string);
            Some(NormalizedMetadata { title, year, rating, overview, genre: None, tmdb_id, imdb_id })
        }
        "anilist" => {
            let media = raw.get("data")?.get("Page")?.get("media")?.as_array()?.first()?.clone();
            let titles = media.get("title")?;
            let title = titles.get("romaji").or_else(|| titles.get("english")).and_then(|v| v.as_str()).map(str::to_string);
            let year = media.get("startDate")?.get("year").and_then(|v| v.as_i64()).map(|y| y as i32);
            let rating = media.get("averageScore").and_then(|v| v.as_f64()).map(|s| s / 10.0);
            let overview = media.get("description").and_then(|v| v.as_str()).map(|s| s.replace("<br>", "\n").replace("<i>", "").replace("</i>", ""));
            Some(NormalizedMetadata { title, year, rating, overview, genre: None, tmdb_id: None, imdb_id: None })
        }
        "kitsu" => {
            let first = raw.get("data")?.as_array()?.first()?;
            let attrs = first.get("attributes")?;
            let title = attrs.get("canonicalTitle").or_else(|| attrs.get("titles")?.get("en_jp")).and_then(|v| v.as_str()).map(str::to_string);
            let year = attrs.get("startDate").and_then(|v| v.as_str()).and_then(|d| d.split('-').next()).and_then(|y| y.parse::<i32>().ok());
            let rating = attrs.get("averageRating").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok()).map(|r| r / 10.0);
            let overview = attrs.get("synopsis").and_then(|v| v.as_str()).map(str::to_string);
            Some(NormalizedMetadata { title, year, rating, overview, genre: None, tmdb_id: None, imdb_id: None })
        }
        "openlibrary" => {
            let docs = raw.get("docs")?.as_array()?;
            if docs.is_empty() { return None; }
            let first = &docs[0];
            let title = first.get("title").and_then(|v| v.as_str()).map(str::to_string);
            let year = first.get("first_publish_year").and_then(|v| v.as_i64()).map(|y| y as i32);
            Some(NormalizedMetadata { title, year, rating: None, overview: None, genre: None, tmdb_id: None, imdb_id: None })
        }
        "lastfm" => {
            let album = raw.get("results")?.get("albummatches")?.get("album")?.as_array()?.first()?.clone();
            let title = album.get("name").and_then(|v| v.as_str()).map(str::to_string);
            Some(NormalizedMetadata { title, year: None, rating: None, overview: None, genre: None, tmdb_id: None, imdb_id: None })
        }
        "discogs" => {
            let results = raw.get("results")?.as_array()?;
            if results.is_empty() { return None; }
            let first = &results[0];
            let title = first.get("title").and_then(|v| v.as_str()).map(str::to_string);
            let year = first.get("year").and_then(|v| v.as_i64()).map(|y| y as i32);
            Some(NormalizedMetadata { title, year, rating: None, overview: None, genre: None, tmdb_id: None, imdb_id: None })
        }
        "trakt" => {
            let first = raw.as_array()?.first()?;
            let entry = first.get("movie").or_else(|| first.get("show"))?;
            let title = entry.get("title").and_then(|v| v.as_str()).map(str::to_string);
            let year = entry.get("year").and_then(|v| v.as_i64()).map(|y| y as i32);
            let overview = entry.get("overview").and_then(|v| v.as_str()).map(str::to_string);
            let rating = entry.get("rating").and_then(|v| v.as_f64());
            let ids = entry.get("ids")?;
            let tmdb_id = ids.get("tmdb").and_then(|v| v.as_i64()).map(|id| id.to_string());
            let imdb_id = ids.get("imdb").and_then(|v| v.as_str()).map(str::to_string);
            Some(NormalizedMetadata { title, year, rating, overview, genre: None, tmdb_id, imdb_id })
        }
        "cinemeta" => {
            let metas = raw.get("metas").or_else(|| raw.get("meta")).and_then(|v| v.as_array())?;
            if metas.is_empty() { return None; }
            let first = &metas[0];
            let title = first.get("name").and_then(|v| v.as_str()).map(str::to_string);
            let year = first.get("year").or_else(|| first.get("releaseInfo")).and_then(|v| v.as_str()).and_then(|y| y.parse::<i32>().ok()).or_else(|| first.get("year").and_then(|v| v.as_i64()).map(|y| y as i32));
            let overview = first.get("description").and_then(|v| v.as_str()).map(str::to_string);
            let imdb_id = first.get("imdb_id").and_then(|v| v.as_str()).map(str::to_string).or_else(|| first.get("id").and_then(|v| v.as_str()).map(str::to_string));
            Some(NormalizedMetadata { title, year, rating: None, overview, genre: None, tmdb_id: None, imdb_id })
        }
        "tpdb" => {
            let data = raw.get("data")?.as_array()?;
            if data.is_empty() { return None; }
            let first = &data[0];
            let title = first.get("title").and_then(|v| v.as_str()).map(str::to_string);
            let year = first.get("date").and_then(|v| v.as_str()).and_then(|d| d.split('-').next()).and_then(|y| y.parse::<i32>().ok());
            let rating = first.get("rating").and_then(|v| v.as_f64());
            let overview = first.get("description").and_then(|v| v.as_str()).map(str::to_string);
            Some(NormalizedMetadata { title, year, rating, overview, genre: None, tmdb_id: None, imdb_id: None })
        }
        "stashdb" => {
            let scenes = raw.get("data")?.get("searchScenes")?.get("scenes")?.as_array()?;
            if scenes.is_empty() { return None; }
            let first = &scenes[0];
            let title = first.get("title").and_then(|v| v.as_str()).map(str::to_string);
            let year = first.get("date")?.get("year").and_then(|v| v.as_i64()).map(|y| y as i32);
            let rating = first.get("rating").and_then(|v| v.as_f64());
            let overview = first.get("details").and_then(|v| v.as_str()).map(str::to_string);
            Some(NormalizedMetadata { title, year, rating, overview, genre: None, tmdb_id: None, imdb_id: None })
        }
        _ => None,
    }
}

fn get_api_key_map(state: &State<'_, AppState>) -> Result<HashMap<String, String>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.conn.prepare("SELECT provider, api_key FROM api_keys").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))).map_err(|e| e.to_string())?;
    let mut map = HashMap::new();
    for row in rows {
        let (provider, key) = row.map_err(|e| e.to_string())?;
        map.insert(provider, key);
    }
    Ok(map)
}

fn default_provider_list(state: &State<'_, AppState>) -> Vec<String> {
    let db = match state.db.lock() { Ok(db) => db, Err(_) => return vec!["tmdb".to_string(), "omdb".to_string(), "tvmaze".to_string()] };
    let configured = db.get_setting_data("metadata_selected_providers").ok().flatten();
    if let Some(raw) = configured {
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&raw) {
            let filtered: Vec<String> = parsed.into_iter().filter(|s| !s.trim().is_empty()).collect();
            if !filtered.is_empty() { return filtered; }
        }
    }
    vec!["tmdb".to_string(), "omdb".to_string(), "tvmaze".to_string()]
}

#[tauri::command]
pub fn get_metadata_providers() -> Vec<MetadataProvider> {
    PROVIDERS.iter().map(|(name, key, url, req, cat)| MetadataProvider {
        name: name.to_string(),
        key: key.to_string(),
        base_url: url.to_string(),
        requires_key: *req,
        category: cat.to_string(),
        implemented: IMPLEMENTED_PROVIDERS.contains(key),
    }).collect()
}

#[tauri::command]
pub async fn fetch_metadata(provider: String, query: String, api_key: Option<String>) -> Result<serde_json::Value, String> {
    run_provider_search(&provider, &query, api_key).await
}

#[tauri::command]
pub async fn search_metadata(provider: String, query: String, _media_type: Option<String>, api_key: Option<String>) -> Result<serde_json::Value, String> {
    run_provider_search(&provider, &query, api_key).await
}

#[tauri::command]
pub async fn batch_search_metadata(state: State<'_, AppState>, query: String, providers: Vec<String>, media_type: Option<String>) -> Result<serde_json::Value, String> {
    let selected = if providers.is_empty() { default_provider_list(&state) } else { providers };
    let key_map = get_api_key_map(&state)?;
    let mut results = Vec::new();
    for provider in selected {
        if provider_requires_key(&provider) && !key_map.contains_key(&provider) {
            results.push(serde_json::json!({ "provider": provider, "ok": false, "error": "API key not configured" }));
            continue;
        }
        let api_key = key_map.get(&provider).cloned();
        match run_provider_search(&provider, &query, api_key).await {
            Ok(data) => results.push(serde_json::json!({ "provider": provider, "ok": true, "data": data })),
            Err(error) => results.push(serde_json::json!({ "provider": provider, "ok": false, "error": error })),
        }
    }
    Ok(serde_json::json!({ "query": query, "media_type": media_type, "providers": results }))
}

#[tauri::command]
pub async fn run_metadata_correction(state: State<'_, AppState>, providers: Vec<String>, limit: Option<i64>) -> Result<serde_json::Value, String> {
    let selected = if providers.is_empty() { default_provider_list(&state) } else { providers };
    let key_map = get_api_key_map(&state)?;
    let candidates: Vec<(i64, String)> = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let mut stmt = db.conn.prepare("SELECT id, title FROM media_items WHERE verified = 0 ORDER BY date_added DESC LIMIT ?1").map_err(|e| e.to_string())?;
        let rows = stmt.query_map(params![limit.unwrap_or(25)], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))).map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    };
    let mut updated = 0;
    let mut attempted = 0;
    for (media_id, title) in candidates {
        attempted += 1;
        let mut chosen: Option<NormalizedMetadata> = None;
        for provider in &selected {
            if provider_requires_key(provider) && !key_map.contains_key(provider) { continue; }
            let api_key = key_map.get(provider).cloned();
            let raw = match run_provider_search(provider, &title, api_key).await { Ok(data) => data, Err(_) => continue, };
            if let Some(meta) = normalize_metadata(provider, &raw) { chosen = Some(meta); break; }
        }
        if let Some(meta) = chosen {
            let db = state.db.lock().map_err(|e| e.to_string())?;
            db.conn.execute(
                "UPDATE media_items SET title = COALESCE(?1, title), year = COALESCE(?2, year), rating = COALESCE(?3, rating), overview = COALESCE(?4, overview), genre = COALESCE(?5, genre), tmdb_id = COALESCE(?6, tmdb_id), imdb_id = COALESCE(?7, imdb_id), verified = 1 WHERE id = ?8",
                params![meta.title, meta.year, meta.rating, meta.overview, meta.genre, meta.tmdb_id, meta.imdb_id, media_id],
            ).map_err(|e| e.to_string())?;
            updated += 1;
        }
    }
    Ok(serde_json::json!({ "attempted": attempted, "updated": updated, "providers_used": selected }))
}

#[tauri::command]
pub fn replace_media_metadata(state: State<'_, AppState>, replacement: MetadataReplacement) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn.execute(
        "UPDATE media_items SET title = COALESCE(?1, title), year = COALESCE(?2, year), rating = COALESCE(?3, rating), overview = COALESCE(?4, overview), genre = COALESCE(?5, genre), tmdb_id = COALESCE(?6, tmdb_id), imdb_id = COALESCE(?7, imdb_id), verified = COALESCE(?8, verified) WHERE id = ?9",
        params![replacement.title, replacement.year, replacement.rating, replacement.overview, replacement.genre, replacement.tmdb_id, replacement.imdb_id, replacement.verified, replacement.media_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn replace_media_metadata_batch(state: State<'_, AppState>, replacements: Vec<MetadataReplacement>) -> Result<serde_json::Value, String> {
    let mut updated = 0;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    for replacement in replacements {
        db.conn.execute(
            "UPDATE media_items SET title = COALESCE(?1, title), year = COALESCE(?2, year), rating = COALESCE(?3, rating), overview = COALESCE(?4, overview), genre = COALESCE(?5, genre), tmdb_id = COALESCE(?6, tmdb_id), imdb_id = COALESCE(?7, imdb_id), verified = COALESCE(?8, verified) WHERE id = ?9",
            params![replacement.title, replacement.year, replacement.rating, replacement.overview, replacement.genre, replacement.tmdb_id, replacement.imdb_id, replacement.verified, replacement.media_id],
        ).map_err(|e| e.to_string())?;
        updated += 1;
    }
    Ok(serde_json::json!({ "updated": updated }))
}

#[tauri::command]
pub fn get_provider_status(state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.conn.prepare("SELECT provider, api_key FROM api_keys").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))).map_err(|e| e.to_string())?;
    let mut configured = serde_json::Map::new();
    for row in rows {
        let (provider, _key) = row.map_err(|e| e.to_string())?;
        configured.insert(provider, serde_json::Value::Bool(true));
    }
    Ok(serde_json::json!({ "total_providers": PROVIDERS.len(), "configured": configured }))
}

#[tauri::command]
pub async fn test_api_key(provider: String, api_key: String) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build().map_err(|e| e.to_string())?;
    let result = match provider.as_str() {
        "tmdb" => {
            let resp = client.get(format!("https://api.themoviedb.org/3/configuration?api_key={}", api_key)).send().await.map_err(|e| e.to_string())?;
            resp.status().is_success()
        }
        "omdb" => {
            let resp = client.get(format!("https://www.omdbapi.com/?apikey={}&t=inception", api_key)).send().await.map_err(|e| e.to_string())?;
            let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            data.get("Response").and_then(|v| v.as_str()) == Some("True")
        }
        "tvdb" => {
            tvdb_get_token(&api_key).await.is_ok()
        }
        "fanart" => {
            let resp = client.get(format!("https://webservice.fanart.tv/v3/search?api_key={}&q=test", api_key)).send().await.map_err(|e| e.to_string())?;
            resp.status().is_success()
        }
        "audiodb" => {
            let resp = client.get(format!("https://theaudiodb.com/api/v1/json/{}/searchalbum.php?s=test", api_key)).send().await.map_err(|e| e.to_string())?;
            resp.status().is_success()
        }
        "lastfm" => {
            let resp = client.get(format!("https://ws.audioscrobbler.com/2.0/?method=album.search&album=test&api_key={}&format=json", api_key)).send().await.map_err(|e| e.to_string())?;
            resp.status().is_success()
        }
        "discogs" => {
            let resp = client.get("https://api.discogs.com/database/search?q=test&type=release&per_page=1").header("Authorization", format!("Discogs token={}", api_key)).send().await.map_err(|e| e.to_string())?;
            resp.status().is_success()
        }
        "trakt" => {
            let resp = client.get("https://api.trakt.tv/search/movie?query=test").header("Content-Type", "application/json").header("trakt-api-version", "2").header("trakt-api-key", &api_key).send().await.map_err(|e| e.to_string())?;
            resp.status().is_success()
        }
        "opensubtitles" => {
            let resp = client.get("https://api.opensubtitles.com/api/v1/subtitles?query=test&languages=en").header("Api-Key", &api_key).send().await.map_err(|e| e.to_string())?;
            resp.status().is_success()
        }
        "tpdb" | "stashdb" => {
            let base = if provider == "tpdb" { "https://api.theporndb.net" } else { "https://stashdb.org/graphql" };
            let resp = client.get(base).header("ApiKey", &api_key).send().await;
            match resp { Ok(r) => r.status().is_success(), Err(_) => false }
        }
        "anidb" => {
            let resp = client.get(&format!("https://api.anidb.net:9001/httpapi?request=test&client=cinavault&clientver=1&protover=1&apikey={}", api_key)).send().await;
            match resp { Ok(r) => r.status().is_success(), Err(_) => false }
        }
        "mal" => {
            let resp = client.get("https://api.myanimelist.net/v2/anime?q=test&limit=1").header("X-MAL-CLIENT-ID", &api_key).send().await;
            match resp { Ok(r) => r.status().is_success(), Err(_) => false }
        }
        "igdb" => {
            let resp = client.post("https://api.igdb.com/v4/games").header("Client-ID", "test").header("Authorization", format!("Bearer {}", api_key)).body("search \"test\"; fields name; limit 1;").send().await;
            match resp { Ok(r) => r.status().is_success() || r.status().as_u16() == 400, Err(_) => false }
        }
        "goodreads" => {
            let resp = client.get(format!("https://www.goodreads.com/search/index.xml?key={}&q=test", api_key)).send().await;
            match resp { Ok(r) => r.status().is_success(), Err(_) => false }
        }
        _ => true,
    };
    Ok(serde_json::json!({ "provider": provider, "valid": result }))
}

#[tauri::command]
pub fn set_api_key(state: State<AppState>, provider: String, api_key: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn.execute("INSERT OR REPLACE INTO api_keys (provider, api_key) VALUES (?1, ?2)", params![provider, api_key]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_api_keys(state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.conn.prepare("SELECT provider, api_key FROM api_keys").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))).map_err(|e| e.to_string())?;
    let mut keys = serde_json::Map::new();
    for row in rows {
        let (provider, key) = row.map_err(|e| e.to_string())?;
        let masked = if key.len() > 4 { format!("{}...{}", &key[..2], &key[key.len()-2..]) } else { "****".to_string() };
        keys.insert(provider, serde_json::Value::String(masked));
    }
    Ok(serde_json::Value::Object(keys))
}
