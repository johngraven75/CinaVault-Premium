use crate::db::Database;
use crate::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use tauri::State;
use uuid::Uuid;

const SAMPLE_BYTES: usize = 1_048_576;
const PURGE_CONFIRMATION: &str = "ERASE LUMASIFT QUARANTINE";
const SUPPORTED_SELECTION_TYPES: &[&str] = &["video", "audio", "document", "image"];
static CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);
static RUNTIME: OnceLock<Mutex<LumaSiftRuntime>> = OnceLock::new();
static COORDINATOR_CONFIG: OnceLock<LumaSiftCoordinatorConfig> = OnceLock::new();

#[derive(Clone)]
struct LumaSiftCoordinatorConfig {
    app_data_dir: PathBuf,
    database_path: String,
}

pub fn configure(app_data_dir: PathBuf, database_path: String) {
    let _ = COORDINATOR_CONFIG.set(LumaSiftCoordinatorConfig {
        app_data_dir,
        database_path,
    });
}

fn coordinator_config() -> Result<LumaSiftCoordinatorConfig, String> {
    COORDINATOR_CONFIG
        .get()
        .cloned()
        .ok_or_else(|| "LumaSift coordinator is not configured.".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LumaSiftProgress {
    pub scanning: bool,
    pub phase: String,
    pub current: u64,
    pub total: u64,
    pub percentage: u8,
    pub current_path: Option<String>,
    pub files_considered: u64,
    pub message: String,
    pub error: Option<String>,
}

impl Default for LumaSiftProgress {
    fn default() -> Self {
        Self {
            scanning: false,
            phase: "Idle".to_string(),
            current: 0,
            total: 0,
            percentage: 0,
            current_path: None,
            files_considered: 0,
            message: "Ready to build an exact-duplicate resolution plan.".to_string(),
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QualityEvidence {
    pub width: Option<u64>,
    pub height: Option<u64>,
    pub pixel_count: u64,
    pub bitrate: Option<u64>,
    pub bit_depth: Option<u64>,
    pub duration_millis: Option<u64>,
    pub file_size_bytes: u64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LumaSiftCandidate {
    pub id: String,
    pub file_path: String,
    pub display_name: String,
    pub media_kind: String,
    pub exact_hash: String,
    pub quality_score: u64,
    pub quality: QualityEvidence,
    pub disposition: String,
    pub disposition_detail: String,
    pub quarantine_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LumaSiftGroup {
    pub id: String,
    pub exact_hash: String,
    pub winner_id: String,
    pub reclaimable_bytes: u64,
    pub candidates: Vec<LumaSiftCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LumaSiftDisposition {
    pub occurred_at: String,
    pub file_path: String,
    pub display_name: String,
    pub disposition: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LumaSiftPlan {
    pub id: String,
    pub status: String,
    pub selected_types: Vec<String>,
    pub created_at: String,
    pub groups: Vec<LumaSiftGroup>,
    pub reclaimable_bytes: u64,
    pub queued_file_count: u64,
    pub dispositions: Vec<LumaSiftDisposition>,
}

#[derive(Debug, Clone)]
struct IndexedFile {
    file_path: String,
    media_kind: String,
    bytes: u64,
}

#[derive(Debug, Clone)]
struct CandidateSeed {
    file: IndexedFile,
    sampled_hash: String,
}

#[derive(Default)]
struct LumaSiftRuntime {
    progress: LumaSiftProgress,
    plan: Option<LumaSiftPlan>,
}

fn runtime() -> &'static Mutex<LumaSiftRuntime> {
    RUNTIME.get_or_init(|| Mutex::new(LumaSiftRuntime::default()))
}

fn runtime_lock() -> std::sync::MutexGuard<'static, LumaSiftRuntime> {
    runtime().lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn update_progress(
    phase: impl Into<String>,
    current: u64,
    total: u64,
    percentage: u8,
    path: Option<String>,
    message: impl Into<String>,
) {
    let mut state = runtime_lock();
    state.progress.scanning = true;
    state.progress.phase = phase.into();
    state.progress.current = current;
    state.progress.total = total;
    state.progress.percentage = percentage.min(99);
    state.progress.current_path = path;
    state.progress.message = message.into();
    state.progress.error = None;
}

fn finish_with_error(message: impl Into<String>) {
    let mut state = runtime_lock();
    state.progress.scanning = false;
    state.progress.phase = "Failed".to_string();
    state.progress.error = Some(message.into());
    state.progress.message = "LumaSift stopped without changing any media files.".to_string();
}

fn finish_plan(plan: LumaSiftPlan) {
    let mut state = runtime_lock();
    state.progress.scanning = false;
    state.progress.phase = "Review ready".to_string();
    state.progress.current = state.progress.total;
    state.progress.percentage = 100;
    state.progress.current_path = None;
    state.progress.message = format!(
        "{} exact-duplicate groups are ready for review; {} file(s) are queued for quarantine.",
        plan.groups.len(),
        plan.queued_file_count
    );
    state.progress.error = None;
    state.plan = Some(plan);
}

fn append_disposition(
    dispositions: &mut Vec<LumaSiftDisposition>,
    path: &str,
    disposition: &str,
    detail: impl Into<String>,
) {
    dispositions.push(LumaSiftDisposition {
        occurred_at: chrono::Utc::now().to_rfc3339(),
        file_path: path.to_string(),
        display_name: display_name(path),
        disposition: disposition.to_string(),
        detail: detail.into(),
    });
}

fn display_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

fn sample_hash(path: &Path, size: u64) -> Result<String, String> {
    let mut file = File::open(path)
        .map_err(|error| format!("Unable to open {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(size.to_le_bytes());

    let mut head = vec![0u8; SAMPLE_BYTES.min(size as usize)];
    let head_read = file
        .read(&mut head)
        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    hasher.update(&head[..head_read]);

    if size > SAMPLE_BYTES as u64 {
        let tail_start = size.saturating_sub(SAMPLE_BYTES as u64);
        file.seek(SeekFrom::Start(tail_start))
            .map_err(|error| format!("Unable to seek {}: {error}", path.display()))?;
        let mut tail = vec![0u8; SAMPLE_BYTES];
        let tail_read = file
            .read(&mut tail)
            .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
        hasher.update(&tail[..tail_read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn full_hash(path: &Path) -> Result<String, String> {
    let mut file = File::open(path)
        .map_err(|error| format!("Unable to open {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1_048_576];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn parse_optional_u64(value: Option<&serde_json::Value>) -> Option<u64> {
    value
        .and_then(|value| value.as_u64())
        .or_else(|| value.and_then(|value| value.as_str()).and_then(|value| value.parse().ok()))
}

fn probe_quality(path: &Path, bytes: u64) -> QualityEvidence {
    let mut evidence = QualityEvidence {
        file_size_bytes: bytes,
        ..Default::default()
    };
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=width,height,bit_rate,bits_per_raw_sample,duration:format=duration,bit_rate",
            "-of",
            "json",
            &path.to_string_lossy(),
        ])
        .output();

    let Ok(output) = output else {
        evidence
            .reasons
            .push("Media probe unavailable; file size was used as the quality fallback.".to_string());
        return evidence;
    };
    if !output.status.success() {
        evidence
            .reasons
            .push("Media metadata was unavailable; file size was used as the quality fallback.".to_string());
        return evidence;
    }
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        evidence
            .reasons
            .push("Media metadata could not be read; file size was used as the quality fallback.".to_string());
        return evidence;
    };
    let stream = json.get("streams").and_then(|value| value.as_array()).and_then(|items| items.first());
    evidence.width = parse_optional_u64(stream.and_then(|item| item.get("width")));
    evidence.height = parse_optional_u64(stream.and_then(|item| item.get("height")));
    evidence.bitrate = parse_optional_u64(stream.and_then(|item| item.get("bit_rate"))).or_else(|| {
        parse_optional_u64(json.get("format").and_then(|item| item.get("bit_rate")))
    });
    evidence.bit_depth = parse_optional_u64(stream.and_then(|item| item.get("bits_per_raw_sample")));
    evidence.duration_millis = stream
        .and_then(|item| item.get("duration"))
        .or_else(|| json.get("format").and_then(|item| item.get("duration")))
        .and_then(|value| value.as_str())
        .and_then(|value| value.parse::<f64>().ok())
        .map(|seconds| (seconds * 1_000.0).max(0.0) as u64);
    evidence.pixel_count = evidence.width.unwrap_or(0).saturating_mul(evidence.height.unwrap_or(0));

    if evidence.pixel_count > 0 {
        evidence.reasons.push(format!(
            "{}×{} pixels",
            evidence.width.unwrap_or(0),
            evidence.height.unwrap_or(0)
        ));
    }
    if let Some(bitrate) = evidence.bitrate {
        evidence.reasons.push(format!("{} kb/s bitrate", bitrate / 1_000));
    }
    if let Some(bit_depth) = evidence.bit_depth {
        evidence.reasons.push(format!("{}-bit source", bit_depth));
    }
    if let Some(duration) = evidence.duration_millis {
        evidence
            .reasons
            .push(format!("{} ms duration", duration));
    }
    evidence
        .reasons
        .push(format!("{} bytes", evidence.file_size_bytes));
    evidence
}

fn quality_score(evidence: &QualityEvidence) -> u64 {
    evidence
        .pixel_count
        .saturating_mul(10)
        .saturating_add(evidence.bitrate.unwrap_or(0) / 100)
        .saturating_add(evidence.bit_depth.unwrap_or(0).saturating_mul(1_000_000))
        .saturating_add(evidence.duration_millis.unwrap_or(0) / 10)
        .saturating_add(evidence.file_size_bytes / 1_024)
}

fn candidate_from_seed(seed: CandidateSeed, exact_hash: String) -> LumaSiftCandidate {
    let quality = probe_quality(Path::new(&seed.file.file_path), seed.file.bytes);
    let score = quality_score(&quality);
    LumaSiftCandidate {
        id: Uuid::new_v4().to_string(),
        file_path: seed.file.file_path.clone(),
        display_name: display_name(&seed.file.file_path),
        media_kind: seed.file.media_kind,
        exact_hash,
        quality_score: score,
        quality,
        disposition: "pending_review".to_string(),
        disposition_detail: "Awaiting a LumaSift resolution plan review.".to_string(),
        quarantine_path: None,
    }
}

fn build_plan(files: Vec<IndexedFile>, app_data_dir: PathBuf, selected_types: Vec<String>) {
    let total = files.len() as u64;
    let mut sampled: HashMap<(u64, String), Vec<CandidateSeed>> = HashMap::new();
    let mut dispositions = Vec::new();

    for (index, file) in files.into_iter().enumerate() {
        if CANCEL_REQUESTED.load(Ordering::Relaxed) {
            cancel_plan(dispositions, selected_types.clone());
            return;
        }
        let percentage = if total == 0 { 60 } else { ((index as u64 + 1) * 60 / total) as u8 };
        update_progress(
            "Sampling content",
            index as u64 + 1,
            total,
            percentage,
            Some(file.file_path.clone()),
            "Building safe collision candidates without changing media files.",
        );
        match sample_hash(Path::new(&file.file_path), file.bytes) {
            Ok(hash) => sampled
                .entry((file.bytes, hash.clone()))
                .or_default()
                .push(CandidateSeed {
                    file,
                    sampled_hash: hash,
                }),
            Err(error) => append_disposition(&mut dispositions, &file.file_path, "skipped", error),
        }
    }

    let candidate_groups: Vec<Vec<CandidateSeed>> = sampled
        .into_values()
        .filter(|group| group.len() > 1)
        .collect();
    let verification_total = candidate_groups.iter().map(Vec::len).sum::<usize>() as u64;
    let mut verified: HashMap<String, Vec<CandidateSeed>> = HashMap::new();
    let mut verified_count = 0u64;

    for group in candidate_groups {
        for seed in group {
            if CANCEL_REQUESTED.load(Ordering::Relaxed) {
                cancel_plan(dispositions);
                return;
            }
            verified_count += 1;
            let percentage = if verification_total == 0 {
                90
            } else {
                60 + ((verified_count * 30 / verification_total) as u8)
            };
            update_progress(
                "Verifying exact matches",
                verified_count,
                verification_total,
                percentage,
                Some(seed.file.file_path.clone()),
                "Calculating complete SHA-256 digests before a file may enter a plan.",
            );
            match full_hash(Path::new(&seed.file.file_path)) {
                Ok(hash) => verified.entry(hash).or_default().push(seed),
                Err(error) => append_disposition(&mut dispositions, &seed.file.file_path, "skipped", error),
            }
        }
    }

    let mut groups = Vec::new();
    let mut reclaimable_bytes = 0u64;
    let mut queued_file_count = 0u64;
    let exact_groups: Vec<(String, Vec<CandidateSeed>)> = verified
        .into_iter()
        .filter(|(_, group)| group.len() > 1)
        .collect();
    let exact_total = exact_groups.len() as u64;

    for (index, (exact_hash, seeds)) in exact_groups.into_iter().enumerate() {
        if CANCEL_REQUESTED.load(Ordering::Relaxed) {
            cancel_plan(dispositions, selected_types.clone());
            return;
        }
        let percentage = if exact_total == 0 {
            99
        } else {
            90 + (((index as u64 + 1) * 9 / exact_total) as u8)
        };
        update_progress(
            "Ranking quality",
            index as u64 + 1,
            exact_total,
            percentage,
            None,
            "Comparing transparent quality signals for each proven duplicate group.",
        );
        let mut candidates: Vec<LumaSiftCandidate> = seeds
            .into_iter()
            .map(|seed| candidate_from_seed(seed, exact_hash.clone()))
            .collect();
        candidates.sort_by(|left, right| {
            right
                .quality_score
                .cmp(&left.quality_score)
                .then_with(|| right.quality.file_size_bytes.cmp(&left.quality.file_size_bytes))
                .then_with(|| left.file_path.cmp(&right.file_path))
        });
        let winner_id = candidates
            .first()
            .map(|candidate| candidate.id.clone())
            .expect("exact duplicate groups contain at least two candidates");
        let mut group_reclaimable = 0u64;
        for candidate in &mut candidates {
            if candidate.id == winner_id {
                candidate.disposition = "retain".to_string();
                candidate.disposition_detail = "Highest deterministic quality score in this exact-content group.".to_string();
                append_disposition(
                    &mut dispositions,
                    &candidate.file_path,
                    "retain",
                    &candidate.disposition_detail,
                );
            } else {
                candidate.disposition = "queued_for_quarantine".to_string();
                candidate.disposition_detail = "Lower-ranked exact duplicate. It will move to quarantine only after your approval.".to_string();
                group_reclaimable = group_reclaimable.saturating_add(candidate.quality.file_size_bytes);
                queued_file_count += 1;
                append_disposition(
                    &mut dispositions,
                    &candidate.file_path,
                    "queued_for_quarantine",
                    &candidate.disposition_detail,
                );
            }
        }
        reclaimable_bytes = reclaimable_bytes.saturating_add(group_reclaimable);
        groups.push(LumaSiftGroup {
            id: Uuid::new_v4().to_string(),
            exact_hash,
            winner_id,
            reclaimable_bytes: group_reclaimable,
            candidates,
        });
    }

    let plan = LumaSiftPlan {
        id: Uuid::new_v4().to_string(),
        status: "ready_for_review".to_string(),
        selected_types,
        created_at: chrono::Utc::now().to_rfc3339(),
        groups,
        reclaimable_bytes,
        queued_file_count,
        dispositions,
    };
    let plan_directory = app_data_dir.join("lumasift");
    if let Err(error) = fs::create_dir_all(&plan_directory) {
        log::warn!("LumaSift could not create its plan directory: {error}");
    } else if let Ok(json) = serde_json::to_vec_pretty(&plan) {
        if let Err(error) = fs::write(plan_directory.join("last-resolution-plan.json"), json) {
            log::warn!("LumaSift could not persist its review plan: {error}");
        }
    }
    finish_plan(plan);
}

fn cancel_plan(dispositions: Vec<LumaSiftDisposition>, selected_types: Vec<String>) {
    let plan = LumaSiftPlan {
        id: Uuid::new_v4().to_string(),
        status: "cancelled".to_string(),
        selected_types,
        created_at: chrono::Utc::now().to_rfc3339(),
        groups: Vec::new(),
        reclaimable_bytes: 0,
        queued_file_count: 0,
        dispositions,
    };
    let mut state = runtime_lock();
    state.progress.scanning = false;
    state.progress.phase = "Cancelled".to_string();
    state.progress.message = "LumaSift cancelled before producing an actionable resolution plan.".to_string();
    state.progress.current_path = None;
    state.plan = Some(plan);
}

fn normalized_selected_types(selected_types: Option<Vec<String>>) -> Result<Vec<String>, String> {
    let requested = selected_types.unwrap_or_else(|| {
        SUPPORTED_SELECTION_TYPES.iter().map(|value| value.to_string()).collect()
    });
    let mut normalized = Vec::new();
    for value in requested {
        let value = value.trim().to_ascii_lowercase();
        if value.is_empty() || normalized.contains(&value) {
            continue;
        }
        if !SUPPORTED_SELECTION_TYPES.contains(&value.as_str()) {
            return Err(format!("Unsupported LumaSift selection type: {value}"));
        }
        normalized.push(value);
    }
    if normalized.is_empty() {
        return Err("Select at least one LumaSift file type before starting a scan.".to_string());
    }
    Ok(normalized)
}

fn selected_media_type(media_type: &str, selected_types: &[String]) -> bool {
    match media_type {
        "movie" => selected_types.iter().any(|value| value == "video"),
        "music" => selected_types.iter().any(|value| value == "audio"),
        "image" | "photo" => selected_types.iter().any(|value| value == "image"),
        "document" => selected_types.iter().any(|value| value == "document"),
        _ => false,
    }
}

fn source_files(selected_types: &[String]) -> Result<Vec<IndexedFile>, String> {
    let config = coordinator_config()?;
    let database = Database::new(&config.database_path).map_err(|error| error.to_string())?;
    let mut statement = database
        .conn
        .prepare(
            "SELECT file_path, media_type, file_size FROM media_items
             WHERE media_type IN ('movie', 'music', 'image', 'photo', 'document') ORDER BY file_path",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            let file_path: String = row.get(0)?;
            let media_kind: String = row.get(1)?;
            let size: Option<i64> = row.get(2)?;
            Ok((file_path, media_kind, size))
        })
        .map_err(|error| error.to_string())?;

    Ok(rows
        .filter_map(Result::ok)
        .filter(|(_, media_kind, _)| selected_media_type(media_kind, selected_types))
        .filter_map(|(file_path, media_kind, size)| {
            let path = Path::new(&file_path);
            let is_mp3 = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mp3"));
            if media_kind == "music" && !is_mp3 {
                return None;
            }
            let bytes = size
                .and_then(|value| u64::try_from(value).ok())
                .or_else(|| path.metadata().ok().map(|metadata| metadata.len()))?;
            path.is_file().then_some(IndexedFile {
                file_path,
                media_kind,
                bytes,
            })
        })
        .collect())
}

#[tauri::command]
pub async fn start_lumasift_resolution(
    _state: State<'_, AppState>,
    selected_types: Option<Vec<String>>,
) -> Result<serde_json::Value, String> {
    start_configured_lumasift_resolution(selected_types)
}

pub fn start_configured_lumasift_resolution(
    selected_types: Option<Vec<String>>,
) -> Result<serde_json::Value, String> {
    {
        let active = runtime_lock();
        if active.progress.scanning {
            return Err("A LumaSift resolution scan is already in progress.".to_string());
        }
    }
    let selected_types = normalized_selected_types(selected_types)?;
    let files = source_files(&selected_types)?;
    CANCEL_REQUESTED.store(false, Ordering::Relaxed);
    {
        let mut active = runtime_lock();
        active.plan = None;
        active.progress = LumaSiftProgress {
            scanning: true,
            phase: "Preparing".to_string(),
            current: 0,
            total: files.len() as u64,
            percentage: 0,
            current_path: None,
            files_considered: files.len() as u64,
            message: format!("Preparing an exact-content duplicate scan for {}. No files will be changed.", selected_types.join(", ")),
            error: None,
        };
    }
    let app_data_dir = coordinator_config()?.app_data_dir;
    let request_id = Uuid::new_v4().to_string();
    let worker_selected_types = selected_types.clone();
    thread::Builder::new()
        .name("lumasift-resolution".to_string())
        .spawn(move || build_plan(files, app_data_dir, worker_selected_types))
        .map_err(|error| format!("Unable to start LumaSift resolution scan: {error}"))?;
    Ok(serde_json::json!({
        "request_id": request_id,
        "status": "started",
        "selected_types": selected_types,
        "message": "LumaSift is building a review-only plan. No files are being moved or deleted.",
    }))
}

#[tauri::command]
pub fn get_lumasift_progress() -> LumaSiftProgress {
    runtime_lock().progress.clone()
}

#[tauri::command]
pub fn get_lumasift_plan() -> Option<LumaSiftPlan> {
    runtime_lock().plan.clone()
}

#[tauri::command]
pub fn cancel_lumasift_resolution() -> Result<(), String> {
    if !runtime_lock().progress.scanning {
        return Err("No LumaSift resolution scan is active.".to_string());
    }
    CANCEL_REQUESTED.store(true, Ordering::Relaxed);
    Ok(())
}

fn unique_destination(directory: &Path, source: &Path) -> Result<PathBuf, String> {
    let file_name = source
        .file_name()
        .ok_or_else(|| format!("Source path has no file name: {}", source.display()))?;
    let direct = directory.join(file_name);
    if !direct.exists() {
        return Ok(direct);
    }
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("duplicate");
    let extension = source.extension().and_then(|value| value.to_str());
    for suffix in 1..=10_000u32 {
        let candidate = match extension {
            Some(extension) if !extension.is_empty() => {
                directory.join(format!("{stem} ({suffix}).{extension}"))
            }
            _ => directory.join(format!("{stem} ({suffix})")),
        };
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err("LumaSift could not allocate a unique quarantine destination.".to_string())
}

fn move_without_overwrite(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        return Err(format!(
            "LumaSift refused to overwrite an existing quarantine file: {}",
            destination.display()
        ));
    }
    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            fs::copy(source, destination).map_err(|copy_error| {
                format!(
                    "LumaSift could not move {} ({rename_error}); copy fallback failed: {copy_error}",
                    source.display()
                )
            })?;
            if let Err(remove_error) = fs::remove_file(source) {
                let _ = fs::remove_file(destination);
                return Err(format!(
                    "LumaSift copied {} but could not remove the source: {remove_error}",
                    source.display()
                ));
            }
            Ok(())
        }
    }
}

#[tauri::command]
pub fn apply_lumasift_plan(
    _state: State<'_, AppState>,
    plan_id: String,
) -> Result<serde_json::Value, String> {
    apply_configured_lumasift_plan(plan_id)
}

pub fn apply_configured_lumasift_plan(plan_id: String) -> Result<serde_json::Value, String> {
    let config = coordinator_config()?;
    let mut plan = runtime_lock()
        .plan
        .clone()
        .ok_or("No LumaSift plan is available for review.")?;
    if plan.id != plan_id {
        return Err("The requested LumaSift plan is stale. Refresh and review the current plan.".to_string());
    }
    if plan.status != "ready_for_review" {
        return Err("Only a ready-for-review LumaSift plan may be applied.".to_string());
    }

    let destination_directory = config
        .app_data_dir
        .join("lumasift")
        .join("quarantine")
        .join(&plan.id);
    fs::create_dir_all(&destination_directory)
        .map_err(|error| format!("Unable to create LumaSift quarantine: {error}"))?;

    let mut quarantined = 0u64;
    let mut failed = 0u64;
    for group in &mut plan.groups {
        for candidate in &mut group.candidates {
            if candidate.disposition != "queued_for_quarantine" {
                continue;
            }
            let source = PathBuf::from(&candidate.file_path);
            let result = (|| -> Result<PathBuf, String> {
                let actual_hash = full_hash(&source)?;
                if actual_hash != candidate.exact_hash {
                    return Err("Content changed since the plan was created; the file was left in place.".to_string());
                }
                let destination = unique_destination(&destination_directory, &source)?;
                move_without_overwrite(&source, &destination)?;
                Ok(destination)
            })();
            match result {
                Ok(destination) => {
                    let destination_text = destination.to_string_lossy().into_owned();
                    candidate.disposition = "quarantined".to_string();
                    candidate.disposition_detail = "Moved to LumaSift quarantine. It has not been permanently deleted.".to_string();
                    candidate.quarantine_path = Some(destination_text.clone());
                    append_disposition(
                        &mut plan.dispositions,
                        &candidate.file_path,
                        "quarantined",
                        &candidate.disposition_detail,
                    );
                    let database = Database::new(&config.database_path).map_err(|error| error.to_string())?;
                    database
                        .conn
                        .execute("DELETE FROM media_items WHERE file_path = ?1", params![candidate.file_path])
                        .map_err(|error| error.to_string())?;
                    quarantined += 1;
                }
                Err(error) => {
                    candidate.disposition = "failed".to_string();
                    candidate.disposition_detail = error.clone();
                    append_disposition(&mut plan.dispositions, &candidate.file_path, "failed", error);
                    failed += 1;
                }
            }
        }
    }
    plan.status = if failed == 0 {
        "applied_to_quarantine".to_string()
    } else {
        "partially_applied".to_string()
    };
    plan.queued_file_count = plan
        .groups
        .iter()
        .flat_map(|group| group.candidates.iter())
        .filter(|candidate| candidate.disposition == "queued_for_quarantine")
        .count() as u64;
    let mut active = runtime_lock();
    active.plan = Some(plan.clone());
    active.progress.message = format!(
        "LumaSift quarantined {quarantined} file(s); {failed} file(s) were left in place with an error disposition."
    );
    Ok(serde_json::json!({
        "status": plan.status,
        "plan_id": plan.id,
        "quarantined": quarantined,
        "failed": failed,
        "message": "Files were moved to quarantine. Permanent erase remains a separate explicit action.",
    }))
}

#[tauri::command]
pub fn purge_lumasift_quarantine(
    _state: State<'_, AppState>,
    confirmation: String,
) -> Result<serde_json::Value, String> {
    purge_configured_lumasift_quarantine(confirmation)
}

pub fn purge_configured_lumasift_quarantine(
    confirmation: String,
) -> Result<serde_json::Value, String> {
    if confirmation.trim() != PURGE_CONFIRMATION {
        return Err(format!(
            "Permanent erase requires the exact confirmation: {PURGE_CONFIRMATION}"
        ));
    }
    let quarantine = coordinator_config()?.app_data_dir.join("lumasift").join("quarantine");
    if !quarantine.exists() {
        return Ok(serde_json::json!({"status": "empty", "erased": 0}));
    }
    let mut erased = 0u64;
    for entry in walkdir::WalkDir::new(&quarantine)
        .follow_links(false)
        .contents_first(true)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if entry.file_type().is_file() {
            fs::remove_file(path)
                .map_err(|error| format!("Unable to permanently erase {}: {error}", path.display()))?;
            erased += 1;
        } else if path != quarantine {
            let _ = fs::remove_dir(path);
        }
    }
    let _ = fs::remove_dir(&quarantine);
    Ok(serde_json::json!({
        "status": "erased",
        "erased": erased,
        "message": "LumaSift quarantine was permanently erased after explicit confirmation.",
    }))
}

#[cfg(test)]
mod tests {
    use super::{quality_score, QualityEvidence};

    #[test]
    fn higher_pixel_count_wins_before_file_size() {
        let high_resolution = QualityEvidence {
            pixel_count: 3_840 * 2_160,
            file_size_bytes: 1_000,
            ..Default::default()
        };
        let larger_but_lower_resolution = QualityEvidence {
            pixel_count: 1_920 * 1_080,
            file_size_bytes: 100_000_000,
            ..Default::default()
        };
        assert!(quality_score(&high_resolution) > quality_score(&larger_but_lower_resolution));
    }
}
