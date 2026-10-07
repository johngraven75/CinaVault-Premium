// CinaVault Premium — NAS Device Integration
// Synology QuickConnect + WD My Cloud Home
use crate::AppState;
use serde::{Deserialize, Serialize};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
use std::path::Path;
use std::path::{Component, PathBuf};
#[cfg(target_os = "windows")]
use std::process::Command;
use tauri::State;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NasCredentials {
    pub device_type: String,
    pub host: String,
    pub username: String,
    pub password: String,
    pub port: Option<u16>,
    pub use_https: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NasLibrary {
    pub id: String,
    pub name: String,
    pub path: String,
    pub share_name: String,
    pub media_type: String,
    pub item_count: u64,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NasConnectionResult {
    pub success: bool,
    pub device_name: String,
    pub device_model: String,
    pub firmware: String,
    pub host_resolved: String,
    pub libraries: Vec<NasLibrary>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NasBrowseEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
}

fn http_client(cookie_store: bool) -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .danger_accept_invalid_certs(true)
        .cookie_store(cookie_store)
        .build()
        .map_err(|error| error.to_string())
}

fn resolve_quickconnect(quickconnect_id: &str) -> Result<String, String> {
    let client = http_client(false)?;
    let body = serde_json::json!({
        "version": "1",
        "command": "get_server_info",
        "stop_when_error": false,
        "stop_when_success": false,
        "id": "dsm_portal_https",
        "serverID": quickconnect_id,
        "is_gofile": false
    });

    let response = client
        .post("https://global.quickconnect.to/Serv.php")
        .json(&body)
        .send()
        .map_err(|error| format!("QuickConnect relay error: {error}"))?;
    let json: serde_json::Value = response.json().map_err(|error| error.to_string())?;

    if let Some(hosts) = json
        .get("env")
        .and_then(|value| value.get("relay_region"))
        .and_then(|value| value.get("hosts"))
        .and_then(|value| value.as_array())
    {
        if let Some(host) = hosts
            .iter()
            .filter_map(|value| value.get("host").and_then(|host| host.as_str()))
            .find(|host| !host.trim().is_empty())
        {
            return Ok(host.to_string());
        }
    }

    if let Some(host) = json
        .get("env")
        .and_then(|value| value.get("control_host"))
        .and_then(|value| value.as_str())
        .filter(|host| !host.trim().is_empty())
    {
        return Ok(host.to_string());
    }

    Ok(format!("{}.quickconnect.to", quickconnect_id))
}

fn synology_login(
    host: &str,
    port: u16,
    use_https: bool,
    username: &str,
    password: &str,
) -> Result<String, String> {
    let scheme = if use_https { "https" } else { "http" };
    let url = format!(
        "{}://{}:{}/webapi/auth.cgi?api=SYNO.API.Auth&version=3&method=login&account={}&passwd={}&session=CinaVault&format=sid",
        scheme,
        host,
        port,
        urlencoding_simple(username),
        urlencoding_simple(password)
    );
    let response = http_client(false)?
        .get(&url)
        .send()
        .map_err(|error| format!("Synology login error: {error}"))?;
    let json: serde_json::Value = response.json().map_err(|error| error.to_string())?;

    if json
        .get("success")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        json["data"]["sid"]
            .as_str()
            .filter(|sid| !sid.is_empty())
            .map(ToString::to_string)
            .ok_or_else(|| "Synology login succeeded but returned no session id".to_string())
    } else {
        Err(format!(
            "Synology auth failed (error code {})",
            json["error"]["code"].as_u64().unwrap_or(0)
        ))
    }
}

fn synology_get_shares(
    host: &str,
    port: u16,
    use_https: bool,
    sid: &str,
) -> Result<Vec<NasLibrary>, String> {
    let scheme = if use_https { "https" } else { "http" };
    let url = format!(
        "{}://{}:{}/webapi/entry.cgi?api=SYNO.FileStation.List&version=2&method=list_share&_sid={}",
        scheme, host, port, sid
    );
    let response = http_client(false)?
        .get(&url)
        .send()
        .map_err(|error| error.to_string())?;
    let json: serde_json::Value = response.json().map_err(|error| error.to_string())?;
    if json.get("success").and_then(|value| value.as_bool()) == Some(false) {
        return Err(format!(
            "Synology share listing failed (error code {})",
            json["error"]["code"].as_u64().unwrap_or(0)
        ));
    }

    let libraries = json["data"]["shares"]
        .as_array()
        .map(|shares| {
            shares
                .iter()
                .filter_map(|share| {
                    let name = share["name"].as_str()?.trim().to_string();
                    if name.is_empty() {
                        return None;
                    }
                    let path = share["path"].as_str().unwrap_or("").to_string();
                    let size_bytes = share["additional"]["volume_status"]["totalspace"]
                        .as_u64()
                        .unwrap_or(0);
                    Some(NasLibrary {
                        id: format!("synology-{}", slug(&name)),
                        name: name.clone(),
                        path,
                        share_name: name.clone(),
                        media_type: infer_media_type_from_name(&name),
                        item_count: 0,
                        size_bytes,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(libraries)
}

fn synology_get_info(
    host: &str,
    port: u16,
    use_https: bool,
    sid: &str,
) -> (String, String, String) {
    let scheme = if use_https { "https" } else { "http" };
    let url = format!(
        "{}://{}:{}/webapi/entry.cgi?api=SYNO.DSM.Info&version=2&method=getinfo&_sid={}",
        scheme, host, port, sid
    );
    if let Ok(client) = http_client(false) {
        if let Ok(response) = client.get(&url).send() {
            if let Ok(json) = response.json::<serde_json::Value>() {
                return (
                    json["data"]["hostname"]
                        .as_str()
                        .unwrap_or(host)
                        .to_string(),
                    json["data"]["model"]
                        .as_str()
                        .unwrap_or("Synology NAS")
                        .to_string(),
                    json["data"]["version_string"]
                        .as_str()
                        .unwrap_or("DSM")
                        .to_string(),
                );
            }
        }
    }
    (
        host.to_string(),
        "Synology NAS".to_string(),
        "DSM".to_string(),
    )
}

#[derive(Clone)]
struct WdSession {
    client: reqwest::blocking::Client,
    token: String,
}

fn wd_mycloud_login(
    host: &str,
    port: u16,
    use_https: bool,
    username: &str,
    password: &str,
) -> Result<WdSession, String> {
    let scheme = if use_https { "https" } else { "http" };
    let login_url = format!(
        "{}://{}:{}/api/2.1/rest/users?method=login",
        scheme, host, port
    );
    let client = http_client(true)?;
    let response = client
        .post(&login_url)
        .json(&serde_json::json!({ "username": username, "password": password }))
        .send()
        .map_err(|error| format!("WD My Cloud login error: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("WD My Cloud auth failed (HTTP {status})"));
    }
    let json: serde_json::Value = response.json().map_err(|error| error.to_string())?;
    let token = json["token"]
        .as_str()
        .or_else(|| json["data"]["token"].as_str())
        .or_else(|| json["session_id"].as_str())
        .unwrap_or("authenticated")
        .to_string();
    Ok(WdSession { client, token })
}

fn wd_mycloud_get_shares(
    host: &str,
    port: u16,
    use_https: bool,
    session: &WdSession,
) -> Result<Vec<NasLibrary>, String> {
    let scheme = if use_https { "https" } else { "http" };
    let url = format!("{}://{}:{}/api/2.1/rest/shares", scheme, host, port);
    let response = session
        .client
        .get(&url)
        .header("Authorization", format!("Bearer {}", session.token))
        .send()
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "WD My Cloud share listing failed (HTTP {})",
            response.status()
        ));
    }
    let json: serde_json::Value = response.json().map_err(|error| error.to_string())?;
    let shares = json["shares"]
        .as_array()
        .or_else(|| json["data"].as_array())
        .or_else(|| json.as_array())
        .cloned()
        .unwrap_or_default();
    let mut libraries = shares
        .iter()
        .map(|share| {
            let name = share["name"]
                .as_str()
                .or_else(|| share["share_name"].as_str())
                .unwrap_or("Share")
                .to_string();
            NasLibrary {
                id: format!("wd-{}", slug(&name)),
                name: name.clone(),
                path: share["path"]
                    .as_str()
                    .or_else(|| share["mount_path"].as_str())
                    .unwrap_or("")
                    .to_string(),
                share_name: name.clone(),
                media_type: infer_media_type_from_name(&name),
                item_count: 0,
                size_bytes: share["total_size"]
                    .as_u64()
                    .or_else(|| share["size"].as_u64())
                    .unwrap_or(0),
            }
        })
        .collect::<Vec<_>>();
    if libraries.is_empty() {
        libraries.push(NasLibrary {
            id: "wd-public".to_string(),
            name: "Public".to_string(),
            path: "/Public".to_string(),
            share_name: "Public".to_string(),
            media_type: "mixed".to_string(),
            item_count: 0,
            size_bytes: 0,
        });
    }
    Ok(libraries)
}

fn wd_mycloud_get_info(
    host: &str,
    port: u16,
    use_https: bool,
    session: &WdSession,
) -> (String, String, String) {
    let scheme = if use_https { "https" } else { "http" };
    let url = format!("{}://{}:{}/api/2.1/rest/device", scheme, host, port);
    if let Ok(response) = session
        .client
        .get(&url)
        .header("Authorization", format!("Bearer {}", session.token))
        .send()
    {
        if let Ok(json) = response.json::<serde_json::Value>() {
            return (
                json["name"]
                    .as_str()
                    .or_else(|| json["device_name"].as_str())
                    .unwrap_or("WD My Cloud")
                    .to_string(),
                json["model"]
                    .as_str()
                    .or_else(|| json["device_model"].as_str())
                    .unwrap_or("WD My Cloud")
                    .to_string(),
                json["firmware"]
                    .as_str()
                    .or_else(|| json["firmware_version"].as_str())
                    .unwrap_or("")
                    .to_string(),
            );
        }
    }
    (
        "WD My Cloud".to_string(),
        "WD My Cloud".to_string(),
        "".to_string(),
    )
}

fn slug(name: &str) -> String {
    name.trim().to_lowercase().replace(' ', "-")
}

fn infer_media_type_from_name(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("movie") || lower.contains("film") || lower.contains("cinema") {
        "movies".to_string()
    } else if lower.contains("tv")
        || lower.contains("series")
        || lower.contains("show")
        || lower.contains("episode")
    {
        "tv".to_string()
    } else if lower.contains("music") || lower.contains("audio") || lower.contains("song") {
        "music".to_string()
    } else if lower.contains("photo") || lower.contains("picture") || lower.contains("image") {
        "photos".to_string()
    } else {
        "mixed".to_string()
    }
}

fn urlencoding_simple(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => character.to_string(),
            _ => format!("%{:02X}", character as u32),
        })
        .collect()
}

fn connection_setting_key(device_type: &str) -> Result<&'static str, String> {
    match device_type.trim().to_ascii_lowercase().as_str() {
        "synology" => Ok("synology_connection"),
        "wd" | "wd_mycloud" | "wd-mycloud" => Ok("wd_mycloud_connection"),
        _ => Err("deviceType must be 'synology' or 'wd_mycloud'".to_string()),
    }
}

fn read_connection(
    state: &State<AppState>,
    device_type: &str,
) -> Result<serde_json::Value, String> {
    let key = connection_setting_key(device_type)?;
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let raw = db
        .get_setting_data(key)
        .map_err(|error| error.to_string())?
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{} NAS is not connected", device_type))?;
    serde_json::from_str(&raw).map_err(|error| format!("Stored NAS session is invalid: {error}"))
}

fn network_source_path(host: &str, share_name: &str, share_path: &str) -> String {
    let share = if share_name.trim().is_empty() {
        share_path
            .trim_matches(|character| character == '/' || character == '\\')
            .split(|character| character == '/' || character == '\\')
            .next()
            .unwrap_or("Public")
    } else {
        share_name.trim()
    };
    #[cfg(target_os = "windows")]
    {
        format!(r"\\{}\{}", host, share)
    }
    #[cfg(not(target_os = "windows"))]
    {
        format!("smb://{}/{}", host, share)
    }
}

fn safe_relative_path(relative_path: &str) -> Result<PathBuf, String> {
    let mut clean = PathBuf::new();
    for component in PathBuf::from(relative_path.replace('\\', "/")).components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::RootDir | Component::ParentDir | Component::Prefix(_) => {
                return Err("NAS browse path must stay inside the selected share".to_string())
            }
        }
    }
    Ok(clean)
}

#[cfg(target_os = "windows")]
fn authenticate_windows_shares(
    host: &str,
    libraries: &[NasLibrary],
    username: &str,
    password: &str,
) -> Vec<String> {
    let mut errors = Vec::new();
    for library in libraries {
        let remote = network_source_path(host, &library.share_name, &library.path);
        let user_argument = format!("/user:{username}");
        let mut command = Command::new("net");
        command.args([
            "use",
            remote.as_str(),
            password,
            user_argument.as_str(),
            "/persistent:no",
        ]);
        command.creation_flags(CREATE_NO_WINDOW);
        match command.output() {
            Ok(output) if output.status.success() => {}
            Ok(output) => errors.push(format!(
                "{}: {}",
                library.share_name,
                String::from_utf8_lossy(&output.stderr).trim()
            )),
            Err(error) => errors.push(format!("{}: {error}", library.share_name)),
        }
    }
    errors
}

#[cfg(not(target_os = "windows"))]
fn authenticate_windows_shares(
    _host: &str,
    _libraries: &[NasLibrary],
    _username: &str,
    _password: &str,
) -> Vec<String> {
    Vec::new()
}

fn ensure_network_source_reachable(source_path: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    if !Path::new(source_path).is_dir() {
        return Err(format!(
            "NAS share is not mounted or reachable: {source_path}. Reconnect with the NAS username and password, then try again."
        ));
    }
    Ok(())
}

/// Return the shares from a connected NAS session.
#[tauri::command]
pub fn list_nas_shares(
    state: State<AppState>,
    device_type: String,
) -> Result<Vec<NasLibrary>, String> {
    let connection = read_connection(&state, &device_type)?;
    serde_json::from_value(
        connection
            .get("libraries")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([])),
    )
    .map_err(|error| format!("Stored NAS share list is invalid: {error}"))
}

/// Browse a path inside one connected NAS share. Path traversal outside the share is rejected.
#[tauri::command]
pub fn browse_nas_path(
    state: State<AppState>,
    device_type: String,
    share_name: String,
    relative_path: Option<String>,
) -> Result<Vec<NasBrowseEntry>, String> {
    let connection = read_connection(&state, &device_type)?;
    let host = connection["host"]
        .as_str()
        .filter(|host| !host.trim().is_empty())
        .ok_or_else(|| "Stored NAS connection has no host".to_string())?;
    let libraries: Vec<NasLibrary> = serde_json::from_value(
        connection
            .get("libraries")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([])),
    )
    .map_err(|error| format!("Stored NAS share list is invalid: {error}"))?;
    let library = libraries
        .iter()
        .find(|library| {
            library.share_name.eq_ignore_ascii_case(share_name.trim())
                || library.name.eq_ignore_ascii_case(share_name.trim())
        })
        .ok_or_else(|| format!("NAS share not found: {share_name}"))?;
    let clean_relative = safe_relative_path(relative_path.as_deref().unwrap_or(""))?;

    #[cfg(target_os = "windows")]
    {
        let root = PathBuf::from(network_source_path(
            host,
            &library.share_name,
            &library.path,
        ));
        ensure_network_source_reachable(root.to_string_lossy().as_ref())?;
        let target = root.join(&clean_relative);
        if !target.is_dir() {
            return Err(format!("NAS folder is not reachable: {}", target.display()));
        }
        let mut entries = std::fs::read_dir(&target)
            .map_err(|error| format!("Unable to browse NAS folder {}: {error}", target.display()))?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let metadata = entry.metadata().ok()?;
                let relative = clean_relative.join(entry.file_name());
                Some(NasBrowseEntry {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    path: relative.to_string_lossy().replace('\\', "/"),
                    is_dir: metadata.is_dir(),
                    size_bytes: if metadata.is_file() {
                        metadata.len()
                    } else {
                        0
                    },
                })
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            right
                .is_dir
                .cmp(&left.is_dir)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });
        Ok(entries)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (host, library, clean_relative);
        Err("Direct SMB filesystem browsing is currently available on the Windows desktop build. NAS connection and share discovery remain cross-platform.".to_string())
    }
}

#[tauri::command]
pub fn synology_connect(
    state: State<AppState>,
    quickconnect_id: String,
    username: String,
    password: String,
    use_https: bool,
    port: Option<u16>,
) -> Result<NasConnectionResult, String> {
    let quickconnect_id = quickconnect_id.trim();
    if quickconnect_id.is_empty() || username.trim().is_empty() || password.is_empty() {
        return Err("QuickConnect ID, username, and password are required".to_string());
    }
    log::info!("Synology connect: id={quickconnect_id}");
    let host =
        if quickconnect_id.contains('.') || quickconnect_id.parse::<std::net::IpAddr>().is_ok() {
            quickconnect_id.to_string()
        } else {
            resolve_quickconnect(quickconnect_id)
                .unwrap_or_else(|_| format!("{}.quickconnect.to", quickconnect_id))
        };
    let resolved_port = port.unwrap_or(if use_https { 5001 } else { 5000 });
    let sid = synology_login(&host, resolved_port, use_https, &username, &password)?;
    let (device_name, device_model, firmware) =
        synology_get_info(&host, resolved_port, use_https, &sid);
    let libraries = synology_get_shares(&host, resolved_port, use_https, &sid)?;
    let share_auth_errors = authenticate_windows_shares(&host, &libraries, &username, &password);
    if !share_auth_errors.is_empty() {
        log::warn!(
            "Some Synology shares were not mounted: {}",
            share_auth_errors.join("; ")
        );
    }
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let stored = serde_json::json!({
        "device_type": "synology",
        "host": host,
        "port": resolved_port,
        "use_https": use_https,
        "username": username,
        "sid": sid,
        "device_name": device_name,
        "device_model": device_model,
        "firmware": firmware,
        "libraries": libraries,
        "connected_at": chrono::Utc::now().to_rfc3339()
    });
    db.set_setting_data("synology_connection", &stored.to_string())
        .map_err(|error| error.to_string())?;
    Ok(NasConnectionResult {
        success: true,
        device_name,
        device_model,
        firmware,
        host_resolved: host,
        libraries,
        error: None,
    })
}

#[tauri::command]
pub fn synology_disconnect(state: State<AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.set_setting_data("synology_connection", "")
        .map_err(|error| error.to_string())?;
    log::info!("Synology disconnected");
    Ok(())
}

#[tauri::command]
pub fn synology_get_status(state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let raw = db
        .get_setting_data("synology_connection")
        .map_err(|error| error.to_string())?;
    match raw {
        Some(value) if !value.trim().is_empty() => {
            let data = serde_json::from_str::<serde_json::Value>(&value)
                .map_err(|error| error.to_string())?;
            Ok(serde_json::json!({ "connected": true, "data": data }))
        }
        _ => Ok(serde_json::json!({ "connected": false })),
    }
}

#[tauri::command]
pub fn synology_add_library(
    state: State<AppState>,
    share_name: String,
    share_path: String,
    media_type: String,
) -> Result<(), String> {
    let connection = read_connection(&state, "synology")?;
    let host = connection["host"]
        .as_str()
        .ok_or_else(|| "Stored Synology connection has no host".to_string())?;
    let source_path = network_source_path(host, &share_name, &share_path);
    ensure_network_source_reachable(&source_path)?;
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let source = crate::db::MediaSource {
        id: None,
        path: source_path.clone(),
        source_type: media_type,
        name: share_name.clone(),
        enabled: true,
        last_scanned: None,
        item_count: 0,
    };
    db.add_source_data(&source)
        .map_err(|error| error.to_string())?;
    log::info!("Synology library added: {share_name} -> {source_path}");
    Ok(())
}

#[tauri::command]
pub fn wd_mycloud_connect(
    state: State<AppState>,
    host: String,
    username: String,
    password: String,
    use_https: bool,
    port: Option<u16>,
) -> Result<NasConnectionResult, String> {
    let host = host.trim().to_string();
    if host.is_empty() || username.trim().is_empty() || password.is_empty() {
        return Err("Host/IP, username, and password are required".to_string());
    }
    log::info!("WD My Cloud connect: host={host}");
    let resolved_port = port.unwrap_or(if use_https { 443 } else { 80 });
    let session = wd_mycloud_login(&host, resolved_port, use_https, &username, &password)?;
    let (device_name, device_model, firmware) =
        wd_mycloud_get_info(&host, resolved_port, use_https, &session);
    let libraries = wd_mycloud_get_shares(&host, resolved_port, use_https, &session)?;
    let share_auth_errors = authenticate_windows_shares(&host, &libraries, &username, &password);
    if !share_auth_errors.is_empty() {
        log::warn!(
            "Some WD My Cloud shares were not mounted: {}",
            share_auth_errors.join("; ")
        );
    }
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let stored = serde_json::json!({
        "device_type": "wd_mycloud",
        "host": host,
        "port": resolved_port,
        "use_https": use_https,
        "username": username,
        "token": session.token,
        "device_name": device_name,
        "device_model": device_model,
        "firmware": firmware,
        "libraries": libraries,
        "connected_at": chrono::Utc::now().to_rfc3339()
    });
    db.set_setting_data("wd_mycloud_connection", &stored.to_string())
        .map_err(|error| error.to_string())?;
    Ok(NasConnectionResult {
        success: true,
        device_name,
        device_model,
        firmware,
        host_resolved: host,
        libraries,
        error: None,
    })
}

#[tauri::command]
pub fn wd_mycloud_disconnect(state: State<AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.set_setting_data("wd_mycloud_connection", "")
        .map_err(|error| error.to_string())?;
    log::info!("WD My Cloud disconnected");
    Ok(())
}

#[tauri::command]
pub fn wd_mycloud_get_status(state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let raw = db
        .get_setting_data("wd_mycloud_connection")
        .map_err(|error| error.to_string())?;
    match raw {
        Some(value) if !value.trim().is_empty() => {
            let data = serde_json::from_str::<serde_json::Value>(&value)
                .map_err(|error| error.to_string())?;
            Ok(serde_json::json!({ "connected": true, "data": data }))
        }
        _ => Ok(serde_json::json!({ "connected": false })),
    }
}

#[tauri::command]
pub fn wd_mycloud_add_library(
    state: State<AppState>,
    share_name: String,
    share_path: String,
    media_type: String,
) -> Result<(), String> {
    let connection = read_connection(&state, "wd_mycloud")?;
    let host = connection["host"]
        .as_str()
        .ok_or_else(|| "Stored WD My Cloud connection has no host".to_string())?;
    let source_path = network_source_path(host, &share_name, &share_path);
    ensure_network_source_reachable(&source_path)?;
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let source = crate::db::MediaSource {
        id: None,
        path: source_path.clone(),
        source_type: media_type,
        name: share_name.clone(),
        enabled: true,
        last_scanned: None,
        item_count: 0,
    };
    db.add_source_data(&source)
        .map_err(|error| error.to_string())?;
    log::info!("WD My Cloud library added: {share_name} -> {source_path}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{network_source_path, safe_relative_path, urlencoding_simple};

    #[test]
    fn nas_library_paths_are_scanner_compatible_network_paths() {
        let path = network_source_path("192.168.1.50", "Movies", "/Movies");
        #[cfg(target_os = "windows")]
        assert_eq!(path, r"\\192.168.1.50\Movies");
        #[cfg(not(target_os = "windows"))]
        assert_eq!(path, "smb://192.168.1.50/Movies");
    }

    #[test]
    fn nas_credentials_are_url_encoded_for_synology_authentication() {
        assert_eq!(urlencoding_simple("name+space"), "name%2Bspace");
        assert_eq!(urlencoding_simple("p@ss word"), "p%40ss%20word");
    }

    #[test]
    fn nas_browse_rejects_parent_traversal() {
        assert!(safe_relative_path("Movies/Action").is_ok());
        assert!(safe_relative_path("../Secrets").is_err());
        assert!(safe_relative_path("Movies/../../Secrets").is_err());
    }
}
