// CinaVault Premium — Downloads Module (yt-dlp + ffmpeg)
use crate::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::State;

static DOWNLOADING: AtomicBool = AtomicBool::new(false);
static CANCEL_DL: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct DownloadItem {
    pub id: Option<i64>,
    pub url: String,
    pub title: Option<String>,
    pub status: String,
    pub file_path: Option<String>,
    pub file_size: Option<i64>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn check_download_tools() -> Result<serde_json::Value, String> {
    let ytdlp = Command::new("yt-dlp").arg("--version").output();
    let ffmpeg = Command::new("ffmpeg").arg("-version").output();
    let ffprobe = Command::new("ffprobe").arg("-version").output();

    Ok(serde_json::json!({
        "yt_dlp": {
            "installed": ytdlp.is_ok() && ytdlp.as_ref().unwrap().status.success(),
            "version": ytdlp.ok().map(|o| String::from_utf8_lossy(&o.stdout).lines().next().unwrap_or("").to_string()),
        },
        "ffmpeg": {
            "installed": ffmpeg.is_ok() && ffmpeg.as_ref().unwrap().status.success(),
            "version": ffmpeg.ok().map(|o| String::from_utf8_lossy(&o.stdout).lines().next().unwrap_or("").to_string()),
        },
        "ffprobe": {
            "installed": ffprobe.is_ok() && ffprobe.as_ref().unwrap().status.success(),
        },
    }))
}

#[tauri::command]
pub async fn install_download_tools() -> Result<serde_json::Value, String> {
    #[cfg(target_os = "windows")]
    {
        let mut results = Vec::new();

        // Install yt-dlp
        let ytdlp = Command::new("winget")
            .args([
                "install",
                "--id",
                "yt-dlp.yt-dlp",
                "--accept-package-agreements",
                "--accept-source-agreements",
            ])
            .output();
        results.push(serde_json::json!({
            "tool": "yt-dlp",
            "success": ytdlp.as_ref().map(|o| o.status.success()).unwrap_or(false),
            "output": ytdlp.ok().map(|o| String::from_utf8_lossy(&o.stdout).to_string()),
        }));

        // Install ffmpeg
        let ffmpeg = Command::new("winget")
            .args([
                "install",
                "--id",
                "Gyan.FFmpeg",
                "--accept-package-agreements",
                "--accept-source-agreements",
            ])
            .output();
        results.push(serde_json::json!({
            "tool": "ffmpeg",
            "success": ffmpeg.as_ref().map(|o| o.status.success()).unwrap_or(false),
            "output": ffmpeg.ok().map(|o| String::from_utf8_lossy(&o.stdout).to_string()),
        }));

        Ok(serde_json::json!({ "results": results }))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(serde_json::json!({
            "status": "unsupported",
            "message": "Auto-install via winget is only available on Windows",
        }))
    }
}

#[tauri::command]
pub async fn start_download(
    state: State<'_, AppState>,
    url: String,
    output_dir: Option<String>,
    format: Option<String>,
) -> Result<serde_json::Value, String> {
    if DOWNLOADING.load(Ordering::Relaxed) {
        return Err("A download is already in progress".into());
    }
    DOWNLOADING.store(true, Ordering::Relaxed);
    CANCEL_DL.store(false, Ordering::Relaxed);

    let out_dir = output_dir.unwrap_or_else(|| {
        dirs::download_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("Downloads"))
            .to_string_lossy()
            .to_string()
    });

    let fmt = format.unwrap_or_else(|| "bestvideo+bestaudio/best".to_string());

    let now = chrono::Utc::now().to_rfc3339();
    let db_id = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.conn.execute(
            "INSERT INTO download_history (url, status, started_at) VALUES (?1, 'downloading', ?2)",
            params![url, now],
        ).map_err(|e| e.to_string())?;
        db.conn.last_insert_rowid()
    };

    let output = Command::new("yt-dlp")
        .args([
            "-f",
            &fmt,
            "--merge-output-format",
            "mp4",
            "-o",
            &format!("{}/%(title)s.%(ext)s", out_dir),
            "--no-playlist",
            "--newline",
            &url,
        ])
        .output()
        .map_err(|e| {
            DOWNLOADING.store(false, Ordering::Relaxed);
            format!("yt-dlp failed: {}", e)
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let success = output.status.success();

    // Extract title from output
    let title = stdout
        .lines()
        .find(|l| l.contains("[download] Destination:"))
        .map(|l| l.replace("[download] Destination:", "").trim().to_string());

    let completed_at = chrono::Utc::now().to_rfc3339();
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        if success {
            db.conn.execute(
                "UPDATE download_history SET status = 'completed', title = ?1, completed_at = ?2 WHERE id = ?3",
                params![title, completed_at, db_id],
            ).map_err(|e| e.to_string())?;
        } else {
            db.conn.execute(
                "UPDATE download_history SET status = 'failed', error = ?1, completed_at = ?2 WHERE id = ?3",
                params![stderr, completed_at, db_id],
            ).map_err(|e| e.to_string())?;
        }
    }

    DOWNLOADING.store(false, Ordering::Relaxed);

    Ok(serde_json::json!({
        "id": db_id,
        "status": if success { "completed" } else { "failed" },
        "title": title,
        "output": stdout,
        "error": if stderr.is_empty() { None } else { Some(stderr) },
    }))
}

#[tauri::command]
pub async fn start_playlist_download(
    state: State<'_, AppState>,
    url: String,
    output_dir: Option<String>,
) -> Result<serde_json::Value, String> {
    let out_dir = output_dir.unwrap_or_else(|| {
        dirs::download_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("Downloads"))
            .to_string_lossy()
            .to_string()
    });

    let now = chrono::Utc::now().to_rfc3339();
    let db_id = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.conn.execute(
            "INSERT INTO download_history (url, title, status, started_at) VALUES (?1, 'Playlist', 'downloading', ?2)",
            params![url, now],
        ).map_err(|e| e.to_string())?;
        db.conn.last_insert_rowid()
    };

    let output = Command::new("yt-dlp")
        .args([
            "-f",
            "bestvideo+bestaudio/best",
            "--merge-output-format",
            "mp4",
            "-o",
            &format!("{}/%(playlist_title)s/%(title)s.%(ext)s", out_dir),
            "--yes-playlist",
            "--newline",
            &url,
        ])
        .output()
        .map_err(|e| format!("yt-dlp playlist failed: {}", e))?;

    let success = output.status.success();
    let completed_at = chrono::Utc::now().to_rfc3339();
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let status = if success { "completed" } else { "failed" };
        let err = if success {
            None
        } else {
            Some(String::from_utf8_lossy(&output.stderr).to_string())
        };
        db.conn.execute(
            "UPDATE download_history SET status = ?1, error = ?2, completed_at = ?3 WHERE id = ?4",
            params![status, err, completed_at, db_id],
        ).map_err(|e| e.to_string())?;
    }

    Ok(serde_json::json!({
        "id": db_id,
        "status": if success { "completed" } else { "failed" },
        "output": String::from_utf8_lossy(&output.stdout).to_string(),
    }))
}

#[tauri::command]
pub fn get_download_progress() -> serde_json::Value {
    serde_json::json!({
        "downloading": DOWNLOADING.load(Ordering::Relaxed),
    })
}

#[tauri::command]
pub fn cancel_download() -> Result<(), String> {
    CANCEL_DL.store(true, Ordering::Relaxed);
    Ok(())
}
