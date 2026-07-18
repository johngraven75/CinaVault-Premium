// CinaVault Premium — shared Tauri v2 backend for desktop and Android
// Build 168 feature surface retained across supported platforms.

mod db;
mod iptv;
mod jellyfin;
mod player;
mod plugins;
mod plugin_configs;
mod scanner;
mod metadata {
    include!(concat!(env!("OUT_DIR"), "/metadata_without_commands.rs"));
}
mod adult_site_provider;
mod ai;
mod ai_automation;
mod chapters;
mod cloud_storage;
mod downloads;
mod duplicates;
mod enrichment;
mod library_artifacts;
mod media_tools;
mod metadata_bridge;
mod metadata_ext;
#[cfg(test)]
mod metadata_posting_tests;
mod nas_devices;
mod pgma_bridge;
mod task_progress;
mod vpn;

use db::Database;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Database>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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

            let plugin_dirs = ["jellyfin", "emby", "plex", "native"];
            for dir in &plugin_dirs {
                std::fs::create_dir_all(app_dir.join("plugins").join(dir)).ok();
            }

            let db_path = app_dir.join("cinavault.db");
            let database = Database::new(db_path.to_str().unwrap())
                .expect("Failed to initialize database");
            app.manage(AppState { db: Mutex::new(database) });
            log::info!("CinaVault Premium Build 168 initialized successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
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
            db::get_media_items,
            db::get_media_item,
            db::add_media_item,
            db::update_media_item,
            db::delete_media_item,
            db::purge_photo_items,
            db::search_media,
            db::get_recent_media,
            db::get_unverified_media,
            db::verify_media_item,
            db::get_sources,
            db::add_source,
            db::remove_source,
            scanner::scan_sources,
            scanner::scan_single_source,
            scanner::discover_media_sources,
            scanner::get_scan_progress,
            scanner::cancel_scan,
            scanner::apply_embedded_titles,
            duplicates::find_duplicates,
            duplicates::get_duplicate_groups,
            duplicates::remove_duplicate,
            iptv::add_xtream_profile,
            iptv::get_xtream_profiles,
            iptv::remove_xtream_profile,
            iptv::sync_xtream_streams,
            iptv::sync_epg,
            iptv::get_live_channels,
            iptv::play_channel,
            jellyfin::start_server,
            jellyfin::stop_server,
            jellyfin::get_server_status,
            jellyfin::get_server_info,
            jellyfin::import_libraries,
            jellyfin::check_emby_compat,
            jellyfin::open_admin_page,
            plugins::get_plugin_repos,
            plugins::add_plugin_repo,
            plugins::remove_plugin_repo,
            plugins::sync_plugin_catalog,
            plugins::get_plugin_catalog,
            plugins::install_plugin,
            plugins::uninstall_plugin,
            plugins::run_plugin,
            plugins::get_installed_plugins,
            plugin_configs::ensure_plugin_config_files,
            pgma_bridge::find_local_candidates,
            pgma_bridge::refresh_pgma_library,
            player::play_media,
            player::get_available_players,
            player::set_default_player,
            metadata_ext::fetch_metadata,
            metadata_ext::search_metadata,
            metadata_ext::check_media_item_metadata,
            metadata_ext::get_provider_status,
            metadata_ext::test_api_key,
            metadata_ext::set_api_key,
            metadata_ext::get_api_keys,
            metadata_ext::get_metadata_providers,
            chapters::generate_chapter_thumbs,
            chapters::get_chapter_thumbs,
            downloads::start_download,
            downloads::start_playlist_download,
            downloads::get_download_progress,
            downloads::cancel_download,
            downloads::install_download_tools,
            downloads::check_download_tools,
            media_tools::get_media_tools_status,
            media_tools::ensure_media_tools,
            media_tools::inspect_with_mediainfo,
            media_tools::inspect_with_mkvtoolnix,
            vpn::vpn_connect,
            vpn::vpn_disconnect,
            vpn::vpn_status,
            vpn::run_antivirus_scan,
            vpn::update_av_signatures,
            vpn::install_security_tools,
            ai::ai_query,
            ai::ai_inference,
            ai::set_hf_token,
            ai::ensure_hf_token,
            ai::get_ai_config,
            ai::set_ai_model,
            ai_automation::ai_library_manage,
            enrichment::run_library_enrichment,
            enrichment::gather_adult_metadata,
            task_progress::get_metadata_task_progress,
            cloud_storage::cloud_auth_start,
            cloud_storage::cloud_disconnect,
            cloud_storage::cloud_sync,
            cloud_storage::cloud_browse,
            cloud_storage::cloud_list_files,
            cloud_storage::cloud_get_status,
            nas_devices::synology_connect,
            nas_devices::synology_disconnect,
            nas_devices::synology_get_status,
            nas_devices::synology_add_library,
            nas_devices::wd_mycloud_connect,
            nas_devices::wd_mycloud_disconnect,
            nas_devices::wd_mycloud_get_status,
            nas_devices::wd_mycloud_add_library,
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
        "version": "1.6.8",
        "build": "168",
        "edition": "Premium"
    })
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    open::that(url).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_system_info() -> serde_json::Value {
    serde_json::json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "family": std::env::consts::FAMILY
    })
}

#[tauri::command]
fn pick_folder() -> Result<Option<String>, String> {
    Ok(None)
}

#[tauri::command]
fn pick_file() -> Result<Option<String>, String> {
    Ok(None)
}
