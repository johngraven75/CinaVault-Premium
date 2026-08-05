// CinaVault Premium — responsive external-drive media scanner
use crate::db::{MediaItem, MediaSource};
use crate::library_artifacts::{is_generated_chapter_image_path, is_sidecar_artwork_image, sidecar_poster_path_for_video};
use crate::AppState;
use rusqlite::OptionalExtension;
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
impl Drop for ScanGuard { fn drop(&mut self) { SCANNING.store(false, Ordering::Relaxed); } }

#[derive(Debug, Default)]
struct ScanFileCollection { files: Vec<(String, String, u64)>, errors: Vec<String> }
#[derive(Debug, Default)]
struct ScanDirectoryReport { found: u64, added: u64, updated: u64, errors: Vec<String> }

const VIDEO_EXTS: &[&str] = &["mp4","mkv","avi","mov","wmv","flv","webm","m4v","mpg","mpeg","ts","m2ts","vob","ogv","3gp","divx","rm","rmvb","asf"];
const AUDIO_EXTS: &[&str] = &["mp3","flac","aac","ogg","wma","wav","m4a","opus","alac","aiff"];
const IMAGE_EXTS: &[&str] = &["jpg","jpeg","png","gif","bmp","webp","tiff","svg"];

fn detect_media_type(ext: &str) -> Option<&'static str> {
    let ext = ext.to_ascii_lowercase();
    if VIDEO_EXTS.contains(&ext.as_str()) { Some("movie") }
    else if AUDIO_EXTS.contains(&ext.as_str()) { Some("music") }
    else { None }
}
fn should_index_path(path: &Path) -> bool {
    if is_generated_chapter_image_path(path) || is_sidecar_artwork_image(path) { return false; }
    path.extension().and_then(|e| e.to_str()).map(|e| !IMAGE_EXTS.contains(&e.to_ascii_lowercase().as_str())).unwrap_or(false)
}
fn title_from_filename(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Unknown".into()).replace('_', " ").replace('.', " ")
}
fn normalize_source_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim().trim_matches('"');
    #[cfg(target_os = "windows")]
    if trimmed.len() == 2 && trimmed.as_bytes()[1] == b':' { return PathBuf::from(format!("{}\\", trimmed)); }
    PathBuf::from(trimmed)
}
fn collect_media_files(path: &Path) -> Result<ScanFileCollection, String> {
    if !path.exists() { return Err(format!("Source path does not exist or drive is offline: {}", path.display())); }
    if !path.is_dir() { return Err(format!("Source path is not a directory: {}", path.display())); }
    let mut result = ScanFileCollection::default();
    for entry in WalkDir::new(path).follow_links(false).into_iter() {
        if CANCEL_FLAG.load(Ordering::Relaxed) { break; }
        let entry = match entry { Ok(v) => v, Err(e) => { result.errors.push(format!("walk error: {e}")); continue; } };
        let p = entry.path();
        if !entry.file_type().is_file() || !should_index_path(p) { continue; }
        let Some(ext) = p.extension().and_then(|e| e.to_str()) else { continue; };
        let Some(media_type) = detect_media_type(ext) else { continue; };
        match entry.metadata() {
            Ok(metadata) => result.files.push((p.to_string_lossy().to_string(), media_type.to_string(), metadata.len())),
            Err(e) => result.errors.push(format!("metadata error for {}: {e}", p.display())),
        }
    }
    Ok(result)
}
fn extract_embedded_title(file_path: &str) -> Option<String> {
    let mut cmd = Command::new("ffprobe");
    cmd.args(["-v","error","-show_entries","format_tags=title:stream_tags=title","-of","default=nw=1:nk=1",file_path]);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let output = cmd.output().ok()?;
    if !output.status.success() { return None; }
    String::from_utf8_lossy(&output.stdout).lines().map(str::trim).find(|s| !s.is_empty()).map(str::to_string)
}
fn source_report_json(source: &MediaSource, status: &str, found: u64, added: u64, updated: u64, errors: &[String]) -> serde_json::Value {
    serde_json::json!({"source_id":source.id,"name":source.name,"path":source.path,"enabled":source.enabled,"status":status,"found":found,"added":added,"updated":updated,"errors":errors})
}
fn looks_like_media_directory(path: &Path) -> bool {
    let name = path.file_name().and_then(|v| v.to_str()).unwrap_or_default().to_ascii_lowercase();
    ["movie","film","cinema","video","tv","television","series","show","music","audio","media","adult","personal vids"].iter().any(|k| name.contains(k))
}
fn discover_media_directories(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = BTreeSet::new();
    for root in roots {
        if root.is_dir() && looks_like_media_directory(root) { found.insert(root.clone()); }
        let Ok(entries) = std::fs::read_dir(root) else { continue; };
        for entry in entries.filter_map(Result::ok) { let p = entry.path(); if p.is_dir() && looks_like_media_directory(&p) { found.insert(p); } }
    }
    found.into_iter().collect()
}
fn platform_discovery_roots() -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();
    #[cfg(target_os = "windows")]
    for drive in b'C'..=b'Z' { let p = PathBuf::from(format!("{}:\\", drive as char)); if p.is_dir() { roots.insert(p); } }
    #[cfg(not(target_os = "windows"))]
    roots.insert(PathBuf::from("/"));
    if let Some(p) = dirs::home_dir() { roots.insert(p); }
    if let Some(p) = dirs::video_dir() { roots.insert(p); }
    if let Some(p) = dirs::audio_dir() { roots.insert(p); }
    roots.into_iter().collect()
}
fn discover_and_add_sources(db: &crate::db::Database, roots: &[PathBuf]) -> Result<(Vec<String>, usize), String> {
    let candidates = discover_media_directories(roots);
    let existing = db.get_sources_data().map_err(|e| e.to_string())?.into_iter().map(|s| s.path.to_ascii_lowercase()).collect::<HashSet<_>>();
    let mut paths = Vec::new(); let mut added = 0;
    for path in candidates {
        let value = path.to_string_lossy().to_string(); paths.push(value.clone());
        if existing.contains(&value.to_ascii_lowercase()) { continue; }
        let name = path.file_name().and_then(|v| v.to_str()).unwrap_or("Discovered Media").to_string();
        db.add_source_data(&MediaSource{id:None,path:value,source_type:"folder".into(),name,enabled:true,last_scanned:None,item_count:0}).map_err(|e| e.to_string())?;
        added += 1;
    }
    Ok((paths, added))
}

#[tauri::command]
pub async fn discover_media_sources(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let roots = platform_discovery_roots();
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let (paths, added) = discover_and_add_sources(&db, &roots)?;
    Ok(serde_json::json!({"type":"source_discovery","status":"success","roots_checked":roots.len(),"discovered":paths.len(),"added":added,"existing":paths.len().saturating_sub(added),"paths":paths}))
}

fn scan_directory(state: &State<AppState>, source: &MediaSource) -> Result<ScanDirectoryReport, String> {
    let path = normalize_source_path(&source.path);
    let collection = collect_media_files(&path)?;
    SCAN_TOTAL.store(collection.files.len() as u64, Ordering::Relaxed);
    let now = chrono::Utc::now().to_rfc3339();
    let mut report = ScanDirectoryReport{found:collection.files.len() as u64,errors:collection.errors,..Default::default()};
    for (i,(file_path,media_type,file_size)) in collection.files.iter().enumerate() {
        if CANCEL_FLAG.load(Ordering::Relaxed) { break; }
        SCAN_CURRENT.store(i as u64 + 1, Ordering::Relaxed);
        // Fast path: use adjacent artwork now. Expensive ffprobe/ffmpeg enrichment runs separately.
        let sidecar = sidecar_poster_path_for_video(Path::new(file_path)).map(|p| p.to_string_lossy().to_string());
        let existing = {
            let db = state.db.lock().map_err(|e| e.to_string())?;
            db.conn.query_row("SELECT poster_path FROM media_items WHERE file_path = ?1",rusqlite::params![file_path],|row| row.get::<_,Option<String>>(0)).optional().map_err(|e| e.to_string())?.flatten()
        };
        let poster_path = if existing.as_deref().map(str::trim).filter(|v| !v.is_empty()).is_some() { None } else { sidecar };
        let item = MediaItem{id:None,title:title_from_filename(Path::new(file_path)),file_path:file_path.clone(),media_type:media_type.clone(),year:None,rating:None,overview:None,poster_path,backdrop_path:None,genre:None,duration:None,file_size:Some(*file_size as i64),resolution:None,codec:None,verified:false,watched:false,favorite:false,date_added:now.clone(),last_played:None,tmdb_id:None,imdb_id:None,source_id:source.id};
        let result = { let db = state.db.lock().map_err(|e| e.to_string())?; db.upsert_scanned_media_item_data(&item) };
        match result { Ok(true) => report.added += 1, Ok(false) => report.updated += 1, Err(e) => report.errors.push(format!("library upsert failed for {file_path}: {e}")) }
    }
    let result = { let db = state.db.lock().map_err(|e| e.to_string())?; db.conn.execute("UPDATE media_sources SET last_scanned = ?1, item_count = ?2 WHERE id = ?3",rusqlite::params![now,report.found as i64,source.id]) };
    if let Err(e) = result { report.errors.push(format!("source status update failed: {e}")); }
    Ok(report)
}

#[tauri::command]
pub async fn scan_sources(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    if SCANNING.swap(true, Ordering::Relaxed) { return Err("Scan already in progress".into()); }
    let _guard = ScanGuard; CANCEL_FLAG.store(false, Ordering::Relaxed); SCAN_CURRENT.store(0, Ordering::Relaxed); SCAN_TOTAL.store(0, Ordering::Relaxed);
    let sources = { let db = state.db.lock().map_err(|e| e.to_string())?; db.get_sources_data().map_err(|e| e.to_string())? };
    let enabled = sources.iter().filter(|s| s.enabled).count() as u64;
    let mut totals = ScanDirectoryReport::default(); let mut scanned = 0u64; let mut failed = 0u64; let mut skipped = 0u64; let mut reports = Vec::new();
    for source in &sources {
        if CANCEL_FLAG.load(Ordering::Relaxed) { break; }
        if !source.enabled { skipped += 1; reports.push(source_report_json(source,"disabled",0,0,0,&[])); continue; }
        match scan_directory(&state, source) {
            Ok(r) => { scanned += 1; totals.found += r.found; totals.added += r.added; totals.updated += r.updated; totals.errors.extend(r.errors.iter().map(|e| format!("{}: {e}",source.name))); reports.push(source_report_json(source,if r.errors.is_empty(){"success"}else{"partial"},r.found,r.added,r.updated,&r.errors)); }
            Err(e) => { failed += 1; totals.errors.push(format!("{}: {e}",source.name)); reports.push(source_report_json(source,"failed",0,0,0,&[e])); }
        }
    }
    let status = if failed == 0 && totals.errors.is_empty(){"success"}else if scanned > 0{"partial"}else{"failed"};
    Ok(serde_json::json!({"status":status,"total_found":totals.found,"total_added":totals.added,"total_updated":totals.updated,"sources_total":sources.len(),"sources_enabled":enabled,"sources_scanned":scanned,"sources_failed":failed,"sources_skipped_disabled":skipped,"errors":totals.errors,"source_reports":reports}))
}

#[tauri::command]
pub async fn scan_single_source(state: State<'_, AppState>, source_id: i64) -> Result<serde_json::Value, String> {
    if SCANNING.swap(true, Ordering::Relaxed) { return Err("Scan already in progress".into()); }
    let _guard = ScanGuard; CANCEL_FLAG.store(false, Ordering::Relaxed); SCAN_CURRENT.store(0, Ordering::Relaxed);
    let source = { let db = state.db.lock().map_err(|e| e.to_string())?; db.get_sources_data().map_err(|e| e.to_string())?.into_iter().find(|s| s.id == Some(source_id)).ok_or("Source not found")? };
    if !source.enabled { return Err("Source is disabled".into()); }
    let r = scan_directory(&state,&source)?;
    Ok(serde_json::json!({"status":if r.errors.is_empty(){"success"}else{"partial"},"total_found":r.found,"total_added":r.added,"total_updated":r.updated,"errors":r.errors}))
}

#[tauri::command]
pub fn get_scan_progress() -> serde_json::Value { serde_json::json!({"scanning":SCANNING.load(Ordering::Relaxed),"total":SCAN_TOTAL.load(Ordering::Relaxed),"current":SCAN_CURRENT.load(Ordering::Relaxed)}) }
#[tauri::command]
pub fn cancel_scan() -> Result<(), String> { CANCEL_FLAG.store(true, Ordering::Relaxed); Ok(()) }

#[tauri::command]
pub async fn apply_embedded_titles(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let rows: Vec<(i64,String,String)> = { let db = state.db.lock().map_err(|e| e.to_string())?; let mut stmt = db.conn.prepare("SELECT id, file_path, title FROM media_items ORDER BY id").map_err(|e| e.to_string())?; let rows = stmt.query_map([],|row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).map_err(|e| e.to_string())?; rows.filter_map(Result::ok).collect() };
    let mut checked=0u64; let mut updated=0u64; let mut missing_files=0u64;
    for (id,file_path,current_title) in rows { checked += 1; if !Path::new(&file_path).exists(){missing_files += 1;continue;} let Some(title)=extract_embedded_title(&file_path) else{continue}; if title.trim().is_empty()||title.eq_ignore_ascii_case(&current_title){continue;} let db=state.db.lock().map_err(|e|e.to_string())?; db.conn.execute("UPDATE media_items SET title = ?1 WHERE id = ?2",rusqlite::params![title,id]).map_err(|e|e.to_string())?; updated += 1; }
    Ok(serde_json::json!({"checked":checked,"updated":updated,"missing_files":missing_files}))
}

#[cfg(test)]
mod tests {
    use super::{collect_media_files, should_index_path};
    use std::path::Path;
    #[test] fn media_filter_excludes_artwork(){assert!(should_index_path(Path::new("movie.mkv")));assert!(!should_index_path(Path::new("poster.jpg")));}
    #[test] fn missing_drive_reports_clear_error(){let e=collect_media_files(Path::new("/path/that/does/not/exist")).unwrap_err();assert!(e.contains("offline")||e.contains("does not exist"));}
}
