// CinaVault Premium — Media Scanner Module
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tauri::State;
use walkdir::WalkDir;
use crate::AppState;
use crate::db::{MediaItem, MediaSource};
use rusqlite::OptionalExtension;
use crate::library_artifacts::{
    available_poster_path_for_media, is_artwork_image_for_nearby_media,
    is_generated_chapter_image_path, is_internal_artwork_cache_path, is_sidecar_artwork_image,
};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::process::Command;

static SCANNING: AtomicBool = AtomicBool::new(false);
static SCAN_TOTAL: AtomicU64 = AtomicU64::new(0);
static SCAN_CURRENT: AtomicU64 = AtomicU64::new(0);
static CANCEL_FLAG: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

struct ScanGuard;

impl Drop for ScanGuard {
    fn drop(&mut self) {
        SCANNING.store(false, Ordering::Relaxed);
    }
}

const VIDEO_EXTS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg",
    "ts", "m2ts", "vob", "ogv", "3gp", "divx", "rm", "rmvb", "asf",
];
const AUDIO_EXTS: &[&str] = &[
    "mp3", "flac", "aac", "ogg", "wma", "wav", "m4a", "opus", "alac", "aiff",
];
const IMAGE_EXTS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "svg",
];

fn detect_media_type(ext: &str) -> Option<&'static str> {
    let ext_lower = ext.to_lowercase();
    if VIDEO_EXTS.contains(&ext_lower.as_str()) {
        Some("movie")
    } else if AUDIO_EXTS.contains(&ext_lower.as_str()) {
        Some("music")
    } else if IMAGE_EXTS.contains(&ext_lower.as_str()) {
        Some("photo")
    } else {
        None
    }
}

fn should_index_path(path: &Path) -> bool {
    !is_generated_chapter_image_path(path)
        && !is_internal_artwork_cache_path(path)
        && !is_sidecar_artwork_image(path)
        && !is_artwork_image_for_nearby_media(path)
}

fn title_from_filename(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown".to_string())
        .replace('_', " ")
        .replace('.', " ")
}

fn extract_embedded_title(file_path: &str) -> Option<String> {
    let mut cmd = Command::new("ffprobe");
    cmd.args([
        "-v", "error",
        "-show_entries", "format_tags=title:stream_tags=title",
        "-of", "default=nw=1:nk=1",
        file_path,
    ]);
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.to_string())
}

fn poster_cache_path_for_file(file_path: &str) -> Option<std::path::PathBuf> {
    let base = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("CinaVault")
        .join("embedded-posters");
    std::fs::create_dir_all(&base).ok()?;

    let mut hasher = Sha256::new();
    hasher.update(file_path.as_bytes());
    let digest = hasher.finalize();
    Some(base.join(format!("{digest:x}.jpg")))
}

fn screenshot_poster_cache_path_for_file(file_path: &str) -> Option<std::path::PathBuf> {
    let base = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("CinaVault")
        .join("generated-posters");
    std::fs::create_dir_all(&base).ok()?;

    let mut hasher = Sha256::new();
    hasher.update(file_path.as_bytes());
    let digest = hasher.finalize();
    Some(base.join(format!("{digest:x}.jpg")))
}

fn extract_embedded_poster(file_path: &str) -> Option<String> {
    let output_path = poster_cache_path_for_file(file_path)?;
    if output_path.exists() {
        return Some(output_path.to_string_lossy().to_string());
    }

    let output_arg = output_path.to_string_lossy().to_string();
    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-y",
        "-v",
        "error",
        "-i",
        file_path,
        "-map",
        "0:v:m:attached_pic",
        "-frames:v",
        "1",
        &output_arg,
    ]);
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd.output().ok()?;
    if output.status.success() && output_path.exists() {
        Some(output_path.to_string_lossy().to_string())
    } else {
        let _ = std::fs::remove_file(output_path);
        None
    }
}

fn extract_video_screenshot_poster(file_path: &str) -> Option<String> {
    let output_path = screenshot_poster_cache_path_for_file(file_path)?;
    if output_path.exists() {
        return Some(output_path.to_string_lossy().to_string());
    }

    let output_arg = output_path.to_string_lossy().to_string();
    for timestamp in ["00:00:10", "00:00:03", "00:00:01"] {
        let mut cmd = Command::new("ffmpeg");
        cmd.args([
            "-y",
            "-v",
            "error",
            "-ss",
            timestamp,
            "-i",
            file_path,
            "-frames:v",
            "1",
            "-q:v",
            "3",
            &output_arg,
        ]);
        #[cfg(target_os = "windows")]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let output = cmd.output().ok()?;
        if output.status.success() && output_path.exists() {
            return Some(output_path.to_string_lossy().to_string());
        }
        let _ = std::fs::remove_file(&output_path);
    }

    None
}

fn should_extract_poster_for_scan(existing_poster_path: Option<&str>) -> bool {
    existing_poster_path
        .map(|path| path.trim().is_empty())
        .unwrap_or(true)
}

fn source_uses_virtual_protocol(source: &MediaSource) -> bool {
    source.source_type == "synology_quickconnect"
        || ((source.source_type == "nas" || source.source_type == "cloud")
            && source.path.contains("://"))
}

#[tauri::command]
pub async fn scan_sources(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    if SCANNING.load(Ordering::Relaxed) {
        return Err("Scan already in progress".into());
    }
    SCANNING.store(true, Ordering::Relaxed);
    let _scan_guard = ScanGuard;
    CANCEL_FLAG.store(false, Ordering::Relaxed);
    SCAN_CURRENT.store(0, Ordering::Relaxed);

    let (sources, prefer_embedded_titles) = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let sources = db.get_sources_data().map_err(|e| e.to_string())?;
        let prefer_embedded_titles = db
            .get_setting_data("prefer_embedded_titles")
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| "true".to_string())
            == "true";
        (sources, prefer_embedded_titles)
    };

    let mut total_found: u64 = 0;
    let mut total_added: u64 = 0;

    for source in &sources {
        if !source.enabled { continue; }
        if CANCEL_FLAG.load(Ordering::Relaxed) { break; }

        let (found, added) = scan_directory(&state, source, prefer_embedded_titles)?;
        total_found += found;
        total_added += added;
    }

    Ok(serde_json::json!({
        "total_found": total_found,
        "total_added": total_added,
        "sources_scanned": sources.len(),
    }))
}

#[tauri::command]
pub async fn scan_single_source(state: State<'_, AppState>, source_id: i64) -> Result<serde_json::Value, String> {
    if SCANNING.load(Ordering::Relaxed) {
        return Err("Scan already in progress".into());
    }
    SCANNING.store(true, Ordering::Relaxed);
    let _scan_guard = ScanGuard;
    CANCEL_FLAG.store(false, Ordering::Relaxed);
    SCAN_CURRENT.store(0, Ordering::Relaxed);

    let (source, prefer_embedded_titles) = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let sources = db.get_sources_data().map_err(|e| e.to_string())?;
        let source = sources.into_iter().find(|s| s.id == Some(source_id))
            .ok_or("Source not found")?;
        let prefer_embedded_titles = db
            .get_setting_data("prefer_embedded_titles")
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| "true".to_string())
            == "true";
        (source, prefer_embedded_titles)
    };

    let (found, added) = scan_directory(&state, &source, prefer_embedded_titles)?;
    Ok(serde_json::json!({
        "total_found": found,
        "total_added": added,
    }))
}

fn scan_directory(state: &State<AppState>, source: &MediaSource, prefer_embedded_titles: bool) -> Result<(u64, u64), String> {
    if source_uses_virtual_protocol(source) {
        return Ok((0, 0));
    }

    let path = Path::new(&source.path);
    if !path.exists() {
        return Err(format!("Source path does not exist: {}", source.path));
    }

    let mut files: Vec<(String, String, u64)> = Vec::new();

    // Following links on Windows can recurse through junction/symlink loops forever.
    for entry in WalkDir::new(path).follow_links(false).into_iter().filter_map(|e| e.ok()) {
        if CANCEL_FLAG.load(Ordering::Relaxed) { break; }
        let p = entry.path();
        if !p.is_file() { continue; }
        if !should_index_path(p) { continue; }

        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            if let Some(media_type) = detect_media_type(ext) {
                let size = p.metadata().map(|m| m.len()).unwrap_or(0);
                files.push((p.to_string_lossy().to_string(), media_type.to_string(), size));
            }
        }
    }

    SCAN_TOTAL.store(files.len() as u64, Ordering::Relaxed);
    let found = files.len() as u64;
    let mut added: u64 = 0;

    let db = state.db.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    for (i, (file_path, media_type, file_size)) in files.iter().enumerate() {
        if CANCEL_FLAG.load(Ordering::Relaxed) { break; }
        SCAN_CURRENT.store(i as u64 + 1, Ordering::Relaxed);

        let existing_poster_path = db
            .conn
            .query_row(
                "SELECT poster_path FROM media_items WHERE file_path = ?1",
                rusqlite::params![file_path],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .flatten();
        let title = if prefer_embedded_titles {
            extract_embedded_title(file_path).unwrap_or_else(|| title_from_filename(Path::new(file_path)))
        } else {
            title_from_filename(Path::new(file_path))
        };
        let poster_path = if should_extract_poster_for_scan(existing_poster_path.as_deref()) {
            available_poster_path_for_media(Path::new(file_path))
                .or_else(|| extract_embedded_poster(file_path))
                .or_else(|| extract_video_screenshot_poster(file_path))
        } else {
            None
        };

        let item = MediaItem {
            id: None,
            title,
            file_path: file_path.clone(),
            media_type: media_type.clone(),
            year: None,
            rating: None,
            overview: None,
            poster_path,
            backdrop_path: None,
            genre: None,
            duration: None,
            file_size: Some(*file_size as i64),
            resolution: None,
            codec: None,
            verified: false,
            watched: false,
            favorite: false,
            date_added: now.clone(),
            last_played: None,
            tmdb_id: None,
            imdb_id: None,
            source_id: source.id,
        };

        match db.upsert_scanned_media_item_data(&item) {
            Ok(inserted) => {
                if inserted {
                    added += 1;
                }
            }
            Err(_) => {}
        }
    }

    // Update source last_scanned and item_count
    let _ = db.conn.execute(
        "UPDATE media_sources SET last_scanned = ?1, item_count = ?2 WHERE id = ?3",
        rusqlite::params![now, added as i64, source.id],
    );

    Ok((found, added))
}

#[cfg(test)]
mod tests {
    use super::{
        poster_cache_path_for_file, screenshot_poster_cache_path_for_file,
        should_extract_poster_for_scan, should_index_path, source_uses_virtual_protocol,
    };
    use crate::db::MediaSource;
    use std::path::Path;

    #[test]
    fn skips_generated_chapter_images() {
        assert!(!should_index_path(Path::new(r"E:\Videos\sample_chapters\chapter_0001.jpg")));
    }

    #[test]
    fn skips_sidecar_artwork_images() {
        assert!(!should_index_path(Path::new(r"E:\Videos\Movie\poster.jpg")));
        assert!(!should_index_path(Path::new(r"E:\Videos\Movie\backdrop.jpg")));
        assert!(!should_index_path(Path::new(r"E:\Videos\Movie\folder.jpg")));
        assert!(!should_index_path(Path::new(r"E:\Videos\Movie\cover.png")));
        assert!(!should_index_path(Path::new(r"E:\Videos\Movie\scene-poster.webp")));
    }

    #[test]
    fn keeps_real_media_files() {
        assert!(should_index_path(Path::new(r"E:\Videos\sample.mp4")));
        assert!(should_index_path(Path::new(r"E:\Photos\Vacation\beach-day.jpg")));
    }

    #[test]
    fn poster_cache_path_is_stable_and_uses_jpg_extension() {
        let first = poster_cache_path_for_file(r"E:\Videos\Movie.mkv").expect("path should build");
        let second = poster_cache_path_for_file(r"E:\Videos\Movie.mkv").expect("path should build");

        assert_eq!(first, second);
        assert_eq!(first.extension().and_then(|ext| ext.to_str()), Some("jpg"));
    }

    #[test]
    fn screenshot_poster_cache_path_is_stable_and_uses_jpg_extension() {
        let first = screenshot_poster_cache_path_for_file(r"E:\Videos\Movie.mkv").expect("path should build");
        let second = screenshot_poster_cache_path_for_file(r"E:\Videos\Movie.mkv").expect("path should build");

        assert_eq!(first, second);
        assert_eq!(first.extension().and_then(|ext| ext.to_str()), Some("jpg"));
    }

    #[test]
    fn poster_extraction_runs_for_new_or_blank_poster_items_only() {
        assert!(should_extract_poster_for_scan(None));
        assert!(should_extract_poster_for_scan(Some("")));
        assert!(should_extract_poster_for_scan(Some("   ")));
        assert!(!should_extract_poster_for_scan(Some(r"E:\Videos\Movie.jpg")));
        assert!(!should_extract_poster_for_scan(Some("https://example.com/poster.jpg")));
    }

    #[test]
    fn synology_quickconnect_sources_are_preserved_without_local_path_scans() {
        let source = MediaSource {
            id: Some(1),
            path: "synology_quickconnect://owner@cinavault/video".to_string(),
            source_type: "synology_quickconnect".to_string(),
            name: "Synology QuickConnect".to_string(),
            enabled: true,
            last_scanned: None,
            item_count: 0,
        };

        assert!(source_uses_virtual_protocol(&source));
    }
}

#[tauri::command]
pub fn get_scan_progress() -> serde_json::Value {
    serde_json::json!({
        "scanning": SCANNING.load(Ordering::Relaxed),
        "total": SCAN_TOTAL.load(Ordering::Relaxed),
        "current": SCAN_CURRENT.load(Ordering::Relaxed),
    })
}

#[tauri::command]
pub fn cancel_scan() -> Result<(), String> {
    CANCEL_FLAG.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub async fn apply_embedded_titles(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let rows: Vec<(i64, String, String)> = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let mut stmt = db
            .conn
            .prepare("SELECT id, file_path, title FROM media_items ORDER BY id")
            .map_err(|e| e.to_string())?;
        let iter = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        iter.filter_map(|r| r.ok()).collect()
    };

    let mut checked = 0u64;
    let mut updated = 0u64;
    let mut missing_files = 0u64;

    for (id, file_path, current_title) in rows {
        checked += 1;
        let path = Path::new(&file_path);
        if !path.exists() {
            missing_files += 1;
            continue;
        }

        if let Some(embedded_title) = extract_embedded_title(&file_path) {
            if !embedded_title.trim().is_empty() && !embedded_title.eq_ignore_ascii_case(&current_title) {
                let db = state.db.lock().map_err(|e| e.to_string())?;
                db.conn
                    .execute(
                        "UPDATE media_items SET title = ?1 WHERE id = ?2",
                        rusqlite::params![embedded_title, id],
                    )
                    .map_err(|e| e.to_string())?;
                updated += 1;
            }
        }
    }

    Ok(serde_json::json!({
        "checked": checked,
        "updated": updated,
        "missing_files": missing_files,
    }))
}
