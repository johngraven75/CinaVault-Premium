// CinaVault Premium — Jellyfin/Emby Server Management
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    pub url: String,
    pub running: bool,
}

#[tauri::command]
pub async fn start_server(server_type: String) -> Result<serde_json::Value, String> {
    let exe_name = match server_type.as_str() {
        "jellyfin" => "jellyfin.exe",
        "emby" => "EmbyServer.exe",
        _ => return Err("Unknown server type".into()),
    };

    let preparation = prepare_server_startup_config(&server_type);

    // Try common install paths
    let paths = vec![
        format!(
            "C:\\Program Files\\{}",
            if server_type == "jellyfin" {
                "Jellyfin\\Server"
            } else {
                "Emby-Server"
            }
        ),
        format!(
            "C:\\Program Files (x86)\\{}",
            if server_type == "jellyfin" {
                "Jellyfin\\Server"
            } else {
                "Emby-Server"
            }
        ),
    ];

    for path in &paths {
        let exe_path = format!("{}\\{}", path, exe_name);
        if std::path::Path::new(&exe_path).exists() {
            Command::new(&exe_path).spawn().map_err(|e| e.to_string())?;
            return Ok(serde_json::json!({
                "status": "started",
                "server": server_type,
                "path": exe_path,
                "preparation": preparation,
            }));
        }
    }

    Err(format!(
        "{} server executable not found in standard paths",
        server_type
    ))
}

fn prepare_server_startup_config(server_type: &str) -> Vec<String> {
    let mut actions = Vec::new();
    for root in server_data_roots(server_type) {
        if let Some(action) = patch_network_config(&root) {
            actions.push(action);
        }
        actions.extend(disable_heavy_startup_tasks(&root));
    }
    actions
}

fn server_data_roots(server_type: &str) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut push_existing = |path: PathBuf| {
        if path.exists() && !roots.iter().any(|root| root == &path) {
            roots.push(path);
        }
    };

    match server_type {
        "jellyfin" => {
            if let Some(local) = env::var_os("LOCALAPPDATA") {
                push_existing(PathBuf::from(local).join("jellyfin"));
            }
            if let Some(program_data) = env::var_os("PROGRAMDATA") {
                push_existing(PathBuf::from(program_data).join("Jellyfin").join("Server"));
            }
        }
        "emby" => {
            if let Some(app_data) = env::var_os("APPDATA") {
                let app_data = PathBuf::from(app_data);
                push_existing(app_data.join("Emby-Server").join("programdata"));
                push_existing(app_data.join("Emby-Server"));
            }
            if let Some(local) = env::var_os("LOCALAPPDATA") {
                push_existing(PathBuf::from(local).join("Emby-Server"));
            }
            if let Some(program_data) = env::var_os("PROGRAMDATA") {
                push_existing(PathBuf::from(program_data).join("Emby-Server"));
            }
        }
        _ => {}
    }

    roots
}

fn patch_network_config(root: &Path) -> Option<String> {
    let path = root.join("config").join("network.xml");
    let original = fs::read_to_string(&path).ok()?;
    let mut updated = original.clone();
    updated = set_xml_bool(&updated, "AutoDiscovery", false);
    updated = set_xml_bool(&updated, "EnableUPnP", false);

    if updated == original {
        return None;
    }

    if let Err(error) = backup_once(&path).and_then(|_| fs::write(&path, updated.as_bytes())) {
        return Some(format!(
            "Could not update network discovery settings at {}: {}",
            path.display(),
            error
        ));
    }

    Some(format!(
        "Disabled server auto discovery/UPnP at {}",
        path.display()
    ))
}

fn set_xml_bool(content: &str, tag: &str, value: bool) -> String {
    let desired = if value { "true" } else { "false" };
    content
        .replace(
            &format!("<{}>true</{}>", tag, tag),
            &format!("<{}>{}</{}>", tag, desired, tag),
        )
        .replace(
            &format!("<{}>True</{}>", tag, tag),
            &format!("<{}>{}</{}>", tag, desired, tag),
        )
}

fn disable_heavy_startup_tasks(root: &Path) -> Vec<String> {
    let mut actions = Vec::new();
    let data_dir = root.join("data").join("ScheduledTasks");
    let config_dir = root.join("config").join("ScheduledTasks");
    if !data_dir.exists() || !config_dir.exists() {
        return actions;
    }

    let heavy_task_keys = ["RefreshPeople", "RefreshTrickplayImages"];
    let Ok(entries) = fs::read_dir(&data_dir) else {
        return actions;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        let key = value.get("Key").and_then(|v| v.as_str()).unwrap_or("");
        if !heavy_task_keys.contains(&key) {
            continue;
        }
        let Some(id) = value.get("Id").and_then(|v| v.as_str()) else {
            continue;
        };
        if let Some(config_path) = find_scheduled_task_config(&config_dir, id) {
            if let Some(action) = remove_startup_trigger_from_task(&config_path, key) {
                actions.push(action);
            }
        }
    }

    actions
}

fn find_scheduled_task_config(config_dir: &Path, id: &str) -> Option<PathBuf> {
    let normalized_id = id.replace('-', "");
    let entries = fs::read_dir(config_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let stem = path.file_stem()?.to_string_lossy().replace('-', "");
        if stem.eq_ignore_ascii_case(&normalized_id) {
            return Some(path);
        }
    }
    None
}

fn remove_startup_trigger_from_task(path: &Path, task_key: &str) -> Option<String> {
    let original = fs::read_to_string(path).ok()?;
    let updated = remove_startup_trigger_json(&original)?;
    if updated == original {
        return None;
    }

    if let Err(error) = backup_once(path).and_then(|_| fs::write(path, updated.as_bytes())) {
        return Some(format!(
            "Could not update {} startup trigger at {}: {}",
            task_key,
            path.display(),
            error
        ));
    }

    Some(format!(
        "Removed startup trigger for {} at {}",
        task_key,
        path.display()
    ))
}

fn remove_startup_trigger_json(raw: &str) -> Option<String> {
    let mut triggers = serde_json::from_str::<Vec<serde_json::Value>>(raw).ok()?;
    let before = triggers.len();
    triggers.retain(|trigger| {
        trigger
            .get("Type")
            .and_then(|value| value.as_str())
            .map(|kind| kind != "StartupTrigger")
            .unwrap_or(true)
    });
    if triggers.len() == before {
        return Some(raw.to_string());
    }
    serde_json::to_string(&triggers).ok()
}

fn backup_once(path: &Path) -> std::io::Result<()> {
    let file_name = path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "config".to_string());
    let backup = path.with_file_name(format!("{}.cinavault.bak", file_name));
    if !backup.exists() {
        fs::copy(path, backup)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{remove_startup_trigger_json, set_xml_bool};

    #[test]
    fn disables_xml_boolean_settings() {
        let xml = "<NetworkConfiguration><AutoDiscovery>true</AutoDiscovery><EnableUPnP>True</EnableUPnP></NetworkConfiguration>";
        let xml = set_xml_bool(xml, "AutoDiscovery", false);
        let xml = set_xml_bool(&xml, "EnableUPnP", false);
        assert!(xml.contains("<AutoDiscovery>false</AutoDiscovery>"));
        assert!(xml.contains("<EnableUPnP>false</EnableUPnP>"));
    }

    #[test]
    fn removes_only_startup_trigger_from_task_json() {
        let raw = r#"[{"Type":"IntervalTrigger","IntervalTicks":6048000000000},{"Type":"StartupTrigger"}]"#;
        let updated = remove_startup_trigger_json(raw).expect("task json should parse");
        assert!(!updated.contains("StartupTrigger"));
        assert!(updated.contains("IntervalTrigger"));
    }
}

#[tauri::command]
pub async fn stop_server(server_type: String) -> Result<serde_json::Value, String> {
    let process_name = match server_type.as_str() {
        "jellyfin" => "jellyfin",
        "emby" => "EmbyServer",
        _ => return Err("Unknown server type".into()),
    };

    #[cfg(target_os = "windows")]
    {
        Command::new("taskkill")
            .args(["/IM", &format!("{}.exe", process_name), "/F"])
            .output()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("pkill")
            .arg("-f")
            .arg(process_name)
            .output()
            .map_err(|e| e.to_string())?;
    }

    Ok(serde_json::json!({
        "status": "stopped",
        "server": server_type,
    }))
}

#[tauri::command]
pub async fn get_server_status(
    server_type: String,
    base_url: Option<String>,
) -> Result<serde_json::Value, String> {
    let url = base_url.unwrap_or_else(|| match server_type.as_str() {
        "jellyfin" => "http://localhost:8096".to_string(),
        "emby" => "http://localhost:8096".to_string(),
        _ => "http://localhost:8096".to_string(),
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let info_url = format!("{}/System/Info/Public", url);
    match client.get(&info_url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
                Ok(serde_json::json!({
                    "running": true,
                    "server_name": data.get("ServerName").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                    "version": data.get("Version").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                    "url": url,
                    "id": data.get("Id").and_then(|v| v.as_str()),
                }))
            } else {
                Ok(serde_json::json!({ "running": false, "url": url }))
            }
        }
        Err(_) => Ok(serde_json::json!({ "running": false, "url": url })),
    }
}

#[tauri::command]
pub async fn get_server_info(
    base_url: String,
    api_key: Option<String>,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let mut req = client.get(format!("{}/System/Info", base_url));
    if let Some(key) = &api_key {
        req = req.header("X-Emby-Token", key);
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(data)
}

#[tauri::command]
pub async fn import_libraries(
    base_url: String,
    api_key: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/Library/VirtualFolders", base_url))
        .header("X-Emby-Token", &api_key)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let libraries: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;

    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut imported = 0u64;

    for lib in &libraries {
        if let Some(locations) = lib.get("Locations").and_then(|v| v.as_array()) {
            let lib_name = lib
                .get("Name")
                .and_then(|v| v.as_str())
                .unwrap_or("Library");
            for loc in locations {
                if let Some(path) = loc.as_str() {
                    let _ = db.conn.execute(
                        "INSERT OR IGNORE INTO media_sources (path, source_type, name, enabled, item_count) VALUES (?1, 'folder', ?2, 1, 0)",
                        rusqlite::params![path, format!("{} (Imported)", lib_name)],
                    );
                    imported += 1;
                }
            }
        }
    }

    Ok(serde_json::json!({
        "libraries_found": libraries.len(),
        "sources_imported": imported,
    }))
}

#[tauri::command]
pub async fn check_emby_compat(base_url: String) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    match client
        .get(format!("{}/System/Info/Public", base_url))
        .send()
        .await
    {
        Ok(resp) => {
            let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            let version = data.get("Version").and_then(|v| v.as_str()).unwrap_or("");
            let product = data
                .get("ProductName")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Ok(serde_json::json!({
                "compatible": true,
                "product": product,
                "version": version,
                "emby_api": product.to_lowercase().contains("emby"),
                "jellyfin_api": product.to_lowercase().contains("jellyfin"),
            }))
        }
        Err(e) => Ok(serde_json::json!({
            "compatible": false,
            "error": e.to_string(),
        })),
    }
}

#[tauri::command]
pub async fn open_admin_page(base_url: String, page: String) -> Result<(), String> {
    let url = match page.as_str() {
        "dashboard" => format!("{}/web/index.html#!/dashboard", base_url),
        "libraries" => format!("{}/web/index.html#!/libraries", base_url),
        "users" => format!("{}/web/index.html#!/users", base_url),
        "plugins" => format!("{}/web/index.html#!/plugins", base_url),
        "tasks" => format!("{}/web/index.html#!/scheduledtasks", base_url),
        "logs" => format!("{}/web/index.html#!/log", base_url),
        "sessions" => format!("{}/Sessions", base_url),
        "devices" => format!("{}/Devices", base_url),
        _ => format!("{}/web/index.html", base_url),
    };
    open::that(&url).map_err(|e| e.to_string())
}
