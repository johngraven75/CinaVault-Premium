#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adult_site_provider;
mod ai;
mod ai_automation;
mod atomic_file;
mod build_identity;
mod casting;
mod chapters;
mod cloud_storage;
mod db;
mod downloads;
mod duplicates;
mod embedded_server;
mod enrichment {
    include!(concat!(env!("OUT_DIR"), "/enrichment_atomic.rs"));
}
mod iptv;
mod jellyfin;
mod library_artifacts;
mod media_tools;
mod metadata {
    include!(concat!(env!("OUT_DIR"), "/metadata_without_commands.rs"));
}
mod metadata_bridge;
mod metadata_enrichment_runtime;
mod metadata_ext;
mod metadata_guard {
    include!(concat!(
        env!("OUT_DIR"),
        "/metadata_guard_without_wrapped_commands.rs"
    ));
}
mod metadata_keyless;
mod metadata_provider_config;
#[cfg(test)]
mod metadata_posting_tests;
mod nas_devices;
mod pgma_bridge;
mod player;
mod plugin_configs;
mod plugins;
mod remote_connectivity;
mod scanner;
mod shared_contracts;
mod source_health;
mod task_progress;
mod vpn;
mod vpn_profile_store;

use db::Database;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Database>,
    pub app_data_dir: PathBuf,
}

fn main() {
    env_logger::init();
    let build = build_identity::current();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(move |app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data dir");
            std::fs::create_dir_all(&app_dir).ok();
            std::fs::create_dir_all(app_dir.join("artwork")).ok();

            for directory in ["jellyfin", "emby", "plex", "native"] {
                std::fs::create_dir_all(app_dir.join("plugins").join(directory)).ok();
            }

            let db_path = app_dir.join("cinavault.db");
            let db_path_string = db_path.to_string_lossy().to_string();
            let database = Database::new(&db_path_string).expect("Failed to initialize database");

            metadata_provider_config::configure(app_dir.clone());
            match metadata_provider_config::ensure_registry(&database) {
                Ok(registry) => log::info!(
                    "Metadata provider registry ready: {} providers enabled",
                    registry.providers.len()
                ),
                Err(error) => log::warn!("Metadata provider registry repair failed: {error}"),
            }

            embedded_server::configure(db_path_string);

            let resource_dir = app
                .path()
                .resource_dir()
                .unwrap_or_else(|_| app_dir.clone());
            remote_connectivity::configure(
                resource_dir
                    .join("tools")
                    .join("cloudflared")
                    .join("cloudflared.exe"),
            );

            app.manage(AppState {
                db: Mutex::new(database),
                app_data_dir: app_dir.clone(),
            });
            tauri::async_runtime::spawn(async {
                match embedded_server::start_embedded_server(Some(32400)).await {
                    Ok(status) => {
                        log::info!("Embedded media server ready: {status}");
                        match remote_connectivity::start_remote_connectivity(
                            Some(32400),
                            Some(true),
                            Some(true),
                            Some(true),
                            Some(true),
                        )
                        .await
                        {
                            Ok(connectivity) => {
                                log::info!("Encrypted remote connectivity ready: {connectivity:?}")
                            }
                            Err(error) => {
                                log::warn!("Encrypted remote connectivity unavailable: {error}")
                            }
                        }
                    }
                    Err(error) => log::error!("Embedded media server failed to start: {error}"),
                }
            });

            log::info!(
                "{} {} initialized successfully",
                build.product_name,
                build.display_name
            );
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
            source_health::validate_source_path,
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
            embedded_server::start_embedded_server,
            embedded_server::stop_embedded_server,
            embedded_server::get_embedded_server_status,
            remote_connectivity::start_remote_connectivity,
            remote_connectivity::stop_remote_connectivity,
            remote_connectivity::get_remote_connectivity_status,
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
            metadata_guard::fetch_metadata,
            metadata_guard::search_metadata,
            metadata_enrichment_runtime::check_media_item_metadata,
            metadata_guard::get_provider_status,
            metadata_guard::test_api_key,
            metadata_guard::set_api_key,
            metadata_guard::get_api_keys,
            metadata_guard::get_metadata_providers,
            metadata_provider_config::get_metadata_provider_registry,
            metadata_provider_config::ensure_metadata_provider_registry,
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
            vpn::vpn_import_profile,
            vpn::vpn_profiles,
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
            metadata_enrichment_runtime::run_library_enrichment,
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
            casting::discover_casting_devices,
            casting::connect_casting_device,
            casting::disconnect_casting_device,
            casting::start_casting,
            casting::update_casting_playback,
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
    build_identity::get_current_build_info()
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    open::that(url).map_err(|error| error.to_string())
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

#[tauri::command]
fn get_hf_token() -> String {
    std::env::var("HF_TOKEN").unwrap_or_default()
}
