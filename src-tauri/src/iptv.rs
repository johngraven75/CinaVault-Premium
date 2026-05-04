// CinaVault Premium â€” IPTV / Xtream Codes Module
use serde::{Deserialize, Serialize};
use std::process::Command;
use tauri::State;
use crate::AppState;
use rusqlite::params;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct XtreamProfile {
    pub id: Option<i64>,
    pub name: String,
    pub server_url: String,
    pub username: String,
    pub password: String,
    pub output_format: Option<String>,
    pub user_agent: Option<String>,
    pub enabled: bool,
    pub last_synced: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LiveChannel {
    pub id: Option<i64>,
    pub profile_id: i64,
    pub name: String,
    pub stream_url: String,
    pub logo_url: Option<String>,
    pub group_name: Option<String>,
    pub epg_id: Option<String>,
}

const IPTV_FALLBACK_PLAYERS: &[&str] = &[
    "C:\\Program Files\\VideoLAN\\VLC\\vlc.exe",
    "C:\\Program Files (x86)\\VideoLAN\\VLC\\vlc.exe",
    "C:\\Program Files\\mpv\\mpv.exe",
    "C:\\ProgramData\\chocolatey\\bin\\mpv.exe",
];

fn open_stream_url(db: &crate::db::Database, stream_url: &str) -> Result<(), String> {
    let preferred = db.get_setting_data("default_player")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "system".to_string());

    if preferred != "system" && std::path::Path::new(&preferred).exists() {
        Command::new(&preferred)
            .arg(stream_url)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    for player in IPTV_FALLBACK_PLAYERS {
        if std::path::Path::new(player).exists() {
            Command::new(player)
                .arg(stream_url)
                .spawn()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
    }

    open::that(stream_url).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_xtream_profile(
    state: State<AppState>,
    name: String,
    server_url: String,
    username: String,
    password: String,
    output_format: Option<String>,
    user_agent: Option<String>,
) -> Result<i64, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let output = output_format
        .unwrap_or_else(|| "ts".to_string())
        .to_lowercase();
    db.conn.execute(
        "INSERT INTO xtream_profiles (name, server_url, username, password, output_format, user_agent, enabled) VALUES (?1,?2,?3,?4,?5,?6,1)",
        params![name, server_url, username, password, output, user_agent.unwrap_or_default()],
    ).map_err(|e| e.to_string())?;
    Ok(db.conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_xtream_profiles(state: State<AppState>) -> Result<Vec<XtreamProfile>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.conn.prepare("SELECT id, name, server_url, username, password, output_format, user_agent, enabled, last_synced FROM xtream_profiles")
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        Ok(XtreamProfile {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            server_url: row.get(2)?,
            username: row.get(3)?,
            password: row.get(4)?,
            output_format: row.get(5).ok(),
            user_agent: row.get(6).ok(),
            enabled: row.get(7)?,
            last_synced: row.get(8)?,
        })
    }).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_xtream_profile(state: State<AppState>, id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn.execute("DELETE FROM live_channels WHERE profile_id = ?1", params![id]).map_err(|e| e.to_string())?;
    db.conn.execute("DELETE FROM xtream_profiles WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn sync_xtream_streams(state: State<'_, AppState>, profile_id: i64) -> Result<serde_json::Value, String> {
    let profile = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let mut stmt = db.conn.prepare("SELECT server_url, username, password, output_format, user_agent FROM xtream_profiles WHERE id = ?1")
            .map_err(|e| e.to_string())?;
        stmt.query_row(params![profile_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3).unwrap_or_else(|_| "ts".to_string()),
                row.get::<_, String>(4).unwrap_or_default(),
            ))
        }).map_err(|e| e.to_string())?
    };

    let (server_url, username, password, output_format, user_agent) = profile;
    let url = format!("{}/player_api.php?username={}&password={}&action=get_live_streams",
        server_url.trim_end_matches('/'), username, password);

    let mut request = reqwest::Client::new().get(&url);
    if !user_agent.is_empty() {
        request = request.header(reqwest::header::USER_AGENT, user_agent);
    }
    let resp = request.send().await.map_err(|e| e.to_string())?;
    let channels: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;

    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn.execute("DELETE FROM live_channels WHERE profile_id = ?1", params![profile_id]).map_err(|e| e.to_string())?;

    let mut count = 0u64;
    for ch in &channels {
        let name = ch.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
        let stream_id = ch.get("stream_id").and_then(|v| v.as_u64()).unwrap_or(0);
        let ext = if output_format == "m3u8" { "m3u8" } else { "ts" };
        let stream_url = format!("{}/live/{}/{}/{}.{}", server_url.trim_end_matches('/'), username, password, stream_id, ext);
        let logo = ch.get("stream_icon").and_then(|v| v.as_str()).map(String::from);
        let group = ch.get("category_name").and_then(|v| v.as_str()).map(String::from);
        let epg = ch.get("epg_channel_id").and_then(|v| v.as_str()).map(String::from);

        db.conn.execute(
            "INSERT INTO live_channels (profile_id, name, stream_url, logo_url, group_name, epg_id) VALUES (?1,?2,?3,?4,?5,?6)",
            params![profile_id, name, stream_url, logo, group, epg],
        ).map_err(|e| e.to_string())?;
        count += 1;
    }

    let now = chrono::Utc::now().to_rfc3339();
    db.conn.execute("UPDATE xtream_profiles SET last_synced = ?1 WHERE id = ?2", params![now, profile_id]).map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "channels_synced": count }))
}

#[tauri::command]
pub async fn sync_epg(state: State<'_, AppState>, profile_id: i64) -> Result<serde_json::Value, String> {
    let profile = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let mut stmt = db.conn.prepare("SELECT server_url, username, password FROM xtream_profiles WHERE id = ?1")
            .map_err(|e| e.to_string())?;
        stmt.query_row(params![profile_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        }).map_err(|e| e.to_string())?
    };

    let (server_url, username, password) = profile;
    let url = format!("{}/xmltv.php?username={}&password={}", server_url.trim_end_matches('/'), username, password);

    let client = reqwest::Client::new();
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let _epg_data = resp.text().await.map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "status": "epg_synced" }))
}

#[tauri::command]
pub fn get_live_channels(state: State<AppState>, profile_id: Option<i64>) -> Result<Vec<LiveChannel>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let sql = match profile_id {
        Some(_) => "SELECT id, profile_id, name, stream_url, logo_url, group_name, epg_id FROM live_channels WHERE profile_id = ?1 ORDER BY name",
        None => "SELECT id, profile_id, name, stream_url, logo_url, group_name, epg_id FROM live_channels ORDER BY name",
    };
    let mut stmt = db.conn.prepare(sql).map_err(|e| e.to_string())?;

    let row_to_channel: fn(&rusqlite::Row) -> rusqlite::Result<LiveChannel> = |row| {
        Ok(LiveChannel {
            id: Some(row.get(0)?),
            profile_id: row.get(1)?,
            name: row.get(2)?,
            stream_url: row.get(3)?,
            logo_url: row.get(4)?,
            group_name: row.get(5)?,
            epg_id: row.get(6)?,
        })
    };
    let rows = if let Some(pid) = profile_id {
        stmt.query_map(params![pid], row_to_channel).map_err(|e| e.to_string())?
    } else {
        stmt.query_map([], row_to_channel).map_err(|e| e.to_string())?
    };
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn play_channel(state: State<'_, AppState>, stream_url: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    open_stream_url(&db, &stream_url)
}
