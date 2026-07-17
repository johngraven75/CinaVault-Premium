// CinaVault Premium — Media Scanner Module
use crate::db::{MediaItem, MediaSource};
use crate::library_artifacts::{
    is_generated_chapter_image_path, is_sidecar_artwork_image, sidecar_poster_path_for_video,
};
use crate::AppState;
use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashSet};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tauri::State;
use walkdir::WalkDir;

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

#[derive(Debug, Default)]
struct ScanFileCollection {
    files: Vec<(String, String, u64)>,
    errors: Vec<String>,
}

#[derive(Debug, Default)]
struct ScanDirectoryReport {
    found: u64,
    added: u64,
    updated: u64,
    errors: Vec<String>,
}

const VIDEO_EXTS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "ts", "m2ts", "vob",
    "ogv", "3gp", "divx", "rm", "rmvb", "asf",
];
const AUDIO_EXTS: &[&str] = &[
    "mp3", "flac", "aac", "ogg", "wma", "wav", "m4a", "opus", "alac", "aiff",
];
const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "svg"];

fn detect_media_type(ext: &str) -> Option<&'static str> {
    let ext_lower = ext.to_lowercase();
    if VIDEO_EXTS.contains(&ext_lower.as_str()) {
        Some("movie")
    } else if AUDIO_EXTS.contains(&ext_lower.as_str()) {
        Some("music")
    } else {
        // Image files (.jpg, .png, .webp, etc.) are NEVER indexed as standalone media items.
        // They are poster/artwork files for video entries and are handled by the poster pipeline.
        None
    }
}

fn should_index_path(path: &Path) -> bool {
    // Skip chapter images and all sidecar artwork
    if is_generated_chapter_image_path(path) || is_sidecar_artwork_image(path) {
        return false;
    }
    // Skip ALL image files — they are never standalone media, only posters/artwork
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if IMAGE_EXTS.contains(&ext.to_ascii_lowercase().as_str()) {
            return false;
        }
    }
    true
}

fn title_from_filename(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown".to_string())
        .replace('_', " ")
        .replace('.', " ")
}

fn collect_media_files(path: &Path) -> Result<ScanFileCollection, String> {
    if !path.exists() {
        return Err(format!(
            "Source path does not exist: {}",
            path.to_string_lossy()
        ));
    }
    if !path.is_dir() {
        return Err(format!(
            "Source path is not a directory: {}",
            path.to_string_lossy()
        ));
    }

    let mut collection = ScanFileCollection::default();
    for entry in WalkDir::new(path).follow_links(false).into_iter() {
        if CANCEL_FLAG.load(Ordering::Relaxed) {
            break;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                collection.errors.push(format!("walk error: {err}"));
                continue;
            }
        };
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        if !should_index_path(p) {
            continue;
        }

        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            if let Some(media_type) = detect_media_type(ext) {
                match p.metadata() {
                    Ok(metadata) => collection.files.push((
                        p.to_string_lossy().to_string(),
                        media_type.to_string(),
                        metadata.len(),
                    )),
                    Err(err) => collection
                        .errors
                        .push(format!("metadata error for {}: {err}", p.to_string_lossy())),
                }
            }
        }
    }

    Ok(collection)
}

fn extract_embedded_title(file_path: &str) -> Option<String> {
    let mut cmd = Command::new("ffprobe");
    cmd.args([
        "-v",
        "error",
        "-show_entries",
        "format_tags=title:stream_tags=title",
        "-of",
        "default=nw=1:nk=1",
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

fn should_extract_poster_for_scan(existing_poster_path: Option<&str>) -> bool {
    existing_poster_path
        .map(|path| path.trim().is_empty())
        .unwrap_or(true)
}

fn source_report_json(
    source: &MediaSource,
    status: &str,
    found: u64,
    added: u64,
    updated: u64,
    errors: &[String],
) -> serde_json::Value {
    serde_json::json!({
        "source_id": source.id,
        "name": source.name.clone(),
        "path": source.path.clone(),
        "enabled": source.enabled,
        "status": status,
        "found": found,
        "added": added,
        "updated": updated,
        "errors": errors,
    })
}

fn looks_like_media_directory(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    [
        "movie",
        "film",
        "cinema",
        "video",
        "tv",
        "television",
        "series",
        "show",
        "music",
        "audio",
        "media",
        "adult",
        "personal vids",
    ]
    .iter()
    .any(|keyword| name.contains(keyword))
}

fn discover_media_directories(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut discovered = BTreeSet::new();
    for root in roots {
        if root.is_dir() && looks_like_media_directory(root) {
            discovered.insert(root.clone());
        }
        let entries = match std::fs::read_dir(root) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() && looks_like_media_directory(&path) {
                discovered.insert(path);
            }
        }
    }
    discovered.into_iter().collect()
}

fn platform_discovery_roots() -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();
    #[cfg(target_os = "windows")]
    for drive in b'C'..=b'Z' {
        let root = PathBuf::from(format!("{}:\\", drive as char));
        if root.is_dir() {
            roots.insert(root);
        }
    }
    #[cfg(not(target_os = "windows"))]
    roots.insert(PathBuf::from("/"));

    if let Some(home) = dirs::home_dir() {
        roots.insert(home);
    }
    if let Some(videos) = dirs::video_dir() {
        roots.insert(videos);
    }
    if let Some(audio) = dirs::audio_dir() {
        roots.insert(audio);
    }
    roots.into_iter().collect()
}

fn discover_and_add_sources(
    db: &crate::db::Database,
    roots: &[PathBuf],
) -> Result<(Vec<String>, usize), String> {
    let candidates = discover_media_directories(roots);
    let existing = db
        .get_sources_data()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|source| source.path.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    let mut added = 0usize;
    let mut discovered = Vec::new();

    for path in candidates {
        let path_string = path.to_string_lossy().to_string();
        discovered.push(path_string.clone());
        if existing.contains(&path_string.to_ascii_lowercase()) {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("Discovered Media")
            .to_string();
        db.add_source_data(&MediaSource {
            id: None,
            path: path_string,
            source_type: "folder".to_string(),
            name,
            enabled: true,
            last_scanned: None,
            item_count: 0,
        })
        .map_err(|error| error.to_string())?;
        added += 1;
    }

    Ok((discovered, added))
}

#[tauri::command]
pub async fn discover_media_sources(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let roots = platform_discovery_roots();
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let (paths, added) = discover_and_add_sources(&db, &roots)?;
    Ok(serde_json::json!({
        "type": "source_discovery",
        "status": "success",
        "roots_checked": roots.len(),
        "discovered": paths.len(),
        "added": added,
        "existing": paths.len().saturating_sub(added),
        "paths": paths,
        "message": format!("Discovered {} media folders and added {} new sources", paths.len(), added),
    }))
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

    let enabled_sources = sources.iter().filter(|source| source.enabled).count() as u64;
    let mut total_found: u64 = 0;
    let mut total_added: u64 = 0;
    let mut total_updated: u64 = 0;
    let mut sources_scanned: u64 = 0;
    let mut sources_failed: u64 = 0;
    let mut skipped_disabled: u64 = 0;
    let mut errors: Vec<String> = Vec::new();
    let mut source_reports = Vec::new();

    for source in &sources {
        if CANCEL_FLAG.load(Ordering::Relaxed) {
            break;
        }
        if !source.enabled {
            skipped_disabled += 1;
            source_reports.push(source_report_json(source, "disabled", 0, 0, 0, &[]));
            continue;
        }

        match scan_directory(&state, source, prefer_embedded_titles) {
            Ok(report) => {
                total_found += report.found;
                total_added += report.added;
                total_updated += report.updated;
                sources_scanned += 1;
                let status = if report.errors.is_empty() {
                    "success"
                } else {
                    "partial"
                };
                errors.extend(
                    report
                        .errors
                        .iter()
                        .map(|err| format!("{}: {err}", source.name)),
                );
                source_reports.push(source_report_json(
                    source,
                    status,
                    report.found,
                    report.added,
                    report.updated,
                    &report.errors,
                ));
            }
            Err(err) => {
                sources_failed += 1;
                let source_error = format!("{}: {err}", source.name);
                errors.push(source_error);
                let failed_errors = vec![err];
                source_reports.push(source_report_json(
                    source,
                    "failed",
                    0,
                    0,
                    0,
                    &failed_errors,
                ));
            }
        }
    }

    let status = if sources_failed == 0 && errors.is_empty() {
        "success"
    } else if sources_scanned > 0 || total_added > 0 || total_updated > 0 {
        "partial"
    } else {
        "failed"
    };

    Ok(serde_json::json!({
        "status": status,
        "total_found": total_found,
        "total_added": total_added,
        "total_updated": total_updated,
        "sources_total": sources.len(),
        "sources_enabled": enabled_sources,
        "sources_scanned": sources_scanned,
        "sources_failed": sources_failed,
        "sources_skipped_disabled": skipped_disabled,
        "errors": errors,
        "source_reports": source_reports,
    }))
}

#[tauri::command]
pub async fn scan_single_source(
    state: State<'_, AppState>,
    source_id: i64,
) -> Result<serde_json::Value, String> {
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
        let source = sources
            .into_iter()
            .find(|s| s.id == Some(source_id))
            .ok_or("Source not found")?;
        let prefer_embedded_titles = db
            .get_setting_data("prefer_embedded_titles")
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| "true".to_string())
            == "true";
        (source, prefer_embedded_titles)
    };

    let report = scan_directory(&state, &source, prefer_embedded_titles)?;
    Ok(serde_json::json!({
        "status": if report.errors.is_empty() { "success" } else { "partial" },
        "total_found": report.found,
        "total_added": report.added,
        "total_updated": report.updated,
        "errors": report.errors,
    }))
}

fn scan_directory(
    state: &State<AppState>,
    source: &MediaSource,
    prefer_embedded_titles: bool,
) -> Result<ScanDirectoryReport, String> {
    let path = Path::new(&source.path);
    let collection = collect_media_files(path)?;
    SCAN_TOTAL.store(collection.files.len() as u64, Ordering::Relaxed);

    let mut report = ScanDirectoryReport {
        found: collection.files.len() as u64,
        added: 0,
        updated: 0,
        errors: collection.errors,
    };

    let db = state.db.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    for (i, (file_path, media_type, file_size)) in collection.files.iter().enumerate() {
        if CANCEL_FLAG.load(Ordering::Relaxed) {
            break;
        }
        SCAN_CURRENT.store(i as u64 + 1, Ordering::Relaxed);

        let existing_poster_path = match db
            .conn
            .query_row(
                "SELECT poster_path FROM media_items WHERE file_path = ?1",
                rusqlite::params![file_path],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
        {
            Ok(value) => value.flatten(),
            Err(err) => {
                report
                    .errors
                    .push(format!("poster lookup failed for {file_path}: {err}"));
                None
            }
        };
        let title = if prefer_embedded_titles {
            extract_embedded_title(file_path)
                .unwrap_or_else(|| title_from_filename(Path::new(file_path)))
        } else {
            title_from_filename(Path::new(file_path))
        };
        let poster_path = if should_extract_poster_for_scan(existing_poster_path.as_deref()) {
            extract_embedded_poster(file_path).or_else(|| {
                sidecar_poster_path_for_video(Path::new(file_path))
                    .map(|path| path.to_string_lossy().to_string())
            })
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
                    report.added += 1;
                } else {
                    report.updated += 1;
                }
            }
            Err(err) => report
                .errors
                .push(format!("library upsert failed for {file_path}: {err}")),
        }
    }

    if let Err(err) = db.conn.execute(
        "UPDATE media_sources SET last_scanned = ?1, item_count = ?2 WHERE id = ?3",
        rusqlite::params![now, report.found as i64, source.id],
    ) {
        report
            .errors
            .push(format!("source status update failed: {err}"));
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::{
        collect_media_files, discover_and_add_sources, poster_cache_path_for_file,
        should_extract_poster_for_scan, should_index_path,
    };
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_path(parts: &[&str]) -> PathBuf {
        parts.iter().collect()
    }

    #[test]
    fn discovery_adds_real_database_sources_from_media_directories() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("cinavault-discovery-test-{stamp}"));
        std::fs::create_dir_all(root.join("Movies")).unwrap();
        std::fs::create_dir_all(root.join("TV Shows")).unwrap();
        std::fs::create_dir_all(root.join("Documents")).unwrap();
        let db_path = root.join("discovery.sqlite");
        let db = crate::db::Database::new(db_path.to_string_lossy().as_ref()).unwrap();

        let (paths, added) = discover_and_add_sources(&db, &[root.clone()]).unwrap();
        let sources = db.get_sources_data().unwrap();

        assert_eq!(added, 2);
        assert_eq!(paths.len(), 2);
        assert_eq!(sources.len(), 2);
        assert!(sources.iter().any(|source| source.name == "Movies"));
        assert!(sources.iter().any(|source| source.name == "TV Shows"));
        assert!(!sources.iter().any(|source| source.name == "Documents"));

        drop(db);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn skips_generated_chapter_images() {
        let path = test_path(&["Videos", "sample_chapters", "chapter_0001.jpg"]);
        assert!(!should_index_path(&path));
    }

    #[test]
    fn skips_sidecar_artwork_images() {
        for file_name in [
            "poster.jpg",
            "backdrop.jpg",
            "folder.jpg",
            "cover.png",
            "scene-poster.webp",
        ] {
            let path = test_path(&["Videos", "Movie", file_name]);
            assert!(
                !should_index_path(&path),
                "expected sidecar artwork to be skipped: {}",
                path.display()
            );
        }
    }

    #[test]
    fn keeps_real_media_files_skips_all_images() {
        let video = test_path(&["Videos", "sample.mp4"]);
        let photo = test_path(&["Photos", "Vacation", "beach-day.jpg"]);
        let png_art = test_path(&["Videos", "Movie", "Movie.png"]);
        // Video files should be indexed
        assert!(should_index_path(&video));
        // Image files are NEVER indexed as standalone media — they are poster/artwork
        assert!(
            !should_index_path(&photo),
            "jpg files should be excluded from indexing"
        );
        assert!(
            !should_index_path(&png_art),
            "png files should be excluded from indexing"
        );
    }

    #[test]
    fn poster_cache_path_is_stable_and_uses_jpg_extension() {
        let first = poster_cache_path_for_file(r"E:\Videos\Movie.mkv").expect("path should build");
        let second = poster_cache_path_for_file(r"E:\Videos\Movie.mkv").expect("path should build");

        assert_eq!(first, second);
        assert_eq!(first.extension().and_then(|ext| ext.to_str()), Some("jpg"));
    }

    #[test]
    fn poster_extraction_runs_for_new_or_blank_poster_items_only() {
        assert!(should_extract_poster_for_scan(None));
        assert!(should_extract_poster_for_scan(Some("")));
        assert!(should_extract_poster_for_scan(Some("   ")));
        assert!(!should_extract_poster_for_scan(Some(
            r"E:\Videos\Movie.jpg"
        )));
        assert!(!should_extract_poster_for_scan(Some(
            "https://example.com/poster.jpg"
        )));
    }

    #[test]
    fn collect_media_files_recurses_and_excludes_generated_artifacts() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("cinavault-scan-test-{stamp}"));
        let nested = root.join("Movies").join("Movie One");
        let chapters = nested.join("sample_chapters");
        std::fs::create_dir_all(&chapters).unwrap();
        std::fs::write(nested.join("Movie.One.2026.mkv"), b"test").unwrap();
        std::fs::write(root.join("song.flac"), b"test").unwrap();
        std::fs::write(root.join("family-vacation.jpg"), b"photo").unwrap();
        std::fs::write(root.join("poster.jpg"), b"art").unwrap();
        std::fs::write(chapters.join("chapter_0001.jpg"), b"chapter").unwrap();
        std::fs::write(root.join("notes.txt"), b"ignore").unwrap();

        let collection = collect_media_files(&root).expect("scan collection should succeed");
        let paths = collection
            .files
            .iter()
            .map(|file| file.0.clone())
            .collect::<Vec<_>>();

        assert_eq!(collection.errors.len(), 0);
        assert!(paths
            .iter()
            .any(|path| path.ends_with("Movie.One.2026.mkv")));
        assert!(paths.iter().any(|path| path.ends_with("song.flac")));
        // Image files are never indexed as standalone media items
        assert!(!paths
            .iter()
            .any(|path| path.ends_with("family-vacation.jpg")));
        assert!(!paths.iter().any(|path| path.ends_with("chapter_0001.jpg")));
        assert!(!paths.iter().any(|path| path.ends_with("poster.jpg")));
        assert!(!paths.iter().any(|path| path.ends_with("notes.txt")));

        let _ = std::fs::remove_dir_all(&root);
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
pub async fn apply_embedded_titles(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
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
            if !embedded_title.trim().is_empty()
                && !embedded_title.eq_ignore_ascii_case(&current_title)
            {
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
