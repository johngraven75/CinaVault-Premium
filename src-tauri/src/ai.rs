// CinaVault Premium — AI Diagnostics Module (HuggingFace Inference)
use serde::{Deserialize, Serialize};
use tauri::State;
use rusqlite::params;
use crate::AppState;
use std::sync::atomic::{AtomicBool, Ordering};

const DEFAULT_MODEL: &str = "katanemo/Arch-Router-1.5B:hf-inference";
const HF_BASE_URL: &str = "https://router.huggingface.co/v1/chat/completions";
static ADULT_GATHER_RUNNING: AtomicBool = AtomicBool::new(false);

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
    if lower.contains("adult metadata")
        || lower.contains("gather metadata")
        || lower.contains("chapter images")
        || lower.contains("adult providers")
    {
        return gather_adult_metadata_assets(state).await;
    }
    if lower.contains("provider") || lower.contains("api") || lower.contains("metadata") {
        return check_providers(state).await;
    }

    // Default: run AI inference
    ai_inference(state, prompt, None, None).await
}

#[tauri::command]
pub async fn ai_inference(
    state: State<'_, AppState>,
    input: String,
    model: Option<String>,
    image_url: Option<String>,
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

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let user_content = if let Some(url) = image_url.filter(|url| !url.trim().is_empty()) {
        serde_json::json!([
            { "type": "text", "text": input },
            { "type": "image_url", "image_url": { "url": url } }
        ])
    } else {
        serde_json::json!(input)
    };

    let mut req = client.post(HF_BASE_URL).json(&serde_json::json!({
        "model": model_id,
        "messages": [
            {
                "role": "system",
                "content": "You are CineVault Premium's AI assistant for media server operations, metadata workflows, and diagnostics. Give concise, practical answers."
            },
            {
                "role": "user",
                "content": user_content
            }
        ],
        "temperature": 0.2,
        "max_tokens": 512
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
    let content = data
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    Ok(serde_json::json!({
        "status": "success",
        "model": model_id,
        "message": content,
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

fn detect_local_poster(file_path: &str) -> Option<String> {
    let media = std::path::Path::new(file_path);
    let parent = media.parent()?;
    let stem = media.file_stem()?.to_string_lossy();
    let candidates = [
        parent.join("poster.jpg"),
        parent.join("folder.jpg"),
        parent.join("cover.jpg"),
        parent.join(format!("{stem}.jpg")),
        parent.join(format!("{stem}.png")),
        parent.join(format!("{stem}-poster.jpg")),
    ];
    candidates
        .iter()
        .find(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string())
}

fn chapter_dir_for(file_path: &str) -> Option<String> {
    let p = std::path::Path::new(file_path);
    let parent = p.parent()?;
    let stem = p.file_stem()?.to_string_lossy();
    Some(parent.join(format!("{stem}_chapters")).to_string_lossy().to_string())
}

fn count_existing_chapter_images(chapter_dir: &str) -> usize {
    let dir = std::path::Path::new(chapter_dir);
    if !dir.exists() {
        return 0;
    }
    std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "jpg" || ext == "png" || ext == "webp")
                .unwrap_or(false)
        })
        .count()
}

async fn gather_adult_metadata_assets(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    if ADULT_GATHER_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(serde_json::json!({
            "type": "adult_metadata_gather",
            "status": "busy",
            "message": "Adult metadata gather is already running. Please wait for completion.",
        }));
    }

    let result = gather_adult_metadata_assets_inner(state).await;
    ADULT_GATHER_RUNNING.store(false, Ordering::SeqCst);
    result
}

async fn gather_adult_metadata_assets_inner(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let configured_adult_providers: Vec<String> = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let mut stmt = db.conn.prepare(
            "SELECT provider FROM api_keys WHERE provider IN ('tpdb','stashdb','phoenixadult','iafd')"
        ).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    let media_items: Vec<(i64, String, String, Option<String>, Option<String>)> = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let mut stmt = db.conn.prepare(
            "SELECT id, title, file_path, poster_path, overview
             FROM media_items
             WHERE media_type = 'adult'
                OR lower(file_path) LIKE '%adult%'
                OR lower(file_path) LIKE '%xxx%'
                OR lower(file_path) LIKE '%porn%'
                OR lower(title) LIKE '%adult%'
                OR lower(title) LIKE '%xxx%'
                OR lower(title) LIKE '%porn%'
             ORDER BY date_added DESC
             LIMIT 200"
        ).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    let mut posters_updated = 0usize;
    let mut chapters_generated_for_items = 0usize;
    let mut chapter_images_generated = 0usize;
    let mut items_needing_metadata = 0usize;
    let mut errors: Vec<String> = Vec::new();

    for (id, title, file_path, poster_path, overview) in media_items.iter() {
        let has_overview = overview
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        if !has_overview {
            items_needing_metadata += 1;
        }

        let has_poster = poster_path
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);

        if !has_poster {
            if let Some(local_poster) = detect_local_poster(file_path) {
                let db = state.db.lock().map_err(|e| e.to_string())?;
                db.conn.execute(
                    "UPDATE media_items SET poster_path = ?1 WHERE id = ?2",
                    params![local_poster, id],
                ).map_err(|e| e.to_string())?;
                posters_updated += 1;
            }
        }

        if let Some(chapter_dir) = chapter_dir_for(file_path) {
            let existing = count_existing_chapter_images(&chapter_dir);
            if existing == 0 {
                match crate::chapters::generate_chapter_thumbs(file_path.clone(), None, Some(300), None).await {
                    Ok(thumbs) if !thumbs.is_empty() => {
                        chapters_generated_for_items += 1;
                        chapter_images_generated += thumbs.len();
                    }
                    Ok(_) => {}
                    Err(e) => errors.push(format!("{title}: {e}")),
                }
            }
        }
    }

    Ok(serde_json::json!({
        "type": "adult_metadata_gather",
        "status": "success",
        "configured_adult_providers": configured_adult_providers,
        "provider_count": configured_adult_providers.len(),
        "items_scanned": media_items.len(),
        "posters_updated": posters_updated,
        "chapter_sets_generated": chapters_generated_for_items,
        "chapter_images_generated": chapter_images_generated,
        "items_needing_metadata": items_needing_metadata,
        "note": "Adult provider metadata writeback is limited to configured integrations currently available in this build. Poster and chapter image gathering run locally.",
        "errors": errors,
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
