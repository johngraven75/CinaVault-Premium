// CinaVault Premium — Media Player Module
use serde::{Deserialize, Serialize};
use std::process::Command;
use tauri::State;
use crate::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlayerInfo {
    pub name: String,
    pub executable: String,
    pub available: bool,
}

const KNOWN_PLAYERS: &[(&str, &[&str])] = &[
    ("VLC Media Player", &["C:\\Program Files\\VideoLAN\\VLC\\vlc.exe", "C:\\Program Files (x86)\\VideoLAN\\VLC\\vlc.exe"]),
    ("mpv", &["C:\\Program Files\\mpv\\mpv.exe", "C:\\ProgramData\\chocolatey\\bin\\mpv.exe"]),
    ("MPC-HC", &["C:\\Program Files\\MPC-HC\\mpc-hc64.exe", "C:\\Program Files (x86)\\MPC-HC\\mpc-hc.exe"]),
    ("PotPlayer", &["C:\\Program Files\\DAUM\\PotPlayer\\PotPlayerMini64.exe"]),
    ("Windows Media Player", &["C:\\Program Files\\Windows Media Player\\wmplayer.exe"]),
];

#[tauri::command]
pub fn get_available_players() -> Vec<PlayerInfo> {
    let mut players = vec![
        PlayerInfo {
            name: "System Default".to_string(),
            executable: "system".to_string(),
            available: true,
        }
    ];

    for (name, paths) in KNOWN_PLAYERS {
        let mut found = false;
        let mut exe_path = String::new();
        for path in *paths {
            if std::path::Path::new(path).exists() {
                found = true;
                exe_path = path.to_string();
                break;
            }
        }
        players.push(PlayerInfo {
            name: name.to_string(),
            executable: exe_path,
            available: found,
        });
    }

    players
}

#[tauri::command]
pub async fn play_media(file_path: String, player: Option<String>) -> Result<(), String> {
    let player_exe = player.unwrap_or_else(|| "system".to_string());

    if player_exe == "system" {
        open::that(&file_path).map_err(|e| e.to_string())?;
    } else if std::path::Path::new(&player_exe).exists() {
        Command::new(&player_exe)
            .arg(&file_path)
            .spawn()
            .map_err(|e| e.to_string())?;
    } else {
        // Fallback to system default
        open::that(&file_path).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn set_default_player(state: State<AppState>, player: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.set_setting_data("default_player", &player).map_err(|e| e.to_string())
}
