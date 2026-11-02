// CinaVault Premium — Enrichment background-job registry (Issue 1).
//
// Problem fixed: `ai_query` (adult route), `gather_adult_metadata`, and
// `run_library_enrichment` used to await whole-library enrichment loops
// inline, so the invoking command did not return until every item had been
// tried. A library with slow providers could therefore stall the caller for
// minutes to hours, with no job identity, no in-flight guard, and a stop
// request that was only honoured between items.
//
// This module owns the job lifecycle instead:
//   * `start_adult_metadata_job` / `start_library_enrichment_job` claim the
//     single in-flight slot and spawn the work on Tauri's async runtime,
//     returning an `EnrichmentJobStart` immediately.
//   * At most one enrichment job runs at a time. A second start attempt
//     returns `status: "busy"` with the running job's id instead of
//     stacking a second whole-library loop on top of the first.
//   * `get_enrichment_job_status` is the poll endpoint. While a job runs it
//     overlays the live `MetadataTaskProgress` snapshot (current / total /
//     percent / message) so the frontend progress popup keeps working
//     unchanged. When the job finishes, `result` holds the full report
//     (`AdultMetadataReport` or `LibraryEnrichmentReport`) in exactly the
//     shape the old synchronous commands used to return.
//   * `request_stop_active_job` is called by `task_progress::stop_ai_agent`.
//     It aborts the spawned task (so a hanging provider call is dropped at
//     its next await point instead of stalling the stop) and marks the job
//     `cancelled` immediately, so polling never observes a stale `running`
//     job. If stop arrives in the tiny window before the task's abort handle
//     is registered, a `stop_pending` flag is recorded instead and the task
//     exits as `cancelled` before starting work. The global cancellation
//     flag is still set as well, so the per-item / per-provider stop checks
//     inside the loops remain the second line of defence.
//
// Time bounds (see also the per-item / per-provider timeouts applied inside
// `enrichment::resolve_provider_match` and friends):
//   * `PROVIDER_TIMEOUT_SECS` bounds one provider call chain. The reqwest
//     clients already carry 12s (library) / 15s (adult) / 20s (keyless)
//     timeouts; this wrapper is the backstop for multi-call provider chains
//     (e.g. the keyless extra-adult batch chain) whose total can exceed any
//     single request timeout.
//   * `ITEM_TIMEOUT_SECS` bounds one item's whole remote phase. On expiry
//     the item is skipped with a recorded error and retried on the next run
//     (nothing was written for it, and skip-complete only skips items whose
//     metadata is already fully stored).
//   * `JOB_TIMEOUT_SECS` is a backstop for the whole job, not a normal
//     limit: per-item timeouts already bound the worst case to
//     `items x ITEM_TIMEOUT_SECS`. A job that exceeds it fails with a clear
//     error; rerunning resumes, because already-complete items are skipped.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex, OnceLock,
};
use tauri::{AppHandle, Manager};

/// Maximum provider calls are capped by query count, not by a separate
/// provider-count constant: each item tries at most this many query
/// candidates (highest-confidence first: embedded title, then library
/// title), and each candidate tries its providers in order, stopping at the
/// first match. Previously up to 4 candidates were tried per item.
pub const MAX_QUERY_CANDIDATES_PER_ITEM: usize = 2;

/// Backstop for a single provider call chain (see module docs).
pub const PROVIDER_TIMEOUT_SECS: u64 = 20;

/// Backstop for one item's whole remote phase (resolve + adult fallback).
pub const ITEM_TIMEOUT_SECS: u64 = 120;

/// Backstop for a whole job (see module docs). Six hours.
pub const JOB_TIMEOUT_SECS: u64 = 6 * 60 * 60;

/// How many finished jobs are remembered for polling by id. Older finished
/// jobs are pruned; the active job is never pruned while running.
const MAX_REMEMBERED_JOBS: usize = 8;

/// Immediate return value of a job-starting command.
///
/// `status` is `"started"` for a freshly claimed job and `"busy"` when a job
/// is already running. In the busy case `job_id` is the *running* job's id,
/// so the caller can poll that job instead of starting a second one.
#[derive(Debug, Clone, Serialize)]
pub struct EnrichmentJobStart {
    pub job_id: String,
    pub kind: String,
    pub status: String,
    pub message: String,
}

/// Pollable snapshot of an enrichment job.
///
/// Field names are snake_case to match the existing report shapes
/// (`items_scanned`, `metadata_items_enriched`, ...). `result` is present
/// only once `status` is `"completed"` and holds the full report the old
/// synchronous command used to return. `error` is present only when
/// `status` is `"failed"` or `"cancelled"`.
#[derive(Debug, Clone, Serialize)]
pub struct EnrichmentJobStatus {
    pub job_id: String,
    pub kind: String,
    /// `"idle" | "running" | "completed" | "failed" | "cancelled"`.
    pub status: String,
    pub current: usize,
    pub total: usize,
    pub percent: u8,
    pub message: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrichmentJobKind {
    AdultMetadata,
    LibraryEnrichment,
}

impl EnrichmentJobKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EnrichmentJobKind::AdultMetadata => "adult_metadata",
            EnrichmentJobKind::LibraryEnrichment => "library_enrichment",
        }
    }

    /// Starting message shown while the job is in its pre-guard phase
    /// (item loading / keyless prepass), before `MetadataTaskGuard` starts
    /// publishing live progress.
    fn starting_message(self) -> &'static str {
        match self {
            EnrichmentJobKind::AdultMetadata => "Starting adult metadata gather...",
            EnrichmentJobKind::LibraryEnrichment => "Starting library enrichment...",
        }
    }

    /// The `MetadataTaskGuard` task name the work function publishes under,
    /// used to overlay only matching live progress onto this job's status.
    fn progress_task(self) -> &'static str {
        match self {
            EnrichmentJobKind::AdultMetadata => "adult_metadata_gather",
            EnrichmentJobKind::LibraryEnrichment => "library_enrichment",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "adult_metadata" => Some(EnrichmentJobKind::AdultMetadata),
            "library_enrichment" => Some(EnrichmentJobKind::LibraryEnrichment),
            _ => None,
        }
    }
}

struct JobRecord {
    status: EnrichmentJobStatus,
    stop_pending: bool,
}

struct Registry {
    jobs: HashMap<String, JobRecord>,
    /// Insertion order (oldest first) for pruning remembered jobs.
    order: Vec<String>,
    active_job_id: Option<String>,
    abort_handles: HashMap<String, tokio::task::AbortHandle>,
}

fn lock_registry() -> std::sync::MutexGuard<'static, Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY
        .get_or_init(|| {
            Mutex::new(Registry {
                jobs: HashMap::new(),
                order: Vec::new(),
                active_job_id: None,
                abort_handles: HashMap::new(),
            })
        })
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn next_job_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    format!("job-{}", COUNTER.fetch_add(1, Ordering::SeqCst) + 1)
}

fn prune_remembered_jobs_locked(registry: &mut Registry) {
    while registry.order.len() > MAX_REMEMBERED_JOBS {
        let Some(oldest) = registry.order.first().cloned() else {
            break;
        };
        if registry.active_job_id.as_deref() == Some(oldest.as_str()) {
            break;
        }
        registry.order.remove(0);
        registry.jobs.remove(&oldest);
        registry.abort_handles.remove(&oldest);
    }
}

/// Claim the single in-flight slot for `kind`.
///
/// Returns the new job id, or — when a job is already running — a `busy`
/// start response naming the running job, so the caller can poll it instead
/// of stacking a second loop.
fn claim_job(kind: EnrichmentJobKind) -> Result<String, EnrichmentJobStart> {
    let mut registry = lock_registry();
    if let Some(active_id) = registry.active_job_id.clone() {
        if let Some(record) = registry.jobs.get(&active_id) {
            if record.status.status == "running" {
                return Err(EnrichmentJobStart {
                    job_id: active_id,
                    kind: record.status.kind.clone(),
                    status: "busy".to_string(),
                    message: "An enrichment job is already running; follow the active job instead of starting a second one.".to_string(),
                });
            }
        }
        // Stale active pointer (job finished without clearing it): clear it
        // rather than blocking all future jobs.
        registry.active_job_id = None;
    }

    let job_id = next_job_id();
    registry.jobs.insert(
        job_id.clone(),
        JobRecord {
            status: EnrichmentJobStatus {
                job_id: job_id.clone(),
                kind: kind.as_str().to_string(),
                status: "running".to_string(),
                current: 0,
                total: 0,
                percent: 0,
                message: kind.starting_message().to_string(),
                result: None,
                error: None,
            },
            stop_pending: false,
        },
    );
    registry.order.push(job_id.clone());
    registry.active_job_id = Some(job_id.clone());
    prune_remembered_jobs_locked(&mut registry);
    Ok(job_id)
}

fn register_abort_handle(job_id: &str, handle: tokio::task::AbortHandle) {
    let mut registry = lock_registry();
    if let Some(record) = registry.jobs.get(job_id) {
        if record.status.status == "running" {
            registry.abort_handles.insert(job_id.to_string(), handle);
        }
    }
}

/// Called by the spawned task before starting work. Returns true when the
/// task must exit without running (stop arrived before the abort handle was
/// registered, or the job was already cancelled).
fn take_pending_stop(job_id: &str) -> bool {
    let mut registry = lock_registry();
    let Some(record) = registry.jobs.get_mut(job_id) else {
        return true;
    };
    if record.status.status != "running" {
        return true;
    }
    if record.stop_pending {
        record.status.status = "cancelled".to_string();
        record.status.message = "Stopped by user before the job started".to_string();
        record.status.error = Some("Stopped by user".to_string());
        if registry.active_job_id.as_deref() == Some(job_id) {
            registry.active_job_id = None;
        }
        registry.abort_handles.remove(job_id);
        return true;
    }
    false
}

/// Transition a job to a terminal state, but only if it is still running.
/// This is what makes the stop path race-free: whichever of "abort + mark
/// cancelled" and "work finished" wins the registry lock first owns the
/// final state; the loser is ignored.
fn finish_job(job_id: &str, outcome: Result<serde_json::Value, String>) {
    let mut registry = lock_registry();
    let Some(record) = registry.jobs.get_mut(job_id) else {
        return;
    };
    if record.status.status != "running" {
        return;
    }
    match outcome {
        Ok(result) => {
            record.status.status = "completed".to_string();
            record.status.percent = 100;
            record.status.message = "Enrichment job complete".to_string();
            record.status.result = Some(result);
            record.status.error = None;
        }
        Err(error) => {
            record.status.status = "failed".to_string();
            record.status.message = format!("Enrichment job failed: {error}");
            record.status.error = Some(error);
        }
    }
    if registry.active_job_id.as_deref() == Some(job_id) {
        registry.active_job_id = None;
    }
    registry.abort_handles.remove(job_id);
}

/// Stop path used by `task_progress::stop_ai_agent`.
///
/// Aborts the spawned task when its handle is registered and marks the job
/// `cancelled` immediately (so polling never sees a stale `running` job);
/// otherwise records `stop_pending` so the task exits as `cancelled` before
/// starting work. The global cancellation flag is set separately by the
/// caller, so loops that are mid-item still observe the stop at their next
/// per-item / per-provider check.
pub fn request_stop_active_job() {
    let mut registry = lock_registry();
    let Some(active_id) = registry.active_job_id.clone() else {
        return;
    };
    if let Some(handle) = registry.abort_handles.remove(&active_id) {
        handle.abort();
        if let Some(record) = registry.jobs.get_mut(&active_id) {
            if record.status.status == "running" {
                record.status.status = "cancelled".to_string();
                record.status.message = "Stopped by user".to_string();
                record.status.error = Some("Stopped by user".to_string());
            }
        }
        registry.active_job_id = None;
    } else if let Some(record) = registry.jobs.get_mut(&active_id) {
        if record.status.status == "running" {
            record.stop_pending = true;
        }
    }
}

/// True while any enrichment job is running. Used as a guard by the
/// still-synchronous composite paths (`ai_library_manage`,
/// `convert_entire_library_to_adult`) so they skip / refuse enrichment
/// instead of stacking a second loop on top of a running job.
pub fn active_job_running() -> bool {
    let registry = lock_registry();
    match registry.active_job_id.as_deref() {
        Some(id) => registry
            .jobs
            .get(id)
            .map(|record| record.status.status == "running")
            .unwrap_or(false),
        None => false,
    }
}

/// Start an adult-metadata job. Returns immediately; poll
/// `get_enrichment_job_status` for progress and the final report.
pub fn start_adult_metadata_job(app: &AppHandle) -> EnrichmentJobStart {
    let kind = EnrichmentJobKind::AdultMetadata;
    let job_id = match claim_job(kind) {
        Ok(job_id) => job_id,
        Err(busy) => return busy,
    };
    let handle = app.clone();
    let task_job_id = job_id.clone();
    let task = tauri::async_runtime::spawn(async move {
        if take_pending_stop(&task_job_id) {
            return;
        }
        let state = handle.state::<crate::AppState>();
        let outcome = match tokio::time::timeout(
            std::time::Duration::from_secs(JOB_TIMEOUT_SECS),
            crate::enrichment::gather_adult_metadata_work(state),
        )
        .await
        {
            Ok(Ok(report)) => serde_json::to_value(report).map_err(|error| error.to_string()),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(format!(
                "Enrichment job timed out after {JOB_TIMEOUT_SECS} seconds; rerun to resume the remaining items (already-complete items are skipped)"
            )),
        };
        finish_job(&task_job_id, outcome);
    });
    register_abort_handle(&job_id, task.abort_handle());
    EnrichmentJobStart {
        job_id,
        kind: kind.as_str().to_string(),
        status: "started".to_string(),
        message: kind.starting_message().to_string(),
    }
}

/// Start a library-enrichment job (keyless prepass + enrichment). Returns
/// immediately; poll `get_enrichment_job_status` for progress and the final
/// report.
pub fn start_library_enrichment_job(
    app: &AppHandle,
    rename_files: bool,
) -> EnrichmentJobStart {
    let kind = EnrichmentJobKind::LibraryEnrichment;
    let job_id = match claim_job(kind) {
        Ok(job_id) => job_id,
        Err(busy) => return busy,
    };
    let handle = app.clone();
    let task_job_id = job_id.clone();
    let task = tauri::async_runtime::spawn(async move {
        if take_pending_stop(&task_job_id) {
            return;
        }
        let state = handle.state::<crate::AppState>();
        let outcome = match tokio::time::timeout(
            std::time::Duration::from_secs(JOB_TIMEOUT_SECS),
            crate::metadata_enrichment_runtime::run_library_enrichment_work(
                state,
                rename_files,
            ),
        )
        .await
        {
            Ok(Ok(report)) => serde_json::to_value(report).map_err(|error| error.to_string()),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(format!(
                "Enrichment job timed out after {JOB_TIMEOUT_SECS} seconds; rerun to resume the remaining items (already-complete items are skipped)"
            )),
        };
        finish_job(&task_job_id, outcome);
    });
    register_abort_handle(&job_id, task.abort_handle());
    EnrichmentJobStart {
        job_id,
        kind: kind.as_str().to_string(),
        status: "started".to_string(),
        message: kind.starting_message().to_string(),
    }
}

/// Poll endpoint for enrichment jobs.
///
/// With `job_id`, returns that job (or the latest job when the id is
/// unknown / pruned). Without `job_id`, returns the active job, else the
/// most recently finished job, else an `idle` status. While a job is
/// running, the live `MetadataTaskProgress` snapshot is overlaid onto
/// current / total / percent / message, so the existing progress popup and
/// `get_metadata_task_progress` polling keep working unchanged.
#[tauri::command]
pub fn get_enrichment_job_status(job_id: Option<String>) -> EnrichmentJobStatus {
    let registry = lock_registry();
    let record = match job_id.as_deref() {
        Some(id) => registry.jobs.get(id),
        None => match registry.active_job_id.as_deref() {
            Some(active) => registry.jobs.get(active),
            None => registry.order.last().and_then(|id| registry.jobs.get(id)),
        },
    };
    let Some(record) = record else {
        return EnrichmentJobStatus {
            job_id: String::new(),
            kind: String::new(),
            status: "idle".to_string(),
            current: 0,
            total: 0,
            percent: 0,
            message: "No enrichment job has run yet".to_string(),
            result: None,
            error: None,
        };
    };
    let mut status = record.status.clone();
    if status.status == "running" {
        if let Some(kind) = EnrichmentJobKind::from_str(&status.kind) {
            let progress = crate::task_progress::get_metadata_task_progress();
            if progress.active && progress.task == kind.progress_task() {
                status.current = progress.current;
                status.total = progress.total;
                status.percent = progress.percent;
                status.message = progress.message;
            }
        }
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(test)]
    fn reset_registry_for_test() {
        let mut registry = lock_registry();
        registry.jobs.clear();
        registry.order.clear();
        registry.active_job_id = None;
        registry.abort_handles.clear();
    }

    #[test]
    fn timeout_bounds_are_ordered_provider_then_item_then_job() {
        assert!(PROVIDER_TIMEOUT_SECS < ITEM_TIMEOUT_SECS);
        assert!(ITEM_TIMEOUT_SECS < JOB_TIMEOUT_SECS);
        assert_eq!(MAX_QUERY_CANDIDATES_PER_ITEM, 2);
    }

    #[test]
    fn job_registry_claim_busy_finish_sequence() {
        reset_registry_for_test();

        assert!(!active_job_running());
        assert_eq!(
            get_enrichment_job_status(None).status,
            "idle",
            "no job has run yet in a fresh registry"
        );

        let first = claim_job(EnrichmentJobKind::AdultMetadata)
            .expect("first claim succeeds");
        assert!(active_job_running());

        let busy = claim_job(EnrichmentJobKind::LibraryEnrichment)
            .expect_err("second claim while a job runs must be busy");
        assert_eq!(busy.status, "busy");
        assert_eq!(busy.job_id, first, "busy names the running job");

        let running = get_enrichment_job_status(Some(first.clone()));
        assert_eq!(running.status, "running");
        assert_eq!(running.job_id, first);

        finish_job(
            &first,
            Ok(serde_json::json!({ "type": "adult_metadata_gather", "status": "success" })),
        );
        assert!(!active_job_running());
        let finished = get_enrichment_job_status(Some(first.clone()));
        assert_eq!(finished.status, "completed");
        assert_eq!(finished.percent, 100);
        assert_eq!(
            finished.result.and_then(|value| value
                .get("type")
                .and_then(|value| value.as_str())
                .map(str::to_string))
                .as_deref(),
            Some("adult_metadata_gather")
        );

        // A finished job no longer blocks the slot.
        let second = claim_job(EnrichmentJobKind::LibraryEnrichment)
            .expect("claim after finish succeeds");
        assert_ne!(second, first);
        finish_job(&second, Err("simulated failure".to_string()));
        assert_eq!(
            get_enrichment_job_status(Some(second)).status,
            "failed"
        );
    }
}
