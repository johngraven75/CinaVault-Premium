// CinaVault Premium — Plugin System Module
use crate::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PluginRepo {
    pub id: Option<i64>,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub last_synced: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Plugin {
    pub id: Option<i64>,
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub repo_id: Option<i64>,
    pub installed: bool,
    pub config_json: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPlugin {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub version: String,
    pub install_path: String,
    pub config_json: String,
    pub enabled: bool,
}

#[tauri::command]
pub fn get_plugin_repos(state: State<AppState>) -> Result<Vec<PluginRepo>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db
        .conn
        .prepare("SELECT id, name, url, enabled, last_synced FROM plugin_repos ORDER BY name")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(PluginRepo {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                url: row.get(2)?,
                enabled: row.get(3)?,
                last_synced: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_plugin_repo(state: State<AppState>, name: String, url: String) -> Result<i64, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn
        .execute(
            "INSERT OR IGNORE INTO plugin_repos (name, url, enabled) VALUES (?1, ?2, 1)",
            params![name, url],
        )
        .map_err(|e| e.to_string())?;
    Ok(db.conn.last_insert_rowid())
}

#[tauri::command]
pub fn remove_plugin_repo(state: State<AppState>, id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn
        .execute("DELETE FROM plugins WHERE repo_id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    db.conn
        .execute("DELETE FROM plugin_repos WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn sync_plugin_catalog(
    state: State<'_, AppState>,
    repo_id: i64,
) -> Result<serde_json::Value, String> {
    let repo = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let mut stmt = db
            .conn
            .prepare("SELECT url FROM plugin_repos WHERE id = ?1")
            .map_err(|e| e.to_string())?;
        stmt.query_row(params![repo_id], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?
    };

    let client = reqwest::Client::new();
    let resp = client.get(&repo).send().await.map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("Failed to fetch manifest: HTTP {}", resp.status()));
    }

    let manifest: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let mut count = 0u64;
    for plugin in &manifest {
        let name = plugin
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let version = plugin
            .get("versions")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.get("version"))
            .and_then(|v| v.as_str())
            .map(String::from);
        let desc = plugin
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);
        let author = plugin
            .get("owner")
            .and_then(|v| v.as_str())
            .map(String::from);

        db.conn.execute(
            "INSERT OR REPLACE INTO plugins (name, version, description, author, repo_id, installed, config_json) \
             VALUES (?1, ?2, ?3, ?4, ?5, COALESCE((SELECT installed FROM plugins WHERE name = ?1 AND repo_id = ?5), 0), '{}')",
            params![name, version, desc, author, repo_id],
        ).map_err(|e| e.to_string())?;
        count += 1;
    }

    let now = chrono::Utc::now().to_rfc3339();
    db.conn
        .execute(
            "UPDATE plugin_repos SET last_synced = ?1 WHERE id = ?2",
            params![now, repo_id],
        )
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "plugins_synced": count }))
}

#[tauri::command]
pub fn get_plugin_catalog(
    state: State<AppState>,
    repo_id: Option<i64>,
) -> Result<Vec<Plugin>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let sql = match repo_id {
        Some(_) => "SELECT id, name, version, description, author, repo_id, installed, config_json FROM plugins WHERE repo_id = ?1 ORDER BY name",
        None => "SELECT id, name, version, description, author, repo_id, installed, config_json FROM plugins ORDER BY name",
    };
    let mut stmt = db.conn.prepare(sql).map_err(|e| e.to_string())?;
    if let Some(rid) = repo_id {
        let rows = stmt
            .query_map(params![rid], row_to_plugin)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    } else {
        let rows = stmt
            .query_map([], row_to_plugin)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn install_plugin(
    state: State<AppState>,
    plugin_id: String,
    name: String,
    version: String,
    platforms: Vec<String>,
    repo_url: String,
) -> Result<(), String> {
    let platform = platforms
        .first()
        .cloned()
        .unwrap_or_else(|| "cinavault".to_string());
    let install_path = format!("plugins/{platform}/{plugin_id}");
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let updated = db
        .conn
        .execute(
            "UPDATE plugins
             SET name = ?2,
                 version = ?3,
                 installed = 1,
                 enabled = 1,
                 platform = ?4,
                 install_path = ?5,
                 repo_url = ?6
             WHERE plugin_key = ?1",
            params![plugin_id, name, version, platform, install_path, repo_url],
        )
        .map_err(|e| e.to_string())?;

    if updated == 0 {
        db.conn
            .execute(
                "INSERT INTO plugins
                 (plugin_key, name, version, description, author, repo_id, installed, config_json, platform, install_path, enabled, repo_url)
                 VALUES (?1, ?2, ?3, NULL, NULL, NULL, 1, '{}', ?4, ?5, 1, ?6)",
                params![plugin_id, name, version, platform, install_path, repo_url],
            )
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn uninstall_plugin(state: State<AppState>, plugin_id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn
        .execute(
            "UPDATE plugins SET installed = 0, enabled = 0 WHERE plugin_key = ?1 OR name = ?1",
            params![plugin_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn run_plugin(
    state: State<AppState>,
    plugin_id: String,
    action: Option<String>,
    config: Option<String>,
) -> Result<serde_json::Value, String> {
    let action = action.unwrap_or_else(|| "run".to_string());
    let db = state.db.lock().map_err(|e| e.to_string())?;

    match action.as_str() {
        "configure" => {
            let config = config.unwrap_or_else(|| "{}".to_string());
            db.conn
                .execute(
                    "UPDATE plugins SET config_json = ?2 WHERE plugin_key = ?1 OR name = ?1",
                    params![plugin_id, config],
                )
                .map_err(|e| e.to_string())?;
        }
        "enable" | "start" => {
            db.conn
                .execute(
                    "UPDATE plugins SET installed = 1, enabled = 1 WHERE plugin_key = ?1 OR name = ?1",
                    params![plugin_id],
                )
                .map_err(|e| e.to_string())?;
        }
        "disable" | "stop" => {
            db.conn
                .execute(
                    "UPDATE plugins SET enabled = 0 WHERE plugin_key = ?1 OR name = ?1",
                    params![plugin_id],
                )
                .map_err(|e| e.to_string())?;
        }
        _ => {}
    }

    Ok(serde_json::json!({
        "status": "plugin_executed",
        "plugin_id": plugin_id,
        "action": action,
    }))
}

#[tauri::command]
pub fn get_installed_plugins(state: State<AppState>) -> Result<Vec<InstalledPlugin>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db
        .conn
        .prepare(
            "SELECT COALESCE(plugin_key, name) AS plugin_id,
                    name,
                    COALESCE(version, '1.0.0'),
                    COALESCE(platform, 'cinavault'),
                    COALESCE(install_path, ''),
                    COALESCE(config_json, '{}'),
                    COALESCE(enabled, installed)
             FROM plugins
             WHERE installed = 1
             ORDER BY name",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(InstalledPlugin {
                id: row.get(0)?,
                name: row.get(1)?,
                version: row.get(2)?,
                platform: row.get(3)?,
                install_path: row.get(4)?,
                config_json: row.get(5)?,
                enabled: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn row_to_plugin(row: &rusqlite::Row) -> rusqlite::Result<Plugin> {
    Ok(Plugin {
        id: Some(row.get(0)?),
        name: row.get(1)?,
        version: row.get(2)?,
        description: row.get(3)?,
        author: row.get(4)?,
        repo_id: row.get(5)?,
        installed: row.get(6)?,
        config_json: row.get(7)?,
    })
}
