// CinaVault Premium — IPTV / Xtream Codes Module
use crate::AppState;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct XtreamProfile {
    pub id: Option<i64>,
    pub name: String,
    pub server_url: String,
    pub username: String,
    pub password: String,
    pub enabled: bool,
    pub last_synced: Option<String>,
    /// "xtream" (Xtream Codes login) or "m3u" (playlist URL in server_url).
    pub source_kind: String,
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

#[tauri::command]
pub fn add_xtream_profile(
    state: State<AppState>,
    name: String,
    server_url: String,
    username: String,
    password: String,
) -> Result<i64, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn.execute(
        "INSERT INTO xtream_profiles (name, server_url, username, password, enabled) VALUES (?1,?2,?3,?4,1)",
        params![name, server_url, username, password],
    ).map_err(|e| e.to_string())?;
    Ok(db.conn.last_insert_rowid())
}

/// Adds an M3U / M3U8 playlist as a Live TV source.
#[tauri::command]
pub fn add_m3u_playlist(state: State<AppState>, name: String, url: String) -> Result<i64, String> {
    let url = crate::epg::validate_source_url(&url)
        .map_err(|_| "Enter the full playlist URL starting with http:// or https://".to_string())?;
    let name = if name.trim().is_empty() { "M3U playlist".to_string() } else { name.trim().to_string() };
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn
        .execute(
            "INSERT INTO xtream_profiles (name, server_url, username, password, enabled, source_kind) VALUES (?1, ?2, '', '', 1, 'm3u')",
            params![name, url],
        )
        .map_err(|e| e.to_string())?;
    Ok(db.conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_xtream_profiles(state: State<AppState>) -> Result<Vec<XtreamProfile>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.conn.prepare("SELECT id, name, server_url, username, password, enabled, last_synced, COALESCE(source_kind, 'xtream') FROM xtream_profiles")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(XtreamProfile {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                server_url: row.get(2)?,
                username: row.get(3)?,
                password: row.get(4)?,
                enabled: row.get(5)?,
                last_synced: row.get(6)?,
                source_kind: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_xtream_profile(state: State<AppState>, id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn
        .execute(
            "DELETE FROM live_channels WHERE profile_id = ?1",
            params![id],
        )
        .map_err(|e| e.to_string())?;
    db.conn
        .execute("DELETE FROM xtream_profiles WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    let linked: Vec<i64> = {
        let mut stmt = db
            .conn
            .prepare("SELECT id FROM epg_sources WHERE profile_id = ?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![id], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    };
    for source in linked {
        crate::epg::delete_source(&db, source)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn sync_xtream_streams(
    state: State<'_, AppState>,
    profile_id: i64,
) -> Result<serde_json::Value, String> {
    let (server_url, username, password, kind) = load_profile(&state, profile_id)?;
    if kind == "m3u" {
        return sync_m3u_playlist(&state, profile_id, &server_url).await;
    }
    let url = build_player_api_url(&server_url, &username, &password);

    let client = reqwest::Client::new();
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let channels: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;

    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn
        .execute(
            "DELETE FROM live_channels WHERE profile_id = ?1",
            params![profile_id],
        )
        .map_err(|e| e.to_string())?;

    let mut count = 0u64;
    for ch in &channels {
        let name = ch.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
        let stream_id = ch.get("stream_id").and_then(|v| v.as_u64()).unwrap_or(0);
        let stream_url = build_live_stream_url(&server_url, &username, &password, stream_id);
        let logo = ch
            .get("stream_icon")
            .and_then(|v| v.as_str())
            .map(String::from);
        let group = ch
            .get("category_name")
            .and_then(|v| v.as_str())
            .map(String::from);
        let epg = ch
            .get("epg_channel_id")
            .and_then(|v| v.as_str())
            .map(String::from);

        db.conn.execute(
            "INSERT INTO live_channels (profile_id, name, stream_url, logo_url, group_name, epg_id) VALUES (?1,?2,?3,?4,?5,?6)",
            params![profile_id, name, stream_url, logo, group, epg],
        ).map_err(|e| e.to_string())?;
        count += 1;
    }

    let now = chrono::Utc::now().to_rfc3339();
    db.conn
        .execute(
            "UPDATE xtream_profiles SET last_synced = ?1 WHERE id = ?2",
            params![now, profile_id],
        )
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "channels_synced": count }))
}

#[tauri::command]
pub async fn sync_epg(
    state: State<'_, AppState>,
    profile_id: i64,
) -> Result<serde_json::Value, String> {
    let (server_url, username, password, kind) = load_profile(&state, profile_id)?;
    let source_ids: Vec<i64> = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        if kind == "xtream" {
            let url = build_epg_url(&server_url, &username, &password);
            let name: String = db
                .conn
                .query_row("SELECT name FROM xtream_profiles WHERE id = ?1", params![profile_id], |row| row.get(0))
                .map_err(|e| e.to_string())?;
            crate::epg::upsert_source(&db, &format!("{name} guide"), &url, Some(profile_id))?;
        }
        let mut stmt = db
            .conn
            .prepare("SELECT id FROM epg_sources WHERE profile_id = ?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![profile_id], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    };
    if source_ids.is_empty() {
        return Err("This playlist does not name a guide. Add an EPG source URL instead.".into());
    }
    let mut programmes = 0;
    for id in source_ids {
        programmes += crate::epg::sync_source_by_id(&state, id).await?.programmes;
    }
    Ok(serde_json::json!({ "status": "epg_synced", "programmes": programmes }))
}

#[tauri::command]
pub fn get_live_channels(
    state: State<AppState>,
    profile_id: Option<i64>,
) -> Result<Vec<LiveChannel>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let sql = match profile_id {
        Some(_) => "SELECT id, profile_id, name, stream_url, logo_url, group_name, epg_id FROM live_channels WHERE profile_id = ?1 ORDER BY name",
        None => "SELECT id, profile_id, name, stream_url, logo_url, group_name, epg_id FROM live_channels ORDER BY name",
    };
    let mut stmt = db.conn.prepare(sql).map_err(|e| e.to_string())?;

    if let Some(pid) = profile_id {
        let rows = stmt
            .query_map(params![pid], row_to_live_channel)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    } else {
        let rows = stmt
            .query_map([], row_to_live_channel)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn play_channel(stream_url: String) -> Result<(), String> {
    open::that(&stream_url).map_err(|e| e.to_string())
}

fn load_profile(state: &AppState, profile_id: i64) -> Result<(String, String, String, String), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn
        .query_row(
            "SELECT server_url, username, password, COALESCE(source_kind, 'xtream') FROM xtream_profiles WHERE id = ?1",
            params![profile_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|e| e.to_string())
}

/// One channel from an M3U playlist.
#[derive(Debug, Clone, PartialEq)]
pub struct M3uEntry {
    pub name: String,
    pub url: String,
    pub tvg_id: Option<String>,
    pub logo: Option<String>,
    pub group: Option<String>,
}

fn m3u_attribute(line: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let start = line.find(&needle)? + needle.len();
    let end = line[start..].find('"')? + start;
    let value = line[start..end].trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// Parses an extended M3U playlist into channels plus any guide URLs named
/// in the header (`url-tvg` / `x-tvg-url`, comma separated).
pub fn parse_m3u(text: &str) -> (Vec<M3uEntry>, Vec<String>) {
    let mut entries = Vec::new();
    let mut guides = Vec::new();
    let mut pending: Option<M3uEntry> = None;
    let mut group_override: Option<String> = None;
    for raw in text.lines() {
        let line = raw.trim().trim_start_matches('\u{feff}');
        if line.is_empty() {
            continue;
        }
        if line.starts_with("#EXTM3U") {
            for key in ["url-tvg", "x-tvg-url"] {
                if let Some(value) = m3u_attribute(line, key) {
                    guides.extend(
                        value
                            .split(',')
                            .map(str::trim)
                            .filter(|url| !url.is_empty())
                            .map(String::from),
                    );
                }
            }
        } else if let Some(info) = line.strip_prefix("#EXTINF:") {
            // The display name follows the last comma outside quotes.
            let mut in_quotes = false;
            let mut split = None;
            for (index, character) in info.char_indices() {
                match character {
                    '"' => in_quotes = !in_quotes,
                    ',' if !in_quotes => split = Some(index),
                    _ => {}
                }
            }
            let name = split
                .map(|index| info[index + 1..].trim())
                .unwrap_or("")
                .to_string();
            let name = if name.is_empty() {
                m3u_attribute(info, "tvg-name").unwrap_or_default()
            } else {
                name
            };
            pending = Some(M3uEntry {
                name,
                url: String::new(),
                tvg_id: m3u_attribute(info, "tvg-id"),
                logo: m3u_attribute(info, "tvg-logo"),
                group: m3u_attribute(info, "group-title"),
            });
            group_override = None;
        } else if let Some(group) = line.strip_prefix("#EXTGRP:") {
            group_override = Some(group.trim().to_string()).filter(|group| !group.is_empty());
        } else if line.starts_with('#') {
            continue;
        } else {
            let mut entry = pending.take().unwrap_or(M3uEntry {
                name: String::new(),
                url: String::new(),
                tvg_id: None,
                logo: None,
                group: None,
            });
            entry.url = line.to_string();
            if entry.group.is_none() {
                entry.group = group_override.take();
            }
            if entry.name.is_empty() {
                entry.name = line.rsplit('/').next().unwrap_or(line).to_string();
            }
            entries.push(entry);
        }
    }
    (entries, guides)
}

async fn sync_m3u_playlist(
    state: &AppState,
    profile_id: i64,
    playlist_url: &str,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(playlist_url)
        .send()
        .await
        .map_err(|e| format!("Could not reach the playlist: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("The playlist server answered {}", resp.status()));
    }
    let base = resp.url().clone();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    let (entries, guides) = parse_m3u(&text);
    if entries.is_empty() {
        return Err("No channels were found. Check that the URL is an M3U playlist.".into());
    }

    let db = state.db.lock().map_err(|e| e.to_string())?;
    let tx = db.conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM live_channels WHERE profile_id = ?1",
        params![profile_id],
    )
    .map_err(|e| e.to_string())?;
    let mut count = 0u64;
    {
        let mut insert = tx
            .prepare("INSERT INTO live_channels (profile_id, name, stream_url, logo_url, group_name, epg_id) VALUES (?1,?2,?3,?4,?5,?6)")
            .map_err(|e| e.to_string())?;
        for entry in &entries {
            // Relative entries are resolved against the playlist address.
            let Ok(url) = base.join(&entry.url) else {
                continue;
            };
            if !matches!(url.scheme(), "http" | "https") {
                continue;
            }
            insert
                .execute(params![
                    profile_id,
                    entry.name,
                    url.as_str(),
                    entry.logo,
                    entry.group,
                    entry.tvg_id
                ])
                .map_err(|e| e.to_string())?;
            count += 1;
        }
    }
    tx.execute(
        "UPDATE xtream_profiles SET last_synced = ?1 WHERE id = ?2",
        params![chrono::Utc::now().to_rfc3339(), profile_id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;

    let mut guides_added = 0;
    for guide in guides {
        if crate::epg::upsert_source(&db, "", &guide, Some(profile_id)).is_ok() {
            guides_added += 1;
        }
    }
    Ok(serde_json::json!({ "channels_synced": count, "guides_found": guides_added }))
}

fn row_to_live_channel(row: &rusqlite::Row) -> rusqlite::Result<LiveChannel> {
    Ok(LiveChannel {
        id: Some(row.get(0)?),
        profile_id: row.get(1)?,
        name: row.get(2)?,
        stream_url: row.get(3)?,
        logo_url: row.get(4)?,
        group_name: row.get(5)?,
        epg_id: row.get(6)?,
    })
}

fn build_player_api_url(server_url: &str, username: &str, password: &str) -> String {
    format!(
        "{}/player_api.php?username={}&password={}&action=get_live_streams",
        normalize_server_base(server_url),
        encode_url_component(username),
        encode_url_component(password),
    )
}

fn build_epg_url(server_url: &str, username: &str, password: &str) -> String {
    format!(
        "{}/xmltv.php?username={}&password={}",
        normalize_server_base(server_url),
        encode_url_component(username),
        encode_url_component(password),
    )
}

fn build_live_stream_url(
    server_url: &str,
    username: &str,
    password: &str,
    stream_id: u64,
) -> String {
    format!(
        "{}/live/{}/{}/{}.ts",
        normalize_server_base(server_url),
        encode_url_component(username),
        encode_url_component(password),
        stream_id,
    )
}

fn normalize_server_base(server_url: &str) -> String {
    server_url.trim().trim_end_matches('/').to_string()
}

fn encode_url_component(value: &str) -> String {
    utf8_percent_encode(value, NON_ALPHANUMERIC).to_string()
}

#[cfg(test)]
mod tests {
    use super::{build_live_stream_url, build_player_api_url, parse_m3u, M3uEntry};

    #[test]
    fn m3u_playlists_parse_channels_groups_and_guides() {
        let text = "\u{feff}#EXTM3U url-tvg=\"http://epg.example/a.xml.gz, http://epg.example/b.xml\"
#EXTINF:-1 tvg-id=\"bbc1.uk\" tvg-logo=\"http://logo.example/bbc1.png\" group-title=\"UK, General\",BBC One HD
http://stream.example/live/1.m3u8
#EXTINF:-1,Local News
#EXTGRP:Local
#EXTVLCOPT:http-user-agent=Thing
relative/2.ts

#EXTINF:-1 tvg-name=\"Named Only\",
http://stream.example/3
";
        let (entries, guides) = parse_m3u(text);
        assert_eq!(
            guides,
            vec!["http://epg.example/a.xml.gz", "http://epg.example/b.xml"]
        );
        assert_eq!(
            entries[0],
            M3uEntry {
                name: "BBC One HD".into(),
                url: "http://stream.example/live/1.m3u8".into(),
                tvg_id: Some("bbc1.uk".into()),
                logo: Some("http://logo.example/bbc1.png".into()),
                group: Some("UK, General".into()),
            }
        );
        assert_eq!(entries[1].name, "Local News");
        assert_eq!(entries[1].group.as_deref(), Some("Local"));
        assert_eq!(entries[1].url, "relative/2.ts");
        assert_eq!(entries[2].name, "Named Only");
        assert_eq!(entries.len(), 3);
    }


    #[test]
    fn xtream_api_url_encodes_credentials_and_trims_server_slashes() {
        assert_eq!(
            build_player_api_url("http://provider.example.com:8080/", "viewer@example.com", "p&a s"),
            "http://provider.example.com:8080/player_api.php?username=viewer%40example%2Ecom&password=p%26a%20s&action=get_live_streams",
        );
    }

    #[test]
    fn xtream_live_url_encodes_path_credentials() {
        assert_eq!(
            build_live_stream_url("http://provider.example.com:8080/", "view er", "p/a", 42),
            "http://provider.example.com:8080/live/view%20er/p%2Fa/42.ts",
        );
    }
}
