use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Manager};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn profile_directory(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("vpn")
        .join("profiles");
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    restrict_to_current_user(&directory)?;
    Ok(directory)
}

pub fn validate_profile(content: &str) -> Result<(), String> {
    for required in [
        "[Interface]",
        "PrivateKey",
        "[Peer]",
        "PublicKey",
        "Endpoint",
        "AllowedIPs",
    ] {
        if !content.contains(required) {
