// CinaVault Premium — AI Diagnostics Module (HuggingFace Inference)
use serde::{Deserialize, Serialize};
use tauri::State;
use crate::AppState;

const DEFAULT_MODEL: &str = "facebook/bart-large-cnn";
const HF_BASE_URL: &str = "https://router.huggingface.co/hf-inference/models";

#[tauri::command]
pub async fn ai_query(
    state: State<'_, AppState>,
    prompt: String,
) -> Result<serde_json::Value, String> {
    // Route query to appropriate handler
    let lower = prompt.to_lowercase();

    if lower.contains("network") || lower.contains("ping") || lower.contains("dns") || lower.contains("connection") {
        return run_network_diagnostics().await;
    }
    if lower.contains("source") || lower.contains("folder") || lower.contains("media") || lower.contains("library") {
        return check_sources(state).await;
    }
    if lower.contains("provider") || lower.contains("api") || lower.contains("metadata") {
        return check_providers(state).await;
    }

    // Default: run AI inference
    ai_inference(state, prompt, None).await
}

#[tauri::command]
pub async fn ai_inference(
    state: State<'_, AppState>,
    input: String,
    model: Option<String>,
) -> Result<serde_json::Value, String> {
    let token = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_setting_data("hf_token").map_err(|e| e.to_string())?
            .or_else(|| std::env::var("CINAVAULT_HF_TOKEN").ok())
    };

    let model_id = model.unwrap_or_else(|| {
        let db = state.db.lock().ok();
        db.and_then(|d| d.get_setting_data("ai_model").ok().flatten())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string())
    });

    let url = format!("{}/{}", HF_BASE_URL, model_id);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let mut req = client.post(&url)
        .json(&serde_json::json!({
            "inputs": input,
            "parameters": {
                "max_length": 256,
                "min_length": 30,
            }
        }));

    if let Some(t) = &token {
        if !t.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", t));
        }
    }

    let resp = req.send().await.map_err(|e| format!("AI request failed: {}", e))?;
    let status = resp.status();

    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Ok(serde_json::json!({
            "status": "error",
            "code": status.as_u16(),
            "message": body,
            "model": model_id,
        }));
    }

    let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "status": "success",
        "model": model_id,
        "result": data,
    }))
}

async fn run_network_diagnostics() -> Result<serde_json::Value, String> {
    let mut results = serde_json::Map::new();

    // DNS check
    let dns = std::process::Command::new("nslookup")
        .arg("google.com")
        .output();
    results.insert("dns".to_string(), serde_json::json!({
        "test": "DNS Resolution",
        "target": "google.com",
        "success": dns.as_ref().map(|o| o.status.success()).unwrap_or(false),
        "output": dns.ok().map(|o| String::from_utf8_lossy(&o.stdout).to_string()),
    }));

    // Ping check
    #[cfg(target_os = "windows")]
    let ping = std::process::Command::new("ping")
        .args(&["-n", "3", "8.8.8.8"])
        .output();
    #[cfg(not(target_os = "windows"))]
    let ping = std::process::Command::new("ping")
        .args(&["-c", "3", "8.8.8.8"])
        .output();

    results.insert("ping".to_string(), serde_json::json!({
        "test": "Ping (Google DNS)",
        "target": "8.8.8.8",
        "success": ping.as_ref().map(|o| o.status.success()).unwrap_or(false),
        "output": ping.ok().map(|o| String::from_utf8_lossy(&o.stdout).to_string()),
    }));

    // HTTP check
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build().map_err(|e| e.to_string())?;
    let http = client.get("https://www.google.com").send().await;
    results.insert("http".to_string(), serde_json::json!({
        "test": "HTTPS Connectivity",
        "target": "https://www.google.com",
        "success": http.as_ref().map(|r| r.status().is_success()).unwrap_or(false),
    }));

    Ok(serde_json::json!({
        "type": "network_diagnostics",
        "results": results,
    }))
}

async fn check_sources(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let sources = db.get_sources_data().map_err(|e| e.to_string())?;

    let mut checks = Vec::new();
    for source in &sources {
        let exists = std::path::Path::new(&source.path).exists();
        checks.push(serde_json::json!({
            "name": source.name,
            "path": source.path,
            "exists": exists,
            "enabled": source.enabled,
            "items": source.item_count,
        }));
    }

    Ok(serde_json::json!({
        "type": "source_check",
        "total_sources": sources.len(),
        "results": checks,
    }))
}

async fn check_providers(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.conn.prepare("SELECT provider FROM api_keys").map_err(|e| e.to_string())?;
    let providers: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(serde_json::json!({
        "type": "provider_check",
        "configured_providers": providers,
        "total_configured": providers.len(),
    }))
}

#[tauri::command]
pub fn set_hf_token(state: State<AppState>, token: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.set_setting_data("hf_token", &token).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_ai_config(state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let model = db.get_setting_data("ai_model").map_err(|e| e.to_string())?
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
    let has_token = db.get_setting_data("hf_token").map_err(|e| e.to_string())?
        .map(|t| !t.is_empty())
        .unwrap_or(false);

    Ok(serde_json::json!({
        "model": model,
        "has_token": has_token,
        "default_model": DEFAULT_MODEL,
        "inference_url": HF_BASE_URL,
    }))
}

#[tauri::command]
pub fn set_ai_model(state: State<AppState>, model: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.set_setting_data("ai_model", &model).map_err(|e| e.to_string())
}
