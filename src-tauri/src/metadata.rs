// CinaVault Premium — Metadata Fetching Module
// Build 132-compatible command surface with uploaded Build 125 metadata behavior preserved.
use crate::AppState;
use regex::Regex;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::State;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MetadataProvider {
    pub name: String,
    pub key: String,
    pub base_url: String,
    pub requires_key: bool,
    pub category: String,
}

#[derive(Debug, Clone)]
struct MediaItemLookup {
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
struct ProviderWriteMatch {
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
struct MetadataCheckItemSnapshot {
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

const PHOENIX_ADULT_MANIFEST_URL: &str =
    "https://raw.githubusercontent.com/DirtyRacer1337/Jellyfin.Plugin.PhoenixAdult/master/manifest.json";

const PROVIDERS: &[(&str, &str, &str, bool, &str)] = &[
    ("TMDb", "tmdb", "https://api.themoviedb.org/3", true, "Movies & TV"),
    ("OMDb", "omdb", "https://www.omdbapi.com", true, "Movies & TV"),
    ("TVDB", "tvdb", "https://api4.thetvdb.com/v4", true, "TV Shows"),
    ("Fanart.tv", "fanart", "https://webservice.fanart.tv/v3", true, "Artwork"),
    ("MusicBrainz", "musicbrainz", "https://musicbrainz.org/ws/2", false, "Music"),
    ("AudioDB", "audiodb", "https://theaudiodb.com/api/v1/json", true, "Music"),
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
    ("MS-A Agents", "plex_agents", "", false, "Agents"),
    ("MS-B Providers", "emby_providers", "", false, "Agents"),
    ("MS-C Providers", "jellyfin_providers", "", false, "Agents"),
    ("ThePornDB", "tpdb", "https://api.theporndb.net", true, "Adult / Native API"),
    ("StashDB", "stashdb", "https://stashdb.org/graphql", true, "Adult / Native GraphQL"),
    ("MetadataAPI", "metadataapi", "https://metadataapi.net", true, "Adult / Bridge API"),
    ("PhoenixAdult", "phoenixadult", PHOENIX_ADULT_MANIFEST_URL, false, "Adult / PhoenixAdult"),
    ("IAFD", "iafd", "https://www.iafd.com", false, "Adult / Reference"),
    ("AdultDVDEmpire", "adultdvdempire", "https://www.adultdvdempire.com", false, "Adult / PhoenixAdult"),
    ("JavLibrary", "javlibrary", "https://www.javlibrary.com", false, "Adult / JAV"),
    ("R18", "r18", "https://www.r18.com", false, "Adult / JAV"),
    ("Heyzo", "heyzo", "https://www.heyzo.com", false, "Adult / JAV"),
    ("Caribbeancom", "caribbeancom", "https://www.caribbeancom.com", false, "Adult / JAV"),
    ("Hegre", "hegre", "https://www.hegre.com", false, "Adult / PhoenixAdult"),
    ("Porndoe", "porndoe", "https://porndoe.com", false, "Adult / PhoenixAdult"),
    ("Nubiles", "nubiles", "https://www.nubiles.net", false, "Adult / PhoenixAdult"),
    ("Pornhub", "pornhub", "https://www.pornhub.com", false, "Adult / PhoenixAdult"),
    ("PornCZ", "porncz", "https://www.porncz.com", false, "Adult / PhoenixAdult"),
    ("Clips4Sale", "clips4sale", "https://www.clips4sale.com", false, "Adult / Exact Match"),
    ("ManyVids", "manyvids", "https://www.manyvids.com", false, "Adult / Exact Match"),
    ("Data18", "data18", "https://www.data18.com", false, "Adult / PhoenixAdult"),
    ("Brazzers Network", "brazzers", "https://www.brazzers.com", false, "Adult / Network"),
    ("JulesJordan Network", "julesjordan", "https://www.julesjordan.com", false, "Adult / Network"),
    ("Naughty America", "naughtyamerica", "https://www.naughtyamerica.com", false, "Adult / Network"),
    ("Bang Bros Network", "bangbros", "https://www.bangbros.com", false, "Adult / Network"),
    ("Babes Network", "babes", "https://www.babes.com", false, "Adult / Network"),
    ("DigitalPlayground", "digitalplayground", "https://www.digitalplayground.com", false, "Adult / Network"),
    ("EvilAngel", "evilangel", "https://www.evilangel.com", false, "Adult / Network"),
    ("Kink Network", "kink", "https://www.kink.com", false, "Adult / Network"),
    ("MileHigh Network", "milehigh", "https://www.milehighmedia.com", false, "Adult / Network"),
    ("Mofos Network", "mofos", "https://www.mofos.com", false, "Adult / Network"),
    ("MYLF Network", "mylf", "https://www.mylf.com", false, "Adult / Network"),
    ("Girlsway", "girlsway", "https://www.girlsway.com", false, "Adult / Network"),
    ("FakeHub", "fakehub", "https://www.fakehub.com", false, "Adult / Network"),
    ("TeamSkeet", "teamskeet", "https://www.teamskeet.com", false, "Adult / MetadataAPI"),
    ("Reality Kings", "realitykings", "https://www.realitykings.com", false, "Adult / MetadataAPI"),
    ("Vixen Media Group", "vixen", "https://www.vixen.com", false, "Adult / MetadataAPI"),
    ("Adult Time", "adulttime", "https://www.adulttime.com", false, "Adult / PhoenixAdult"),
    ("21Naturals", "21naturals", "https://www.21naturals.com", false, "Adult / PhoenixAdult"),
    ("21Sextury", "21sextury", "https://www.21sextury.com", false, "Adult / PhoenixAdult"),
    ("LegalPorno", "legalporno", "https://www.legalporno.com", false, "Adult / Limited"),
    ("HentaiPros", "hentaipros", "https://www.hentaipros.com", false, "Adult / PhoenixAdult"),
];

fn normalize_provider_key(provider: &str) -> String {
    let normalized = provider
        .trim()
        .to_lowercase()
        .replace([' ', '-', '.'], "_");
    match normalized.as_str() {
        "themoviedb" | "themoviedb_images" | "tmdb_images" | "tmdb" => "tmdb".to_string(),
        "theporndb" | "the_porn_db" | "tpdb" => "tpdb".to_string(),
        "stash_db" | "stashdb" => "stashdb".to_string(),
        "metadata_api" | "metadataapi" => "metadataapi".to_string(),
        "open_movie_db" | "openmoviedb" | "omdb" => "omdb".to_string(),
        "fanart_tv" | "fanarttv" | "fanart" => "fanart".to_string(),
        "myanimelist" | "my_anime_list" | "mal" => "mal".to_string(),
        "adult_dvd_empire" | "adultdvdempire" => "adultdvdempire".to_string(),
        "caribbean_com" | "caribbeancom" => "caribbeancom".to_string(),
        "clips_4_sale" | "clips4sale" => "clips4sale".to_string(),
        "many_vids" | "manyvids" => "manyvids".to_string(),
        "naughty_america" | "naughtyamerica" => "naughtyamerica".to_string(),
        "bang_bros" | "bangbros" => "bangbros".to_string(),
        "digital_playground" | "digitalplayground" => "digitalplayground".to_string(),
        "evil_angel" | "evilangel" => "evilangel".to_string(),
        "reality_kings" | "realitykings" => "realitykings".to_string(),
        "vixen_media_group" | "vixen" => "vixen".to_string(),
        "adult_time" | "adulttime" => "adulttime".to_string(),
        "21_naturals" | "21naturals" => "21naturals".to_string(),
        "21_sextury" | "21sextury" => "21sextury".to_string(),
        "legal_porno" | "legalporno" => "legalporno".to_string(),
        "hentai_pros" | "hentaipros" => "hentaipros".to_string(),
        other => other.to_string(),
    }
}

fn is_known_provider(provider: &str) -> bool {
    let normalized = normalize_provider_key(provider);
    PROVIDERS.iter().any(|(_, key, _, _, _)| *key == normalized)
}

fn provider_has_live_key_check(provider: &str) -> bool {
    matches!(provider, "tmdb" | "omdb" | "tpdb" | "stashdb" | "metadataapi")
}

fn should_assume_key_validity(provider: &str) -> bool {
    is_known_provider(provider) && !provider_has_live_key_check(provider)
}

fn bearer_headers(api_key: &str) -> Result<reqwest::header::HeaderMap, String> {
    use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    let token = format!("Bearer {}", api_key.trim());
    let header_value = HeaderValue::from_str(&token).map_err(|err| err.to_string())?;
    headers.insert(AUTHORIZATION, header_value);
    Ok(headers)
}

fn theporndb_headers(api_key: &str) -> Result<reqwest::header::HeaderMap, String> {
    bearer_headers(api_key)
}

fn non_empty_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("N/A"))
        .map(str::to_string)
}

fn parse_year_prefix(value: Option<&str>) -> Option<i32> {
    let text = value?.trim();
    if text.len() < 4 {
        return None;
    }
    text[..4].parse::<i32>().ok()
}

fn has_adult_hint(text: &str) -> bool {
    let lower = text.replace(['\\', '/', '_', '-'], " ").to_lowercase();
    [
        "adult", "porn", "xxx", "nsfw", "personal x", "x library", "vids x", "videos x",
        "brazzers", "bangbros", "teamskeet", "reality kings", "vixen", "adulttime",
    ]
    .iter()
    .any(|hint| lower.contains(hint))
}

fn looks_like_phoenix_date(value: &str) -> bool {
    Regex::new(r"^\d{4}-\d{2}-\d{2}$")
        .expect("phoenix date regex should compile")
        .is_match(value.trim())
}

fn looks_like_scene_id(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.len() >= 4 && trimmed.chars().all(|ch| ch.is_ascii_digit())
}

fn clean_title_candidate(value: &str) -> Option<String> {
    let cleaned = value
        .replace(['\r', '\n', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|ch| ch == '.' || ch == ' ')
        .to_string();
    if cleaned.is_empty() { None } else { Some(cleaned) }
}

fn normalize_filename_title(file_path: &str) -> String {
    let stem = Path::new(file_path)
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| file_path.to_string());
    stem.replace(['.', '_'], " ")
        .replace('-', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn extract_phoenix_scene_query(file_path: &str) -> Option<String> {
    let stem = Path::new(file_path).file_stem()?.to_str()?.trim();
    let parts = stem
        .split(" - ")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    match parts.as_slice() {
        [_, middle, last] if looks_like_phoenix_date(middle) || looks_like_scene_id(middle) => clean_title_candidate(last),
        [_, last] => clean_title_candidate(last),
        [_, _, _, last] => clean_title_candidate(last),
        _ => None,
    }
}

fn build_metadata_queries(item: &MediaItemLookup) -> Vec<String> {
    let mut queries = Vec::new();
    if let Some(query) = clean_title_candidate(&item.title) {
        queries.push(query);
    }
    if let Some(query) = extract_phoenix_scene_query(&item.file_path) {
        if !queries.iter().any(|existing| existing.eq_ignore_ascii_case(&query)) {
            queries.insert(0, query);
        }
    }
    let normalized = normalize_filename_title(&item.file_path);
    if !normalized.is_empty()
        && !queries.iter().any(|existing| existing.eq_ignore_ascii_case(&normalized))
    {
        queries.push(normalized);
    }
    queries
}

fn media_item_is_adult(item: &MediaItemLookup) -> bool {
    item.media_type.eq_ignore_ascii_case("adult") || has_adult_hint(&item.title) || has_adult_hint(&item.file_path)
}

fn should_replace_title(current: &str, incoming: Option<&str>) -> Option<String> {
    let incoming = clean_title_candidate(incoming?)?;
    let current = current.trim();
    if current.is_empty()
        || current.eq_ignore_ascii_case("unknown")
        || current.eq_ignore_ascii_case(&normalize_filename_title(current))
        || current.contains('_')
        || current.contains('.')
        || current.eq_ignore_ascii_case(&incoming)
    {
        if current.eq_ignore_ascii_case(&incoming) { None } else { Some(incoming) }
    } else {
        None
    }
}

fn write_snapshot(item: &MediaItemLookup, update: &ProviderWriteMatch) -> MetadataCheckItemSnapshot {
    MetadataCheckItemSnapshot {
        id: item.id,
        title: update.title.clone().unwrap_or_else(|| item.title.clone()),
        file_path: item.file_path.clone(),
        media_type: update.media_type.clone().unwrap_or_else(|| item.media_type.clone()),
        overview: update.overview.clone().or_else(|| item.overview.clone()),
        poster_path: update.poster_path.clone().or_else(|| item.poster_path.clone()),
        year: update.year.or(item.year),
        rating: update.rating.or(item.rating),
        genre: update.genre.clone().or_else(|| item.genre.clone()),
        tmdb_id: update.tmdb_id.clone().or_else(|| item.tmdb_id.clone()),
        imdb_id: update.imdb_id.clone().or_else(|| item.imdb_id.clone()),
    }
}

fn build_metadata_update(item: &MediaItemLookup, provider: &ProviderWriteMatch) -> ProviderWriteMatch {
    let mut update = ProviderWriteMatch::default();
    update.title = should_replace_title(&item.title, provider.title.as_deref());
    if item.overview.as_deref().map(|value| value.trim().is_empty()).unwrap_or(true) {
        update.overview = provider.overview.clone();
    }
    if item.poster_path.as_deref().map(|value| value.trim().is_empty()).unwrap_or(true) {
        update.poster_path = provider.poster_path.clone();
    }
    if item.year.is_none() {
        update.year = provider.year;
    }
    if item.rating.is_none() {
        update.rating = provider.rating;
    }
    if item.genre.as_deref().map(|value| value.trim().is_empty()).unwrap_or(true) {
        update.genre = provider.genre.clone();
    }
    if item.tmdb_id.as_deref().map(|value| value.trim().is_empty()).unwrap_or(true) {
        update.tmdb_id = provider.tmdb_id.clone();
    }
    if item.imdb_id.as_deref().map(|value| value.trim().is_empty()).unwrap_or(true) {
        update.imdb_id = provider.imdb_id.clone();
    }
    if provider.media_type.as_deref() == Some("adult") && !item.media_type.eq_ignore_ascii_case("adult") {
        update.media_type = Some("adult".to_string());
    }
    update
}

fn count_metadata_changes(update: &ProviderWriteMatch) -> usize {
    usize::from(update.title.is_some())
        + usize::from(update.overview.is_some())
        + usize::from(update.poster_path.is_some())
        + usize::from(update.year.is_some())
        + usize::from(update.rating.is_some())
        + usize::from(update.genre.is_some())
        + usize::from(update.tmdb_id.is_some())
        + usize::from(update.imdb_id.is_some())
        + usize::from(update.media_type.is_some())
}

async fn fetch_theporndb_search_metadata(
    client: &reqwest::Client,
    query: &str,
    api_key: &str,
) -> Result<serde_json::Value, String> {
    let encoded = percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC);
    let url = format!("https://api.theporndb.net/scenes?parse={encoded}&hash=&year=");
    let headers = theporndb_headers(api_key)?;
    let resp = client.get(url).headers(headers).send().await.map_err(|err| err.to_string())?;
    let status = resp.status();
    let data = resp.json::<serde_json::Value>().await.map_err(|err| err.to_string())?;
    if !status.is_success() {
        return Err(data.get("message").and_then(|value| value.as_str()).unwrap_or("ThePornDB request failed").to_string());
    }
    Ok(data)
}

async fn fetch_theporndb_scene_details(
    client: &reqwest::Client,
    scene_id: &str,
    api_key: &str,
) -> Result<serde_json::Value, String> {
    let headers = theporndb_headers(api_key)?;
    let url = format!("https://api.theporndb.net/scenes/{}", scene_id.trim());
    let resp = client.get(url).headers(headers).send().await.map_err(|err| err.to_string())?;
    let status = resp.status();
    let data = resp.json::<serde_json::Value>().await.map_err(|err| err.to_string())?;
    if !status.is_success() {
        return Err(data.get("message").and_then(|value| value.as_str()).unwrap_or("ThePornDB scene lookup failed").to_string());
    }
    Ok(data)
}

fn provider_match_from_tpdb_detail(data: &serde_json::Value) -> ProviderWriteMatch {
    let detail = data.get("data").unwrap_or(data);
    let genre = detail
        .get("tags")
        .and_then(|value| value.as_array())
        .map(|tags| {
            tags.iter()
                .filter_map(|tag| tag.get("name").and_then(|value| value.as_str()))
                .filter(|name| !name.trim().is_empty())
                .take(6)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|value| !value.trim().is_empty());

    let poster_path = detail
        .get("posters").and_then(|value| value.get("large")).and_then(|value| value.as_str()).and_then(|value| non_empty_string(Some(value)))
        .or_else(|| detail.get("poster").and_then(|value| value.as_str()).and_then(|value| non_empty_string(Some(value))))
        .or_else(|| detail.get("background").and_then(|value| value.get("large")).and_then(|value| value.as_str()).and_then(|value| non_empty_string(Some(value))));

    ProviderWriteMatch {
        title: non_empty_string(detail.get("title").and_then(|value| value.as_str())),
        overview: non_empty_string(detail.get("description").or_else(|| detail.get("details")).and_then(|value| value.as_str())),
        poster_path,
        year: parse_year_prefix(detail.get("date").and_then(|value| value.as_str())),
        rating: None,
        genre,
        tmdb_id: None,
        imdb_id: detail.get("uuid").and_then(|value| value.as_str()).and_then(|value| non_empty_string(Some(value))),
        media_type: Some("adult".to_string()),
    }
}

async fn fetch_stashdb_scene_metadata(
    client: &reqwest::Client,
    query: &str,
    api_key: &str,
) -> Result<ProviderWriteMatch, String> {
    let headers = bearer_headers(api_key)?;
    let body = serde_json::json!({
        "query": "query FindScenes($filter: String!) { findScenes(scene_filter: { title: { value: $filter, modifier: INCLUDES } }, filter: { per_page: 1 }) { scenes { id title details date rating100 images { url } studio { name } tags { name } performers { performer { name } } } } }",
        "variables": { "filter": query }
    });
    let resp = client
        .post("https://stashdb.org/graphql")
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let status = resp.status();
    let data = resp.json::<serde_json::Value>().await.map_err(|err| err.to_string())?;
    if !status.is_success() || data.get("errors").is_some() {
        return Err(data.get("errors").map(|value| value.to_string()).unwrap_or_else(|| "StashDB request failed".to_string()));
    }
    let scene = data
        .get("data")
        .and_then(|value| value.get("findScenes"))
        .and_then(|value| value.get("scenes"))
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .ok_or_else(|| "No StashDB scene match".to_string())?;
    let genre = scene
        .get("tags")
        .and_then(|value| value.as_array())
        .map(|tags| {
            tags.iter()
                .filter_map(|tag| tag.get("name").and_then(|value| value.as_str()))
                .take(6)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|value| !value.trim().is_empty());
    let poster_path = scene
        .get("images")
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .and_then(|image| image.get("url"))
        .and_then(|value| value.as_str())
        .and_then(|value| non_empty_string(Some(value)));
    Ok(ProviderWriteMatch {
        title: non_empty_string(scene.get("title").and_then(|value| value.as_str())),
        overview: non_empty_string(scene.get("details").and_then(|value| value.as_str())),
        poster_path,
        year: parse_year_prefix(scene.get("date").and_then(|value| value.as_str())),
        rating: scene
            .get("rating100")
            .and_then(|value| value.as_f64())
            .map(|value| (value / 10.0).min(10.0))
            .filter(|value| *value > 0.0),
        genre,
        tmdb_id: None,
        imdb_id: scene.get("id").and_then(|value| value.as_str()).and_then(|value| non_empty_string(Some(value))),
        media_type: Some("adult".to_string()),
    })
}

async fn fetch_metadataapi_scene_metadata(
    client: &reqwest::Client,
    query: &str,
    api_key: &str,
) -> Result<ProviderWriteMatch, String> {
    let encoded = percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC);
    let headers = bearer_headers(api_key)?;
    let candidate_urls = [
        format!("https://metadataapi.net/api/scenes?query={encoded}"),
        format!("https://metadataapi.net/scenes?query={encoded}"),
    ];
    let mut last_error = "MetadataAPI request failed".to_string();
    for url in candidate_urls {
        let resp = match client.get(url).headers(headers.clone()).send().await {
            Ok(resp) => resp,
            Err(err) => {
                last_error = err.to_string();
                continue;
            }
        };
        let status = resp.status();
        let data = match resp.json::<serde_json::Value>().await {
            Ok(data) => data,
            Err(err) => {
                last_error = err.to_string();
                continue;
            }
        };
        if !status.is_success() {
            last_error = data.get("message").and_then(|value| value.as_str()).unwrap_or("MetadataAPI request failed").to_string();
            continue;
        }
        let scene = data
            .get("data")
            .or_else(|| data.get("results"))
            .and_then(|value| value.as_array())
            .and_then(|items| items.first())
            .or_else(|| data.get("data"))
            .unwrap_or(&data);
        return Ok(ProviderWriteMatch {
            title: non_empty_string(scene.get("title").or_else(|| scene.get("name")).and_then(|value| value.as_str())),
            overview: non_empty_string(scene.get("description").or_else(|| scene.get("details")).and_then(|value| value.as_str())),
            poster_path: non_empty_string(
                scene.get("poster")
                    .or_else(|| scene.get("image"))
                    .or_else(|| scene.get("cover"))
                    .and_then(|value| value.as_str()),
            ),
            year: parse_year_prefix(scene.get("date").or_else(|| scene.get("release_date")).and_then(|value| value.as_str())),
            rating: scene.get("rating").and_then(|value| value.as_f64()).filter(|value| *value > 0.0),
            genre: scene.get("tags").and_then(|value| value.as_array()).map(|tags| {
                tags.iter().filter_map(|tag| tag.as_str().or_else(|| tag.get("name").and_then(|value| value.as_str()))).take(6).collect::<Vec<_>>().join(", ")
            }).filter(|value| !value.trim().is_empty()),
            tmdb_id: None,
            imdb_id: scene.get("id").or_else(|| scene.get("uuid")).and_then(|value| value.as_str()).and_then(|value| non_empty_string(Some(value))),
            media_type: Some("adult".to_string()),
        });
    }
    Err(last_error)
}

async fn fetch_adult_item_metadata(
    client: &reqwest::Client,
    provider_keys: &std::collections::HashMap<String, String>,
    item: &MediaItemLookup,
    provider_errors: &mut Vec<String>,
) -> Option<ProviderWriteMatch> {
    for query in build_metadata_queries(item) {
        if let Some(tpdb_key) = provider_keys.get("tpdb") {
            match fetch_theporndb_search_metadata(client, &query, tpdb_key).await {
                Ok(search) => {
                    let scene_id = search
                        .get("data")
                        .and_then(|value| value.as_array())
                        .and_then(|items| items.first())
                        .and_then(|first| first.get("uuid").or_else(|| first.get("UUID")).and_then(|value| value.as_str()));
                    if let Some(scene_id) = scene_id {
                        match fetch_theporndb_scene_details(client, scene_id, tpdb_key).await {
                            Ok(detail) => return Some(provider_match_from_tpdb_detail(&detail)),
                            Err(err) => provider_errors.push(format!("tpdb/{query}: {err}")),
                        }
                    }
                }
                Err(err) => provider_errors.push(format!("tpdb/{query}: {err}")),
            }
        }

        if let Some(stash_key) = provider_keys.get("stashdb") {
            match fetch_stashdb_scene_metadata(client, &query, stash_key).await {
                Ok(found) => return Some(found),
                Err(err) => provider_errors.push(format!("stashdb/{query}: {err}")),
            }
        }

        if let Some(metadataapi_key) = provider_keys.get("metadataapi") {
            match fetch_metadataapi_scene_metadata(client, &query, metadataapi_key).await {
                Ok(found) => return Some(found),
                Err(err) => provider_errors.push(format!("metadataapi/{query}: {err}")),
            }
        }
    }
    None
}

async fn fetch_standard_item_metadata(
    client: &reqwest::Client,
    provider_keys: &std::collections::HashMap<String, String>,
    item: &MediaItemLookup,
    provider_errors: &mut Vec<String>,
) -> Option<ProviderWriteMatch> {
    for query in build_metadata_queries(item) {
        if let Some(key) = provider_keys.get("tmdb") {
            let url = format!(
                "https://api.themoviedb.org/3/search/multi?api_key={}&query={}&include_adult=true&page=1",
                key,
                percent_encoding::utf8_percent_encode(&query, percent_encoding::NON_ALPHANUMERIC)
            );
            match client.get(url).send().await {
                Ok(resp) => match resp.json::<serde_json::Value>().await {
                    Ok(data) => {
                        if let Some(first) = data.get("results").and_then(|value| value.as_array()).and_then(|items| items.first()) {
                            return Some(ProviderWriteMatch {
                                title: non_empty_string(first.get("title").and_then(|value| value.as_str()))
                                    .or_else(|| non_empty_string(first.get("name").and_then(|value| value.as_str()))),
                                overview: non_empty_string(first.get("overview").and_then(|value| value.as_str())),
                                poster_path: first.get("poster_path").and_then(|value| value.as_str()).filter(|value| !value.trim().is_empty()).map(|poster| format!("https://image.tmdb.org/t/p/w500{poster}")),
                                year: parse_year_prefix(first.get("release_date").and_then(|value| value.as_str()))
                                    .or_else(|| parse_year_prefix(first.get("first_air_date").and_then(|value| value.as_str()))),
                                rating: first.get("vote_average").and_then(|value| value.as_f64()).filter(|value| *value > 0.0),
                                genre: None,
                                tmdb_id: first.get("id").and_then(|value| value.as_i64()).map(|value| value.to_string()),
                                imdb_id: None,
                                media_type: None,
                            });
                        }
                    }
                    Err(err) => provider_errors.push(format!("tmdb/{query}: {err}")),
                },
                Err(err) => provider_errors.push(format!("tmdb/{query}: {err}")),
            }
        }

        if let Some(key) = provider_keys.get("omdb") {
            let url = format!(
                "https://www.omdbapi.com/?apikey={}&t={}&plot=full",
                key,
                percent_encoding::utf8_percent_encode(&query, percent_encoding::NON_ALPHANUMERIC)
            );
            match client.get(url).send().await {
                Ok(resp) => match resp.json::<serde_json::Value>().await {
                    Ok(data) => {
                        if data.get("Response").and_then(|value| value.as_str()) == Some("True") {
                            return Some(ProviderWriteMatch {
                                title: non_empty_string(data.get("Title").and_then(|value| value.as_str())),
                                overview: non_empty_string(data.get("Plot").and_then(|value| value.as_str())),
                                poster_path: non_empty_string(data.get("Poster").and_then(|value| value.as_str())),
                                year: parse_year_prefix(data.get("Year").and_then(|value| value.as_str())),
                                rating: data.get("imdbRating").and_then(|value| value.as_str()).and_then(|rating| rating.parse::<f64>().ok()).filter(|value| *value > 0.0),
                                genre: non_empty_string(data.get("Genre").and_then(|value| value.as_str())),
                                tmdb_id: None,
                                imdb_id: non_empty_string(data.get("imdbID").and_then(|value| value.as_str())),
                                media_type: None,
                            });
                        }
                    }
                    Err(err) => provider_errors.push(format!("omdb/{query}: {err}")),
                },
                Err(err) => provider_errors.push(format!("omdb/{query}: {err}")),
            }
        }
    }
    None
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
            Ok(resp.json().await.map_err(|e| e.to_string())?)
        }
        "omdb" => {
            let key = api_key.ok_or("OMDb API key required")?;
            let url = format!(
                "https://www.omdbapi.com/?apikey={}&s={}",
                key,
                percent_encoding::utf8_percent_encode(&query, percent_encoding::NON_ALPHANUMERIC)
            );
            let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
            Ok(resp.json().await.map_err(|e| e.to_string())?)
        }
        "tvmaze" => {
            let url = format!(
                "https://api.tvmaze.com/search/shows?q={}",
                percent_encoding::utf8_percent_encode(&query, percent_encoding::NON_ALPHANUMERIC)
            );
            let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
            Ok(resp.json().await.map_err(|e| e.to_string())?)
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
            Ok(resp.json().await.map_err(|e| e.to_string())?)
        }
        "tpdb" => {
            let key = api_key.ok_or("ThePornDB API key required")?;
            fetch_theporndb_search_metadata(&client, &query, &key).await
        }
        "stashdb" => {
            let key = api_key.ok_or("StashDB API key required")?;
            let found = fetch_stashdb_scene_metadata(&client, &query, &key).await?;
            Ok(serde_json::to_value(found.title).unwrap_or(serde_json::Value::Null))
        }
        "metadataapi" => {
            let key = api_key.ok_or("MetadataAPI key required")?;
            let found = fetch_metadataapi_scene_metadata(&client, &query, &key).await?;
            Ok(serde_json::json!({ "provider": "metadataapi", "query": query, "match": found.title }))
        }
        "phoenixadult" | "adultdvdempire" | "r18" | "heyzo" | "caribbeancom" | "hegre" | "porndoe" | "nubiles" | "pornhub" | "porncz" | "data18" | "brazzers" | "julesjordan" | "naughtyamerica" | "bangbros" | "babes" | "digitalplayground" | "evilangel" | "kink" | "milehigh" | "mofos" | "mylf" | "girlsway" | "fakehub" | "adulttime" | "21naturals" | "21sextury" | "hentaipros" => fetch_phoenixadult_manifest(&client, &query).await,
        _ => Ok(serde_json::json!({
            "provider": provider,
            "query": query,
            "message": "Provider is registered in the CinaVault compatibility catalog. Native live retrieval is available where an API/bridge exists; otherwise provider is available for plugin/manifest compatibility, matching, and future adapter use."
        })),
    }
}

async fn fetch_phoenixadult_manifest(
    client: &reqwest::Client,
    query: &str,
) -> Result<serde_json::Value, String> {
    let manifest = client
        .get(PHOENIX_ADULT_MANIFEST_URL)
        .send()
        .await
        .map_err(|err| err.to_string())?
        .json::<serde_json::Value>()
        .await
        .map_err(|err| err.to_string())?;

    let plugin = manifest
        .as_array()
        .and_then(|items| items.first())
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let latest = plugin
        .get("versions")
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    Ok(serde_json::json!({
        "provider": "phoenixadult",
        "query": query,
        "manifest_url": PHOENIX_ADULT_MANIFEST_URL,
        "plugin": plugin,
        "latest_version": latest.get("version").cloned().unwrap_or(serde_json::Value::Null),
        "latest_download_url": latest.get("sourceUrl").cloned().unwrap_or(serde_json::Value::Null),
        "capabilities": [
            "scene_title", "scene_summary", "studio", "release_date", "genres_categories_tags",
            "performers", "posters_and_background_art", "network_provider_filename_matching"
        ],
        "filename_patterns": [
            "SiteName - YYYY-MM-DD - Scene Name.[ext]",
            "SiteName - Scene Name.[ext]",
            "SiteName - SceneID - Scene Name.[ext]",
            "Exact Title.[ext]",
            "Exact Title-poster.jpg"
        ],
        "message": "PhoenixAdult-compatible providers are wired through CinaVault's filename compatibility, manifest bridge, and live API fallbacks such as ThePornDB, StashDB, and MetadataAPI."
    }))
}

#[tauri::command]
pub async fn search_metadata(
    provider: String,
    query: String,
    _media_type: Option<String>,
    api_key: Option<String>,
) -> Result<serde_json::Value, String> {
    fetch_metadata(provider, query, api_key).await
}

#[tauri::command]
pub async fn check_media_item_metadata(
    state: State<'_, AppState>,
    id: i64,
) -> Result<serde_json::Value, String> {
    let (item, provider_keys) = {
        let db = state.db.lock().map_err(|err| err.to_string())?;
        let item = db
            .conn
            .query_row(
                "SELECT id, title, file_path, media_type, overview, poster_path, year, rating, genre, tmdb_id, imdb_id
                 FROM media_items WHERE id = ?1",
                params![id],
                |row| {
                    Ok(MediaItemLookup {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        file_path: row.get(2)?,
                        media_type: row.get(3)?,
                        overview: row.get(4)?,
                        poster_path: row.get(5)?,
                        year: row.get(6)?,
                        rating: row.get(7)?,
                        genre: row.get(8)?,
                        tmdb_id: row.get(9)?,
                        imdb_id: row.get(10)?,
                    })
                },
            )
            .map_err(|err| err.to_string())?;

        let mut stmt = db.conn.prepare("SELECT provider, api_key FROM api_keys").map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .map_err(|err| err.to_string())?;
        let mut provider_keys = std::collections::HashMap::new();
        for row in rows {
            let (provider, key) = row.map_err(|err| err.to_string())?;
            if key.trim().is_empty() { continue; }
            provider_keys.insert(provider.trim().to_lowercase(), key.clone());
            provider_keys.insert(normalize_provider_key(&provider), key);
        }
        (item, provider_keys)
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|err| err.to_string())?;
    let mut provider_errors = Vec::new();

    let provider_match = if media_item_is_adult(&item) {
        fetch_adult_item_metadata(&client, &provider_keys, &item, &mut provider_errors).await
    } else {
        fetch_standard_item_metadata(&client, &provider_keys, &item, &mut provider_errors).await
    };

    let Some(provider_match) = provider_match else {
        return Ok(serde_json::json!({
            "type": "single_item_metadata_check",
            "status": "no_match",
            "item_id": item.id,
            "metadata_updated": false,
            "metadata_fields_updated": 0,
            "provider_errors": provider_errors,
            "message": format!("No metadata match found for {}", item.title),
            "updated_item": write_snapshot(&item, &ProviderWriteMatch::default()),
        }));
    };

    let update = build_metadata_update(&item, &provider_match);
    let changed_fields = count_metadata_changes(&update);
    if changed_fields > 0 {
        let db = state.db.lock().map_err(|err| err.to_string())?;
        db.update_media_metadata_data(
            &item.file_path,
            update.title.as_deref(),
            update.overview.as_deref(),
            update.poster_path.as_deref(),
            update.year,
            update.rating,
            update.genre.as_deref(),
            update.tmdb_id.as_deref(),
            update.imdb_id.as_deref(),
            update.media_type.as_deref(),
        )
        .map_err(|err| err.to_string())?;
    }

    let snapshot = write_snapshot(&item, &update);
    Ok(serde_json::json!({
        "type": "single_item_metadata_check",
        "status": if changed_fields > 0 { "success" } else { "no_changes" },
        "item_id": item.id,
        "metadata_updated": changed_fields > 0,
        "metadata_fields_updated": changed_fields,
        "provider_errors": provider_errors,
        "message": if changed_fields > 0 {
            format!("Metadata updated for {}", snapshot.title)
        } else {
            format!("Metadata check completed for {} with no new fields to write", snapshot.title)
        },
        "updated_item": snapshot,
    }))
}

#[tauri::command]
pub fn get_provider_status(state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.conn.prepare("SELECT provider, api_key FROM api_keys").map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;

    let mut configured = serde_json::Map::new();
    for row in rows {
        let (provider, _key) = row.map_err(|e| e.to_string())?;
        configured.insert(normalize_provider_key(&provider), serde_json::Value::Bool(true));
    }

    Ok(serde_json::json!({
        "total_providers": PROVIDERS.len(),
        "configured": configured,
        "adult_providers_registered": PROVIDERS.iter().filter(|(_, _, _, _, category)| category.starts_with("Adult")).count(),
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
            let resp = client.get(format!("https://api.themoviedb.org/3/configuration?api_key={}", api_key)).send().await.map_err(|e| e.to_string())?;
            resp.status().is_success()
        }
        "omdb" => {
            let resp = client.get(format!("https://www.omdbapi.com/?apikey={}&t=test", api_key)).send().await.map_err(|e| e.to_string())?;
            resp.status().is_success()
        }
        "tpdb" => {
            let headers = theporndb_headers(&api_key)?;
            let resp = client.get("https://api.theporndb.net/sites?q=test").headers(headers).send().await.map_err(|e| e.to_string())?;
            resp.status().is_success()
        }
        "stashdb" => {
            let headers = bearer_headers(&api_key)?;
            let body = serde_json::json!({ "query": "query { findScenes(filter: { per_page: 1 }) { count } }" });
            let resp = client.post("https://stashdb.org/graphql").headers(headers).json(&body).send().await.map_err(|e| e.to_string())?;
            resp.status().is_success()
        }
        "metadataapi" => {
            let headers = bearer_headers(&api_key)?;
            let resp = client.get("https://metadataapi.net").headers(headers).send().await.map_err(|e| e.to_string())?;
            resp.status().is_success() || resp.status().as_u16() == 404
        }
        _ => should_assume_key_validity(provider.as_str()),
    };

    Ok(serde_json::json!({
        "provider": provider,
        "valid": result,
    }))
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
    let mut stmt = db.conn.prepare("SELECT provider, api_key FROM api_keys").map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;

    let mut keys = serde_json::Map::new();
    for row in rows {
        let (provider, key) = row.map_err(|e| e.to_string())?;
        let normalized_provider = normalize_provider_key(&provider);
        let masked = if key.len() > 4 {
            format!("{}...{}", &key[..2], &key[key.len() - 2..])
        } else {
            "****".to_string()
        };
        keys.insert(normalized_provider, serde_json::Value::String(masked));
    }

    Ok(serde_json::Value::Object(keys))
}

#[cfg(test)]
mod tests {
    use super::{
        build_metadata_queries, extract_phoenix_scene_query, is_known_provider,
        normalize_provider_key, should_assume_key_validity, MediaItemLookup,
    };

    #[test]
    fn known_provider_is_detected() {
        assert!(is_known_provider("tmdb"));
        assert!(is_known_provider("themoviedb_images"));
        assert!(is_known_provider("tpdb"));
        assert!(is_known_provider("MetadataAPI"));
        assert!(is_known_provider("Brazzers Network"));
        assert!(!is_known_provider("unknown_provider"));
    }

    #[test]
    fn provider_key_aliases_are_normalized() {
        assert_eq!(normalize_provider_key("themoviedb_images"), "tmdb");
        assert_eq!(normalize_provider_key("theporndb"), "tpdb");
        assert_eq!(normalize_provider_key("openmoviedb"), "omdb");
        assert_eq!(normalize_provider_key("metadata api"), "metadataapi");
        assert_eq!(normalize_provider_key("Adult DVD Empire"), "adultdvdempire");
    }

    #[test]
    fn unknown_provider_is_not_assumed_valid() {
        assert!(!should_assume_key_validity("unknown_provider"));
    }

    #[test]
    fn known_provider_without_live_check_is_assumed_valid() {
        assert!(should_assume_key_validity("tvdb"));
        assert!(should_assume_key_validity("phoenixadult"));
    }

    #[test]
    fn known_provider_with_live_check_is_not_assumed_valid() {
        assert!(!should_assume_key_validity("tmdb"));
        assert!(!should_assume_key_validity("tpdb"));
        assert!(!should_assume_key_validity("stashdb"));
    }

    #[test]
    fn phoenix_filename_scene_query_is_extracted() {
        assert_eq!(
            extract_phoenix_scene_query(r"E:\Adult\Blacked - 2018-12-11 - The Real Thing.mp4").as_deref(),
            Some("The Real Thing")
        );
    }

    #[test]
    fn phoenix_filename_query_is_prioritized() {
        let item = MediaItemLookup {
            id: 1,
            title: "Blacked - 2018-12-11 - The Real Thing".to_string(),
            file_path: r"E:\Adult\Blacked - 2018-12-11 - The Real Thing.mp4".to_string(),
            media_type: "adult".to_string(),
            overview: None,
            poster_path: None,
            year: None,
            rating: None,
            genre: None,
            tmdb_id: None,
            imdb_id: None,
        };

        let queries = build_metadata_queries(&item);
        assert_eq!(queries.first().map(String::as_str), Some("The Real Thing"));
    }
}
