use crate::metadata::MetadataProvider;
use crate::{metadata_ext, AppState};
use tauri::State;

const IMPLEMENTED_PROVIDERS: &[&str] = &[
    "tmdb",
    "omdb",
    "tvmaze",
    "musicbrainz",
    "tpdb",
    "phoenixadult",
    "pgma",
    "porn_site_nuxt",
];

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

#[tauri::command]
pub fn get_metadata_providers() -> Vec<MetadataProvider> {
    metadata_ext::get_metadata_providers()
        .into_iter()
        .filter(|provider| IMPLEMENTED_PROVIDERS.contains(&provider.key.as_str()))
        .collect()
}

#[tauri::command]
pub async fn fetch_metadata(
    provider: String,
    query: String,
    api_key: Option<String>,
) -> Result<serde_json::Value, String> {
    let provider = ensure_implemented(&provider)?;
    metadata_ext::fetch_metadata(provider, query, api_key).await
}

#[tauri::command]
pub async fn search_metadata(
    provider: String,
    query: String,
    media_type: Option<String>,
    api_key: Option<String>,
) -> Result<serde_json::Value, String> {
    let provider = ensure_implemented(&provider)?;
    metadata_ext::search_metadata(provider, query, media_type, api_key).await
}

#[tauri::command]
pub async fn check_media_item_metadata(
    state: State<'_, AppState>,
    id: i64,
) -> Result<serde_json::Value, String> {
    metadata_ext::check_media_item_metadata(state, id).await
}

#[tauri::command]
pub fn get_provider_status(state: State<AppState>) -> Result<serde_json::Value, String> {
    let base = metadata_ext::get_provider_status(state)?;
    let configured = base
        .get("configured")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let providers = get_metadata_providers()
        .into_iter()
        .map(|provider| {
            let configured_value = configured
                .get(&provider.key)
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            serde_json::json!({
                "name": provider.name,
                "key": provider.key,
                "category": provider.category,
                "requires_key": provider.requires_key,
                "implemented": true,
                "configured": configured_value,
            })
        })
        .collect::<Vec<_>>();

    Ok(serde_json::json!({
        "total_providers": providers.len(),
        "implemented_providers": providers,
        "configured": configured,
    }))
}

#[tauri::command]
pub async fn test_api_key(provider: String, api_key: String) -> Result<serde_json::Value, String> {
    let provider = ensure_implemented(&provider)?;
    metadata_ext::test_api_key(provider, api_key).await
}

#[tauri::command]
pub fn set_api_key(
    state: State<AppState>,
    provider: String,
    api_key: String,
) -> Result<(), String> {
    let provider = ensure_implemented(&provider)?;
    metadata_ext::set_api_key(state, provider, api_key)
}

#[tauri::command]
pub fn get_api_keys(state: State<AppState>) -> Result<serde_json::Value, String> {
    metadata_ext::get_api_keys(state)
}

#[cfg(test)]
mod tests {
    use super::{ensure_implemented, get_metadata_providers};

    #[test]
    fn only_implemented_providers_are_advertised() {
        let providers = get_metadata_providers();
        assert!(!providers.is_empty());
        assert!(providers
            .iter()
            .all(|provider| ensure_implemented(&provider.key).is_ok()));
        assert!(!providers.iter().any(|provider| provider.key == "tvdb"));
        assert!(!providers.iter().any(|provider| provider.key == "fanart"));
    }

    #[test]
    fn unsupported_provider_fails_explicitly() {
        assert!(ensure_implemented("tvdb").is_err());
        assert!(ensure_implemented("unknown-provider").is_err());
    }
}
