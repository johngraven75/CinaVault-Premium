// CinaVault Premium — IPTV / Xtream Codes Module
use crate::secure_credentials::{self, SECURE_STORE_MARKER};
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
    /// Always empty: the password (or Stalker MAC) stays in Windows Credential
    /// Manager and never goes back to the WebView. See `has_secret`.
    pub password: String,
    pub enabled: bool,
    pub last_synced: Option<String>,
    /// "xtream" (Xtream Codes login), "m3u" (playlist URL in server_url) or
    /// "stalker" (Stalker / MAG portal URL in server_url, MAC as the secret).
    pub source_kind: String,
    /// Sent with every request for this source unless a channel names its own.
    pub user_agent: Option<String>,
    pub referrer: Option<String>,
    pub has_secret: bool,
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
    /// From the playlist (`#EXTVLCOPT:http-user-agent=` and friends).
    pub user_agent: Option<String>,
    pub referrer: Option<String>,
}

pub const SOURCE_KINDS: [&str; 3] = ["xtream", "m3u", "stalker"];

/// A source as the add / edit form sends it.
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct IptvSourceInput {
    pub kind: String,
    #[serde(default)]
    pub name: String,
    pub server_url: String,
    #[serde(default)]
    pub username: Option<String>,
    /// Xtream password or Stalker MAC address. Empty on edit keeps the saved one.
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub referrer: Option<String>,
}

/// A source after validation, ready to store.
#[derive(Debug, PartialEq)]
pub struct ValidSource {
    pub kind: String,
    pub name: String,
    pub server_url: String,
    pub username: String,
    pub secret: Option<String>,
    pub user_agent: Option<String>,
    pub referrer: Option<String>,
}

const MAX_HEADER_LEN: usize = 512;

/// Trims a User-Agent / Referer value; empty becomes None. Control characters
/// (a CR/LF would split the request) and oversized values are refused.
pub fn clean_header_value(label: &str, value: Option<&str>) -> Result<Option<String>, String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.len() > MAX_HEADER_LEN {
        return Err(format!(
            "{label} is longer than {MAX_HEADER_LEN} characters"
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(format!(
            "{label} cannot contain line breaks or control characters"
        ));
    }
    Ok(Some(value.to_string()))
}

/// Accepts `00:1A:79:AB:CD:EF`, `00-1a-79-ab-cd-ef` or `001A79ABCDEF`.
pub fn normalize_mac(value: &str) -> Result<String, String> {
    let hex: String = value
        .trim()
        .chars()
        .filter(|character| !matches!(character, ':' | '-' | ' '))
        .collect();
    if hex.len() != 12 || !hex.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err("Enter the MAC address as 00:1A:79:XX:XX:XX".into());
    }
    let upper = hex.to_ascii_uppercase();
    Ok(upper
        .as_bytes()
        .chunks(2)
        .map(|pair| String::from_utf8_lossy(pair).into_owned())
        .collect::<Vec<_>>()
        .join(":"))
}

/// Checks a source form. `editing` allows an empty secret (keep the saved one).
pub fn validate_source(input: &IptvSourceInput, editing: bool) -> Result<ValidSource, String> {
    let kind = input.kind.trim().to_ascii_lowercase();
    if !SOURCE_KINDS.contains(&kind.as_str()) {
        return Err(format!("Unknown login type: {}", input.kind));
    }
    // A public playlist is just a URL with no login. An Xtream login with
    // neither a username nor a password can't work, so treat that as an M3U
    // playlist and let the bare URL through (mirrors the frontend builder).
    let kind = if kind == "xtream"
        && input.username.as_deref().unwrap_or("").trim().is_empty()
        && input
            .secret
            .as_deref()
            .map(str::trim)
            .filter(|secret| !secret.is_empty())
            .is_none()
    {
        "m3u".to_string()
    } else {
        kind
    };
    let server_url = crate::epg::validate_source_url(&input.server_url).map_err(|_| {
        match kind.as_str() {
            "m3u" => "Enter the full playlist URL starting with http:// or https://",
            "stalker" => "Enter the portal URL starting with http:// or https://",
            _ => "Enter the server URL starting with http:// or https://",
        }
        .to_string()
    })?;
    let server_url = if kind == "xtream" {
        normalize_server_base(&server_url)
    } else {
        server_url
    };
    let username = input.username.as_deref().unwrap_or("").trim().to_string();
    let secret = input
        .secret
        .as_deref()
        .map(str::trim)
        .filter(|secret| !secret.is_empty());
    let secret = match kind.as_str() {
        "xtream" => {
            if username.is_empty() {
                return Err("Username is required".into());
            }
            if secret.is_none() && !editing {
                return Err("Password is required".into());
            }
            secret.map(String::from)
        }
        "stalker" => match secret {
            Some(mac) => Some(normalize_mac(mac)?),
            None if editing => None,
            None => return Err("The portal's MAC address is required".into()),
        },
        // M3U logins live in the playlist URL itself.
        _ => None,
    };
    let name = match input.name.trim() {
        "" => match kind.as_str() {
            "m3u" => "M3U playlist",
            "stalker" => "Stalker portal",
            _ => "IPTV",
        }
        .to_string(),
        name => name.to_string(),
    };
    Ok(ValidSource {
        username: if kind == "xtream" {
            username
        } else {
            String::new()
        },
        kind,
        name,
        server_url,
        secret,
        user_agent: clean_header_value("User-Agent", input.user_agent.as_deref())?,
        referrer: clean_header_value("Referer", input.referrer.as_deref())?,
    })
}

fn secret_key(profile_id: i64) -> String {
    format!("iptv-source-{profile_id}")
}

/// Saves a source's password / MAC in Windows Credential Manager and marks the
/// row. Builds without a system keychain (Linux / macOS dev) keep it in the
/// database as before, since keyring would otherwise lose it.
fn store_secret(db: &crate::db::Database, profile_id: i64, secret: &str) -> Result<(), String> {
    let stored = match secure_credentials::set(&secret_key(profile_id), secret) {
        Ok(()) => SECURE_STORE_MARKER.to_string(),
        Err(error) if !cfg!(windows) => {
            log::warn!("IPTV source {profile_id}: no system keychain ({error}); keeping the secret in the database");
            secret.to_string()
        }
        Err(error) => {
            return Err(format!(
                "Windows Credential Manager could not save the login: {error}"
            ))
        }
    };
    db.conn
        .execute(
            "UPDATE xtream_profiles SET password = ?1 WHERE id = ?2",
            params![stored, profile_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// The real password / MAC for a stored column value. Plain values left by
/// older builds are moved into Credential Manager on first use.
fn resolve_secret(
    db: &crate::db::Database,
    profile_id: i64,
    stored: &str,
) -> Result<String, String> {
    if stored == SECURE_STORE_MARKER {
        return secure_credentials::get(&secret_key(profile_id))?.ok_or_else(|| {
            "The saved login for this source is missing from Windows Credential Manager. Open the source settings and enter it again.".to_string()
        });
    }
    if cfg!(windows) && !stored.is_empty() {
        if let Err(error) = store_secret(db, profile_id, stored) {
            log::warn!(
                "IPTV source {profile_id}: could not move the login to Credential Manager: {error}"
            );
        }
    }
    Ok(stored.to_string())
}

fn insert_source(db: &crate::db::Database, source: &ValidSource) -> Result<i64, String> {
    db.conn
        .execute(
            "INSERT INTO xtream_profiles (name, server_url, username, password, enabled, source_kind, user_agent, referrer) VALUES (?1, ?2, ?3, '', 1, ?4, ?5, ?6)",
            params![
                source.name,
                source.server_url,
                source.username,
                source.kind,
                source.user_agent,
                source.referrer
            ],
        )
        .map_err(|e| e.to_string())?;
    let id = db.conn.last_insert_rowid();
    if let Some(secret) = &source.secret {
        if let Err(error) = store_secret(db, id, secret) {
            let _ = db
                .conn
                .execute("DELETE FROM xtream_profiles WHERE id = ?1", params![id]);
            return Err(error);
        }
    }
    Ok(id)
}

/// Adds an Xtream Codes, M3U or Stalker portal source.
#[tauri::command]
pub fn add_iptv_source(state: State<AppState>, source: IptvSourceInput) -> Result<i64, String> {
    let source = validate_source(&source, false)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    insert_source(&db, &source)
}

/// Saves the settings panel for a source. The login type cannot change; an
/// empty password / MAC keeps the saved one. Channels keep playing until the
/// next sync picks up a new server or login.
#[tauri::command]
pub fn update_iptv_source(
    state: State<AppState>,
    id: i64,
    source: IptvSourceInput,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let kind: String = db
        .conn
        .query_row(
            "SELECT COALESCE(source_kind, 'xtream') FROM xtream_profiles WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|_| "That source no longer exists".to_string())?;
    let source = validate_source(&IptvSourceInput { kind, ..source }, true)?;
    db.conn
        .execute(
            "UPDATE xtream_profiles SET name = ?1, server_url = ?2, username = ?3, user_agent = ?4, referrer = ?5 WHERE id = ?6",
            params![
                source.name,
                source.server_url,
                source.username,
                source.user_agent,
                source.referrer,
                id
            ],
        )
        .map_err(|e| e.to_string())?;
    if let Some(secret) = &source.secret {
        store_secret(&db, id, secret)?;
    }
    Ok(())
}

#[tauri::command]
pub fn add_xtream_profile(
    state: State<AppState>,
    name: String,
    server_url: String,
    username: String,
    password: String,
) -> Result<i64, String> {
    add_iptv_source(
        state,
        IptvSourceInput {
            kind: "xtream".into(),
            name,
            server_url,
            username: Some(username),
            secret: Some(password),
            ..Default::default()
        },
    )
}

/// Adds an M3U / M3U8 playlist as a Live TV source.
#[tauri::command]
pub fn add_m3u_playlist(state: State<AppState>, name: String, url: String) -> Result<i64, String> {
    add_iptv_source(
        state,
        IptvSourceInput {
            kind: "m3u".into(),
            name,
            server_url: url,
            ..Default::default()
        },
    )
}

#[tauri::command]
pub fn get_xtream_profiles(state: State<AppState>) -> Result<Vec<XtreamProfile>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.conn.prepare("SELECT id, name, server_url, username, password, enabled, last_synced, COALESCE(source_kind, 'xtream'), user_agent, referrer FROM xtream_profiles")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(XtreamProfile {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                server_url: row.get(2)?,
                username: row.get(3)?,
                password: String::new(),
                has_secret: !row.get::<_, String>(4)?.is_empty(),
                enabled: row.get(5)?,
                last_synced: row.get(6)?,
                source_kind: row.get(7)?,
                user_agent: row.get(8)?,
                referrer: row.get(9)?,
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
    if let Err(error) = secure_credentials::delete(&secret_key(id)) {
        log::warn!("IPTV source {id}: could not remove the saved login: {error}");
    }
    crate::epg::unlink_profile(&db, id)
}

#[tauri::command]
pub async fn sync_xtream_streams(
    state: State<'_, AppState>,
    profile_id: i64,
) -> Result<serde_json::Value, String> {
    let profile = load_profile(&state, profile_id)?;
    match profile.kind.as_str() {
        "m3u" => return sync_m3u_playlist(&state, &profile).await,
        "stalker" => return sync_stalker_portal(&state, &profile).await,
        _ => {}
    }
    let LoadedProfile {
        server_url,
        username,
        secret: password,
        ..
    } = &profile;
    let url = build_player_api_url(server_url, username, password);

    let client = source_client(&profile)?;
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
        let stream_url = build_live_stream_url(server_url, username, password, stream_id);
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
    let profile = load_profile(&state, profile_id)?;
    let source_ids: Vec<i64> = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        if profile.kind == "xtream" {
            let url = build_epg_url(&profile.server_url, &profile.username, &profile.secret);
            let name: String = db
                .conn
                .query_row(
                    "SELECT name FROM xtream_profiles WHERE id = ?1",
                    params![profile_id],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            crate::epg::upsert_source(&db, &format!("{name} guide"), &url, Some(profile_id))?;
        }
        crate::epg::sources_for_profile(&db, profile_id)?
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
        Some(_) => "SELECT id, profile_id, name, stream_url, logo_url, group_name, epg_id, user_agent, referrer FROM live_channels WHERE profile_id = ?1 ORDER BY name",
        None => "SELECT id, profile_id, name, stream_url, logo_url, group_name, epg_id, user_agent, referrer FROM live_channels ORDER BY name",
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

/// A source with its login resolved from Credential Manager.
pub(crate) struct LoadedProfile {
    pub id: i64,
    pub server_url: String,
    pub username: String,
    pub secret: String,
    pub kind: String,
    pub user_agent: Option<String>,
    pub referrer: Option<String>,
}

fn load_profile(state: &AppState, profile_id: i64) -> Result<LoadedProfile, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let (server_url, username, stored, kind, user_agent, referrer): (
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    ) = db
        .conn
        .query_row(
            "SELECT server_url, username, password, COALESCE(source_kind, 'xtream'), user_agent, referrer FROM xtream_profiles WHERE id = ?1",
            params![profile_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        )
        .map_err(|e| e.to_string())?;
    let secret = resolve_secret(&db, profile_id, &stored)?;
    Ok(LoadedProfile {
        id: profile_id,
        server_url,
        username,
        secret,
        kind,
        user_agent,
        referrer,
    })
}

/// An HTTP client for talking to a source, sending its User-Agent / Referer.
fn source_client(profile: &LoadedProfile) -> Result<reqwest::Client, String> {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(referrer) = profile
        .referrer
        .as_deref()
        .and_then(|value| reqwest::header::HeaderValue::from_str(value).ok())
    {
        headers.insert(reqwest::header::REFERER, referrer);
    }
    let mut builder = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .timeout(std::time::Duration::from_secs(120))
        .default_headers(headers);
    if let Some(user_agent) = &profile.user_agent {
        builder = builder.user_agent(user_agent.clone());
    }
    builder.build().map_err(|e| e.to_string())
}

/// What the player needs to open a channel: the address plus the headers the
/// relay must send.
#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct PlaybackSource {
    pub url: String,
    pub user_agent: Option<String>,
    pub referrer: Option<String>,
}

/// Channel headers from the playlist win; the source's settings fill the gaps.
pub fn effective_headers(
    channel: (Option<String>, Option<String>),
    source: (Option<String>, Option<String>),
) -> (Option<String>, Option<String>) {
    (channel.0.or(source.0), channel.1.or(source.1))
}

/// Resolves a saved channel to a playable address. Stalker portals hand out a
/// short-lived link per play, so those are asked for one now.
#[tauri::command]
pub async fn resolve_live_stream(
    state: State<'_, AppState>,
    channel_id: i64,
) -> Result<PlaybackSource, String> {
    let (profile_id, stream_url, channel_agent, channel_referrer): (
        i64,
        String,
        Option<String>,
        Option<String>,
    ) = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.conn
            .query_row(
                "SELECT profile_id, stream_url, user_agent, referrer FROM live_channels WHERE id = ?1",
                params![channel_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(|_| "This channel is gone. Sync the source again.".to_string())?
    };
    let profile = load_profile(&state, profile_id)?;
    let (user_agent, referrer) = effective_headers(
        (channel_agent, channel_referrer),
        (profile.user_agent.clone(), profile.referrer.clone()),
    );
    if profile.kind == "stalker" {
        let session = crate::iptv_stalker::StalkerSession::connect(
            &profile.server_url,
            &profile.secret,
            profile.user_agent.as_deref(),
        )
        .await?;
        let url = session.create_link(&stream_url).await?;
        return Ok(PlaybackSource {
            url,
            // MAG portals often check the box's User-Agent on the stream too.
            user_agent: user_agent
                .or_else(|| Some(crate::iptv_stalker::MAG_USER_AGENT.to_string())),
            referrer,
        });
    }
    Ok(PlaybackSource {
        url: stream_url,
        user_agent,
        referrer,
    })
}

async fn sync_stalker_portal(
    state: &AppState,
    profile: &LoadedProfile,
) -> Result<serde_json::Value, String> {
    let session = crate::iptv_stalker::StalkerSession::connect(
        &profile.server_url,
        &profile.secret,
        profile.user_agent.as_deref(),
    )
    .await?;
    let channels = session.channels().await?;
    if channels.is_empty() {
        return Err("The portal returned no channels for this MAC address.".into());
    }
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let tx = db.conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM live_channels WHERE profile_id = ?1",
        params![profile.id],
    )
    .map_err(|e| e.to_string())?;
    {
        let mut insert = tx
            .prepare("INSERT INTO live_channels (profile_id, name, stream_url, logo_url, group_name, epg_id) VALUES (?1,?2,?3,?4,?5,?6)")
            .map_err(|e| e.to_string())?;
        for channel in &channels {
            insert
                .execute(params![
                    profile.id,
                    channel.name,
                    channel.cmd,
                    channel.logo,
                    channel.group,
                    channel.epg_id
                ])
                .map_err(|e| e.to_string())?;
        }
    }
    tx.execute(
        "UPDATE xtream_profiles SET last_synced = ?1 WHERE id = ?2",
        params![chrono::Utc::now().to_rfc3339(), profile.id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "channels_synced": channels.len() }))
}

/// One channel from an M3U playlist.
#[derive(Debug, Clone, PartialEq)]
pub struct M3uEntry {
    pub name: String,
    pub url: String,
    pub tvg_id: Option<String>,
    pub logo: Option<String>,
    pub group: Option<String>,
    pub user_agent: Option<String>,
    pub referrer: Option<String>,
}

/// Request headers a playlist asks for on one channel.
#[derive(Debug, Clone, Default, PartialEq)]
struct EntryHeaders {
    user_agent: Option<String>,
    referrer: Option<String>,
}

impl EntryHeaders {
    fn set(&mut self, name: &str, value: &str) {
        let value = value.trim().trim_matches('"').trim();
        let Ok(Some(value)) = clean_header_value(name, Some(value)) else {
            return;
        };
        match name.trim().to_ascii_lowercase().as_str() {
            "user-agent" | "http-user-agent" => self.user_agent = Some(value),
            "referer" | "referrer" | "http-referrer" | "http-referer" => {
                self.referrer = Some(value)
            }
            _ => {}
        }
    }

    /// `User-Agent=a&Referer=b` (Kodi style, values may be percent-encoded).
    fn set_pairs(&mut self, pairs: &str) {
        for pair in pairs.split('&') {
            if let Some((name, value)) = pair.split_once('=') {
                let value = percent_encoding::percent_decode_str(value).decode_utf8_lossy();
                self.set(name, &value);
            }
        }
    }

    fn merge_into(self, other: &mut EntryHeaders) {
        if self.user_agent.is_some() {
            other.user_agent = self.user_agent;
        }
        if self.referrer.is_some() {
            other.referrer = self.referrer;
        }
    }
}

/// Reads one header-carrying comment line (`#EXTVLCOPT`, `#EXTHTTP`,
/// `#KODIPROP`). Returns false for any other line.
fn parse_header_line(line: &str, headers: &mut EntryHeaders) -> bool {
    if let Some(option) = line.strip_prefix("#EXTVLCOPT:") {
        if let Some((name, value)) = option.split_once('=') {
            headers.set(name, value);
        }
        true
    } else if let Some(json) = line.strip_prefix("#EXTHTTP:") {
        if let Ok(serde_json::Value::Object(map)) = serde_json::from_str(json.trim()) {
            for (name, value) in map {
                if let Some(value) = value.as_str() {
                    headers.set(&name, value);
                }
            }
        }
        true
    } else if let Some(property) = line.strip_prefix("#KODIPROP:") {
        if let Some(pairs) = property
            .trim()
            .strip_prefix("inputstream.adaptive.stream_headers=")
            .or_else(|| {
                property
                    .trim()
                    .strip_prefix("inputstream.adaptive.manifest_headers=")
            })
        {
            headers.set_pairs(pairs);
        }
        true
    } else {
        false
    }
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
    // Options may come before or after #EXTINF; they belong to the next URL.
    let mut headers = EntryHeaders::default();
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
                user_agent: None,
                referrer: None,
            });
            group_override = None;
            // Some playlists put user-agent="…" on the #EXTINF line itself.
            if let Some(agent) = m3u_attribute(info, "user-agent") {
                headers.set("user-agent", &agent);
            }
            if let Some(referrer) =
                m3u_attribute(info, "referrer").or_else(|| m3u_attribute(info, "referer"))
            {
                headers.set("referer", &referrer);
            }
        } else if let Some(group) = line.strip_prefix("#EXTGRP:") {
            group_override = Some(group.trim().to_string()).filter(|group| !group.is_empty());
        } else if parse_header_line(line, &mut headers) || line.starts_with('#') {
            continue;
        } else {
            let mut entry = pending.take().unwrap_or(M3uEntry {
                name: String::new(),
                url: String::new(),
                tvg_id: None,
                logo: None,
                group: None,
                user_agent: None,
                referrer: None,
            });
            // Kodi style: `http://host/stream.m3u8|User-Agent=x&Referer=y`.
            let (url, pipe_headers) = match line.split_once('|') {
                Some((url, pairs)) => {
                    let mut parsed = EntryHeaders::default();
                    parsed.set_pairs(pairs);
                    (url.trim(), parsed)
                }
                None => (line, EntryHeaders::default()),
            };
            let mut entry_headers = std::mem::take(&mut headers);
            pipe_headers.merge_into(&mut entry_headers);
            entry.user_agent = entry_headers.user_agent;
            entry.referrer = entry_headers.referrer;
            entry.url = url.to_string();
            let line = url;
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

/// Largest M3U playlist accepted.
const MAX_M3U_BYTES: usize = 256 * 1024 * 1024;

async fn sync_m3u_playlist(
    state: &AppState,
    profile: &LoadedProfile,
) -> Result<serde_json::Value, String> {
    let profile_id = profile.id;
    let playlist_url = profile.server_url.as_str();
    let client = source_client(profile)?;
    let resp = client
        .get(playlist_url)
        .send()
        .await
        .map_err(|e| format!("Could not reach the playlist: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("The playlist server answered {}", resp.status()));
    }
    let base = resp.url().clone();
    let body = crate::iptv_relay::read_body_capped(resp, MAX_M3U_BYTES)
        .await
        .map_err(|e| format!("Could not read the playlist: {e}"))?;
    let text = String::from_utf8_lossy(&body);
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
            .prepare("INSERT INTO live_channels (profile_id, name, stream_url, logo_url, group_name, epg_id, user_agent, referrer) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)")
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
                    entry.tvg_id,
                    entry.user_agent,
                    entry.referrer
                ])
                .map_err(|e| e.to_string())?;
            count += 1;
        }
    }
    if count == 0 {
        // Dropping the transaction rolls back the delete, keeping the old list.
        return Err("The playlist has no http or https channel addresses.".into());
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
        user_agent: row.get(7)?,
        referrer: row.get(8)?,
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
    use super::{
        build_live_stream_url, build_player_api_url, clean_header_value, effective_headers,
        normalize_mac, parse_m3u, validate_source, IptvSourceInput, M3uEntry,
    };

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
                user_agent: None,
                referrer: None,
            }
        );
        assert_eq!(entries[1].name, "Local News");
        assert_eq!(entries[1].user_agent.as_deref(), Some("Thing"));
        assert_eq!(
            entries[2].user_agent, None,
            "options belong to one channel only"
        );
        assert_eq!(entries[1].group.as_deref(), Some("Local"));
        assert_eq!(entries[1].url, "relative/2.ts");
        assert_eq!(entries[2].name, "Named Only");
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn m3u_header_options_attach_to_their_channel() {
        let text = "#EXTM3U
#EXTINF:-1 tvg-id=\"a\",VLC options
#EXTVLCOPT:http-user-agent=Mozilla/5.0 (SMART-TV)
#EXTVLCOPT:http-referrer=https://site.example/
http://a.example/1.m3u8
#EXTVLCOPT:http-referer=https://before.example/
#EXTINF:-1,Options before EXTINF
http://a.example/2.ts
#EXTINF:-1,TiviMate JSON
#EXTHTTP:{\"User-Agent\":\"TiviMate/4.7\",\"Referer\":\"https://json.example/\"}
http://a.example/3.ts
#EXTINF:-1,Kodi pipe
http://a.example/4.m3u8|User-Agent=Kodi%2F21&Referer=https://pipe.example/
#EXTINF:-1,Kodi prop
#KODIPROP:inputstream.adaptive.stream_headers=User-Agent=ExoPlayer&Referer=https%3A%2F%2Fprop.example%2F
http://a.example/5.mpd
#EXTINF:-1 user-agent=\"Attr/1\",Attribute
http://a.example/6.ts
#EXTINF:-1,Injected
#EXTVLCOPT:http-user-agent=bad\u{7}agent
http://a.example/7.ts
";
        let (entries, _) = parse_m3u(text);
        let headers: Vec<_> = entries
            .iter()
            .map(|entry| {
                (
                    entry.url.as_str(),
                    entry.user_agent.as_deref(),
                    entry.referrer.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            headers,
            vec![
                (
                    "http://a.example/1.m3u8",
                    Some("Mozilla/5.0 (SMART-TV)"),
                    Some("https://site.example/")
                ),
                (
                    "http://a.example/2.ts",
                    None,
                    Some("https://before.example/")
                ),
                (
                    "http://a.example/3.ts",
                    Some("TiviMate/4.7"),
                    Some("https://json.example/")
                ),
                (
                    "http://a.example/4.m3u8",
                    Some("Kodi/21"),
                    Some("https://pipe.example/")
                ),
                (
                    "http://a.example/5.mpd",
                    Some("ExoPlayer"),
                    Some("https://prop.example/")
                ),
                ("http://a.example/6.ts", Some("Attr/1"), None),
                ("http://a.example/7.ts", None, None),
            ]
        );
    }

    #[test]
    fn header_values_refuse_line_breaks() {
        assert_eq!(clean_header_value("User-Agent", Some("  ")).unwrap(), None);
        assert_eq!(
            clean_header_value("User-Agent", Some(" VLC "))
                .unwrap()
                .as_deref(),
            Some("VLC")
        );
        assert!(clean_header_value("Referer", Some("a\r\nX-Evil: 1")).is_err());
        assert!(clean_header_value("Referer", Some(&"a".repeat(600))).is_err());
    }

    #[test]
    fn channel_headers_win_over_source_headers() {
        assert_eq!(
            effective_headers(
                (Some("Chan".into()), None),
                (
                    Some("Source".into()),
                    Some("https://source.example/".into())
                )
            ),
            (Some("Chan".into()), Some("https://source.example/".into()))
        );
    }

    #[test]
    fn source_forms_validate_per_login_type() {
        let xtream = IptvSourceInput {
            kind: "xtream".into(),
            name: " My TV ".into(),
            server_url: "http://provider.example:8080/".into(),
            username: Some("viewer".into()),
            secret: Some("pw".into()),
            user_agent: Some("IPTVSmarters".into()),
            referrer: None,
        };
        let valid = validate_source(&xtream, false).unwrap();
        assert_eq!(valid.name, "My TV");
        assert_eq!(valid.server_url, "http://provider.example:8080");
        assert_eq!(valid.secret.as_deref(), Some("pw"));
        assert_eq!(valid.user_agent.as_deref(), Some("IPTVSmarters"));

        let no_password = IptvSourceInput {
            secret: None,
            ..xtream.clone()
        };
        assert!(validate_source(&no_password, false).is_err());
        assert_eq!(validate_source(&no_password, true).unwrap().secret, None);
        assert!(validate_source(
            &IptvSourceInput {
                username: None,
                ..xtream.clone()
            },
            false
        )
        .is_err());

        let stalker = IptvSourceInput {
            kind: "stalker".into(),
            name: String::new(),
            server_url: "http://portal.example/c/".into(),
            secret: Some("00-1a-79-ab-cd-ef".into()),
            ..Default::default()
        };
        let valid = validate_source(&stalker, false).unwrap();
        assert_eq!(valid.name, "Stalker portal");
        assert_eq!(valid.secret.as_deref(), Some("00:1A:79:AB:CD:EF"));
        assert_eq!(valid.username, "");
        assert!(validate_source(
            &IptvSourceInput {
                secret: Some("00:1A".into()),
                ..stalker.clone()
            },
            false
        )
        .is_err());

        let m3u = IptvSourceInput {
            kind: "m3u".into(),
            server_url: "https://list.example/get.php?username=a&password=b".into(),
            secret: Some("ignored".into()),
            ..Default::default()
        };
        let valid = validate_source(&m3u, false).unwrap();
        assert_eq!(valid.secret, None, "M3U logins live in the URL");
        assert_eq!(
            valid.server_url,
            "https://list.example/get.php?username=a&password=b"
        );

        assert!(validate_source(
            &IptvSourceInput {
                kind: "rtmp".into(),
                ..m3u.clone()
            },
            false
        )
        .is_err());
        assert!(validate_source(
            &IptvSourceInput {
                server_url: "file:///C:/x.m3u".into(),
                ..m3u
            },
            false
        )
        .is_err());
    }

    #[test]
    fn mac_addresses_normalise() {
        assert_eq!(normalize_mac("001a79abcdef").unwrap(), "00:1A:79:AB:CD:EF");
        assert!(normalize_mac("00:1A:79:AB:CD:GG").is_err());
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
