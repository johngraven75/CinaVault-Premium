//! Paywall front for the explicit metadata lookup commands. Asking an adult
//! provider (TPDB, StashDB, IAFD, PGMA, ...) by name needs CinaVault Plus
//! (`adult_metadata`); every other provider is passed straight through.

use crate::entitlements::{ensure_feature_state, is_adult_provider, Feature};
use crate::AppState;
use tauri::State;

fn ensure_provider_allowed(state: &AppState, provider: &str) -> Result<(), String> {
    if is_adult_provider(provider) {
        ensure_feature_state(state, Feature::AdultMetadata)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn fetch_metadata(
    state: State<'_, AppState>,
    provider: String,
    query: String,
    api_key: Option<String>,
) -> Result<serde_json::Value, String> {
    ensure_provider_allowed(state.inner(), &provider)?;
    crate::metadata_ext::fetch_metadata(provider, query, api_key).await
}

#[tauri::command]
pub async fn search_metadata(
    state: State<'_, AppState>,
    provider: String,
    query: String,
    media_type: Option<String>,
    api_key: Option<String>,
) -> Result<serde_json::Value, String> {
    ensure_provider_allowed(state.inner(), &provider)?;
    crate::metadata_ext::search_metadata(provider, query, media_type, api_key).await
}
