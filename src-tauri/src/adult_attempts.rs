// CinaVault Premium — Durable adult-enrichment attempt records (Issue 2).
//
// Problem fixed: post-scan kept its failed/skipped adult items only in
// in-memory queues (`PENDING` / `RETRY` in `post_scan`). If the app closed
// mid-run, or a post-scan pass was skipped, those items were forgotten:
// nothing remembered that they had been tried, what happened, or that they
// still needed another attempt. The scanner's own retry list only covers
// rows a rescan happens to find again, and only via its poster/curated
// heuristic.
//
// This module replaces that memory-only bookkeeping with one SQLite table,
// `adult_enrichment_attempts`, written by the shared per-item adult
// enrichment path (`enrichment::enrich_adult_item`) for every terminal
// outcome — including `skipped_complete`, so a previously pending item that
// later completes clears its pending flag instead of being retried forever:
//
//   * `no_match`, `failed`, `timeout`, and `enriched_poster_missing` are
//     retryable: `needs_retry = 1`, and `post_scan` merges their item ids
//     into its adult pass on the next run (see `pending_item_ids`).
//   * `enriched`, `skipped_complete`, `skipped_missing`, and
//     `skipped_non_video` are not retryable: the item either has what it
//     needs, or a rescan (not post-scan) is what can bring it back — a
//     missing file reappears as a newly added item, and a non-video row is
//     not an enrichment candidate at all.
//
// Recording is deliberately best-effort at the call sites: a failure to
// write an attempt record must never fail the enrichment itself, so callers
// ignore the returned error (and, in the gather, surface it as a
// `provider_errors` entry instead of dropping it silently).
//
// Store-safe builds never reach this module: the adult pass in `post_scan`
// and the adult gather both refuse before any adult item is attempted, so
// no attempt rows are created there.

/// Outcome recorded when an item was enriched and now has a local poster.
pub const OUTCOME_ENRICHED: &str = "enriched";

/// Outcome recorded when an item was enriched but still has no poster
/// (no provider poster, no usable chapter frame). Retryable: a later run
/// may find a provider poster, or chapter frames may exist by then.
pub const OUTCOME_ENRICHED_POSTER_MISSING: &str = "enriched_poster_missing";

/// Outcome recorded when an item already satisfies the skip-complete rule.
pub const OUTCOME_SKIPPED_COMPLETE: &str = "skipped_complete";

/// Outcome recorded when the item's file was not on disk.
pub const OUTCOME_SKIPPED_MISSING: &str = "skipped_missing";

/// Outcome recorded when the item is not a video library item.
pub const OUTCOME_SKIPPED_NON_VIDEO: &str = "skipped_non_video";

/// Outcome recorded when no provider matched the item. Retryable.
pub const OUTCOME_NO_MATCH: &str = "no_match";

/// Outcome recorded when the item's remote phase exceeded its time bound.
/// Retryable: nothing was written for the item.
pub const OUTCOME_TIMEOUT: &str = "timeout";

/// Outcome recorded when persisting the enrichment failed (DB or NFO
/// write). Retryable: the next run retries the write.
pub const OUTCOME_FAILED: &str = "failed";

/// Whether an outcome should be retried by a later post-scan adult pass.
pub fn outcome_needs_retry(outcome: &str) -> bool {
    matches!(
        outcome,
        OUTCOME_NO_MATCH | OUTCOME_FAILED | OUTCOME_TIMEOUT | OUTCOME_ENRICHED_POSTER_MISSING
    )
}

/// Creates the attempts table if it does not exist yet. Safe to call on
/// every record/pending read: it is idempotent and cheap.
pub fn ensure_table(db: &crate::db::Database) -> Result<(), String> {
    db.conn
        .execute(
            "CREATE TABLE IF NOT EXISTS adult_enrichment_attempts (
                item_id INTEGER PRIMARY KEY,
                file_path TEXT NOT NULL,
                last_attempt_at TEXT NOT NULL,
                outcome TEXT NOT NULL,
                attempts INTEGER NOT NULL DEFAULT 0,
                last_error TEXT,
                needs_retry INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Records one terminal attempt for an item, incrementing its attempt
/// counter. A later `skipped_complete` or `enriched` record for the same
/// item clears `needs_retry`; the counter itself is never reset, so the
/// history of how many times an item was tried is preserved.
pub fn record_attempt(
    db: &crate::db::Database,
    item_id: i64,
    file_path: &str,
    outcome: &str,
    last_error: Option<&str>,
) -> Result<(), String> {
    ensure_table(db)?;
    let needs_retry = i64::from(outcome_needs_retry(outcome));
    db.conn
        .execute(
            "INSERT INTO adult_enrichment_attempts
                 (item_id, file_path, last_attempt_at, outcome, attempts, last_error, needs_retry)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)
             ON CONFLICT(item_id) DO UPDATE SET
                 file_path = excluded.file_path,
                 last_attempt_at = excluded.last_attempt_at,
                 outcome = excluded.outcome,
                 attempts = adult_enrichment_attempts.attempts + 1,
                 last_error = excluded.last_error,
                 needs_retry = excluded.needs_retry",
            rusqlite::params![
                item_id,
                file_path,
                chrono::Utc::now().to_rfc3339(),
                outcome,
                last_error,
                needs_retry,
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Item ids whose last adult-enrichment attempt still needs a retry, oldest
/// attempt first. Returns an empty list (never an error) when the table
/// cannot be read, so a post-scan run is never blocked by this lookup.
pub fn pending_item_ids(db: &crate::db::Database) -> Vec<i64> {
    if ensure_table(db).is_err() {
        return Vec::new();
    }
    let mut stmt = match db.conn.prepare(
        "SELECT item_id FROM adult_enrichment_attempts
         WHERE needs_retry = 1 ORDER BY last_attempt_at ASC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map([], |row| row.get::<_, i64>(0)) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };
    rows.filter_map(|row| row.ok()).collect()
}

/// How many adult items are currently waiting for a retry. Best-effort:
/// 0 when the table cannot be read.
pub fn pending_count(db: &crate::db::Database) -> usize {
    if ensure_table(db).is_err() {
        return 0;
    }
    db.conn
        .query_row(
            "SELECT COUNT(*) FROM adult_enrichment_attempts WHERE needs_retry = 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|count| count.max(0) as usize)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn attempt_outcome(db: &Database, item_id: i64) -> (String, i64, i64) {
        db.conn
            .query_row(
                "SELECT outcome, attempts, needs_retry FROM adult_enrichment_attempts WHERE item_id = ?1",
                rusqlite::params![item_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .expect("attempt row should exist")
    }

    #[test]
    fn failed_attempts_stay_pending_until_a_complete_record_clears_them() {
        let db = Database::new(":memory:").unwrap();

        assert!(pending_item_ids(&db).is_empty());
        record_attempt(&db, 7, "/media/scene.mp4", OUTCOME_NO_MATCH, None).unwrap();
        record_attempt(&db, 9, "/media/other.mp4", OUTCOME_FAILED, Some("db locked")).unwrap();
        assert_eq!(pending_item_ids(&db), vec![7, 9]);
        assert_eq!(pending_count(&db), 2);
        assert_eq!(attempt_outcome(&db, 7), ("no_match".to_string(), 1, 1));

        // A retry that still fails keeps the item pending and bumps the count.
        record_attempt(&db, 7, "/media/scene.mp4", OUTCOME_TIMEOUT, None).unwrap();
        assert_eq!(attempt_outcome(&db, 7), ("timeout".to_string(), 2, 1));
        assert_eq!(pending_item_ids(&db), vec![7, 9]);

        // A later complete record clears the pending flag but keeps history.
        record_attempt(&db, 7, "/media/scene.mp4", OUTCOME_SKIPPED_COMPLETE, None).unwrap();
        assert_eq!(
            attempt_outcome(&db, 7),
            ("skipped_complete".to_string(), 3, 0)
        );
        assert_eq!(pending_item_ids(&db), vec![9]);
        assert_eq!(pending_count(&db), 1);
    }

    #[test]
    fn enriched_without_a_poster_stays_pending_but_enriched_with_one_does_not() {
        let db = Database::new(":memory:").unwrap();

        record_attempt(
            &db,
            3,
            "/media/posterless.mp4",
            OUTCOME_ENRICHED_POSTER_MISSING,
            None,
        )
        .unwrap();
        assert_eq!(pending_item_ids(&db), vec![3]);

        record_attempt(&db, 3, "/media/posterless.mp4", OUTCOME_ENRICHED, None).unwrap();
        assert!(pending_item_ids(&db).is_empty());
        assert_eq!(pending_count(&db), 0);
    }

    #[test]
    fn retry_classification_covers_only_the_retryable_outcomes() {
        assert!(outcome_needs_retry(OUTCOME_NO_MATCH));
        assert!(outcome_needs_retry(OUTCOME_FAILED));
        assert!(outcome_needs_retry(OUTCOME_TIMEOUT));
        assert!(outcome_needs_retry(OUTCOME_ENRICHED_POSTER_MISSING));
        assert!(!outcome_needs_retry(OUTCOME_ENRICHED));
        assert!(!outcome_needs_retry(OUTCOME_SKIPPED_COMPLETE));
        assert!(!outcome_needs_retry(OUTCOME_SKIPPED_MISSING));
        assert!(!outcome_needs_retry(OUTCOME_SKIPPED_NON_VIDEO));
    }
}
