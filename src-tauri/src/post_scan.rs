//! Work that follows a library scan, in the background so the scan returns
//! as soon as files are indexed. Each step is its own switch:
//! poster_sync (import shared artwork, later export), auto_metadata (look up
//! the new titles), subtitle_fetch (OpenSubtitles), chapter_thumbs (ffmpeg
//! frames) and collection_auto (regroup the library).
//!
//! Progress goes through task_progress (the same bar and Stop button as the
//! metadata tools); scans that finish while this runs queue their new items.

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

    /// Whether a scan that added `added` items has anything to do.
    pub fn needed(&self, added: usize) -> bool {
        self.collection_auto
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

/// Queues the items a scan added and starts the background run if idle.
pub fn schedule(added: Vec<i64>) {
    let Some(app) = APP.get() else {
        return;
    };
    let needed = {
        let state = app.state::<AppState>();
        let db = match state.db.lock() {
            Ok(db) => db,
            Err(_) => return,
        };
        Steps::read(&db).needed(added.len())
    };
    if !needed {
        return;
    }
    if let Ok(mut pending) = PENDING.lock() {
        pending.extend(added);
    }
    start_worker(app.clone());
}

fn start_worker(app: AppHandle) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return; // the running worker drains the queue
    }
    tauri::async_runtime::spawn(async move {
        loop {
            let batch: Vec<i64> = PENDING
                .lock()
                .map(|mut pending| std::mem::take(&mut *pending).into_iter().collect())
                .unwrap_or_default();
            let summary = run(app.state::<AppState>(), batch).await;
            log::info!("Post-scan work finished: {summary}");
            let more = PENDING.lock().map(|p| !p.is_empty()).unwrap_or(false);
            if !more {
                RUNNING.store(false, Ordering::SeqCst);
                // A scan may have queued items between the check and the store.
                let raced = PENDING.lock().map(|p| !p.is_empty()).unwrap_or(false);
                if !raced || RUNNING.swap(true, Ordering::SeqCst) {
                    break;
                }
            }
        }
    });
}

async fn wait_for_other_tasks() {
    let started = std::time::Instant::now();
    while task_progress::get_metadata_task_progress().active
        && started.elapsed() < WAIT_FOR_OTHER_TASK
    {
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

struct ItemKind {
    id: i64,
    file_path: String,
    video: bool,
    standard_video: bool,
}

fn load_kinds(state: &AppState, ids: &[i64]) -> Vec<ItemKind> {
    let Ok(db) = state.db.lock() else {
        return Vec::new();
    };
    ids.iter()
        .filter_map(|id| {
            db.conn
                .query_row(
                    "SELECT file_path, media_type FROM media_items WHERE id = ?1",
                    [id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .ok()
                .map(|(file_path, media_type)| {
                    let kind = media_type.to_ascii_lowercase();
                    ItemKind {
                        id: *id,
                        file_path,
                        video: kind != "music",
                        standard_video: matches!(kind.as_str(), "movie" | "episode" | "video"),
                    }
                })
        })
        .collect()
}

async fn run(state: State<'_, AppState>, ids: Vec<i64>) -> Value {
    wait_for_other_tasks().await;
    let steps = match state.db.lock() {
        Ok(db) => Steps::read(&db),
        Err(error) => return json!({ "error": error.to_string() }),
    };
    let items = load_kinds(state.inner(), &ids);
    let videos: Vec<&ItemKind> = items.iter().filter(|item| item.video).collect();
    let standard: Vec<i64> = items
        .iter()
        .filter(|item| item.standard_video)
        .map(|item| item.id)
        .collect();

    let total = [
        steps.poster_sync as usize * 2,
        if steps.auto_metadata { items.len() } else { 0 },
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
    let mut summary = json!({ "at": chrono::Utc::now().to_rfc3339(), "items": items.len() });

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

    if steps.auto_metadata && !items.is_empty() {
        let mut updated = 0usize;
        let mut errors = Vec::new();
        for item in &items {
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
        let detail = json!({
            "items": items.len(),
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
            defaults.needed(0),
            "collections are rebuilt after every scan"
        );

        for key in ["auto_metadata", "collection_auto"] {
            db.set_feature_setting_data(key, false, "{}").unwrap();
        }
        let off = Steps::read(&db);
        assert!(
            !off.needed(5),
            "scans only add files when every step is off"
        );

        db.set_feature_setting_data("poster_sync", true, r#"{"folder":"/tmp/x"}"#)
            .unwrap();
        let sync = Steps::read(&db);
        assert!(sync.poster_sync);
        assert!(!sync.needed(0) && sync.needed(1));
    }
}
