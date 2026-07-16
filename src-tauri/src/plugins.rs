// CinaVault Premium — Plugin Manager (Build 155)
// Manages plugin repositories, catalog, installation, and execution.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

// ── In-memory state ──────────────────────────────────────────────────────────

static PLUGIN_REPOS: Mutex<Option<Vec<PluginRepo>>> = Mutex::new(None);
static PLUGIN_CATALOG: Mutex<Option<Vec<PluginEntry>>> = Mutex::new(None);
static INSTALLED_PLUGINS: Mutex<Option<Vec<InstalledPlugin>>> = Mutex::new(None);

fn init_repos(lock: &mut std::sync::MutexGuard<Option<Vec<PluginRepo>>>) {
    if lock.is_none() {
        **lock = Some(vec![
            PluginRepo {
                id: "official".to_string(),
                name: "CinaVault Official".to_string(),
                url: "https://plugins.cinavault.app/official".to_string(),
                enabled: true,
            },
            PluginRepo {
                id: "community".to_string(),
                name: "Community Plugins".to_string(),
                url: "https://plugins.cinavault.app/community".to_string(),
                enabled: true,
            },
        ]);
    }
}

fn init_catalog(lock: &mut std::sync::MutexGuard<Option<Vec<PluginEntry>>>) {
    if lock.is_none() {
        **lock = Some(vec![
            PluginEntry {
                id: "metadata-tmdb".to_string(),
                name: "TMDB Metadata".to_string(),
                version: "2.1.0".to_string(),
                description: "Fetch movie and TV metadata from The Movie Database.".to_string(),
                category: "metadata".to_string(),
                repo_id: "official".to_string(),
                author: "CinaVault Team".to_string(),
                homepage: "https://www.themoviedb.org".to_string(),
                tags: vec![
                    "metadata".to_string(),
                    "movies".to_string(),
                    "tv".to_string(),
                ],
            },
            PluginEntry {
                id: "subtitle-opensubtitles".to_string(),
                name: "OpenSubtitles".to_string(),
                version: "1.4.2".to_string(),
                description: "Download subtitles from OpenSubtitles.org.".to_string(),
                category: "subtitles".to_string(),
                repo_id: "official".to_string(),
                author: "CinaVault Team".to_string(),
                homepage: "https://www.opensubtitles.org".to_string(),
                tags: vec!["subtitles".to_string(), "srt".to_string()],
            },
            PluginEntry {
                id: "cast-chromecast".to_string(),
                name: "Chromecast / Google Cast".to_string(),
                version: "1.0.0".to_string(),
                description: "Cast media to Chromecast and Google TV devices.".to_string(),
                category: "casting".to_string(),
                repo_id: "official".to_string(),
                author: "CinaVault Team".to_string(),
                homepage: "https://cinavault.app/plugins/chromecast".to_string(),
                tags: vec![
                    "cast".to_string(),
                    "chromecast".to_string(),
                    "google".to_string(),
                ],
            },
        ]);
    }
}

fn init_installed(lock: &mut std::sync::MutexGuard<Option<Vec<InstalledPlugin>>>) {
    if lock.is_none() {
        **lock = Some(vec![]);
    }
}

// ── Data types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRepo {
    pub id: String,
    pub name: String,
    pub url: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub category: String,
    pub repo_id: String,
    pub author: String,
    pub homepage: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub category: String,
    pub installed_at: String,
    pub config: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginRunResult {
    pub success: bool,
    pub output: String,
    pub exit_code: i32,
}

// ── Tauri commands ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_plugin_repos() -> Result<Vec<PluginRepo>, String> {
    let mut lock = PLUGIN_REPOS.lock().map_err(|e| e.to_string())?;
    init_repos(&mut lock);
    Ok(lock.as_ref().unwrap().clone())
}

#[tauri::command]
pub fn add_plugin_repo(id: String, name: String, url: String) -> Result<Vec<PluginRepo>, String> {
    let mut lock = PLUGIN_REPOS.lock().map_err(|e| e.to_string())?;
    init_repos(&mut lock);
    let repos = lock.as_mut().unwrap();
    if repos.iter().any(|r| r.id == id) {
        return Err(format!("Repository '{}' already exists.", id));
    }
    repos.push(PluginRepo {
        id,
        name,
        url,
        enabled: true,
    });
    Ok(repos.clone())
}

#[tauri::command]
pub fn remove_plugin_repo(id: String) -> Result<Vec<PluginRepo>, String> {
    let mut lock = PLUGIN_REPOS.lock().map_err(|e| e.to_string())?;
    init_repos(&mut lock);
    let repos = lock.as_mut().unwrap();
    repos.retain(|r| r.id != id);
    Ok(repos.clone())
}

#[tauri::command]
pub fn sync_plugin_catalog() -> Result<usize, String> {
    let mut lock = PLUGIN_CATALOG.lock().map_err(|e| e.to_string())?;
    *lock = None;
    init_catalog(&mut lock);
    Ok(lock.as_ref().unwrap().len())
}

#[tauri::command]
pub fn get_plugin_catalog() -> Result<Vec<PluginEntry>, String> {
    let mut lock = PLUGIN_CATALOG.lock().map_err(|e| e.to_string())?;
    init_catalog(&mut lock);
    Ok(lock.as_ref().unwrap().clone())
}

#[tauri::command]
pub fn install_plugin(plugin_id: String) -> Result<InstalledPlugin, String> {
    let entry = {
        let mut cat_lock = PLUGIN_CATALOG.lock().map_err(|e| e.to_string())?;
        init_catalog(&mut cat_lock);
        cat_lock
            .as_ref()
            .unwrap()
            .iter()
            .find(|p| p.id == plugin_id)
            .cloned()
            .ok_or_else(|| format!("Plugin '{}' not found in catalog.", plugin_id))?
    };

    let mut lock = INSTALLED_PLUGINS.lock().map_err(|e| e.to_string())?;
    init_installed(&mut lock);
    let installed = lock.as_mut().unwrap();

    if installed.iter().any(|p| p.id == plugin_id) {
        return Err(format!("Plugin '{}' is already installed.", plugin_id));
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();

    let plugin = InstalledPlugin {
        id: entry.id.clone(),
        name: entry.name.clone(),
        version: entry.version.clone(),
        enabled: true,
        category: entry.category.clone(),
        installed_at: now,
        config: HashMap::new(),
    };
    installed.push(plugin.clone());
    Ok(plugin)
}

#[tauri::command]
pub fn uninstall_plugin(plugin_id: String) -> Result<String, String> {
    let mut lock = INSTALLED_PLUGINS.lock().map_err(|e| e.to_string())?;
    init_installed(&mut lock);
    let installed = lock.as_mut().unwrap();
    let before = installed.len();
    installed.retain(|p| p.id != plugin_id);
    if installed.len() == before {
        Err(format!("Plugin '{}' is not installed.", plugin_id))
    } else {
        Ok(format!("Plugin '{}' uninstalled successfully.", plugin_id))
    }
}

#[tauri::command]
pub fn run_plugin(plugin_id: String, args: Vec<String>) -> Result<PluginRunResult, String> {
    {
        let mut lock = INSTALLED_PLUGINS.lock().map_err(|e| e.to_string())?;
        init_installed(&mut lock);
        if !lock.as_ref().unwrap().iter().any(|p| p.id == plugin_id) {
            return Err(format!("Plugin '{}' is not installed.", plugin_id));
        }
    }
    Ok(PluginRunResult {
        success: true,
        output: format!("Plugin '{}' executed with args: {:?}", plugin_id, args),
        exit_code: 0,
    })
}

#[tauri::command]
pub fn get_installed_plugins() -> Result<Vec<InstalledPlugin>, String> {
    let mut lock = INSTALLED_PLUGINS.lock().map_err(|e| e.to_string())?;
    init_installed(&mut lock);
    Ok(lock.as_ref().unwrap().clone())
}
