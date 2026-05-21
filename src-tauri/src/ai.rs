// CinaVault Premium — AI Diagnostics Module (HuggingFace Inference)
use crate::adult_site_provider::{
    is_porn_site_nuxt_alias, porn_site_nuxt_base_url, porn_site_nuxt_entries,
    porn_site_nuxt_entry_id, porn_site_nuxt_entry_image, porn_site_nuxt_entry_overview,
    porn_site_nuxt_entry_rating, porn_site_nuxt_entry_source_url, porn_site_nuxt_entry_title,
    porn_site_nuxt_search_url,
};
use crate::enrichment::{classify_library_item, LibraryItemRecord, SourceKind};
use crate::library_artifacts::available_poster_path_for_media;
use crate::phoenix_adult_provider::{phoenix_adult_manifest_summary, PHOENIX_ADULT_VERSION};
use crate::theporndb_provider::{theporndb_provider_manifest_summary, theporndb_scene_search_url};
use crate::{task_progress, AppState};
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::State;

const DEFAULT_MODEL: &str = "katanemo/Arch-Router-1.5B:hf-inference";
const HF_BASE_URL: &str = "https://router.huggingface.co/v1/chat/completions";
static ADULT_GATHER_RUNNING: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiQueryRoute {
    NetworkDiagnostics,
    AdultMetadataGather,
    SourceCheck,
    ProviderCheck,
    Inference,
}

fn classify_ai_query_prompt(prompt: &str) -> AiQueryRoute {
    let lower = prompt.to_lowercase();

    if lower.contains("network")
        || lower.contains("ping")
        || lower.contains("dns")
        || lower.contains("connection")
    {
        return AiQueryRoute::NetworkDiagnostics;
    }
    if lower.contains("adult metadata")
        || lower.contains("gather metadata")
        || lower.contains("chapter images")
        || lower.contains("adult providers")
    {
        return AiQueryRoute::AdultMetadataGather;
    }
    if lower.contains("source")
        || lower.contains("folder")
        || lower.contains("media")
        || lower.contains("library")
    {
        return AiQueryRoute::SourceCheck;
    }
    if lower.contains("provider") || lower.contains("api") || lower.contains("metadata") {
        return AiQueryRoute::ProviderCheck;
    }

    AiQueryRoute::Inference
}

pub(crate) fn is_adult_gather_candidate(media_type: &str, file_path: &str) -> bool {
    let path_lower = file_path.replace('/', "\\").to_lowercase();
    let is_video = [
        ".mp4", ".mkv", ".avi", ".mov", ".wmv", ".flv", ".webm", ".m4v", ".mpg", ".mpeg", ".ts",
        ".m2ts", ".vob", ".ogv", ".3gp", ".divx", ".rm", ".rmvb", ".asf",
    ]
    .iter()
    .any(|ext| path_lower.ends_with(ext));

    if !is_video {
        return false;
    }

    if path_lower.contains("_chapters\\chapter_") {
        return false;
    }

    matches!(media_type, "adult" | "movie" | "video")
}

fn normalize_adult_provider_key(provider: &str) -> String {
    if is_porn_site_nuxt_alias(provider) {
        return "porn_site_nuxt".to_string();
    }
    match provider.trim().to_lowercase().as_str() {
        "theporndb" | "tpdb" => "tpdb".to_string(),
        "phoenix_adult" | "phoenix adult" | "phoenixadult" => "phoenixadult".to_string(),
        other => other.to_string(),
    }
}

fn normalize_provider_key(provider: &str) -> String {
    if is_porn_site_nuxt_alias(provider) {
        return "porn_site_nuxt".to_string();
    }
    match provider.trim().to_lowercase().as_str() {
        "themoviedb" | "themoviedb_images" | "tmdb_images" | "tmdb" => "tmdb".to_string(),
        "theporndb" | "tpdb" => "tpdb".to_string(),
        "open_movie_db" | "openmoviedb" | "omdb" => "omdb".to_string(),
        "phoenix_adult" | "phoenix adult" | "phoenixadult" => "phoenixadult".to_string(),
        other => other.to_string(),
    }
}

fn is_adult_library_item(
    media_type: &str,
    title: &str,
    file_path: &str,
    source_name: Option<&str>,
    source_path: Option<&str>,
) -> bool {
    if !is_adult_gather_candidate(media_type, file_path) {
        return false;
    }

    let item = LibraryItemRecord {
        id: 0,
        title: title.to_string(),
        file_path: file_path.to_string(),
        media_type: media_type.to_string(),
        overview: None,
        poster_path: None,
        year: None,
        rating: None,
        genre: None,
        tmdb_id: None,
        imdb_id: None,
        source_name: source_name.map(str::to_string),
        source_path: source_path.map(str::to_string),
    };

    classify_library_item(&item) == SourceKind::AdultVideo
}

fn title_from_filename(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown".to_string())
        .replace('_', " ")
        .replace('.', " ")
}

fn should_refresh_title_from_embedded(current_title: &str, file_path: &str) -> bool {
    let trimmed = current_title.trim();
    if trimmed.is_empty() {
        return true;
    }

    let filename_title = title_from_filename(Path::new(file_path));
    trimmed.eq_ignore_ascii_case(&filename_title)
}

fn extract_embedded_title(file_path: &str) -> Option<String> {
    let mut cmd = Command::new("ffprobe");
    cmd.args([
        "-v",
        "error",
        "-show_entries",
        "format_tags=title:stream_tags=title",
        "-of",
        "default=nw=1:nk=1",
        file_path,
    ]);
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.to_string())
}

#[tauri::command]
pub async fn ai_query(
    state: State<'_, AppState>,
    prompt: String,
) -> Result<serde_json::Value, String> {
    match classify_ai_query_prompt(&prompt) {
        AiQueryRoute::NetworkDiagnostics => run_network_diagnostics().await,
        AiQueryRoute::AdultMetadataGather => gather_adult_metadata_assets(state).await,
        AiQueryRoute::SourceCheck => check_sources(state).await,
        AiQueryRoute::ProviderCheck => check_providers(state).await,
        AiQueryRoute::Inference => ai_inference(state, prompt, None, None).await,
    }
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
        db.get_setting_data("hf_token")
            .map_err(|e| e.to_string())?
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

    let resp = req
        .send()
        .await
        .map_err(|e| format!("AI request failed: {}", e))?;
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
    results.insert(
        "dns".to_string(),
        serde_json::json!({
            "test": "DNS Resolution",
            "target": "google.com",
            "success": dns.as_ref().map(|o| o.status.success()).unwrap_or(false),
            "output": dns.ok().map(|o| String::from_utf8_lossy(&o.stdout).to_string()),
        }),
    );

    // Ping check
    #[cfg(target_os = "windows")]
    let ping = std::process::Command::new("ping")
        .args(&["-n", "3", "8.8.8.8"])
        .output();
    #[cfg(not(target_os = "windows"))]
    let ping = std::process::Command::new("ping")
        .args(&["-c", "3", "8.8.8.8"])
        .output();

    results.insert(
        "ping".to_string(),
        serde_json::json!({
            "test": "Ping (Google DNS)",
            "target": "8.8.8.8",
            "success": ping.as_ref().map(|o| o.status.success()).unwrap_or(false),
            "output": ping.ok().map(|o| String::from_utf8_lossy(&o.stdout).to_string()),
        }),
    );

    // HTTP check
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;
    let http = client.get("https://www.google.com").send().await;
    results.insert(
        "http".to_string(),
        serde_json::json!({
            "test": "HTTPS Connectivity",
            "target": "https://www.google.com",
            "success": http.as_ref().map(|r| r.status().is_success()).unwrap_or(false),
        }),
    );

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

fn provider_live_check_supported(provider: &str) -> bool {
    matches!(
        normalize_provider_key(provider).as_str(),
        "tmdb" | "omdb" | "stashdb" | "tpdb" | "phoenixadult" | "iafd" | "porn_site_nuxt"
    )
}

async fn check_single_provider_key(
    client: &reqwest::Client,
    provider: &str,
    api_key: &str,
) -> (bool, String) {
    match normalize_provider_key(provider).as_str() {
        "tmdb" => {
            let url = format!("https://api.themoviedb.org/3/configuration?api_key={api_key}");
            match client.get(url).send().await {
                Ok(resp) => (
                    resp.status().is_success(),
                    format!("http_{}", resp.status().as_u16()),
                ),
                Err(err) => (false, err.to_string()),
            }
        }
        "omdb" => {
            let url = format!("https://www.omdbapi.com/?apikey={api_key}&s=test");
            match client.get(url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let data = resp.json::<serde_json::Value>().await.unwrap_or_default();
                    let invalid_key = data
                        .get("Error")
                        .and_then(|v| v.as_str())
                        .map(|v| v.to_lowercase().contains("invalid api key"))
                        .unwrap_or(false);
                    (
                        status.is_success() && !invalid_key,
                        format!("http_{}", status.as_u16()),
                    )
                }
                Err(err) => (false, err.to_string()),
            }
        }
        "stashdb" => {
            let body = serde_json::json!({ "query": "{ __typename }" });
            match client
                .post("https://stashdb.org/graphql")
                .header("Content-Type", "application/json")
                .header("ApiKey", api_key)
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status();
                    let data = resp.json::<serde_json::Value>().await.unwrap_or_default();
                    (
                        status.is_success() && data.get("errors").is_none(),
                        format!("http_{}", status.as_u16()),
                    )
                }
                Err(err) => (false, err.to_string()),
            }
        }
        "tpdb" => {
            match client
                .get(theporndb_scene_search_url("test"))
                .header("Authorization", format!("Bearer {api_key}"))
                .header("Accept", "application/json")
                .header("User-Agent", "CinaVault/1.0")
                .send()
                .await
            {
                Ok(resp) => (
                    resp.status().is_success(),
                    format!("http_{}", resp.status().as_u16()),
                ),
                Err(err) => (false, err.to_string()),
            }
        }
        "phoenixadult" => (
            true,
            format!(
                "phoenixadult_manifest_{PHOENIX_ADULT_VERSION}_local_metadata_screenshot_fallback"
            ),
        ),
        "iafd" => (true, "local_adult_title_fallback".to_string()),
        "porn_site_nuxt" => {
            let base_url = porn_site_nuxt_base_url(Some(api_key));
            let url = porn_site_nuxt_search_url(&base_url, "test");
            match client
                .get(url)
                .timeout(std::time::Duration::from_secs(4))
                .header("Accept", "application/json")
                .header("User-Agent", "CinaVault/1.0")
                .send()
                .await
            {
                Ok(resp) => (
                    resp.status().is_success(),
                    format!("http_{}", resp.status().as_u16()),
                ),
                Err(err) => (false, err.to_string()),
            }
        }
        _ => (true, "local_metadata_fallback".to_string()),
    }
}

async fn check_providers(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let provider_rows: Vec<(String, String)> = {
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
        rows.filter_map(|r| r.ok()).collect()
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let mut providers = Vec::new();
    let mut valid = 0usize;
    let mut invalid = 0usize;
    let mut unsupported = 0usize;

    for (provider, key) in provider_rows {
        let normalized = normalize_provider_key(&provider);
        let key_present = !key.trim().is_empty();
        let live_supported = provider_live_check_supported(&normalized);
        let (is_valid, status) = if matches!(
            normalized.as_str(),
            "phoenixadult" | "iafd" | "porn_site_nuxt"
        ) {
            check_single_provider_key(&client, &normalized, &key).await
        } else if key_present && live_supported {
            check_single_provider_key(&client, &normalized, &key).await
        } else if key_present {
            (true, "local_metadata_fallback".to_string())
        } else {
            (false, "missing_api_key".to_string())
        };

        if is_valid {
            valid += 1;
        } else if live_supported {
            invalid += 1;
        } else {
            unsupported += 1;
        }

        providers.push(serde_json::json!({
            "provider": normalized,
            "saved_as": provider,
            "key_present": key_present,
            "live_check_supported": live_supported,
            "valid": is_valid,
            "status": status,
        }));
    }

    Ok(serde_json::json!({
        "type": "provider_check",
        "configured_providers": providers,
        "total_configured": providers.len(),
        "valid": valid,
        "invalid": invalid,
        "unsupported": unsupported,
    }))
}

fn detect_local_poster(file_path: &str) -> Option<String> {
    available_poster_path_for_media(std::path::Path::new(file_path))
}

fn generated_poster_cache_path_for_file(file_path: &str) -> Option<std::path::PathBuf> {
    let base = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("CinaVault")
        .join("generated-posters");
    std::fs::create_dir_all(&base).ok()?;

    let mut hasher = Sha256::new();
    hasher.update(file_path.as_bytes());
    let digest = hasher.finalize();
    Some(base.join(format!("{digest:x}.jpg")))
}

fn generated_screenshot_posters(file_path: &str) -> Result<Option<String>, String> {
    let Some(output_path) = generated_poster_cache_path_for_file(file_path) else {
        return Ok(None);
    };
    if output_path.exists() {
        return Ok(Some(output_path.to_string_lossy().to_string()));
    }

    let output_arg = output_path.to_string_lossy().to_string();
    for timestamp in ["00:00:10", "00:00:03", "00:00:01"] {
        let mut cmd = Command::new("ffmpeg");
        cmd.args([
            "-y",
            "-v",
            "error",
            "-ss",
            timestamp,
            "-i",
            file_path,
            "-frames:v",
            "1",
            "-q:v",
            "3",
            &output_arg,
        ]);
        #[cfg(target_os = "windows")]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let output = match cmd.output() {
            Ok(output) => output,
            Err(_) => return Ok(None),
        };
        if output.status.success() && output_path.exists() {
            return Ok(Some(output_path.to_string_lossy().to_string()));
        }
        let _ = std::fs::remove_file(&output_path);
    }

    Ok(None)
}

fn chapter_dir_for(file_path: &str) -> Option<String> {
    let p = std::path::Path::new(file_path);
    let parent = p.parent()?;
    let stem = p.file_stem()?.to_string_lossy();
    Some(
        parent
            .join(format!("{stem}_chapters"))
            .to_string_lossy()
            .to_string(),
    )
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

fn metadata_sidecar_path(file_path: &str) -> Option<std::path::PathBuf> {
    let media = std::path::Path::new(file_path);
    let parent = media.parent()?;
    let stem = media.file_stem()?.to_string_lossy();
    Some(parent.join(format!("{stem}.cinavault.json")))
}

fn write_metadata_sidecar(
    file_path: &str,
    title: &str,
    overview: Option<&String>,
    poster_path: Option<&String>,
    year: Option<i32>,
    rating: Option<f64>,
    genre: Option<&String>,
    tmdb_id: Option<&String>,
    imdb_id: Option<&String>,
) -> Result<bool, String> {
    let sidecar_path = metadata_sidecar_path(file_path).ok_or("Unable to resolve sidecar path")?;
    let payload = serde_json::json!({
        "source_file": file_path,
        "title": title,
        "overview": overview,
        "poster_path": poster_path,
        "year": year,
        "rating": rating,
        "genre": genre,
        "tmdb_id": tmdb_id,
        "imdb_id": imdb_id,
        "written_at_utc": chrono::Utc::now().to_rfc3339(),
    });
    let body = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
    std::fs::write(sidecar_path, body).map_err(|e| e.to_string())?;
    Ok(true)
}

#[derive(Default, Debug, Clone)]
struct RemoteMetadata {
    title: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    year: Option<i32>,
    rating: Option<f64>,
    genre: Option<String>,
    tmdb_id: Option<String>,
    imdb_id: Option<String>,
}

#[derive(Debug, Clone)]
struct MetadataCheckItem {
    id: i64,
    title: String,
    file_path: String,
    poster_path: Option<String>,
    overview: Option<String>,
    year: Option<i32>,
    rating: Option<f64>,
    genre: Option<String>,
    tmdb_id: Option<String>,
    imdb_id: Option<String>,
    media_type: String,
    source_name: Option<String>,
    source_path: Option<String>,
}

fn non_empty_string(value: Option<&str>) -> Option<String> {
    value
        .map(|v| v.trim())
        .filter(|v| !v.is_empty() && *v != "N/A")
        .map(|v| v.to_string())
}

fn parse_year_prefix(value: Option<&str>) -> Option<i32> {
    let text = value?.trim();
    if text.len() < 4 {
        return None;
    }
    text[..4].parse::<i32>().ok()
}

fn parse_year_anywhere(value: &str) -> Option<i32> {
    for token in value.split(|c: char| !c.is_ascii_alphanumeric()) {
        if token.len() == 4 {
            if let Ok(year) = token.parse::<i32>() {
                if (1900..=2100).contains(&year) {
                    return Some(year);
                }
            }
        }
    }
    None
}

fn tpdb_search_url(query: &str) -> String {
    theporndb_scene_search_url(query)
}

fn first_image_url(value: &serde_json::Value) -> Option<String> {
    non_empty_string(value.get("poster").and_then(|v| v.as_str()))
        .or_else(|| non_empty_string(value.get("image").and_then(|v| v.as_str())))
        .or_else(|| non_empty_string(value.get("image_url").and_then(|v| v.as_str())))
        .or_else(|| {
            value
                .get("images")
                .and_then(|v| v.as_array())
                .and_then(|images| images.first())
                .and_then(|img| {
                    img.get("url")
                        .and_then(|v| v.as_str())
                        .or_else(|| img.as_str())
                })
                .and_then(|v| non_empty_string(Some(v)))
        })
}

fn tpdb_scene_to_remote_metadata(scene: &serde_json::Value) -> Option<RemoteMetadata> {
    let title = non_empty_string(scene.get("title").and_then(|v| v.as_str()));
    let overview = non_empty_string(scene.get("description").and_then(|v| v.as_str()))
        .or_else(|| non_empty_string(scene.get("details").and_then(|v| v.as_str())));
    let poster_path = first_image_url(scene);
    let year = parse_year_prefix(scene.get("date").and_then(|v| v.as_str()));
    let genre = scene
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|tags| {
            tags.iter()
                .filter_map(|tag| tag.get("name").and_then(|v| v.as_str()))
                .filter(|name| !name.trim().is_empty())
                .take(5)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|g| !g.trim().is_empty());

    if title.is_none() && overview.is_none() && poster_path.is_none() {
        return None;
    }

    Some(RemoteMetadata {
        title,
        overview,
        poster_path,
        year,
        rating: None,
        genre,
        tmdb_id: None,
        imdb_id: None,
    })
}

fn should_prefer_remote_poster(current_poster: Option<&str>) -> bool {
    match current_poster.map(str::trim).filter(|v| !v.is_empty()) {
        None => true,
        Some(path) => {
            if path.starts_with("http://")
                || path.starts_with("https://")
                || path.starts_with("data:")
                || path.starts_with("asset:")
            {
                return false;
            }
            let lower = path.replace('/', "\\").to_lowercase();
            lower.ends_with("-poster.jpg")
                || lower.ends_with("-poster.png")
                || lower.ends_with("\\poster.jpg")
                || lower.ends_with("\\cover.jpg")
                || lower.ends_with("\\folder.jpg")
        }
    }
}

fn clean_local_adult_title(value: &str) -> Option<String> {
    let normalized = value.replace('_', " ").replace('.', " ").replace('-', " ");
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
        None
    } else {
        Some(title.trim().to_string())
    }
}

fn phoenix_adult_local_metadata(
    current_title: &str,
    file_path: &str,
    poster_path: Option<String>,
) -> Option<RemoteMetadata> {
    let filename_title = title_from_filename(Path::new(file_path));
    let title =
        clean_local_adult_title(current_title).or_else(|| clean_local_adult_title(&filename_title));
    let year = title
        .as_deref()
        .and_then(parse_year_anywhere)
        .or_else(|| parse_year_anywhere(file_path));

    if title.is_none() && poster_path.is_none() && year.is_none() {
        return None;
    }

    Some(RemoteMetadata {
        title,
        overview: None,
        poster_path,
        year,
        rating: None,
        genre: Some("Adult".to_string()),
        tmdb_id: None,
        imdb_id: None,
    })
}

fn porn_site_nuxt_entry_to_remote_metadata(entry: &serde_json::Value) -> Option<RemoteMetadata> {
    let title = porn_site_nuxt_entry_title(entry);
    let overview = porn_site_nuxt_entry_overview(entry);
    let poster_path = porn_site_nuxt_entry_image(entry);
    let rating = porn_site_nuxt_entry_rating(entry);
    let source_id =
        porn_site_nuxt_entry_id(entry).or_else(|| porn_site_nuxt_entry_source_url(entry));

    if title.is_none() && overview.is_none() && poster_path.is_none() {
        return None;
    }

    Some(RemoteMetadata {
        title,
        overview,
        poster_path,
        year: None,
        rating,
        genre: Some("Adult".to_string()),
        tmdb_id: None,
        imdb_id: source_id.map(|id| format!("porn_site_nuxt:{id}")),
    })
}

async fn fetch_porn_site_nuxt_metadata(
    client: &reqwest::Client,
    base_url: Option<&str>,
    query: &str,
) -> Result<Option<RemoteMetadata>, String> {
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
        return Err(format!("http_{}", status.as_u16()));
    }

    let data = resp
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    Ok(porn_site_nuxt_entries(&data)
        .into_iter()
        .find_map(porn_site_nuxt_entry_to_remote_metadata))
}

async fn fetch_tpdb_metadata(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
) -> Result<Option<RemoteMetadata>, String> {
    let graph_result =
        fetch_stashbox_metadata(client, "https://theporndb.net/graphql", api_key, query).await;
    if let Ok(Some(meta)) = graph_result {
        return Ok(Some(meta));
    }

    let resp = client
        .get(tpdb_search_url(query))
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("User-Agent", "CinaVault/1.0")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        let graph_status = graph_result
            .err()
            .unwrap_or_else(|| "no graph match".to_string());
        return Err(format!(
            "tpdb http_{}; graph {graph_status}",
            status.as_u16()
        ));
    }

    let data = resp
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    let first = data
        .get("data")
        .and_then(|v| v.as_array())
        .and_then(|items| items.first());

    Ok(first.and_then(tpdb_scene_to_remote_metadata))
}

async fn fetch_stashbox_metadata(
    client: &reqwest::Client,
    endpoint: &str,
    api_key: &str,
    query: &str,
) -> Result<Option<RemoteMetadata>, String> {
    let body = serde_json::json!({
        "query": "query($title:String!){ queryScenes(input:{title:$title, per_page:1, page:1, direction:DESC, sort:DATE}) { scenes { title details release_date date images { url width height } tags { name } urls { url } } } }",
        "variables": {
            "title": query
        }
    });

    let resp = client
        .post(endpoint)
        .header("Content-Type", "application/json")
        .header("ApiKey", api_key)
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("http_{}", status.as_u16()));
    }
    let data = resp
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    if data.get("errors").is_some() {
        return Err("graphql_errors".to_string());
    }

    let first = data
        .get("data")
        .and_then(|v| v.get("queryScenes"))
        .and_then(|v| v.get("scenes"))
        .and_then(|v| v.as_array())
        .and_then(|items| items.first());

    Ok(first.and_then(stashbox_scene_to_remote_metadata))
}

fn stashbox_scene_to_remote_metadata(scene: &serde_json::Value) -> Option<RemoteMetadata> {
    let title = non_empty_string(scene.get("title").and_then(|v| v.as_str()));
    let overview = non_empty_string(scene.get("details").and_then(|v| v.as_str()))
        .or_else(|| non_empty_string(scene.get("description").and_then(|v| v.as_str())));
    let poster_path = first_image_url(scene);
    let year = parse_year_prefix(scene.get("release_date").and_then(|v| v.as_str()))
        .or_else(|| parse_year_prefix(scene.get("date").and_then(|v| v.as_str())));
    let genre = scene
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|tags| {
            tags.iter()
                .filter_map(|tag| tag.get("name").and_then(|v| v.as_str()))
                .filter(|name| !name.trim().is_empty())
                .take(5)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|g| !g.trim().is_empty());

    if title.is_none() && overview.is_none() && poster_path.is_none() {
        return None;
    }

    Some(RemoteMetadata {
        title,
        overview,
        poster_path,
        year,
        rating: None,
        genre,
        tmdb_id: None,
        imdb_id: None,
    })
}

async fn fetch_tmdb_metadata(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
) -> Option<RemoteMetadata> {
    let encoded = percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC);
    let url = format!(
        "https://api.themoviedb.org/3/search/multi?api_key={api_key}&query={encoded}&include_adult=true&page=1"
    );
    let data = client
        .get(url)
        .send()
        .await
        .ok()?
        .json::<serde_json::Value>()
        .await
        .ok()?;
    let first = data.get("results")?.as_array()?.first()?;

    let title = non_empty_string(first.get("title").and_then(|v| v.as_str()))
        .or_else(|| non_empty_string(first.get("name").and_then(|v| v.as_str())));
    let overview = non_empty_string(first.get("overview").and_then(|v| v.as_str()));
    let poster_path = first
        .get("poster_path")
        .and_then(|v| v.as_str())
        .filter(|v| !v.trim().is_empty())
        .map(|p| format!("https://image.tmdb.org/t/p/w500{p}"));
    let year = parse_year_prefix(first.get("release_date").and_then(|v| v.as_str()))
        .or_else(|| parse_year_prefix(first.get("first_air_date").and_then(|v| v.as_str())));
    let rating = first
        .get("vote_average")
        .and_then(|v| v.as_f64())
        .filter(|v| *v > 0.0);
    let tmdb_id = first
        .get("id")
        .and_then(|v| v.as_i64())
        .map(|id| id.to_string());

    Some(RemoteMetadata {
        title,
        overview,
        poster_path,
        year,
        rating,
        genre: None,
        tmdb_id,
        imdb_id: None,
    })
}

async fn fetch_omdb_metadata(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
) -> Option<RemoteMetadata> {
    let encoded = percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC);
    let url = format!("https://www.omdbapi.com/?apikey={api_key}&t={encoded}&plot=full");
    let data = client
        .get(url)
        .send()
        .await
        .ok()?
        .json::<serde_json::Value>()
        .await
        .ok()?;
    if data.get("Response").and_then(|v| v.as_str()) != Some("True") {
        return None;
    }

    let title = non_empty_string(data.get("Title").and_then(|v| v.as_str()));
    let overview = non_empty_string(data.get("Plot").and_then(|v| v.as_str()));
    let poster_path = non_empty_string(data.get("Poster").and_then(|v| v.as_str()));
    let year = parse_year_prefix(data.get("Year").and_then(|v| v.as_str()));
    let rating = data
        .get("imdbRating")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| *v > 0.0);
    let genre = non_empty_string(data.get("Genre").and_then(|v| v.as_str()));
    let imdb_id = non_empty_string(data.get("imdbID").and_then(|v| v.as_str()));

    Some(RemoteMetadata {
        title,
        overview,
        poster_path,
        year,
        rating,
        genre,
        tmdb_id: None,
        imdb_id,
    })
}

async fn fetch_stashdb_metadata(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
) -> Option<RemoteMetadata> {
    fetch_stashbox_metadata(client, "https://stashdb.org/graphql", api_key, query)
        .await
        .ok()
        .flatten()
}

fn merge_remote_metadata(
    primary: Option<RemoteMetadata>,
    secondary: Option<RemoteMetadata>,
) -> Option<RemoteMetadata> {
    let mut merged = primary.or(secondary.clone())?;
    if let Some(extra) = secondary {
        if merged.title.is_none() {
            merged.title = extra.title;
        }
        if merged.overview.is_none() {
            merged.overview = extra.overview;
        }
        if merged.poster_path.is_none() {
            merged.poster_path = extra.poster_path;
        }
        if merged.year.is_none() {
            merged.year = extra.year;
        }
        if merged.rating.is_none() {
            merged.rating = extra.rating;
        }
        if merged.genre.is_none() {
            merged.genre = extra.genre;
        }
        if merged.tmdb_id.is_none() {
            merged.tmdb_id = extra.tmdb_id;
        }
        if merged.imdb_id.is_none() {
            merged.imdb_id = extra.imdb_id;
        }
    }
    Some(merged)
}

fn load_metadata_check_item(
    state: &AppState,
    id: i64,
) -> Result<Option<MetadataCheckItem>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db
        .conn
        .prepare(
            "SELECT mi.id,
                    mi.title,
                    mi.file_path,
                    mi.poster_path,
                    mi.overview,
                    mi.year,
                    mi.rating,
                    mi.genre,
                    mi.tmdb_id,
                    mi.imdb_id,
                    mi.media_type,
                    ms.name,
                    ms.path
             FROM media_items mi
             LEFT JOIN media_sources ms ON ms.id = mi.source_id
             WHERE mi.id = ?1",
        )
        .map_err(|e| e.to_string())?;

    stmt.query_row(params![id], |row| {
        Ok(MetadataCheckItem {
            id: row.get::<_, i64>(0)?,
            title: row.get::<_, String>(1)?,
            file_path: row.get::<_, String>(2)?,
            poster_path: row.get::<_, Option<String>>(3)?,
            overview: row.get::<_, Option<String>>(4)?,
            year: row.get::<_, Option<i32>>(5)?,
            rating: row.get::<_, Option<f64>>(6)?,
            genre: row.get::<_, Option<String>>(7)?,
            tmdb_id: row.get::<_, Option<String>>(8)?,
            imdb_id: row.get::<_, Option<String>>(9)?,
            media_type: row.get::<_, String>(10)?,
            source_name: row.get::<_, Option<String>>(11)?,
            source_path: row.get::<_, Option<String>>(12)?,
        })
    })
    .optional()
    .map_err(|e| e.to_string())
}

fn load_metadata_provider_keys(state: &AppState) -> Result<HashMap<String, String>, String> {
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

    let mut keys = HashMap::new();
    for row in rows.filter_map(|r| r.ok()) {
        let raw_key = row.0.to_lowercase();
        let normalized_key = normalize_provider_key(&raw_key);
        keys.insert(raw_key, row.1.clone());
        keys.insert(normalized_key, row.1);
    }
    Ok(keys)
}

fn blank(value: Option<&String>) -> bool {
    value.map(|v| v.trim().is_empty()).unwrap_or(true)
}

fn string_changed(left: Option<&String>, right: Option<&String>) -> bool {
    left.map(|v| v.as_str()) != right.map(|v| v.as_str())
}

#[tauri::command]
pub async fn check_media_item_metadata(
    state: State<'_, AppState>,
    id: i64,
) -> Result<serde_json::Value, String> {
    let Some(item) = load_metadata_check_item(&state, id)? else {
        return Ok(serde_json::json!({
            "type": "media_item_metadata_check",
            "status": "not_found",
            "id": id,
            "message": "Media item was not found in the library.",
        }));
    };

    let media_path = std::path::Path::new(&item.file_path);
    let file_exists = media_path.exists();
    let adult_match = is_adult_library_item(
        &item.media_type,
        &item.title,
        &item.file_path,
        item.source_name.as_deref(),
        item.source_path.as_deref(),
    );
    let video_candidate = is_adult_gather_candidate(&item.media_type, &item.file_path);

    let provider_keys = load_metadata_provider_keys(&state)?;
    let tmdb_key = provider_keys.get("tmdb").cloned();
    let omdb_key = provider_keys.get("omdb").cloned();
    let stashdb_key = provider_keys.get("stashdb").cloned();
    let tpdb_key = provider_keys.get("tpdb").cloned();
    let porn_site_nuxt_base = provider_keys.get("porn_site_nuxt").cloned();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let mut errors: Vec<String> = Vec::new();
    let mut provider_errors: Vec<String> = Vec::new();
    let mut providers_used: Vec<String> = Vec::new();
    let mut final_title = item.title.clone();
    let mut final_overview = item.overview.clone();
    let mut final_poster = item.poster_path.clone();
    let mut final_year = item.year;
    let mut final_rating = item.rating;
    let mut final_genre = item.genre.clone();
    let mut final_tmdb_id = item.tmdb_id.clone();
    let mut final_imdb_id = item.imdb_id.clone();
    let mut final_media_type = item.media_type.clone();

    if adult_match && final_media_type != "adult" {
        final_media_type = "adult".to_string();
    }

    if file_exists && should_refresh_title_from_embedded(&final_title, &item.file_path) {
        if let Some(embedded_title) = extract_embedded_title(&item.file_path) {
            if !embedded_title.eq_ignore_ascii_case(&final_title) {
                final_title = embedded_title;
            }
        }
    }

    if blank(final_poster.as_ref()) {
        if let Some(local_poster) = detect_local_poster(&item.file_path) {
            final_poster = Some(local_poster);
        } else if file_exists && video_candidate {
            match generated_screenshot_posters(&item.file_path) {
                Ok(Some(screenshot_poster)) => final_poster = Some(screenshot_poster),
                Ok(None) => {}
                Err(e) => errors.push(format!("screenshot poster failed: {e}")),
            }
        }
    }

    let missing_genre = blank(final_genre.as_ref());
    let needs_remote_metadata = blank(final_overview.as_ref())
        || should_prefer_remote_poster(final_poster.as_deref())
        || final_year.is_none()
        || final_rating.is_none()
        || missing_genre
        || blank(final_tmdb_id.as_ref())
        || blank(final_imdb_id.as_ref());

    if needs_remote_metadata {
        let query_title = if file_exists {
            extract_embedded_title(&item.file_path)
                .filter(|embedded| !embedded.trim().is_empty())
                .unwrap_or_else(|| final_title.clone())
        } else {
            final_title.clone()
        };

        let mut remote_meta: Option<RemoteMetadata> = None;
        if adult_match {
            if let Some(key) = tpdb_key.as_deref().filter(|key| !key.trim().is_empty()) {
                match fetch_tpdb_metadata(&client, key, &query_title).await {
                    Ok(result) => {
                        if result.is_some() {
                            providers_used.push("tpdb".to_string());
                        }
                        remote_meta = merge_remote_metadata(remote_meta, result);
                    }
                    Err(err) => provider_errors.push(format!("tpdb: {err}")),
                }
            }
            if let Some(key) = stashdb_key.as_deref().filter(|key| !key.trim().is_empty()) {
                let result = fetch_stashdb_metadata(&client, key, &query_title).await;
                if result.is_some() {
                    providers_used.push("stashdb".to_string());
                }
                remote_meta = merge_remote_metadata(remote_meta, result);
            }
            match fetch_porn_site_nuxt_metadata(
                &client,
                porn_site_nuxt_base.as_deref(),
                &query_title,
            )
            .await
            {
                Ok(result) => {
                    if result.is_some() {
                        providers_used.push("porn_site_nuxt".to_string());
                    }
                    remote_meta = merge_remote_metadata(remote_meta, result);
                }
                Err(err) => provider_errors.push(format!("porn_site_nuxt: {err}")),
            }
            let phoenix_meta =
                phoenix_adult_local_metadata(&query_title, &item.file_path, final_poster.clone());
            if phoenix_meta.is_some() {
                providers_used.push("phoenixadult".to_string());
            }
            remote_meta = merge_remote_metadata(remote_meta, phoenix_meta);
        }

        if let Some(key) = tmdb_key.as_deref().filter(|key| !key.trim().is_empty()) {
            let result = fetch_tmdb_metadata(&client, key, &query_title).await;
            if result.is_some() {
                providers_used.push("tmdb".to_string());
            }
            remote_meta = merge_remote_metadata(remote_meta, result);
        }
        if let Some(key) = omdb_key.as_deref().filter(|key| !key.trim().is_empty()) {
            let result = fetch_omdb_metadata(&client, key, &query_title).await;
            if result.is_some() {
                providers_used.push("omdb".to_string());
            }
            remote_meta = merge_remote_metadata(remote_meta, result);
        }

        if let Some(meta) = remote_meta {
            if should_refresh_title_from_embedded(&item.title, &item.file_path) {
                if let Some(new_title) = meta.title.filter(|v| !v.trim().is_empty()) {
                    if !new_title.eq_ignore_ascii_case(&final_title) {
                        final_title = new_title;
                    }
                }
            }
            if blank(final_overview.as_ref()) {
                if let Some(new_overview) = meta.overview.filter(|v| !v.trim().is_empty()) {
                    final_overview = Some(new_overview);
                }
            }
            if should_prefer_remote_poster(final_poster.as_deref()) {
                if let Some(new_poster) = meta.poster_path.filter(|v| !v.trim().is_empty()) {
                    final_poster = Some(new_poster);
                }
            }
            if final_year.is_none() {
                final_year = meta.year;
            }
            if final_rating.is_none() {
                final_rating = meta.rating;
            }
            if blank(final_genre.as_ref()) {
                final_genre = meta.genre.filter(|v| !v.trim().is_empty());
            }
            if blank(final_tmdb_id.as_ref()) {
                final_tmdb_id = meta.tmdb_id.filter(|v| !v.trim().is_empty());
            }
            if blank(final_imdb_id.as_ref()) {
                final_imdb_id = meta.imdb_id.filter(|v| !v.trim().is_empty());
            }
        }
    }

    let mut changed_fields = 0usize;
    if final_title != item.title {
        changed_fields += 1;
    }
    if string_changed(final_overview.as_ref(), item.overview.as_ref()) {
        changed_fields += 1;
    }
    if string_changed(final_poster.as_ref(), item.poster_path.as_ref()) {
        changed_fields += 1;
    }
    if final_year != item.year {
        changed_fields += 1;
    }
    if final_rating != item.rating {
        changed_fields += 1;
    }
    if string_changed(final_genre.as_ref(), item.genre.as_ref()) {
        changed_fields += 1;
    }
    if string_changed(final_tmdb_id.as_ref(), item.tmdb_id.as_ref()) {
        changed_fields += 1;
    }
    if string_changed(final_imdb_id.as_ref(), item.imdb_id.as_ref()) {
        changed_fields += 1;
    }
    if final_media_type != item.media_type {
        changed_fields += 1;
    }

    if changed_fields > 0 {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.conn
            .execute(
                "UPDATE media_items
                 SET title = ?1,
                     overview = ?2,
                     poster_path = ?3,
                     year = ?4,
                     rating = ?5,
                     genre = ?6,
                     tmdb_id = ?7,
                     imdb_id = ?8,
                     media_type = ?9
                 WHERE id = ?10",
                params![
                    final_title,
                    final_overview,
                    final_poster,
                    final_year,
                    final_rating,
                    final_genre,
                    final_tmdb_id,
                    final_imdb_id,
                    final_media_type,
                    item.id
                ],
            )
            .map_err(|e| e.to_string())?;
    }

    let mut sidecars_written = 0usize;
    if file_exists {
        match write_metadata_sidecar(
            &item.file_path,
            &final_title,
            final_overview.as_ref(),
            final_poster.as_ref(),
            final_year,
            final_rating,
            final_genre.as_ref(),
            final_tmdb_id.as_ref(),
            final_imdb_id.as_ref(),
        ) {
            Ok(true) => sidecars_written += 1,
            Ok(false) => {}
            Err(e) => errors.push(format!("sidecar write failed: {e}")),
        }
    } else {
        errors.push(
            "media file is currently unavailable; sidecar and screenshot poster were skipped"
                .to_string(),
        );
    }

    Ok(serde_json::json!({
        "type": "media_item_metadata_check",
        "status": "success",
        "id": item.id,
        "title": final_title,
        "adult_match": adult_match,
        "file_exists": file_exists,
        "providers_used": providers_used,
        "phoenixadult_manifest": if adult_match { phoenix_adult_manifest_summary() } else { serde_json::Value::Null },
        "theporndb_config": if adult_match { theporndb_provider_manifest_summary() } else { serde_json::Value::Null },
        "metadata_items_enriched": if changed_fields > 0 { 1 } else { 0 },
        "metadata_fields_updated": changed_fields,
        "posters_updated": if string_changed(final_poster.as_ref(), item.poster_path.as_ref()) { 1 } else { 0 },
        "sidecars_written": sidecars_written,
        "provider_errors": provider_errors,
        "errors": errors,
        "message": if changed_fields > 0 {
            "Metadata checked and updated for this item."
        } else {
            "Metadata checked for this item; no field changes were needed."
        },
    }))
}

async fn gather_adult_metadata_assets(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
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

async fn gather_adult_metadata_assets_inner(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let (configured_adult_providers, unsupported_adult_providers): (Vec<String>, Vec<String>) = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let mut stmt = db
            .conn
            .prepare("SELECT provider FROM api_keys")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        let mut providers = BTreeSet::new();

        for provider in rows.filter_map(|r| r.ok()) {
            let normalized = normalize_adult_provider_key(&provider);
            if matches!(
                normalized.as_str(),
                "tpdb" | "stashdb" | "phoenixadult" | "iafd" | "porn_site_nuxt"
            ) {
                providers.insert(normalized);
            }
        }
        providers.insert("porn_site_nuxt".to_string());

        let mut configured = Vec::new();
        let unsupported = Vec::new();
        for provider in providers {
            configured.push(provider);
        }

        (configured, unsupported)
    };

    let provider_keys: HashMap<String, String> = {
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

        let mut keys = HashMap::new();
        for row in rows.filter_map(|r| r.ok()) {
            let raw_key = row.0.to_lowercase();
            let normalized_key = normalize_provider_key(&raw_key);
            keys.insert(raw_key, row.1.clone());
            keys.insert(normalized_key, row.1);
        }
        keys
    };
    let tmdb_key = provider_keys.get("tmdb").cloned();
    let omdb_key = provider_keys.get("omdb").cloned();
    let stashdb_key = provider_keys.get("stashdb").cloned();
    let tpdb_key = provider_keys.get("tpdb").cloned();
    let porn_site_nuxt_base = provider_keys.get("porn_site_nuxt").cloned();

    let media_items: Vec<(
        i64,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<i32>,
        Option<f64>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
    )> = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let mut stmt = db
            .conn
            .prepare(
                "SELECT mi.id,
                    mi.title,
                    mi.file_path,
                    mi.poster_path,
                    mi.overview,
                    mi.year,
                    mi.rating,
                    mi.genre,
                    mi.tmdb_id,
                    mi.imdb_id,
                    mi.media_type,
                    ms.name,
                    ms.path
             FROM media_items mi
             LEFT JOIN media_sources ms ON ms.id = mi.source_id
             WHERE mi.media_type IN ('adult', 'movie', 'video')
             ORDER BY mi.date_added DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<i32>>(5)?,
                    row.get::<_, Option<f64>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, Option<String>>(11)?,
                    row.get::<_, Option<String>>(12)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok())
            .filter(
                |(
                    _,
                    title,
                    file_path,
                    _,
                    _,
                    _,
                    _,
                    _,
                    _,
                    _,
                    media_type,
                    source_name,
                    source_path,
                )| {
                    is_adult_library_item(
                        media_type,
                        title,
                        file_path,
                        source_name.as_deref(),
                        source_path.as_deref(),
                    )
                },
            )
            .collect()
    };

    let mut posters_updated = 0usize;
    let mut screenshot_posters_generated = 0usize;
    let mut chapters_generated_for_items = 0usize;
    let mut chapter_images_generated = 0usize;
    let mut items_needing_metadata = 0usize;
    let mut items_reclassified_as_adult = 0usize;
    let mut titles_refreshed_from_embedded = 0usize;
    let mut metadata_items_enriched = 0usize;
    let mut metadata_fields_updated = 0usize;
    let mut sidecars_written = 0usize;
    let mut skipped_missing_files = 0usize;
    let mut skipped_non_video_items = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut provider_errors: Vec<String> = Vec::new();
    let mut disabled_providers: BTreeSet<String> = BTreeSet::new();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let mut progress = task_progress::MetadataTaskGuard::start(
        "adult_metadata_gather",
        "Adult Metadata Gather",
        media_items.len(),
        "Preparing adult metadata gather",
    );

    for (
        index,
        (
            id,
            title,
            file_path,
            poster_path,
            overview,
            year,
            rating,
            genre,
            tmdb_id,
            imdb_id,
            media_type,
            source_name,
            source_path,
        ),
    ) in media_items.iter().enumerate()
    {
        progress.update(
            index + 1,
            format!(
                "Gathering metadata and poster artwork for {} of {}",
                index + 1,
                media_items.len()
            ),
        );
        let media_path = std::path::Path::new(file_path);
        if !media_path.exists() {
            skipped_missing_files += 1;
            continue;
        }
        if !is_adult_library_item(
            media_type,
            title,
            file_path,
            source_name.as_deref(),
            source_path.as_deref(),
        ) {
            skipped_non_video_items += 1;
            continue;
        }

        if media_type != "adult" {
            let db = state.db.lock().map_err(|e| e.to_string())?;
            db.conn
                .execute(
                    "UPDATE media_items SET media_type = 'adult' WHERE id = ?1",
                    params![id],
                )
                .map_err(|e| e.to_string())?;
            items_reclassified_as_adult += 1;
        }

        let mut final_title = title.clone();
        let mut final_overview = overview.clone();
        let mut final_poster = poster_path.clone();
        let mut final_year = *year;
        let mut final_rating = *rating;
        let mut final_genre = genre.clone();
        let mut final_tmdb_id = tmdb_id.clone();
        let mut final_imdb_id = imdb_id.clone();

        if should_refresh_title_from_embedded(&final_title, file_path) {
            if let Some(embedded_title) = extract_embedded_title(file_path) {
                if !embedded_title.eq_ignore_ascii_case(&final_title) {
                    let db = state.db.lock().map_err(|e| e.to_string())?;
                    db.conn
                        .execute(
                            "UPDATE media_items SET title = ?1 WHERE id = ?2",
                            params![embedded_title, id],
                        )
                        .map_err(|e| e.to_string())?;
                    final_title = embedded_title;
                    titles_refreshed_from_embedded += 1;
                }
            }
        }

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
                db.conn
                    .execute(
                        "UPDATE media_items SET poster_path = ?1 WHERE id = ?2",
                        params![local_poster, id],
                    )
                    .map_err(|e| e.to_string())?;
                posters_updated += 1;
                final_poster = Some(local_poster);
            } else {
                match generated_screenshot_posters(file_path) {
                    Ok(Some(screenshot_poster)) => {
                        let db = state.db.lock().map_err(|e| e.to_string())?;
                        db.conn
                            .execute(
                                "UPDATE media_items SET poster_path = ?1 WHERE id = ?2",
                                params![screenshot_poster, id],
                            )
                            .map_err(|e| e.to_string())?;
                        posters_updated += 1;
                        screenshot_posters_generated += 1;
                        final_poster = Some(screenshot_poster);
                    }
                    Ok(None) => {}
                    Err(e) => errors.push(format!("{title}: screenshot poster failed: {e}")),
                }
            }
        }

        let missing_genre = final_genre
            .as_ref()
            .map(|g| g.trim().is_empty())
            .unwrap_or(true);
        let should_upgrade_poster = should_prefer_remote_poster(final_poster.as_deref());
        let needs_remote_metadata = !has_overview
            || should_upgrade_poster
            || final_year.is_none()
            || final_rating.is_none()
            || missing_genre
            || final_tmdb_id
                .as_deref()
                .map(|v| v.trim().is_empty())
                .unwrap_or(true)
            || final_imdb_id
                .as_deref()
                .map(|v| v.trim().is_empty())
                .unwrap_or(true);

        if needs_remote_metadata {
            let query_title = extract_embedded_title(file_path)
                .filter(|embedded| !embedded.trim().is_empty())
                .unwrap_or_else(|| final_title.clone());

            let tpdb_meta = if let Some(key) = tpdb_key
                .as_deref()
                .filter(|_| !disabled_providers.contains("tpdb"))
            {
                match fetch_tpdb_metadata(&client, key, &query_title).await {
                    Ok(result) => result,
                    Err(err) => {
                        provider_errors.push(format!(
                            "tpdb: disabled for this run after provider error: {err}"
                        ));
                        disabled_providers.insert("tpdb".to_string());
                        None
                    }
                }
            } else {
                None
            };
            let stashdb_meta = if let Some(key) = stashdb_key.as_deref() {
                fetch_stashdb_metadata(&client, key, &query_title).await
            } else {
                None
            };
            let tmdb_meta = if let Some(key) = tmdb_key.as_deref() {
                fetch_tmdb_metadata(&client, key, &query_title).await
            } else {
                None
            };
            let omdb_meta = if let Some(key) = omdb_key.as_deref() {
                fetch_omdb_metadata(&client, key, &query_title).await
            } else {
                None
            };
            let porn_site_nuxt_meta = if !disabled_providers.contains("porn_site_nuxt") {
                match fetch_porn_site_nuxt_metadata(
                    &client,
                    porn_site_nuxt_base.as_deref(),
                    &query_title,
                )
                .await
                {
                    Ok(result) => result,
                    Err(err) => {
                        provider_errors.push(format!(
                            "porn_site_nuxt: disabled for this run after provider error: {err}"
                        ));
                        disabled_providers.insert("porn_site_nuxt".to_string());
                        None
                    }
                }
            } else {
                None
            };
            let phoenix_meta =
                phoenix_adult_local_metadata(&query_title, file_path, final_poster.clone());
            let tpdb_then_stash = merge_remote_metadata(tpdb_meta, stashdb_meta);
            let adult_with_nuxt = merge_remote_metadata(tpdb_then_stash, porn_site_nuxt_meta);
            let adult_with_local = merge_remote_metadata(adult_with_nuxt, phoenix_meta);
            let adult_then_tmdb = merge_remote_metadata(adult_with_local, tmdb_meta);
            let remote_meta = merge_remote_metadata(adult_then_tmdb, omdb_meta);

            if let Some(meta) = remote_meta {
                let mut changed_fields = 0usize;

                if should_refresh_title_from_embedded(title, file_path) {
                    if let Some(new_title) = meta.title.filter(|v| !v.trim().is_empty()) {
                        if !new_title.eq_ignore_ascii_case(&final_title) {
                            final_title = new_title;
                            changed_fields += 1;
                        }
                    }
                }
                if final_overview
                    .as_ref()
                    .map(|v| v.trim().is_empty())
                    .unwrap_or(true)
                {
                    if let Some(new_overview) = meta.overview.filter(|v| !v.trim().is_empty()) {
                        final_overview = Some(new_overview);
                        changed_fields += 1;
                    }
                }
                if should_prefer_remote_poster(final_poster.as_deref()) {
                    if let Some(new_poster) = meta.poster_path.filter(|v| !v.trim().is_empty()) {
                        if final_poster.as_deref() != Some(new_poster.as_str()) {
                            final_poster = Some(new_poster);
                            changed_fields += 1;
                        }
                    }
                }
                if final_year.is_none() {
                    if let Some(new_year) = meta.year {
                        final_year = Some(new_year);
                        changed_fields += 1;
                    }
                }
                if final_rating.is_none() {
                    if let Some(new_rating) = meta.rating {
                        final_rating = Some(new_rating);
                        changed_fields += 1;
                    }
                }
                if final_genre
                    .as_ref()
                    .map(|v| v.trim().is_empty())
                    .unwrap_or(true)
                {
                    if let Some(new_genre) = meta.genre.filter(|v| !v.trim().is_empty()) {
                        final_genre = Some(new_genre);
                        changed_fields += 1;
                    }
                }
                if final_tmdb_id
                    .as_ref()
                    .map(|v| v.trim().is_empty())
                    .unwrap_or(true)
                {
                    if let Some(new_tmdb_id) = meta.tmdb_id.filter(|v| !v.trim().is_empty()) {
                        final_tmdb_id = Some(new_tmdb_id);
                        changed_fields += 1;
                    }
                }
                if final_imdb_id
                    .as_ref()
                    .map(|v| v.trim().is_empty())
                    .unwrap_or(true)
                {
                    if let Some(new_imdb_id) = meta.imdb_id.filter(|v| !v.trim().is_empty()) {
                        final_imdb_id = Some(new_imdb_id);
                        changed_fields += 1;
                    }
                }

                if changed_fields > 0 {
                    let db = state.db.lock().map_err(|e| e.to_string())?;
                    db.conn
                        .execute(
                            "UPDATE media_items
                         SET title = ?1,
                             overview = ?2,
                             poster_path = ?3,
                             year = ?4,
                             rating = ?5,
                             genre = ?6,
                             tmdb_id = ?7,
                             imdb_id = ?8
                         WHERE id = ?9",
                            params![
                                final_title,
                                final_overview,
                                final_poster,
                                final_year,
                                final_rating,
                                final_genre,
                                final_tmdb_id,
                                final_imdb_id,
                                id
                            ],
                        )
                        .map_err(|e| e.to_string())?;
                    metadata_items_enriched += 1;
                    metadata_fields_updated += changed_fields;
                }
            }
        }

        if let Some(chapter_dir) = chapter_dir_for(file_path) {
            let existing = count_existing_chapter_images(&chapter_dir);
            if existing == 0 {
                match crate::chapters::generate_chapter_thumbs(
                    file_path.clone(),
                    None,
                    Some(300),
                    None,
                )
                .await
                {
                    Ok(thumbs) if !thumbs.is_empty() => {
                        chapters_generated_for_items += 1;
                        chapter_images_generated += thumbs.len();
                    }
                    Ok(_) => {}
                    Err(e) => errors.push(format!("{title}: {e}")),
                }
            }
        }

        match write_metadata_sidecar(
            file_path,
            &final_title,
            final_overview.as_ref(),
            final_poster.as_ref(),
            final_year,
            final_rating,
            final_genre.as_ref(),
            final_tmdb_id.as_ref(),
            final_imdb_id.as_ref(),
        ) {
            Ok(true) => sidecars_written += 1,
            Ok(false) => {}
            Err(e) => errors.push(format!("{title}: sidecar write failed: {e}")),
        }
    }

    progress.finish(format!(
        "Adult metadata gather complete: {posters_updated} posters updated, {sidecars_written} sidecars written"
    ));

    Ok(serde_json::json!({
        "type": "adult_metadata_gather",
        "status": "success",
        "configured_adult_providers": configured_adult_providers,
        "unsupported_adult_providers": unsupported_adult_providers,
        "provider_count": configured_adult_providers.len(),
        "items_scanned": media_items.len(),
        "items_reclassified_as_adult": items_reclassified_as_adult,
        "titles_refreshed_from_embedded": titles_refreshed_from_embedded,
        "metadata_items_enriched": metadata_items_enriched,
        "metadata_fields_updated": metadata_fields_updated,
        "sidecars_written": sidecars_written,
        "posters_updated": posters_updated,
        "generated_screenshot_posters": screenshot_posters_generated,
        "chapter_sets_generated": chapters_generated_for_items,
        "chapter_images_generated": chapter_images_generated,
        "chapter_generation_skipped_after_limit": 0,
        "items_needing_metadata": items_needing_metadata,
        "skipped_missing_files": skipped_missing_files,
        "skipped_non_video_items": skipped_non_video_items,
        "provider_errors": provider_errors,
        "note": "Adult metadata gather now supports legacy provider-key aliases, uses ThePornDB/StashDB/Porn Site Nuxt/TMDb/OMDb metadata when available, keeps PhoenixAdult as a local adult metadata fallback, generates screenshot posters from files when no sidecar/embedded poster exists, writes sidecar files, and generates chapter images without a hard item cap.",
        "errors": errors,
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        classify_ai_query_prompt, is_adult_gather_candidate, is_adult_library_item,
        metadata_sidecar_path, normalize_adult_provider_key, normalize_provider_key,
        phoenix_adult_local_metadata, porn_site_nuxt_entry_to_remote_metadata,
        provider_live_check_supported, should_prefer_remote_poster, tpdb_scene_to_remote_metadata,
        AiQueryRoute,
    };

    #[test]
    fn accepts_real_video_candidates_for_adult_gather() {
        assert!(is_adult_gather_candidate("adult", r"E:\Videos\scene.mp4"));
        assert!(is_adult_gather_candidate("movie", r"E:\Videos\scene.mkv"));
    }

    #[test]
    fn adult_metadata_prompt_routes_to_adult_gather_before_generic_media_checks() {
        assert_eq!(
            classify_ai_query_prompt("Run adult metadata gather for installed providers and generate posters and chapter images"),
            AiQueryRoute::AdultMetadataGather
        );
    }

    #[test]
    fn rejects_generated_images_and_non_video_assets_for_adult_gather() {
        assert!(!is_adult_gather_candidate(
            "photo",
            r"E:\Videos\scene_chapters\chapter_0001.jpg"
        ));
        assert!(!is_adult_gather_candidate("photo", r"E:\Videos\poster.jpg"));
    }

    #[test]
    fn treats_items_from_adult_named_sources_as_adult_library_candidates() {
        assert!(is_adult_library_item(
            "movie",
            "2024-08-31 141950",
            r"E:\Personal Vids X\Media\2024-08-31\2024-08-31_141950.mp4",
            Some("Personal Vids X"),
            Some(r"E:\Personal Vids X")
        ));
        assert!(is_adult_library_item(
            "movie",
            "clip",
            r"D:\Library\clip.mp4",
            Some("Personal X Library"),
            Some(r"D:\Personal X Library")
        ));
    }

    #[test]
    fn normalizes_theporndb_alias_for_adult_provider_detection() {
        assert_eq!(normalize_adult_provider_key("theporndb"), "tpdb");
        assert_eq!(normalize_adult_provider_key("tpdb"), "tpdb");
        assert_eq!(normalize_adult_provider_key("stashdb"), "stashdb");
        assert_eq!(
            normalize_adult_provider_key("Phoenix Adult"),
            "phoenixadult"
        );
        assert_eq!(normalize_adult_provider_key("IreneHub"), "porn_site_nuxt");
    }

    #[test]
    fn normalizes_legacy_provider_aliases_for_backward_compatibility() {
        assert_eq!(normalize_provider_key("themoviedb_images"), "tmdb");
        assert_eq!(normalize_provider_key("tmdb_images"), "tmdb");
        assert_eq!(normalize_provider_key("theporndb"), "tpdb");
        assert_eq!(normalize_provider_key("Phoenix_Adult"), "phoenixadult");
        assert_eq!(normalize_provider_key("pornhub-irene"), "porn_site_nuxt");
    }

    #[test]
    fn provider_checks_cover_live_fetchers_and_local_adult_fallbacks() {
        assert!(provider_live_check_supported("theporndb"));
        assert!(provider_live_check_supported("stashdb"));
        assert!(provider_live_check_supported("phoenixadult"));
        assert!(provider_live_check_supported("porn_site_nuxt"));
    }

    #[test]
    fn parses_porn_site_nuxt_entries_into_writeable_metadata() {
        let entry = serde_json::json!({
            "key": { "kind": "PornEntry", "id": "abc123" },
            "name": "Studio Scene Title",
            "sourceUrl": "https://example.test/watch/abc123",
            "duration": 721,
            "publishDate": 1704067200000_i64,
            "rating": 96,
            "thumb": "https://cdn.example.test/poster.jpg"
        });

        let metadata = porn_site_nuxt_entry_to_remote_metadata(&entry)
            .expect("PornEntry should parse into metadata");

        assert_eq!(metadata.title.as_deref(), Some("Studio Scene Title"));
        assert_eq!(
            metadata.poster_path.as_deref(),
            Some("https://cdn.example.test/poster.jpg")
        );
        assert_eq!(metadata.genre.as_deref(), Some("Adult"));
        assert_eq!(metadata.rating, Some(9.6));
        assert!(metadata
            .overview
            .as_deref()
            .unwrap_or("")
            .contains("example.test/watch/abc123"));
    }

    #[test]
    fn phoenix_adult_local_metadata_cleans_filename_title_and_year() {
        let metadata = phoenix_adult_local_metadata(
            "Studio.Scene.2024.1080p.x264",
            r"E:\Adult\Studio.Scene.2024.1080p.x264.mp4",
            Some(r"E:\Adult\poster.jpg".to_string()),
        )
        .expect("local metadata should be derived from the file name");

        assert_eq!(metadata.title.as_deref(), Some("Studio Scene 2024"));
        assert_eq!(metadata.year, Some(2024));
        assert_eq!(metadata.genre.as_deref(), Some("Adult"));
        assert_eq!(
            metadata.poster_path.as_deref(),
            Some(r"E:\Adult\poster.jpg")
        );
    }

    #[test]
    fn parses_theporndb_scene_metadata_with_poster() {
        let scene = serde_json::json!({
            "title": "Sample Scene",
            "description": "Scene overview",
            "poster": "https://img.example/poster.jpg",
            "date": "2024-03-14",
            "tags": [
                { "name": "Featured" },
                { "name": "HD" }
            ]
        });

        let metadata = tpdb_scene_to_remote_metadata(&scene).expect("scene should parse");

        assert_eq!(metadata.title.as_deref(), Some("Sample Scene"));
        assert_eq!(metadata.overview.as_deref(), Some("Scene overview"));
        assert_eq!(
            metadata.poster_path.as_deref(),
            Some("https://img.example/poster.jpg")
        );
        assert_eq!(metadata.year, Some(2024));
        assert_eq!(metadata.genre.as_deref(), Some("Featured, HD"));
    }

    #[test]
    fn derives_sidecar_path_next_to_media_file() {
        let path = metadata_sidecar_path(r"E:\Adult\scene-01.mp4")
            .expect("sidecar path should resolve")
            .to_string_lossy()
            .replace('/', "\\");
        assert!(path.ends_with(r"E:\Adult\scene-01.cinavault.json"));
    }

    #[test]
    fn remote_poster_preferred_for_local_placeholder_files_only() {
        assert!(should_prefer_remote_poster(None));
        assert!(should_prefer_remote_poster(Some(
            r"E:\Library\video-poster.jpg"
        )));
        assert!(should_prefer_remote_poster(Some(r"E:\Library\poster.jpg")));
        assert!(!should_prefer_remote_poster(Some(
            "https://example.com/poster.jpg"
        )));
    }
}

#[tauri::command]
pub fn set_hf_token(state: State<AppState>, token: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.set_setting_data("hf_token", &token)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_ai_config(state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let model = db
        .get_setting_data("ai_model")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
    let has_token = db
        .get_setting_data("hf_token")
        .map_err(|e| e.to_string())?
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
    db.set_setting_data("ai_model", &model)
        .map_err(|e| e.to_string())
}
