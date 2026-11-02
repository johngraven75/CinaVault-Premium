//! Work that follows a library scan, in the background so the scan returns
//! as soon as files are indexed. Each step is its own switch:
//! poster_sync (import shared artwork, later export), auto_metadata (look up
//! the new titles), subtitle_fetch (OpenSubtitles), chapter_thumbs (ffmpeg
//! frames) and collection_auto (regroup the library).
//!
//! Progress goes through task_progress (the same bar and Stop button as the
//! metadata tools); scans that finish while this runs queue their new items.
//!
//! Issue 2: adult items no longer go through the single-item check here.
//! Newly scanned items are first run through the hint classifier (an item
//! from an adult-named source, or with an adult hint in its title or path,
//! is reclassified to `adult` before any lookup), and adult items — newly
//! scanned, rescanned, or still pending from an earlier failed attempt —
//! are enriched with the gather's own per-item path
//! (`enrichment::enrich_adult_item`: provider merge, sidecar/app-cache
//! poster, chapter-frame fallback, NFO + DB writes, durable attempt
//! record). The pass is skipped in store-safe builds, and deferred when an
//! Issue 1 enrichment job is still running after the wait cap, instead of
//! stacking a second whole-library loop on top of it.

use crate::task_progress::{self, MetadataTaskGuard};
use crate::AppState;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

const LAST_RUN_SETTING: &str = "post_scan_last_run";
/// How long to wait for another metadata task to finish before starting.
const WAIT_FOR_OTHER_TASK: Duration = Duration::from_secs(15 * 60);

static APP: OnceLock<AppHandle> = OnceLock::new();
static RUNNING: AtomicBool = AtomicBool::new(false);
static PENDING: Mutex<BTreeSet<i64>> = Mutex::new(BTreeSet::new());
/// Rows a rescan found again without saved metadata or artwork. They only go
/// through the metadata lookup, not the new-title steps (subtitles, frames).
static RETRY: Mutex<BTreeSet<i64>> = Mutex::new(BTreeSet::new());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Steps {
    pub poster_sync: bool,
    pub auto_metadata: bool,
    pub subtitle_fetch: bool,
    pub chapter_thumbs: bool,
    pub collection_auto: bool,
}

impl Steps {
    pub fn read(db: &crate::db::Database) -> Self {
        Self {
            poster_sync: crate::feature_flags::is_enabled(db, "poster_sync")
                && crate::poster_sync::settings(db).folder.is_some(),
            auto_metadata: crate::feature_flags::is_enabled(db, "auto_metadata"),
            subtitle_fetch: crate::feature_flags::is_enabled(db, "subtitle_fetch"),
            chapter_thumbs: crate::feature_flags::is_enabled(db, "chapter_thumbs"),
            collection_auto: crate::feature_flags::is_enabled(db, "collection_auto"),
        }
    }

    /// Whether a scan that added `added` items, and found `retry` older items
    /// still missing metadata, has anything to do.
    pub fn needed(&self, added: usize, retry: usize) -> bool {
        self.collection_auto
            || (retry > 0 && self.auto_metadata)
            || (added > 0
                && (self.poster_sync
                    || self.auto_metadata
                    || self.subtitle_fetch
                    || self.chapter_thumbs))
    }
}

/// Gives the background worker the app handle (called once from setup).
pub fn configure(app: AppHandle) {
    APP.set(app).ok();
}

/// Queues the items a scan added (and older items to look up again) and
/// starts the background run if idle.
pub fn schedule(added: Vec<i64>, retry: Vec<i64>) {
    let Some(app) = APP.get() else {
        return;
    };
    let needed = {
        let state = app.state::<AppState>();
        let db = match state.db.lock() {
            Ok(db) => db,
            Err(_) => return,
        };
        Steps::read(&db).needed(added.len(), retry.len())
    };
    if !needed {
        return;
    }
    if let Ok(mut pending) = PENDING.lock() {
        pending.extend(added);
    }
    if let Ok(mut queued) = RETRY.lock() {
        queued.extend(retry);
    }
    start_worker(app.clone());
}

fn start_worker(app: AppHandle) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return; // the running worker drains the queue
    }
    tauri::async_runtime::spawn(async move {
        loop {
            let batch: Vec<i64> = take_queue(&PENDING);
            let retry: Vec<i64> = take_queue(&RETRY)
                .into_iter()
                .filter(|id| !batch.contains(id))
                .collect();
            let summary = run(app.state::<AppState>(), batch, retry).await;
            log::info!("Post-scan work finished: {summary}");
            let more = queued_work();
            if !more {
                RUNNING.store(false, Ordering::SeqCst);
                // A scan may have queued items between the check and the store.
                let raced = queued_work();
                if !raced || RUNNING.swap(true, Ordering::SeqCst) {
                    break;
                }
            }
        }
    });
}

fn take_queue(queue: &Mutex<BTreeSet<i64>>) -> Vec<i64> {
    queue
        .lock()
        .map(|mut ids| std::mem::take(&mut *ids).into_iter().collect())
        .unwrap_or_default()
}

fn queued_work() -> bool {
    [&PENDING, &RETRY]
        .iter()
        .any(|queue| queue.lock().map(|ids| !ids.is_empty()).unwrap_or(false))
}

/// Waits for another metadata task — or an Issue 1 enrichment job, whose
/// guard is not yet active during its keyless prepass — to finish, up to
/// `WAIT_FOR_OTHER_TASK`. Returns true when something is still busy after
/// the cap, so the caller can defer its lookup passes instead of stacking
/// a second loop on top of the running job.
async fn wait_for_other_tasks() -> bool {
    let started = std::time::Instant::now();
    while (task_progress::get_metadata_task_progress().active
        || crate::enrichment_jobs::active_job_running())
        && started.elapsed() < WAIT_FOR_OTHER_TASK
    {
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    task_progress::get_metadata_task_progress().active
        || crate::enrichment_jobs::active_job_running()
}

#[derive(Clone)]
struct ItemKind {
    id: i64,
    file_path: String,
    video: bool,
    standard_video: bool,
    media_type: String,
    /// True when the row is already `adult`, or when the hint classifier
    /// (title, file path, source name/path) says it is an adult item even
    /// though its stored media type is still `movie`/`video`.
    adult: bool,
}

fn load_kinds(state: &AppState, ids: &[i64]) -> Vec<ItemKind> {
    let Ok(db) = state.db.lock() else {
        return Vec::new();
    };
    ids.iter()
        .filter_map(|id| {
            db.conn
                .query_row(
                    "SELECT mi.file_path, mi.media_type, mi.title, ms.name, ms.path \
                     FROM media_items mi \
                     LEFT JOIN media_sources ms ON ms.id = mi.source_id \
                     WHERE mi.id = ?1",
                    [id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, Option<String>>(3)?,
                            row.get::<_, Option<String>>(4)?,
                        ))
                    },
                )
                .ok()
                .map(|(file_path, media_type, title, source_name, source_path)| {
                    let kind = media_type.to_ascii_lowercase();
                    // Issue 2 (f): the hint classifier runs here, before any
                    // lookup, so an adult item scanned under a generic type
                    // is still routed to the adult pass below.
                    let probe = crate::enrichment::LibraryItemRecord {
                        id: *id,
                        title,
                        file_path: file_path.clone(),
                        media_type: media_type.clone(),
                        overview: None,
                        poster_path: None,
                        year: None,
                        rating: None,
                        genre: None,
                        tmdb_id: None,
                        imdb_id: None,
                        source_name,
                        source_path,
                    };
                    let adult = kind == "adult"
                        || crate::enrichment::classify_library_item(&probe)
                            == crate::enrichment::SourceKind::AdultVideo;
                    ItemKind {
                        id: *id,
                        file_path,
                        video: kind != "music",
                        standard_video: matches!(kind.as_str(), "movie" | "episode" | "video"),
                        media_type,
                        adult,
                    }
                })
        })
        .collect()
}

async fn run(state: State<'_, AppState>, ids: Vec<i64>, retry: Vec<i64>) -> Value {
    let busy_after_wait = wait_for_other_tasks().await;
    let steps = match state.db.lock() {
        Ok(db) => Steps::read(&db),
        Err(error) => return json!({ "error": error.to_string() }),
    };
    let mut items = load_kinds(state.inner(), &ids);
    // Issue 2 (f): newly scanned items the hint classifier flags as adult
    // are reclassified in the DB before any lookup, so the adult pass below
    // (and every later consumer of the row) sees them as adult. Store-safe
    // builds skip this entirely: adult handling stays blocked there.
    // Rescanned items are not rewritten here — when one of them is enriched
    // by the adult pass, its write reclassifies it the same way.
    let mut reclassified = 0usize;
    if crate::edition::ensure_adult_allowed().is_ok() {
        for item in items.iter_mut() {
            if item.adult && !item.media_type.eq_ignore_ascii_case("adult") {
                let updated = state
                    .db
                    .lock()
                    .map(|db| {
                        db.conn
                            .execute(
                                "UPDATE media_items SET media_type = 'adult' WHERE id = ?1",
                                rusqlite::params![item.id],
                            )
                            .map(|count| count > 0)
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                if updated {
                    item.media_type = "adult".to_string();
                    reclassified += 1;
                }
            }
        }
    }
    // New titles first, then older rows whose metadata or poster never saved.
    let lookups: Vec<ItemKind> = if steps.auto_metadata {
        items
            .iter()
            .cloned()
            .chain(load_kinds(state.inner(), &retry))
            .collect()
    } else {
        Vec::new()
    };
    // Issue 2 (a): adult items take the gather's per-item path, not the
    // single-item check. Issue 2 (e): durable pending attempts from earlier
    // failed/skipped adult enrichments join the adult pass, so a rescan is
    // not the only way a failed item is retried — the in-memory queue alone
    // used to forget them when the app closed.
    let adult_allowed = crate::edition::ensure_adult_allowed().is_ok();
    let mut pending_kinds: Vec<ItemKind> = Vec::new();
    if steps.auto_metadata && adult_allowed {
        let pending_ids = match state.db.lock() {
            Ok(db) => crate::adult_attempts::pending_item_ids(&db),
            Err(_) => Vec::new(),
        };
        let known: std::collections::BTreeSet<i64> =
            lookups.iter().map(|item| item.id).collect();
        let outstanding: Vec<i64> = pending_ids
            .into_iter()
            .filter(|id| !known.contains(id))
            .collect();
        pending_kinds = load_kinds(state.inner(), &outstanding)
            .into_iter()
            .filter(|item| item.adult)
            .collect();
    }
    let standard_lookups: Vec<ItemKind> = lookups
        .iter()
        .cloned()
        .filter(|item| !(adult_allowed && item.adult))
        .collect();
    let adult_lookups: Vec<ItemKind> = lookups
        .iter()
        .cloned()
        .filter(|item| adult_allowed && item.adult)
        .chain(pending_kinds.iter().cloned())
        .collect();
    let lookup_total = standard_lookups.len() + adult_lookups.len();
    let retried = lookup_total.saturating_sub(items.len());
    // Issue 2 / Issue 1 job discipline: when a job is still running after
    // the wait cap, the lookup passes are deferred rather than stacked on
    // top of it. The scanner re-queues posterless/uncurated rows on the next
    // rescan, and pending adult attempts stay durable for a later run.
    let metadata_deferred = steps.auto_metadata && busy_after_wait;
    let videos: Vec<&ItemKind> = items.iter().filter(|item| item.video).collect();
    let standard: Vec<i64> = items
        .iter()
        .filter(|item| item.standard_video)
        .map(|item| item.id)
        .collect();

    let total = [
        steps.poster_sync as usize * 2,
        lookup_total,
        steps.subtitle_fetch as usize,
        if steps.chapter_thumbs {
            videos.len()
        } else {
            0
        },
        steps.collection_auto as usize,
    ]
    .iter()
    .sum::<usize>()
    .max(1);
    let mut guard = MetadataTaskGuard::start(
        "post_scan",
        "New titles",
        total,
        format!("Preparing {} new title(s)", items.len()),
    );
    let mut done = 0usize;
    let mut summary = json!({
        "at": chrono::Utc::now().to_rfc3339(),
        "items": items.len(),
        "retried": retried,
        "adultReclassified": reclassified,
    });

    if steps.poster_sync && !items.is_empty() {
        guard.update(done, "Importing shared artwork");
        let ids: Vec<i64> = items.iter().map(|item| item.id).collect();
        let result = state
            .db
            .lock()
            .map_err(|error| error.to_string())
            .and_then(|db| crate::poster_sync::import(&db, &state.app_data_dir, Some(&ids)));
        summary["posterImport"] = match result {
            Ok(counts) => json!({ "imported": counts.imported, "errors": counts.errors }),
            Err(error) => json!({ "error": error }),
        };
        done += 1;
    }

    if lookup_total > 0 {
        let mut updated = 0usize;
        let mut errors: Vec<String> = Vec::new();
        if metadata_deferred {
            // Deferred, not dropped silently: the summary records it, the
            // scanner re-queues these rows on the next rescan, and durable
            // adult attempts remain pending for a later run.
            summary["metadataDeferred"] = json!({
                "reason": "An enrichment job is still running; post-scan lookups were deferred instead of stacking a second loop on it",
                "items": lookup_total,
            });
            done += lookup_total;
        } else {
            for item in &standard_lookups {
                if task_progress::stop_requested() {
                    break;
                }
                guard.update(done, format!("Looking up metadata: {}", item.file_path));
                match crate::metadata_enrichment_runtime::check_media_item_metadata(
                    state.clone(),
                    item.id,
                )
                .await
                {
                    Ok(result) => {
                        if result.get("metadata_updated").and_then(Value::as_bool) == Some(true) {
                            updated += 1;
                        }
                    }
                    Err(error) => errors.push(format!("{}: {error}", item.file_path)),
                }
                done += 1;
            }
            // Issue 2 (a): adult items run through the gather's per-item
            // enrichment — provider merge, sidecar/app-cache poster,
            // chapter-frame fallback, NFO + DB writes — instead of the
            // single-item check's verbatim-title, DB-only path.
            let mut adult_updated = 0usize;
            let mut adult_posters = 0usize;
            let mut adult_chapter_posters = 0usize;
            let mut adult_errors: Vec<String> = Vec::new();
            if !adult_lookups.is_empty() {
                let deps = {
                    let db = state.db.lock().map_err(|error| error.to_string());
                    match db {
                        Ok(db) => crate::enrichment::load_adult_provider_deps(&db),
                        Err(error) => Err(error),
                    }
                };
                match deps {
                    Ok((provider_keys, extra_adult)) => {
                        match reqwest::Client::builder()
                            .timeout(std::time::Duration::from_secs(15))
                            .build()
                            .map_err(|error| error.to_string())
                        {
                            Ok(client) => {
                                for item in &adult_lookups {
                                    if task_progress::stop_requested() {
                                        break;
                                    }
                                    guard.update(
                                        done,
                                        format!("Looking up adult metadata: {}", item.file_path),
                                    );
                                    let record = match state.db.lock() {
                                        Ok(db) => {
                                            crate::enrichment::load_library_item_record(&db, item.id)
                                        }
                                        Err(error) => {
                                            adult_errors.push(format!(
                                                "{}: {}",
                                                item.file_path, error
                                            ));
                                            None
                                        }
                                    };
                                    let Some(record) = record else {
                                        done += 1;
                                        continue;
                                    };
                                    match crate::enrichment::enrich_adult_item(
                                        &state,
                                        &client,
                                        &provider_keys,
                                        &extra_adult,
                                        &record,
                                        &mut adult_errors,
                                    )
                                    .await
                                    {
                                        crate::enrichment::AdultItemOutcome::Enriched {
                                            fields_updated,
                                            poster_saved,
                                            poster_from_chapter_frame,
                                            ..
                                        } => {
                                            if fields_updated > 0 || poster_saved {
                                                updated += 1;
                                                adult_updated += 1;
                                            }
                                            if poster_saved {
                                                adult_posters += 1;
                                            }
                                            if poster_from_chapter_frame {
                                                adult_chapter_posters += 1;
                                            }
                                        }
                                        crate::enrichment::AdultItemOutcome::Failed(error) => {
                                            adult_errors.push(error);
                                        }
                                        _ => {}
                                    }
                                    done += 1;
                                }
                            }
                            Err(error) => {
                                adult_errors.push(format!("adult client: {error}"));
                                done += adult_lookups.len();
                            }
                        }
                    }
                    Err(error) => {
                        adult_errors.push(format!("adult providers: {error}"));
                        done += adult_lookups.len();
                    }
                }
            }
            errors.extend(adult_errors.iter().cloned());
            let detail = json!({
                "items": lookup_total,
                "updated": updated,
                "errors": errors.iter().take(20).collect::<Vec<_>>(),
            });
            if updated > 0 {
                if let Ok(db) = state.db.lock() {
                    crate::user_data::emit(
                        &db,
                        "metadata.updated",
                        &format!("Metadata found for {updated} new title(s)"),
                        detail.clone(),
                    );
                }
            }
            summary["metadata"] = detail;
            if !adult_lookups.is_empty() {
                summary["adultMetadata"] = json!({
                    "items": adult_lookups.len(),
                    "updated": adult_updated,
                    "posters": adult_posters,
                    "chapterFramePosters": adult_chapter_posters,
                    "errors": adult_errors.iter().take(20).collect::<Vec<_>>(),
                });
            }
        }
    }

    if steps.subtitle_fetch && !standard.is_empty() && !task_progress::stop_requested() {
        guard.update(done, "Downloading subtitles");
        summary["subtitles"] = crate::subtitles::fetch_for_items(state.inner(), &standard).await;
        done += 1;
    }

    if steps.chapter_thumbs && !videos.is_empty() && !task_progress::stop_requested() {
        let interval = state
            .db
            .lock()
            .map(|db| crate::chapters::configured_interval_secs(&db))
            .unwrap_or(300);
        let targets = videos
            .iter()
            .map(|item| (item.id, item.file_path.clone()))
            .collect();
        summary["chapterThumbs"] =
            crate::chapters::generate_for_new_items(targets, interval, &mut guard, done).await;
        done += videos.len();
    }

    if steps.poster_sync && !items.is_empty() && !task_progress::stop_requested() {
        guard.update(done, "Copying artwork to the sync folder");
        let ids: Vec<i64> = items.iter().map(|item| item.id).collect();
        summary["posterSync"] = crate::poster_sync::after_scan(state.inner(), &ids).await;
        done += 1;
    }

    if steps.collection_auto && !task_progress::stop_requested() {
        guard.update(done, "Rebuilding collections");
        summary["collections"] = match crate::collections::refresh_and_rebuild(state.inner()).await
        {
            Ok(report) => serde_json::to_value(report).unwrap_or(Value::Null),
            Err(error) => json!({ "error": error }),
        };
    }

    let stopped = task_progress::stop_requested();
    summary["stopped"] = json!(stopped);
    if let Ok(db) = state.db.lock() {
        summary["adultAttemptsPending"] = json!(crate::adult_attempts::pending_count(&db));
    }
    if let Ok(db) = state.db.lock() {
        db.set_setting_data(LAST_RUN_SETTING, &summary.to_string())
            .ok();
    }
    guard.finish(if stopped {
        "Stopped; the remaining new titles were left as scanned"
    } else {
        "New titles are ready"
    });
    summary
}

/// What the last post-scan run did, and whether one is running now.
#[tauri::command]
pub fn post_scan_status(state: State<AppState>) -> Result<Value, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let last = db
        .get_setting_data(LAST_RUN_SETTING)
        .map_err(|error| error.to_string())?
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok());
    Ok(json!({
        "running": RUNNING.load(Ordering::SeqCst),
        "queued": PENDING.lock().map(|p| p.len()).unwrap_or(0),
        "adultAttemptsPending": crate::adult_attempts::pending_count(&db),
        "lastRun": last,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    #[test]
    fn steps_follow_the_switches() {
        let db = Database::new(":memory:").unwrap();
        let defaults = Steps::read(&db);
        assert!(defaults.auto_metadata && defaults.collection_auto);
        assert!(!defaults.subtitle_fetch && !defaults.chapter_thumbs);
        assert!(!defaults.poster_sync, "poster sync needs a folder");
        assert!(
            defaults.needed(0, 0),
            "collections are rebuilt after every scan"
        );

        for key in ["auto_metadata", "collection_auto"] {
            db.set_feature_setting_data(key, false, "{}").unwrap();
        }
        let off = Steps::read(&db);
        assert!(
            !off.needed(5, 5),
            "scans only add files when every step is off"
        );

        db.set_feature_setting_data("poster_sync", true, r#"{"folder":"/tmp/x"}"#)
            .unwrap();
        let sync = Steps::read(&db);
        assert!(sync.poster_sync);
        assert!(!sync.needed(0, 0) && sync.needed(1, 0));
        assert!(
            !sync.needed(0, 3),
            "older rows only come back for the metadata lookup"
        );

        db.set_feature_setting_data("auto_metadata", true, "{}")
            .unwrap();
        assert!(
            Steps::read(&db).needed(0, 3),
            "a rescan retries rows whose metadata never saved"
        );
    }
}
