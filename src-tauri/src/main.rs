// CinaVault Premium — Tauri v2 Rust Backend
// All core operations: DB, scanning, downloads, IPTV, server management, plugins, AI, VPN

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
mod vpnb;
mod avb;

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
            let db_path = app_dir.join("cinavault.db");
            let database = Database::new(db_path.to_str().unwrap())
                .expect("Failed to initialize database");
            app.manage(AppState {
                db: Mutex::new(database),
            });
            log::info!("CinaVault Premium initialized successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Database / Settings
            db::get_all_settings,
            db::get_setting,
            db::set_setting,
            db::set_settings_batch,
            db::get_feature_settings,
            db::set_feature_setting,
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
            // Jellyfin / Emby Server
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
            metadata::batch_search_metadata,
            metadata::run_metadata_correction,
            metadata::replace_media_metadata,
            metadata::replace_media_metadata_batch,
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
            // Built-in VPN
            vpnb::vpnb_status,
            vpnb::vpnb_connect,
            vpnb::vpnb_disconnect,
            vpnb::vpnb_generate_test_config,
            // Built-in Antivirus
            avb::avb_status,
            avb::avb_scan_path,
            avb::avb_update_database,
            avb::avb_install_tools,
            // AI
            ai::ai_query,
            ai::ai_inference,
            ai::set_hf_token,
            ai::get_ai_config,
            ai::set_ai_model,
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

#[tauri::command]
fn get_app_info() -> serde_json::Value {
    serde_json::json!({
        "name": "CinaVault Premium",
        "brand": "CinaVault Emby Fusion",
        "version": "1.0.0-rc3.1",
        "build_tag": "RC3 Build 1",
        "engine": "Tauri v2 + Rust + React",
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
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
    // Handled via tauri-plugin-dialog on frontend
    Ok(None)
}

#[tauri::command]
async fn pick_file() -> Result<Option<String>, String> {
    Ok(None)
}
