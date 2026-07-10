use crate::adult_site_provider::{
    is_porn_site_nuxt_alias, porn_site_nuxt_base_url, porn_site_nuxt_entries,
    porn_site_nuxt_entry_id, porn_site_nuxt_entry_image, porn_site_nuxt_entry_overview,
    porn_site_nuxt_entry_rating, porn_site_nuxt_entry_source_url, porn_site_nuxt_entry_title,
    porn_site_nuxt_search_url, PORN_SITE_NUXT_DEFAULT_BASE_URL,
};
use crate::metadata::MetadataProvider;
use crate::AppState;
use rusqlite::params;
use tauri::State;

const PGMA_PROVIDER_KEY: &str = "pgma";
const PGMA_PROVIDER_BASE_URL: &str = "cinavault://pgma-bridge";

fn is_pgma_alias(provider: &str) -> bool {
    matches!(
        provider.trim().to_lowercase().as_str(),
        "pgma" | "pgma-modernized" | "pgma_modernized" | "pgma modernized" | "plex pgma"
    )
}

fn normalize_provider_key(provider: &str) -> String {
    if is_pgma_alias(provider) {
        return PGMA_PROVIDER_KEY.to_string();
    }
    if is_porn_site_nuxt_alias(provider) {
        return "porn_site_nuxt".to_string();
    }
    match provider.trim().to_lowercase().as_str() {
        "themoviedb" | "themoviedb_images" | "tmdb_images" | "tmdb" => "tmdb".to_string(),
        "theporndb" | "tpdb" => "tpdb".to_string(),
        "open_movie_db" | "openmoviedb" | "omdb" => "omdb".to_string(),
        other => other.to_string(),
    }
}

fn clean_local_metadata_title(query: &str) -> String {
    let normalized = query.replace('_', " ").replace('.', " ").replace('-', " ");
    let noise = [
        "2160p", "1080p", "720p", "480p", "4k", "uhd", "hd", "x264", "x265", "h264", "h265",
        "hevc", "webdl", "webrip", "bluray", "brrip", "dvdrip", "aac", "ddp", "mp4", "mkv", "avi",
        "mov", "wmv",
    ];
    let words: Vec<&str> = normalized
        .split_whitespace()
        .filter(|word| {
            let lowered = word
                .trim_matches(|c: char| !c.is_ascii_alphanumeric())
                .to_ascii_lowercase();
            !lowered.is_empty() && !noise.contains(&lowered.as_str())
        })
        .collect();
    let title = words.join(" ");
    if title.trim().is_empty() {
        query.trim().to_string()
    } else {
        title.trim().to_string()
    }
}

fn pgma_metadata_response(query: &str) -> serde_json::Value {
    let title = clean_local_metadata_title(query);
    serde_json::json!({
        "provider": PGMA_PROVIDER_KEY,
        "query": query,
        "status": "success",
        "local_fallback": true,
        "native_bridge": true,
        "reason": "pgma_modernized_native_sidecar_metadata_bridge",
        "results": [
            {
                "title": title,
                "provider": PGMA_PROVIDER_KEY,
                "source": "pgma_local_sidecar_bridge",
                "genre": "Adult"
            }
        ],
        "capabilities": [
            "local_sidecar_nfo",
            "poster_folder_cover_artwork",
            "native_library_writeback",
            "plex_bundle_staging"
        ]
    })
}

fn porn_site_nuxt_entry_to_result(entry: &serde_json::Value) -> Option<serde_json::Value> {
    let title = porn_site_nuxt_entry_title(entry)?;
    let source_url = porn_site_nuxt_entry_source_url(entry);
    let poster_path = porn_site_nuxt_entry_image(entry);
    let rating = porn_site_nuxt_entry_rating(entry);
    let overview = porn_site_nuxt_entry_overview(entry);
    let id = porn_site_nuxt_entry_id(entry);

    Some(serde_json::json!({
        "title": title,
        "provider": "porn_site_nuxt",
        "id": id,
        "source_url": source_url,
        "poster_path": poster_path,
        "overview": overview,
        "rating": rating,
        "genre": "Adult"
    }))
}

async fn fetch_porn_site_nuxt_results(
    client: &reqwest::Client,
    base_url: Option<&str>,
    query: &str,
) -> Result<serde_json::Value, String> {
    let base_url = porn_site_nuxt_base_url(base_url);
    let resp = client
        .get(porn_site_nuxt_search_url(&base_url, query))
        .timeout(std::time::Duration::from_secs(4))
        .header("Accept", "application/json")
        .header("User-Agent", "CinaVault/1.0")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!(
            "Porn Site Nuxt provider returned http_{}",
            status.as_u16()
        ));
    }
    let data = resp
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    let results: Vec<serde_json::Value> = porn_site_nuxt_entries(&data)
        .into_iter()
        .filter_map(porn_site_nuxt_entry_to_result)
        .collect();

    Ok(serde_json::json!({
        "provider": "porn_site_nuxt",
        "query": query,
        "status": "success",
        "base_url": base_url,
        "results": results
    }))
}

#[tauri::command]
pub fn get_metadata_providers() -> Vec<MetadataProvider> {
    let mut providers = crate::metadata::get_metadata_providers();
    if !providers.iter().any(|provider| provider.key == PGMA_PROVIDER_KEY) {
        providers.push(MetadataProvider {
            name: "PGMA Modernized".to_string(),
            key: PGMA_PROVIDER_KEY.to_string(),
            base_url: PGMA_PROVIDER_BASE_URL.to_string(),
            requires_key: false,
            category: "Adult".to_string(),
        });
    }
    if !providers.iter().any(|provider| provider.key == "porn_site_nuxt") {
        providers.push(MetadataProvider {
            name: "Porn Site Nuxt".to_string(),
            key: "porn_site_nuxt".to_string(),
            base_url: PORN_SITE_NUXT_DEFAULT_BASE_URL.to_string(),
            requires_key: false,
            category: "Adult".to_string(),
        });
    }
    providers
}

#[tauri::command]
pub async fn fetch_metadata(
    provider: String,
    query: String,
    api_key: Option<String>,
) -> Result<serde_json::Value, String> {
    let provider = normalize_provider_key(&provider);
    match provider.as_str() {
        PGMA_PROVIDER_KEY => Ok(pgma_metadata_response(&query)),
        "porn_site_nuxt" => {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(8))
                .build()
                .map_err(|e| e.to_string())?;
            fetch_porn_site_nuxt_results(&client, api_key.as_deref(), &query).await
        }
        _ => crate::metadata::fetch_metadata(provider, query, api_key).await,
    }
}

#[tauri::command]
pub async fn search_metadata(
    provider: String,
    query: String,
    media_type: Option<String>,
    api_key: Option<String>,
) -> Result<serde_json::Value, String> {
    let _ = media_type;
    fetch_metadata(provider, query, api_key).await
}

#[tauri::command]
pub async fn check_media_item_metadata(
    state: State<'_, AppState>,
    id: i64,
) -> Result<serde_json::Value, String> {
    crate::metadata::check_media_item_metadata(state, id).await
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
        configured.insert(
            normalize_provider_key(&provider),
            serde_json::Value::Bool(true),
        );
    }

    Ok(serde_json::json!({
        "total_providers": get_metadata_providers().len(),
        "configured": configured,
    }))
}

#[tauri::command]
pub async fn test_api_key(provider: String, api_key: String) -> Result<serde_json::Value, String> {
    let provider = normalize_provider_key(&provider);
    let valid = match provider.as_str() {
        PGMA_PROVIDER_KEY => true,
        "porn_site_nuxt" => {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(4))
                .build()
                .map_err(|e| e.to_string())?;
            fetch_porn_site_nuxt_results(&client, Some(api_key.as_str()), "test")
                .await
                .is_ok()
        }
        _ => {
            let result = crate::metadata::test_api_key(provider.clone(), api_key).await?;
            result.get("valid").and_then(|value| value.as_bool()).unwrap_or(false)
        }
    };

    Ok(serde_json::json!({
        "provider": provider,
        "valid": valid,
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
    use super::{get_metadata_providers, normalize_provider_key, pgma_metadata_response};

    #[test]
    fn provider_aliases_include_pgma_and_nuxt() {
        assert_eq!(normalize_provider_key("PGMA Modernized"), "pgma");
        assert_eq!(normalize_provider_key("IreneHub"), "porn_site_nuxt");
    }

    #[test]
    fn provider_catalog_includes_pgma_and_nuxt() {
        let providers = get_metadata_providers();
        assert!(providers.iter().any(|provider| provider.key == "pgma"));
        assert!(providers.iter().any(|provider| provider.key == "porn_site_nuxt"));
    }

    #[test]
    fn pgma_returns_local_bridge_metadata() {
        let result = pgma_metadata_response("Example.Scene.1080p.mkv");
        assert_eq!(result.get("provider").and_then(|value| value.as_str()), Some("pgma"));
        assert_eq!(result.get("native_bridge").and_then(|value| value.as_bool()), Some(true));
    }
}
