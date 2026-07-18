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
