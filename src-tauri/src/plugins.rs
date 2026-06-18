// CinaVault Premium — Plugin System Module
use crate::AppState;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Manager, State};
use zip::read::ZipArchive;

const PGMA_PLUGIN_ID: &str = "px-pgma-modernized";
const PGMA_PLUGIN_NAME: &str = "PGMA Modernized";
const PGMA_PLUGIN_VERSION: &str = "master";
const PGMA_REPO_URL: &str = "https://github.com/CodyBerenson/PGMA-Modernized";
const PGMA_ZIP_URL: &str = "https://github.com/CodyBerenson/PGMA-Modernized/archive/refs/heads/master.zip";

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
pub fn get_plugin_repos(state: State<'_, AppState>) -> Result<Vec<PluginRepo>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let mut stmt = db
        .conn
        .prepare("SELECT id, name, url, enabled, last_synced FROM plugin_repos ORDER BY name")
        .map_err(|error| error.to_string())?;
    let repos = {
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
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };
    Ok(repos)
}

#[tauri::command]
pub fn add_plugin_repo(state: State<'_, AppState>, name: String, url: String) -> Result<i64, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.conn
        .execute(
            "INSERT OR IGNORE INTO plugin_repos (name, url, enabled) VALUES (?1, ?2, 1)",
            params![name, url],
        )
        .map_err(|error| error.to_string())?;
    Ok(db.conn.last_insert_rowid())
}

#[tauri::command]
pub fn remove_plugin_repo(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.conn
        .execute("DELETE FROM plugins WHERE repo_id = ?1", params![id])
        .map_err(|error| error.to_string())?;
    db.conn
        .execute("DELETE FROM plugin_repos WHERE id = ?1", params![id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn sync_plugin_catalog(
    state: State<'_, AppState>,
    repo_id: i64,
) -> Result<serde_json::Value, String> {
    let repo = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        let mut stmt = db
            .conn
            .prepare("SELECT url FROM plugin_repos WHERE id = ?1")
            .map_err(|error| error.to_string())?;
        stmt.query_row(params![repo_id], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?
    };

    let resp = reqwest::Client::new()
        .get(&repo)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Failed to fetch manifest: HTTP {}", resp.status()));
    }

    let manifest: Vec<serde_json::Value> = resp.json().await.map_err(|error| error.to_string())?;
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let mut count = 0u64;

    for plugin in &manifest {
        let name = plugin.get("name").and_then(|value| value.as_str()).unwrap_or("Unknown");
        let version = plugin
            .get("versions")
            .and_then(|value| value.as_array())
            .and_then(|items| items.first())
            .and_then(|value| value.get("version"))
            .and_then(|value| value.as_str())
            .map(String::from);
        let desc = plugin.get("description").and_then(|value| value.as_str()).map(String::from);
        let author = plugin.get("owner").and_then(|value| value.as_str()).map(String::from);

        db.conn.execute(
            "INSERT OR REPLACE INTO plugins (name, version, description, author, repo_id, installed, config_json) VALUES (?1, ?2, ?3, ?4, ?5, COALESCE((SELECT installed FROM plugins WHERE name = ?1 AND repo_id = ?5), 0), '{}')",
            params![name, version, desc, author, repo_id],
        ).map_err(|error| error.to_string())?;
        count += 1;
    }

    let now = chrono::Utc::now().to_rfc3339();
    db.conn
        .execute(
            "UPDATE plugin_repos SET last_synced = ?1 WHERE id = ?2",
            params![now, repo_id],
        )
        .map_err(|error| error.to_string())?;

    Ok(serde_json::json!({ "plugins_synced": count }))
}

#[tauri::command]
pub fn get_plugin_catalog(
    state: State<'_, AppState>,
    repo_id: Option<i64>,
) -> Result<Vec<Plugin>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    ensure_pgma_plugin_row(&db.conn).map_err(|error| error.to_string())?;

    let sql = match repo_id {
        Some(_) => "SELECT id, name, version, description, author, repo_id, installed, config_json FROM plugins WHERE repo_id = ?1 ORDER BY name",
        None => "SELECT id, name, version, description, author, repo_id, installed, config_json FROM plugins ORDER BY name",
    };
    let mut stmt = db.conn.prepare(sql).map_err(|error| error.to_string())?;

    let plugins = if let Some(rid) = repo_id {
        let rows = stmt
            .query_map(params![rid], row_to_plugin)
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    } else {
        let rows = stmt
            .query_map([], row_to_plugin)
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };

    Ok(plugins)
}

#[tauri::command]
pub fn install_plugin(
    state: State<'_, AppState>,
    plugin_id: String,
    name: String,
    version: String,
    platforms: Vec<String>,
    repo_url: String,
) -> Result<(), String> {
    let platform = platforms.first().cloned().unwrap_or_else(|| "cinavault".to_string());
    let install_path = if plugin_id == PGMA_PLUGIN_ID {
        "plugins/plex/Plug-ins".to_string()
    } else {
        format!("plugins/{platform}/{plugin_id}")
    };
    let config_json = if plugin_id == PGMA_PLUGIN_ID {
        pgma_default_config_json()
    } else {
        "{}".to_string()
    };

    let db = state.db.lock().map_err(|error| error.to_string())?;
    let updated = db.conn.execute(
        "UPDATE plugins SET name = ?2, version = ?3, installed = 1, enabled = 1, platform = ?4, install_path = ?5, repo_url = ?6, config_json = CASE WHEN ?1 = ?7 AND (config_json IS NULL OR trim(config_json) = '' OR config_json = '{}') THEN ?8 ELSE config_json END WHERE plugin_key = ?1",
        params![plugin_id, name, version, platform, install_path, repo_url, PGMA_PLUGIN_ID, config_json],
    ).map_err(|error| error.to_string())?;

    if updated == 0 {
        db.conn.execute(
            "INSERT INTO plugins (plugin_key, name, version, description, author, repo_id, installed, config_json, platform, install_path, enabled, repo_url) VALUES (?1, ?2, ?3, NULL, NULL, NULL, 1, ?7, ?4, ?5, 1, ?6)",
            params![plugin_id, name, version, platform, install_path, repo_url, config_json],
        ).map_err(|error| error.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn uninstall_plugin(state: State<'_, AppState>, plugin_id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    if plugin_id == PGMA_PLUGIN_ID {
        ensure_pgma_plugin_row(&db.conn).map_err(|error| error.to_string())?;
        return Ok(());
    }
    db.conn
        .execute(
            "UPDATE plugins SET installed = 0, enabled = 0 WHERE plugin_key = ?1 OR name = ?1",
            params![plugin_id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn run_plugin(
    app: AppHandle,
    state: State<'_, AppState>,
    plugin_id: String,
    action: Option<String>,
    config: Option<String>,
) -> Result<serde_json::Value, String> {
    let action = action.unwrap_or_else(|| "run".to_string());
    if plugin_id == PGMA_PLUGIN_ID && matches!(action.as_str(), "run" | "deploy" | "update" | "install") {
        return deploy_pgma_plex_plugin(app, state, config, action).await;
    }

    let db = state.db.lock().map_err(|error| error.to_string())?;
    match action.as_str() {
        "configure" => {
            let config = config.unwrap_or_else(|| "{}".to_string());
            db.conn.execute(
                "UPDATE plugins SET config_json = ?2 WHERE plugin_key = ?1 OR name = ?1",
                params![plugin_id, config],
            ).map_err(|error| error.to_string())?;
        }
        "enable" | "start" => {
            db.conn.execute(
                "UPDATE plugins SET installed = 1, enabled = 1 WHERE plugin_key = ?1 OR name = ?1",
                params![plugin_id],
            ).map_err(|error| error.to_string())?;
        }
        "disable" | "stop" => {
            db.conn.execute(
                "UPDATE plugins SET enabled = 0 WHERE plugin_key = ?1 OR name = ?1",
                params![plugin_id],
            ).map_err(|error| error.to_string())?;
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
pub fn get_installed_plugins(state: State<'_, AppState>) -> Result<Vec<InstalledPlugin>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    ensure_pgma_plugin_row(&db.conn).map_err(|error| error.to_string())?;
    let mut stmt = db.conn.prepare(
        "SELECT COALESCE(plugin_key, name) AS plugin_id, name, COALESCE(version, '1.0.0'), COALESCE(platform, 'cinavault'), COALESCE(install_path, ''), COALESCE(config_json, '{}'), COALESCE(enabled, installed) FROM plugins WHERE installed = 1 ORDER BY name",
    ).map_err(|error| error.to_string())?;
    let installed = {
        let rows = stmt.query_map([], |row| {
            Ok(InstalledPlugin {
                id: row.get(0)?,
                name: row.get(1)?,
                version: row.get(2)?,
                platform: row.get(3)?,
                install_path: row.get(4)?,
                config_json: row.get(5)?,
                enabled: row.get(6)?,
            })
        }).map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };
    Ok(installed)
}

fn row_to_plugin(row: &rusqlite::Row<'_>) -> rusqlite::Result<Plugin> {
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

fn ensure_pgma_plugin_row(conn: &Connection) -> rusqlite::Result<()> {
    let default_config = pgma_default_config_json();
    conn.execute(
        "INSERT INTO plugins (plugin_key, name, version, description, author, repo_id, installed, config_json, platform, install_path, enabled, repo_url) VALUES (?1, ?2, ?3, ?4, ?5, NULL, 1, ?6, 'plex', 'plugins/plex/Plug-ins', 1, ?7) ON CONFLICT(plugin_key) DO UPDATE SET name = excluded.name, version = excluded.version, description = excluded.description, author = excluded.author, installed = 1, enabled = 1, platform = 'plex', install_path = COALESCE(NULLIF(plugins.install_path, ''), excluded.install_path), repo_url = excluded.repo_url, config_json = CASE WHEN plugins.config_json IS NULL OR trim(plugins.config_json) = '' OR plugins.config_json = '{}' THEN excluded.config_json ELSE plugins.config_json END",
        params![PGMA_PLUGIN_ID, PGMA_PLUGIN_NAME, PGMA_PLUGIN_VERSION, "Preinstalled Plex bundle deployer. Downloads PGMA Modernized, extracts only .bundle folders, and stages or deploys them to a Plex Plug-ins folder.", "CodyBerenson / CinaVault", default_config, PGMA_REPO_URL],
    )?;
    Ok(())
}

fn pgma_default_config_json() -> String {
    serde_json::json!({
        "plexPluginPath": "",
        "sourceZipUrl": PGMA_ZIP_URL,
        "defaultTarget": "cinavault-staging",
        "notes": "Leave plexPluginPath blank to deploy into CinaVault's local Plex plugin staging folder. Set it manually only when you want to deploy directly to Plex.",
        "requiresPlexRestart": true,
    })
    .to_string()
}

async fn deploy_pgma_plex_plugin(
    app: AppHandle,
    state: State<'_, AppState>,
    config: Option<String>,
    action: String,
) -> Result<serde_json::Value, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|error| error.to_string())?;
    let default_target = app_data_dir.join("plugins").join("plex").join("Plug-ins");
    let temp_extract = app_data_dir.join("plugins").join("plex").join("_pgma_extract");
    let stored_config = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        ensure_pgma_plugin_row(&db.conn).map_err(|error| error.to_string())?;
        db.conn
            .query_row(
                "SELECT config_json FROM plugins WHERE plugin_key = ?1",
                params![PGMA_PLUGIN_ID],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
    };
    let effective_config = config
        .or(stored_config)
        .filter(|raw| !raw.trim().is_empty())
        .unwrap_or_else(pgma_default_config_json);
    let fallback_config = pgma_default_config_json();
    let config_value: serde_json::Value = serde_json::from_str(&effective_config)
        .or_else(|_| serde_json::from_str(&fallback_config))
        .unwrap_or_default();

    let source_url = config_value
        .get("sourceZipUrl")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| PGMA_ZIP_URL.to_string());
    let configured_target = config_value
        .get("plexPluginPath")
        .or_else(|| config_value.get("nasPath"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let target_dir = configured_target
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| default_target.clone());
    let using_default_staging = configured_target.is_none();

    fs::create_dir_all(&target_dir)
        .map_err(|error| format!("Unable to create Plex plugin target '{}': {error}", target_dir.display()))?;
    if temp_extract.exists() {
        fs::remove_dir_all(&temp_extract).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&temp_extract).map_err(|error| error.to_string())?;

    let response = reqwest::Client::new()
        .get(&source_url)
        .send()
        .await
        .map_err(|error| format!("Unable to download PGMA Modernized: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("PGMA download failed: HTTP {}", response.status()));
    }
    let zip_bytes = response
        .bytes()
        .await
        .map_err(|error| format!("Unable to read PGMA archive: {error}"))?;
    let mut archive = ZipArchive::new(Cursor::new(zip_bytes))
        .map_err(|error| format!("Downloaded PGMA archive is not a valid ZIP: {error}"))?;

    let mut bundle_names = BTreeSet::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| error.to_string())?;
        let Some((bundle_name, relative_path)) = safe_bundle_relative_path(file.name()) else {
            continue;
        };
        bundle_names.insert(bundle_name);
        let out_path = temp_extract.join(relative_path);
        if file.is_dir() {
            fs::create_dir_all(&out_path).map_err(|error| error.to_string())?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            let mut output = fs::File::create(&out_path).map_err(|error| error.to_string())?;
            std::io::copy(&mut file, &mut output).map_err(|error| error.to_string())?;
            output.flush().map_err(|error| error.to_string())?;
        }
    }

    if bundle_names.is_empty() {
        let _ = fs::remove_dir_all(&temp_extract);
        return Err("PGMA archive did not contain any .bundle plugin folders.".to_string());
    }

    let mut deployed = Vec::new();
    for bundle_name in &bundle_names {
        let source_bundle = temp_extract.join(bundle_name);
        if !source_bundle.exists() {
            continue;
        }
        let target_bundle = target_dir.join(bundle_name);
        if target_bundle.exists() {
            fs::remove_dir_all(&target_bundle).map_err(|error| {
                format!("Unable to replace existing bundle '{}': {error}", target_bundle.display())
            })?;
        }
        copy_dir_all(&source_bundle, &target_bundle).map_err(|error| {
            format!(
                "Unable to copy bundle '{}' to '{}': {error}",
                source_bundle.display(),
                target_bundle.display()
            )
        })?;
        deployed.push(bundle_name.clone());
    }

    let _ = fs::remove_dir_all(&temp_extract);
    let target_path = target_dir.to_string_lossy().to_string();
    let saved_config = serde_json::json!({
        "plexPluginPath": configured_target.clone().unwrap_or_default(),
        "sourceZipUrl": source_url,
        "defaultTarget": "cinavault-staging",
        "lastDeployTarget": target_path,
        "lastDeployedAt": chrono::Utc::now().to_rfc3339(),
        "deployedBundles": deployed.clone(),
        "requiresPlexRestart": true,
    })
    .to_string();

    {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        db.conn.execute(
            "UPDATE plugins SET installed = 1, enabled = 1, platform = 'plex', version = ?2, install_path = ?3, repo_url = ?4, config_json = ?5 WHERE plugin_key = ?1",
            params![PGMA_PLUGIN_ID, PGMA_PLUGIN_VERSION, target_path, PGMA_REPO_URL, saved_config],
        ).map_err(|error| error.to_string())?;
    }

    Ok(serde_json::json!({
        "success": true,
        "status": "pgma_deployed",
        "plugin_id": PGMA_PLUGIN_ID,
        "action": action,
        "source": source_url,
        "targetPath": target_path,
        "usingDefaultStaging": using_default_staging,
        "bundles": deployed,
        "requiresPlexRestart": true,
        "message": if using_default_staging {
            "PGMA bundles deployed to CinaVault's local Plex staging folder. Set plexPluginPath in plugin settings to deploy directly to Plex."
        } else {
            "PGMA bundles deployed to the configured Plex Plug-ins folder. Restart Plex to load them."
        }
    }))
}

fn safe_bundle_relative_path(name: &str) -> Option<(String, PathBuf)> {
    let normalized = name.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').filter(|part| !part.is_empty()).collect();
    if parts.iter().any(|part| *part == "." || *part == ".." || part.contains(':')) {
        return None;
    }
    let bundle_index = parts.iter().position(|part| part.ends_with(".bundle"))?;
    let bundle_name = parts[bundle_index].to_string();
    let mut relative_path = PathBuf::new();
    for part in &parts[bundle_index..] {
        relative_path.push(part);
    }
    Some((bundle_name, relative_path))
}

fn copy_dir_all(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let target_path = target.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &target_path)?;
        } else {
            fs::copy(entry.path(), target_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_bundle_path_inside_pgma_archive() {
        let (bundle, rel) = safe_bundle_relative_path("PGMA-Modernized-master/Example.bundle/Contents/Info.plist")
            .expect("bundle path should be accepted");
        assert_eq!(bundle, "Example.bundle");
        assert_eq!(rel, PathBuf::from("Example.bundle").join("Contents").join("Info.plist"));
    }

    #[test]
    fn rejects_zip_slip_paths() {
        assert!(safe_bundle_relative_path("PGMA-Modernized-master/../Bad.bundle/file.py").is_none());
        assert!(safe_bundle_relative_path("C:/Bad.bundle/file.py").is_none());
    }
}
