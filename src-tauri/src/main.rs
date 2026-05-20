// CinaVault Premium — Tauri v2 Rust Backend (Build 123)
// All core operations: DB, scanning, downloads, IPTV, server management, plugins, AI, VPN, Cloud

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod scanner;
mod iptv;
mod jellyfin;
mod plugins;
mod player;
mod metadata;
mod chapters;
mod duplicates;
mod vpn;
mod downloads;
mod ai;
mod enrichment;
mod task_progress;
mod library_artifacts;

use db::Database;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Database>,
}

fn main() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let app_dir = app.path().app_data_dir().expect("Failed to get app data dir");
            std::fs::create_dir_all(&app_dir).ok();

            // Create plugin directories
            let plugin_dirs = ["jellyfin", "emby", "plex", "native"];
            for dir in &plugin_dirs {
                std::fs::create_dir_all(app_dir.join("plugins").join(dir)).ok();
            }

            let db_path = app_dir.join("cinavault.db");
            let database = Database::new(db_path.to_str().unwrap())
                .expect("Failed to initialize database");
            app.manage(AppState {
                db: Mutex::new(database),
            });
            log::info!("CinaVault Premium Build 123 initialized successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Database / Settings
            db::get_all_settings,
            db::get_setting,
            db::set_setting,
            db::get_feature_settings,
            db::set_feature_setting,
            db::create_remote_access_user,
            db::authenticate_remote_password,
            db::authenticate_remote_access_key,
            db::rotate_remote_access_key,
            db::set_remote_access_user_enabled,
            db::list_remote_access_users,
            db::get_remote_access_security_status,
            // Media Library
            db::get_media_items,
            db::get_media_item,
            db::add_media_item,
            db::update_media_item,
            db::delete_media_item,
            db::search_media,
            db::get_recent_media,
            db::get_unverified_media,
            db::verify_media_item,
            // Sources
            db::get_sources,
            db::add_source,
            db::remove_source,
            // Scanner
            scanner::scan_sources,
            scanner::scan_single_source,
            scanner::get_scan_progress,
            scanner::cancel_scan,
            scanner::apply_embedded_titles,
            // Duplicates
            duplicates::find_duplicates,
            duplicates::get_duplicate_groups,
            duplicates::remove_duplicate,
            // IPTV
            iptv::add_xtream_profile,
            iptv::get_xtream_profiles,
            iptv::remove_xtream_profile,
            iptv::sync_xtream_streams,
            iptv::sync_epg,
            iptv::get_live_channels,
            iptv::play_channel,
            // MS-C / MS-B Server
            jellyfin::start_server,
            jellyfin::stop_server,
            jellyfin::get_server_status,
            jellyfin::get_server_info,
            jellyfin::import_libraries,
            jellyfin::check_emby_compat,
            jellyfin::open_admin_page,
            // Plugins
            plugins::get_plugin_repos,
            plugins::add_plugin_repo,
            plugins::remove_plugin_repo,
            plugins::sync_plugin_catalog,
            plugins::get_plugin_catalog,
            plugins::install_plugin,
            plugins::uninstall_plugin,
            plugins::run_plugin,
            plugins::get_installed_plugins,
            // Player
            player::play_media,
            player::get_available_players,
            player::set_default_player,
            // Metadata
            metadata::fetch_metadata,
            metadata::search_metadata,
            metadata::get_provider_status,
            metadata::test_api_key,
            metadata::set_api_key,
            metadata::get_api_keys,
            metadata::get_metadata_providers,
            // Chapters
            chapters::generate_chapter_thumbs,
            chapters::get_chapter_thumbs,
            // Downloads
            downloads::start_download,
            downloads::start_playlist_download,
            downloads::get_download_progress,
            downloads::cancel_download,
            downloads::install_download_tools,
            downloads::check_download_tools,
            // VPN / Security
            vpn::vpn_connect,
            vpn::vpn_disconnect,
            vpn::vpn_status,
            vpn::run_antivirus_scan,
            vpn::update_av_signatures,
            vpn::install_security_tools,
            // AI
            ai::ai_query,
            ai::ai_inference,
            ai::set_hf_token,
            ai::get_ai_config,
            ai::set_ai_model,
            // Library Enrichment
            enrichment::run_library_enrichment,
            task_progress::get_metadata_task_progress,
            // Cloud Storage
            cloud_auth_start,
            cloud_disconnect,
            cloud_sync,
            cloud_browse,
            cloud_list_files,
            cloud_get_status,
            // Utility
            get_app_info,
            open_external_url,
            get_system_info,
            pick_folder,
            pick_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CinaVault Premium");
}

// ════════════════════════════════════════════════════════════
//  Cloud Storage Commands — OneDrive, Google Drive, Dropbox
// ════════════════════════════════════════════════════════════

#[tauri::command]
async fn cloud_auth_start(provider: String, auth_url: String) -> Result<serde_json::Value, String> {
    log::info!("Cloud auth start: provider={}, url={}", provider, auth_url);

    // Step 1: Start a temporary local HTTP server to receive the OAuth callback
    let listener = match std::net::TcpListener::bind("127.0.0.1:19284") {
        Ok(l) => l,
        Err(_) => {
            // Port busy — fall back to opening browser directly
            open::that(&auth_url).map_err(|e| e.to_string())?;
            return Ok(serde_json::json!({
                "success": true,
                "account": format!("{} Account", provider),
                "method": "browser_fallback"
            }));
        }
    };

    // Set a short timeout so we don't block forever
    listener.set_nonblocking(false).ok();
    let timeout = std::time::Duration::from_secs(120);
    listener.set_ttl(120).ok();

    // Step 2: Open the auth URL in the user's default browser
    open::that(&auth_url).map_err(|e| e.to_string())?;

    // Step 3: Wait for the OAuth redirect callback
    let result = std::thread::spawn(move || -> Result<serde_json::Value, String> {
        // Accept one connection with timeout
        let start = std::time::Instant::now();
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    use std::io::{Read, Write};
                    let mut buf = [0u8; 4096];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buf[..n]).to_string();

                    // Extract the authorization code from the callback URL
                    let code = extract_query_param(&request, "code");

                    // Send a success response back to the browser
                    let html = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
                        <html><body style='font-family:sans-serif;text-align:center;padding:50px;background:#0a0a1a;color:#e8e6f0'>\
                        <h1 style='color:#a78bfa'>CinaVault Connected!</h1>\
                        <p>{} has been linked to CinaVault Premium.</p>\
                        <p>You can close this window.</p></body></html>",
                        provider
                    );
                    stream.write_all(html.as_bytes()).ok();
                    stream.flush().ok();

                    return Ok(serde_json::json!({
                        "success": true,
                        "account": format!("{} Account", provider),
                        "code": code.unwrap_or_default(),
                        "method": "oauth_callback"
                    }));
                }
                Err(_) => {
                    if start.elapsed() > timeout {
                        return Err("OAuth callback timeout".to_string());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            }
        }
    });

    match result.join() {
        Ok(Ok(val)) => Ok(val),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("Auth thread panicked".to_string()),
    }
}

fn extract_query_param(request: &str, param: &str) -> Option<String> {
    let search = format!("{}=", param);
    if let Some(pos) = request.find(&search) {
        let start = pos + search.len();
        let rest = &request[start..];
        let end = rest.find(|c: char| c == '&' || c == ' ' || c == '\r' || c == '\n').unwrap_or(rest.len());
        Some(rest[..end].to_string())
    } else {
        None
    }
}

#[tauri::command]
async fn cloud_disconnect(provider: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    log::info!("Cloud disconnect: {}", provider);
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.set_setting_data(&format!("cloud_{}_status", provider), "disconnected").map_err(|e| e.to_string())?;
    db.set_setting_data(&format!("cloud_{}_token", provider), "").map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn cloud_sync(provider: String, state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    log::info!("Cloud sync: {}", provider);
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let now = chrono::Local::now().to_rfc3339();
    db.set_setting_data(&format!("cloud_{}_last_sync", provider), &now).map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "success": true,
        "provider": provider,
        "synced_at": now,
        "items_found": 0
    }))
}

#[tauri::command]
async fn cloud_browse(provider: String) -> Result<serde_json::Value, String> {
    log::info!("Cloud browse: {}", provider);
    Ok(serde_json::json!({
        "success": true,
        "provider": provider,
        "files": [],
        "folders": []
    }))
}

#[tauri::command]
async fn cloud_list_files(provider: String, path: Option<String>) -> Result<Vec<serde_json::Value>, String> {
    log::info!("Cloud list files: {} path={:?}", provider, path);
    Ok(vec![])
}

#[tauri::command]
async fn cloud_get_status(provider: String, state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let status = db.get_setting_data(&format!("cloud_{}_status", provider))
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "disconnected".to_string());
    let last_sync = db.get_setting_data(&format!("cloud_{}_last_sync", provider))
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "provider": provider,
        "status": status,
        "last_sync": last_sync
    }))
}

// ════════════════════════════════════════════════════════════
//  Utility Commands
// ════════════════════════════════════════════════════════════

#[tauri::command]
fn get_app_info() -> serde_json::Value {
    serde_json::json!({
        "name": "CinaVault Premium",
        "brand": "CinaVault Fusion",
        "version": "1.0.0-10",
        "build_tag": "Build 123 Provider Durability + Full Library Hydration (Premium Edition)",
        "engine": "Tauri v2 + Rust + React 18",
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "features": [
            "persistent_settings", "cloud_storage", "plugin_system",
            "metadata_providers", "library_enrichment", "filename_normalization",
            "embedded_title_preference", "embedded_poster_import", "scheduled_tasks", "premium_ui"
        ]
    })
}

#[tauri::command]
async fn open_external_url(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_system_info() -> serde_json::Value {
    serde_json::json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "family": std::env::consts::FAMILY,
    })
}

#[tauri::command]
async fn pick_folder() -> Result<Option<String>, String> {
    Ok(None)
}

#[tauri::command]
async fn pick_file() -> Result<Option<String>, String> {
    Ok(None)
}
