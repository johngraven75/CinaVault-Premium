//! "gpu_accel" switch: hardware-accelerated rendering in the app window.
//!
//! The webview reads its browser arguments when it is created, which happens
//! before the database is opened, so the saved switch is read straight from
//! the SQLite file in the app data folder. A change applies on the next start.

use crate::feature_flags;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::path::{Path, PathBuf};

pub const SWITCH: &str = "gpu_accel";
const WEBVIEW2_ARGS: &str = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";
/// Tauri passes these by default; the environment variable replaces Tauri's
/// own arguments, so they are kept here.
const TAURI_DEFAULT_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

/// The app data folder Tauri uses for this identifier.
pub fn app_data_dir() -> Option<PathBuf> {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).ok()?;
    let identifier = config.get("identifier")?.as_str()?;
    Some(dirs::data_dir()?.join(identifier))
}

/// A saved switch read from the database file, if the file and row exist.
pub fn file_is_enabled(db_path: &Path, key: &str) -> Option<bool> {
    if !db_path.is_file() {
        return None;
    }
    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    conn.query_row(
        "SELECT enabled FROM feature_settings WHERE feature_key = ?1",
        [key],
        |row| row.get::<_, bool>(0),
    )
    .optional()
    .ok()
    .flatten()
}

/// Whether to use the GPU: the saved switch, else its default (on).
pub fn gpu_enabled(db_path: &Path) -> bool {
    file_is_enabled(db_path, "gpu_accel").unwrap_or_else(|| feature_flags::default_enabled(SWITCH))
}

/// WebView2 browser arguments for the switch. Off forces software rendering;
/// on keeps hardware acceleration and enables platform HEVC decoding where the
/// system has a decoder.
pub fn webview2_arguments(gpu: bool) -> String {
    if gpu {
        format!("{TAURI_DEFAULT_ARGS} --enable-features=PlatformHEVCDecoderSupport")
    } else {
        format!("{TAURI_DEFAULT_ARGS} --disable-gpu --disable-gpu-compositing")
    }
}

/// Environment for the webview, given what the user already set. Arguments a
/// user put in the variable themselves are kept after ours.
pub fn environment(gpu: bool, existing_webview2: Option<&str>) -> Vec<(&'static str, String)> {
    let mut args = webview2_arguments(gpu);
    if let Some(existing) = existing_webview2.map(str::trim).filter(|v| !v.is_empty()) {
        args.push(' ');
        args.push_str(existing);
    }
    let mut vars = vec![(WEBVIEW2_ARGS, args)];
    if !gpu {
        // WebKitGTK equivalent on Linux builds.
        vars.push(("WEBKIT_DISABLE_COMPOSITING_MODE", "1".into()));
    }
    vars
}

/// Applies the saved switch to this process's environment. Call before the
/// Tauri builder creates any window.
pub fn apply() {
    let gpu = app_data_dir()
        .map(|dir| gpu_enabled(&dir.join("cinavault.db")))
        .unwrap_or(true);
    let existing = std::env::var(WEBVIEW2_ARGS).ok();
    for (name, value) in environment(gpu, existing.as_deref()) {
        if name != WEBVIEW2_ARGS && std::env::var_os(name).is_some() {
            continue;
        }
        std::env::set_var(name, value);
    }
    log::info!(
        "GPU acceleration {} for the app window",
        if gpu {
            "on"
        } else {
            "off (software rendering)"
        }
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_follow_the_switch() {
        let off = webview2_arguments(false);
        assert!(off.contains("--disable-gpu "));
        assert!(off.ends_with("--disable-gpu-compositing"));
        assert!(off.starts_with(TAURI_DEFAULT_ARGS));
        let on = webview2_arguments(true);
        assert!(!on.contains("--disable-gpu"));
        assert!(on.contains("--enable-features=PlatformHEVCDecoderSupport"));
        let env = environment(false, Some("--remote-debugging-port=9222"));
        assert_eq!(env[0].0, WEBVIEW2_ARGS);
        assert!(env[0]
            .1
            .ends_with("--disable-gpu-compositing --remote-debugging-port=9222"));
        assert_eq!(env[1], ("WEBKIT_DISABLE_COMPOSITING_MODE", "1".to_string()));
        assert_eq!(environment(true, None).len(), 1);
    }

    #[test]
    fn saved_switch_is_read_from_the_database_file() {
        let path = std::env::temp_dir().join(format!("cinavault-gpu-{}.db", uuid::Uuid::new_v4()));
        assert_eq!(file_is_enabled(&path, SWITCH), None);
        assert!(gpu_enabled(&path), "defaults on without a database");
        let db = crate::db::Database::new(path.to_str().unwrap()).unwrap();
        assert!(gpu_enabled(&path), "defaults on without a saved row");
        db.set_feature_setting_data(SWITCH, false, "{}").unwrap();
        assert_eq!(file_is_enabled(&path, SWITCH), Some(false));
        assert!(!gpu_enabled(&path));
        drop(db);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn app_data_dir_uses_the_bundle_identifier() {
        if let Some(dir) = app_data_dir() {
            assert!(dir.ends_with("com.cinavault.premium"));
        }
    }
}
