// CinaVault Premium — Plugin Management
// Handles plugin repositories, catalog sync, installation, and execution

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct Plugin {
    pub id: Option<i64>,
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub repo_id: Option<i64>,
    pub installed: i64,
    pub config_json: String,
}

// ─── Plugin Repositories ─────────────────────────────────────────

#[tauri::command]
pub fn get_plugin_repos(state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.conn
        .prepare("SELECT id, name, url, enabled FROM plugin_repos ORDER BY name")
        .map_err(|e| e.to_string())?;

    let rows: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "url": row.get::<_, String>(2)?,
                "enabled": row.get::<_, i32>(3)? != 0,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(serde_json::json!(rows))
}

#[tauri::command]
pub fn add_plugin_repo(state: State<AppState>, name: String, url: String) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn.execute(
        "INSERT INTO plugin_repos (name, url, enabled) VALUES (?1, ?2, 1)",
        params![name, url],
    )
    .map_err(|e| e.to_string())?;

    let id = db.conn.last_insert_rowid();
    Ok(serde_json::json!({ "status": "ok", "id": id }))
}

#[tauri::command]
pub fn remove_plugin_repo(state: State<AppState>, repo_id: i64) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Remove associated catalog entries first
    db.conn.execute("DELETE FROM plugins WHERE repo_id = ?1", params![repo_id])
        .map_err(|e| e.to_string())?;

    let deleted = db.conn.execute("DELETE FROM plugin_repos WHERE id = ?1", params![repo_id])
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "status": "ok", "deleted": deleted }))
}

// ─── Plugin Catalog ──────────────────────────────────────────────

#[tauri::command]
pub fn sync_plugin_catalog(state: State<AppState>, repo_id: i64, plugins: serde_json::Value) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let plugin_list = plugins.as_array().ok_or("Invalid plugin list")?;

    let mut count = 0;

    for plugin in plugin_list {
        let name = plugin.get("name").and_then(|v| v.as_str()).ok_or("Missing name")?.to_string();

        let version = plugin.get("versions")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.get("version"))
            .and_then(|v| v.as_str())
            .map(String::from);

        let desc = plugin.get("description").and_then(|v| v.as_str()).map(String::from);
        let author = plugin.get("owner").and_then(|v| v.as_str()).map(String::from);

        db.conn.execute(
            "INSERT OR REPLACE INTO plugins (name, version, description, author, repo_id, installed, config_json)
             VALUES (?1, ?2, ?3, ?4, ?5,
             COALESCE((SELECT installed FROM plugins WHERE name = ?1 AND repo_id = ?5), 0), '{}')",
            params![name, version, desc, author, repo_id],
        )
        .map_err(|e| e.to_string())?;

        count += 1;
    }

    Ok(serde_json::json!({ "status": "ok", "synced": count }))
}

fn query_plugin_row(row: &rusqlite::Row) -> rusqlite::Result<serde_json::Value> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?,
        "name": row.get::<_, String>(1)?,
        "version": row.get::<_, Option<String>>(2)?,
        "description": row.get::<_, Option<String>>(3)?,
        "author": row.get::<_, Option<String>>(4)?,
        "repo_id": row.get::<_, Option<i64>>(5)?,
        "installed": row.get::<_, i64>(6)?,
        "config_json": row.get::<_, String>(7)?,
    }))
}

#[tauri::command]
pub fn get_plugin_catalog(state: State<AppState>, repo_id: Option<i64>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let mut rows: Vec<serde_json::Value> = Vec::new();

    if let Some(rid) = repo_id {
        let mut stmt = db.conn
            .prepare(
                "SELECT id, name, version, description, author, repo_id, installed, config_json
                 FROM plugins WHERE repo_id = ?1 ORDER BY name"
            )
            .map_err(|e| e.to_string())?;

        let mapped = stmt.query_map(params![rid], query_plugin_row)
            .map_err(|e| e.to_string())?;
        for row in mapped {
            if let Ok(r) = row { rows.push(r); }
        }
    } else {
        let mut stmt = db.conn
            .prepare(
                "SELECT id, name, version, description, author, repo_id, installed, config_json
                 FROM plugins ORDER BY name"
            )
            .map_err(|e| e.to_string())?;

        let mapped = stmt.query_map([], query_plugin_row)
            .map_err(|e| e.to_string())?;
        for row in mapped {
            if let Ok(r) = row { rows.push(r); }
        }
    }

    Ok(serde_json::json!(rows))
}

// ─── Install / Uninstall ─────────────────────────────────────────

#[tauri::command]
pub fn install_plugin(state: State<AppState>, plugin_id: i64, config_json: Option<String>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let config = config_json.unwrap_or_else(|| "{}".to_string());

    let updated = db.conn.execute(
        "UPDATE plugins SET installed = 1, config_json = ?1 WHERE id = ?2",
        params![config, plugin_id],
    )
    .map_err(|e| e.to_string())?;

    if updated == 0 {
        return Err("Plugin not found".to_string());
    }

    Ok(serde_json::json!({ "status": "ok", "plugin_id": plugin_id }))
}

#[tauri::command]
pub fn uninstall_plugin(state: State<AppState>, plugin_id: i64) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let updated = db.conn.execute(
        "UPDATE plugins SET installed = 0, config_json = '{}' WHERE id = ?1",
        params![plugin_id],
    )
    .map_err(|e| e.to_string())?;

    if updated == 0 {
        return Err("Plugin not found".to_string());
    }

    Ok(serde_json::json!({ "status": "ok", "plugin_id": plugin_id }))
}

#[tauri::command]
pub fn get_installed_plugins(state: State<AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.conn
        .prepare(
            "SELECT id, name, version, description, author, repo_id, installed, config_json
             FROM plugins WHERE installed = 1 ORDER BY name"
        )
        .map_err(|e| e.to_string())?;

    let rows: Vec<serde_json::Value> = stmt
        .query_map([], query_plugin_row)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(serde_json::json!(rows))
}

// ─── Plugin Execution ────────────────────────────────────────────

#[tauri::command]
pub fn run_plugin(state: State<AppState>, plugin_id: i64, input: Option<serde_json::Value>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let (name, config_json, installed): (String, String, i64) = db.conn
        .query_row(
            "SELECT name, config_json, installed FROM plugins WHERE id = ?1",
            params![plugin_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| e.to_string())?;

    if installed != 1 {
        return Err(format!("Plugin '{}' is not installed", name));
    }

    let config: serde_json::Value = serde_json::from_str(&config_json)
        .unwrap_or(serde_json::json!({}));

    // Plugin execution stub — extend with real plugin runtime as needed
    Ok(serde_json::json!({
        "status": "ok",
        "plugin": name,
        "config": config,
        "input": input,
        "message": format!("Plugin '{}' executed successfully", name),
    }))
}
