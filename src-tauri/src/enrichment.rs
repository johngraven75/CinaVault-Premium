use crate::library_artifacts::sidecar_poster_path_for_video;
use crate::{enrichment_jobs, task_progress, AppState};
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, State};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceKind {
    StandardVideo,
    AdultVideo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrichmentMode {
    MetadataOnly,
    MetadataAndRename,
}

impl EnrichmentMode {
    fn as_report_mode(self) -> &'static str {
        match self {
            EnrichmentMode::MetadataOnly => "metadata_only",
            EnrichmentMode::MetadataAndRename => "metadata_and_filename_normalization",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameTarget {
    Ready(PathBuf, String),
    Collision(PathBuf),
    Invalid(String),
    Unchanged(PathBuf),
}

#[derive(Debug, Clone)]
pub struct LibraryItemRecord {
    pub id: i64,
    pub title: String,
    pub file_path: String,
    pub media_type: String,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub year: Option<i32>,
    pub rating: Option<f64>,
    pub genre: Option<String>,
    pub tmdb_id: Option<String>,
    pub imdb_id: Option<String>,
    pub source_name: Option<String>,
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderMatch {
    pub title: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub year: Option<i32>,
    pub rating: Option<f64>,
    pub genre: Option<String>,
    pub tmdb_id: Option<String>,
    pub imdb_id: Option<String>,
}

/// Issue 1: an item counts as already complete — and is skipped before any
/// remote fetch — when it already has a non-blank overview, a *local* poster
/// path, and a year or genre. A remote (http/https) poster does NOT count:
/// those items still need the sidecar migration the loops perform. Skipping
/// is a deliberate trade-off: a "complete" item could theoretically still gain
/// a marginally better title from a provider, but re-querying every complete
/// item on every run is exactly the whole-library stall this fix removes.
/// Single-item checks remain available for individual refreshes.
pub fn item_metadata_complete(item: &LibraryItemRecord) -> bool {
    let overview_ok = item
        .overview
        .as_deref()
        .map(|value| !value.trim().is_empty() && !value.eq_ignore_ascii_case("N/A"))
        .unwrap_or(false);
    let poster_ok = item
        .poster_path
        .as_deref()
        .map(|value| {
            let value = value.trim();
            !value.is_empty()
                && !value.starts_with("http://")
                && !value.starts_with("https://")
        })
        .unwrap_or(false);
    let year_or_genre = item.year.is_some()
        || item
            .genre
            .as_deref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
    overview_ok && poster_ok && year_or_genre
}

/// Issue 1: ffprobe is a blocking subprocess. Running it on an async worker
/// thread stalls that worker for the whole subprocess lifetime, so the
/// async call sites use this `spawn_blocking` wrapper instead. If the job is
/// aborted while ffprobe runs, the blocking thread still finishes on its own
/// (ffprobe itself is short-lived); the awaited result is simply dropped.
async fn extract_embedded_title_async(file_path: &str) -> Option<String> {
    let owned = file_path.to_string();
    tokio::task::spawn_blocking(move || extract_embedded_title(&owned))
        .await
        .ok()
        .flatten()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameDecision {
    pub allow_rename: bool,
    pub normalized_title: Option<String>,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct EnrichmentItemSummary {
    id: i64,
    old_title: String,
    new_title: String,
    file_path: String,
    action: String,
}

#[derive(Debug, Serialize)]
pub struct LibraryEnrichmentReport {
    #[serde(rename = "type")]
    pub result_type: &'static str,
    pub status: &'static str,
    pub mode: &'static str,
    pub items_scanned: usize,
    pub metadata_items_enriched: usize,
    pub metadata_fields_updated: usize,
    pub metadata_updated: usize,
    pub titles_improved: usize,
    pub items_reclassified_as_adult: usize,
    pub files_renamed: usize,
    pub rename_collisions_skipped: usize,
    pub rename_failures: usize,
    pub low_confidence_metadata_only: usize,
    pub skipped_missing_files: usize,
    pub skipped_non_video_items: usize,
    pub skipped_complete: usize,
    pub posters_downloaded: usize,
    pub sidecars_written: usize,
    pub provider_errors: Vec<String>,
    pub samples: Vec<EnrichmentItemSummary>,
}

pub(crate) fn has_adult_hint(text: &str) -> bool {
    let lower = text.replace(['\\', '/', '_', '-', '.'], " ").to_lowercase();

    [
        "adult",
        "porn",
        "xxx",
        "nsfw",
        "personal x",
        "x library",
        "vids x",
        "videos x",
        "18 plus",
        "18+",
        "erotic",
        "explicit",
        "onlyfans",
        "fansly",
        "brazzers",
        "pornhub",
        "xvideos",
        "xnxx",
    ]
    .iter()
    .any(|hint| lower.contains(hint))
}

pub fn classify_library_item(item: &LibraryItemRecord) -> SourceKind {
    if item.media_type.eq_ignore_ascii_case("adult")
        || has_adult_hint(&item.title)
        || has_adult_hint(&item.file_path)
        || item
            .source_name
            .as_deref()
            .map(has_adult_hint)
            .unwrap_or(false)
        || item
            .source_path
            .as_deref()
            .map(has_adult_hint)
            .unwrap_or(false)
    {
        SourceKind::AdultVideo
    } else {
        SourceKind::StandardVideo
    }
}

pub fn normalize_filename_title(filename: &str) -> String {
    let stem = Path::new(filename)
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| filename.to_string());
    let stem = stem.trim();
    if stem.is_empty() || looks_like_timestamp_only(stem) {
        return String::new();
    }

    let mut text = stem.replace(['.', '_'], " ");
    text = text.replace('-', " ");
    text = Regex::new(
        r"(?i)\[[^\]]*\]|\([^\)]*(1080p|720p|2160p|x264|x265|hevc|aac|web[- ]?dl|bluray)[^\)]*\)",
    )
    .expect("release group regex should compile")
    .replace_all(&text, " ")
    .to_string();

    let year = Regex::new(r"\b(19\d{2}|20\d{2})\b")
        .expect("year regex should compile")
        .captures(&text)
        .and_then(|captures| captures.get(1).map(|year| year.as_str().to_string()));

    text = Regex::new(
        r"(?i)\b(480p|576p|720p|1080p|1440p|2160p|4k|8k|x264|x265|h264|h265|hevc|avc|aac|ac3|dts|ddp5?|atmos|web\s?dl|webrip|bluray|brrip|dvdrip|hdrip|proper|repack|extended|remux|yts|rarbg|eztv|group)\b",
    )
    .expect("release token regex should compile")
    .replace_all(&text, " ")
    .to_string();

    if let Some(ref year) = year {
        text = Regex::new(&format!(r"\b{}\b", regex::escape(year)))
            .expect("escaped year regex should compile")
            .replace_all(&text, " ")
            .to_string();
    }

    text = Regex::new(r"\s+")
        .expect("space regex should compile")
        .replace_all(text.trim(), " ")
        .trim_matches(|c: char| c == '-' || c == '.' || c.is_whitespace())
        .to_string();

    if text.is_empty() || looks_like_timestamp_only(&text) {
        return String::new();
    }

    let titled = title_case(&text);
    match year {
        Some(year) if !titled.contains(&year) => format!("{titled} ({year})"),
        _ => titled,
    }
}

pub fn build_query_candidates(
    item: &LibraryItemRecord,
    embedded_title: Option<String>,
) -> Vec<String> {
    let mut queries = Vec::new();
    push_unique_query(&mut queries, embedded_title);
    push_unique_query(&mut queries, Some(item.title.trim().to_string()));

    let normalized = normalize_filename_title(&item.file_path);
    push_unique_query(&mut queries, Some(normalized));

    if let Some(stem) = Path::new(&item.file_path).file_stem() {
        let reduced = stem
            .to_string_lossy()
            .replace(['.', '_', '-'], " ")
            .split_whitespace()
            .filter(|part| !looks_like_timestamp_only(part))
            .collect::<Vec<_>>()
            .join(" ");
        push_unique_query(&mut queries, Some(reduced));
    }

    queries
}

pub fn rename_confidence(
    item: &LibraryItemRecord,
    embedded_title: Option<&str>,
    provider: &ProviderMatch,
    mode: EnrichmentMode,
) -> RenameDecision {
    if mode != EnrichmentMode::MetadataAndRename {
        return RenameDecision {
            allow_rename: false,
            normalized_title: provider.title.as_deref().and_then(clean_title_for_display),
            reason: "metadata-only mode".to_string(),
        };
    }

    let provider_title = provider.title.as_deref().and_then(clean_title_for_display);
    let Some(provider_title) = provider_title else {
        return RenameDecision {
            allow_rename: false,
            normalized_title: None,
            reason: "provider did not return a usable title".to_string(),
        };
    };

    let embedded_support = embedded_title
        .and_then(clean_title_for_display)
        .map(|title| strong_title_match(&title, &provider_title))
        .unwrap_or(false);
    let filename_title = normalize_filename_title(&item.file_path);
    let filename_support =
        !filename_title.is_empty() && strong_title_match(&filename_title, &provider_title);
    let current_support =
        !is_low_quality_title(&item.title) && strong_title_match(&item.title, &provider_title);
    let stable_id = provider
        .tmdb_id
        .as_deref()
        .map(|id| !id.trim().is_empty())
        .unwrap_or(false)
        || provider
            .imdb_id
            .as_deref()
            .map(|id| !id.trim().is_empty())
            .unwrap_or(false);

    let local_title_support = embedded_support || filename_support || current_support;
    let external_or_embedded_support = embedded_support || stable_id;
    let allow_rename = local_title_support && external_or_embedded_support;
    let reason = match (
        allow_rename,
        local_title_support,
        external_or_embedded_support,
    ) {
        (true, _, _) => "balanced confidence satisfied".to_string(),
        (false, false, _) => "provider title lacks local title support".to_string(),
        (false, true, false) => {
            "provider title lacks stable id or embedded title support".to_string()
        }
        (false, true, true) => "balanced confidence was not satisfied".to_string(),
    };

    RenameDecision {
        allow_rename,
        normalized_title: Some(provider_title),
        reason,
    }
}

pub fn safe_rename_target(source: &Path, normalized_title: &str) -> RenameTarget {
    let Some(parent) = source.parent() else {
        return RenameTarget::Invalid("source file has no parent".to_string());
    };
    let Some(extension) = source.extension().and_then(|ext| ext.to_str()) else {
        return RenameTarget::Invalid("source file has no extension".to_string());
    };

    let cleaned = sanitize_windows_filename(normalized_title);
    if cleaned.is_empty() {
        return RenameTarget::Invalid("normalized title is empty".to_string());
    }

    let candidate = parent.join(format!("{cleaned}.{extension}"));
    if candidate == source {
        return RenameTarget::Unchanged(candidate);
    }
    if candidate.exists() {
        return RenameTarget::Collision(candidate);
    }

    RenameTarget::Ready(candidate, cleaned)
}

#[tauri::command]
pub async fn run_library_enrichment(
    state: State<'_, AppState>,
    rename_files: bool,
) -> Result<LibraryEnrichmentReport, String> {
    let mode = if rename_files {
        EnrichmentMode::MetadataAndRename
    } else {
        EnrichmentMode::MetadataOnly
    };

    let (items, provider_keys) = {
        let db = state.db.lock().map_err(|err| err.to_string())?;
        let mut stmt = db
            .conn
            .prepare(
                "SELECT mi.id,
                        mi.title,
                        mi.file_path,
                        mi.media_type,
                        mi.overview,
                        mi.poster_path,
                        mi.year,
                        mi.rating,
                        mi.genre,
                        mi.tmdb_id,
                        mi.imdb_id,
                        ms.name,
                        ms.path
                 FROM media_items mi
                 LEFT JOIN media_sources ms ON ms.id = mi.source_id
                 WHERE mi.media_type IN ('adult', 'movie', 'episode', 'video')
                 ORDER BY date_added DESC",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(LibraryItemRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    file_path: row.get(2)?,
                    media_type: row.get(3)?,
                    overview: row.get(4)?,
                    poster_path: row.get(5)?,
                    year: row.get(6)?,
                    rating: row.get(7)?,
                    genre: row.get(8)?,
                    tmdb_id: row.get(9)?,
                    imdb_id: row.get(10)?,
                    source_name: row.get(11)?,
                    source_path: row.get(12)?,
                })
            })
            .map_err(|err| err.to_string())?;
        let items = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())?;

        let provider_keys = load_provider_keys(&db)?;
        (items, provider_keys)
    };

    let mut report = LibraryEnrichmentReport {
        result_type: "library_enrichment",
        status: "success",
        mode: mode.as_report_mode(),
        items_scanned: items.len(),
        metadata_items_enriched: 0,
        metadata_fields_updated: 0,
        metadata_updated: 0,
        titles_improved: 0,
        items_reclassified_as_adult: 0,
        files_renamed: 0,
        rename_collisions_skipped: 0,
        rename_failures: 0,
        low_confidence_metadata_only: 0,
        skipped_missing_files: 0,
        skipped_non_video_items: 0,
        skipped_complete: 0,
        posters_downloaded: 0,
        sidecars_written: 0,
        provider_errors: Vec::new(),
        samples: Vec::new(),
    };
    let mut progress = task_progress::MetadataTaskGuard::start(
        "library_enrichment",
        if rename_files {
            "Normalize Filenames"
        } else {
            "Enrich Metadata"
        },
        items.len(),
        "Preparing library metadata enrichment",
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|err| err.to_string())?;

    let total_items = items.len();
    for (index, item) in items.into_iter().enumerate() {
        if task_progress::stop_requested() {
            report
                .provider_errors
                .push("Stopped by user before processing the next item".to_string());
            break;
        }
        progress.update(
            index + 1,
            format!("Enriching metadata for {} of {}", index + 1, total_items),
        );
        if !is_video_library_item(&item) {
            report.skipped_non_video_items += 1;
            continue;
        }

        let source = Path::new(&item.file_path);
        if !source.exists() {
            report.skipped_missing_files += 1;
            continue;
        }

        // Rename mode still needs a provider match to judge rename
        // confidence, so skip-complete applies to metadata-only runs only.
        if mode == EnrichmentMode::MetadataOnly && item_metadata_complete(&item) {
            report.skipped_complete += 1;
            continue;
        }

        let embedded_title = extract_embedded_title_async(&item.file_path).await;
        let source_kind = classify_library_item(&item);
        let mut queries = build_query_candidates(&item, embedded_title.clone());
        queries.truncate(enrichment_jobs::MAX_QUERY_CANDIDATES_PER_ITEM);
        let remote_provider = match tokio::time::timeout(
            std::time::Duration::from_secs(enrichment_jobs::ITEM_TIMEOUT_SECS),
            resolve_provider_match(
                &client,
                &provider_keys,
                source_kind.clone(),
                &queries,
                &item.file_path,
                &mut report.provider_errors,
            ),
        )
        .await
        {
            Ok(provider) => provider,
            Err(_) => {
                report.provider_errors.push(format!(
                    "item_timeout/{}: provider phase exceeded {}s; item will be retried on the next run",
                    item.file_path,
                    enrichment_jobs::ITEM_TIMEOUT_SECS
                ));
                continue;
            }
        };
        let local_title_provider = (source_kind != SourceKind::AdultVideo)
            .then(|| {
                local_embedded_title_match(embedded_title.as_deref())
                    .or_else(|| local_display_title_match(&item))
            })
            .flatten();
        let local_artwork_provider = (source_kind != SourceKind::AdultVideo)
            .then(|| local_sidecar_artwork_match(&item))
            .flatten();
        let provider = [
            remote_provider,
            local_title_provider,
            local_artwork_provider,
        ]
        .into_iter()
        .flatten()
        .reduce(merge_provider_matches);

        let Some(provider) = provider else {
            report.low_confidence_metadata_only += 1;
            continue;
        };

        let mut update = build_metadata_update(&item, &provider, &source_kind);

        // Acquire new provider posters and migrate previously stored remote URLs to local sidecars.
        let poster_candidate = update.poster_path.clone().or_else(|| {
            item.poster_path
                .clone()
                .filter(|path| path.starts_with("http://") || path.starts_with("https://"))
        });
        if let Some(remote_url) = poster_candidate {
            if remote_url.starts_with("http://") || remote_url.starts_with("https://") {
                match save_remote_poster(
                    &client,
                    &remote_url,
                    &item.file_path,
                    &state.app_data_dir,
                    item.id,
                )
                .await
                {
                    Ok(local_path) => {
                        if update.poster_path.is_none() {
                            update.changed_fields += 1;
                        }
                        update.poster_path = Some(local_path);
                        report.posters_downloaded += 1;
                    }
                    Err(err) => {
                        report
                            .provider_errors
                            .push(format!("poster_download/{}: {}", item.file_path, err));
                    }
                }
            }
        }

        // Write NFO sidecar with all metadata fields
        if update.changed_fields > 0 || update.poster_path.is_some() {
            let nfo_title = update.title.as_deref().unwrap_or(&item.title);
            let nfo_year = update.year.or(item.year);
            let nfo_overview = update.overview.as_deref().or(item.overview.as_deref());
            let nfo_rating = update.rating.or(item.rating);
            let nfo_genre = update.genre.as_deref().or(item.genre.as_deref());
            let nfo_tmdb = update.tmdb_id.as_deref().or(item.tmdb_id.as_deref());
            let nfo_imdb = update.imdb_id.as_deref().or(item.imdb_id.as_deref());
            let nfo_poster = update.poster_path.as_deref();
            match write_nfo_sidecar(
                &item.file_path,
                nfo_title,
                nfo_year,
                nfo_overview,
                nfo_rating,
                nfo_genre,
                nfo_tmdb,
                nfo_imdb,
                nfo_poster,
            ) {
                Ok(()) => report.sidecars_written += 1,
                Err(err) => report
                    .provider_errors
                    .push(format!("nfo_write/{}: {}", item.file_path, err)),
            }
        }

        if update.changed_fields > 0 {
            let db = state.db.lock().map_err(|err| err.to_string())?;
            db.update_media_metadata_data(
                &item.file_path,
                update.title.as_deref(),
                update.overview.as_deref(),
                update.poster_path.as_deref(),
                update.year,
                update.rating,
                update.genre.as_deref(),
                update.tmdb_id.as_deref(),
                update.imdb_id.as_deref(),
                update.media_type.as_deref(),
            )
            .map_err(|err| err.to_string())?;
            report.metadata_items_enriched += 1;
            report.metadata_fields_updated += update.changed_fields;
            report.metadata_updated += 1;
            if update.title.is_some() {
                report.titles_improved += 1;
            }
            if update.media_type.as_deref() == Some("adult") {
                report.items_reclassified_as_adult += 1;
            }
            push_sample(
                &mut report.samples,
                item.id,
                &item.title,
                update.title.as_deref().unwrap_or(&item.title),
                &item.file_path,
                "metadata_updated",
            );
        }

        if mode != EnrichmentMode::MetadataAndRename {
            continue;
        }

        let decision = rename_confidence(&item, embedded_title.as_deref(), &provider, mode);
        if !decision.allow_rename {
            report.low_confidence_metadata_only += 1;
            continue;
        }

        let Some(normalized_title) = decision.normalized_title else {
            report.low_confidence_metadata_only += 1;
            continue;
        };

        match safe_rename_target(source, &normalized_title) {
            RenameTarget::Ready(target, cleaned_title) => match std::fs::rename(source, &target) {
                Ok(()) => {
                    let new_path = target.to_string_lossy().to_string();
                    let db = state.db.lock().map_err(|err| err.to_string())?;
                    db.update_media_file_path_data(&item.file_path, &new_path, &cleaned_title)
                        .map_err(|err| err.to_string())?;
                    report.files_renamed += 1;
                    push_sample(
                        &mut report.samples,
                        item.id,
                        &item.title,
                        &cleaned_title,
                        &item.file_path,
                        "file_renamed",
                    );
                }
                Err(_) => report.rename_failures += 1,
            },
            RenameTarget::Collision(_) => report.rename_collisions_skipped += 1,
            RenameTarget::Invalid(_) | RenameTarget::Unchanged(_) => {
                report.low_confidence_metadata_only += 1;
            }
        }
    }

    progress.finish(format!(
        "Metadata enrichment complete: {} metadata updates, {} files renamed",
        report.metadata_updated, report.files_renamed
    ));

    Ok(report)
}

#[derive(Debug, Default)]
struct MetadataUpdate {
    title: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    year: Option<i32>,
    rating: Option<f64>,
    genre: Option<String>,
    tmdb_id: Option<String>,
    imdb_id: Option<String>,
    media_type: Option<String>,
    changed_fields: usize,
}

fn load_provider_keys(db: &crate::db::Database) -> Result<HashMap<String, String>, String> {
    let mut stmt = db
        .conn
        .prepare("SELECT provider, api_key FROM api_keys")
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|err| err.to_string())?;

    let mut keys = HashMap::new();
    for row in rows {
        let (provider, key) = row.map_err(|err| err.to_string())?;
        let Some(key) = crate::metadata_ext::resolve_stored_provider_key(&provider, &key)? else {
            continue;
        };
        keys.insert(provider.trim().to_lowercase(), key.clone());
        keys.insert(normalize_provider_key(&provider), key);
    }
    crate::library_metadata_policy::retain_allowed_keys(&mut keys);
    Ok(keys)
}

fn normalize_provider_key(provider: &str) -> String {
    match provider.trim().to_lowercase().as_str() {
        "themoviedb" | "themoviedb_images" | "tmdb_images" | "tmdb" => "tmdb".to_string(),
        "theporndb" | "tpdb" => "tpdb".to_string(),
        "open_movie_db" | "openmoviedb" | "omdb" => "omdb".to_string(),
        other => other.to_string(),
    }
}

async fn resolve_provider_match(
    client: &reqwest::Client,
    provider_keys: &HashMap<String, String>,
    source_kind: SourceKind,
    queries: &[String],
    _file_path: &str,
    provider_errors: &mut Vec<String>,
) -> Option<ProviderMatch> {
    // Issue 2: best adult match seen so far across queries (see below).
    let mut best_adult_match: Option<ProviderMatch> = None;
    for query in queries {
        // Issue 1: cancellation used to be checked only between items, so a
        // stop request waited out every provider call for the current item.
        // Checking here (and before each provider below) bounds the wait to
        // one provider call, itself bounded by the per-provider timeout.
        if task_progress::stop_requested() {
            return None;
        }
        let mut matches = Vec::new();
        match source_kind {
            SourceKind::AdultVideo => {
                if let Some(key) = provider_keys.get("tpdb") {
                    if task_progress::stop_requested() {
                        return None;
                    }
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(enrichment_jobs::PROVIDER_TIMEOUT_SECS),
                        fetch_tpdb_metadata(client, key, query),
                    )
                    .await
                    {
                        Ok(Ok(Some(result))) => matches.push(result),
                        Ok(Ok(None)) => {}
                        Ok(Err(err)) => provider_errors.push(format!("tpdb/{query}: {err}")),
                        Err(_) => provider_errors.push(format!(
                            "tpdb/{query}: provider timed out after {}s",
                            enrichment_jobs::PROVIDER_TIMEOUT_SECS
                        )),
                    }
                }
                if let Some(key) = provider_keys.get("stashdb") {
                    if task_progress::stop_requested() {
                        return None;
                    }
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(enrichment_jobs::PROVIDER_TIMEOUT_SECS),
                        fetch_stashdb_metadata(client, key, query),
                    )
                    .await
                    {
                        Ok(Ok(Some(result))) => matches.push(result),
                        Ok(Ok(None)) => {}
                        Ok(Err(err)) => provider_errors.push(format!("stashdb/{query}: {err}")),
                        Err(_) => provider_errors.push(format!(
                            "stashdb/{query}: provider timed out after {}s",
                            enrichment_jobs::PROVIDER_TIMEOUT_SECS
                        )),
                    }
                }
            }
            SourceKind::StandardVideo
                if crate::library_metadata_policy::standard_providers_enabled() =>
            {
                fetch_standard_metadata(
                    client,
                    provider_keys,
                    query,
                    provider_errors,
                    &mut matches,
                )
                .await;
            }
            SourceKind::StandardVideo => {}
        }

        let merged = matches.into_iter().reduce(merge_provider_matches);
        if let Some(merged) = merged.filter(|m| m.title.is_some()) {
            // Issue 2: for adult items, a title-only match must not stop the
            // search when a later query (or, in the gather, a later provider)
            // could still supply a poster. Previously the first title match
            // returned immediately, so a TPDB title-only match suppressed a
            // StashDB/Nuxt poster. Keep the best title match and try the
            // remaining queries; return early only once title AND poster are
            // both present. Standard items keep the old first-title behaviour.
            if source_kind == SourceKind::AdultVideo
                && merged
                    .poster_path
                    .as_deref()
                    .map(|value| value.trim().is_empty())
                    .unwrap_or(true)
            {
                best_adult_match = Some(match best_adult_match {
                    Some(best) => merge_provider_matches(best, merged),
                    None => merged,
                });
                continue;
            }
            return Some(match best_adult_match {
                Some(best) => merge_provider_matches(merged, best),
                None => merged,
            });
        }
    }

    best_adult_match
}

/// ThePornDB is an authenticated adult-only scene source.  Search results are
/// deliberately scanned for a strict title match rather than accepting the
/// first scene returned by the provider.
async fn fetch_tpdb_metadata(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
) -> Result<Option<ProviderMatch>, String> {
    let encoded = percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC);
    let search = client
        .get(format!("https://api.theporndb.net/scenes?parse={encoded}&hash=&year="))
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json::<serde_json::Value>()
        .await
        .map_err(|error| error.to_string())?;
    let Some(id) = search
        .get("data")
        .and_then(|value| value.as_array())
        .and_then(|scenes| scenes.iter().find(|scene| {
            scene
                .get("title")
                .and_then(|value| value.as_str())
                .is_some_and(|title| strong_title_match(query, title))
        }))
        .and_then(|scene| scene.get("uuid").or_else(|| scene.get("UUID")))
        .and_then(|value| value.as_str())
    else {
        return Ok(None);
    };
    let detail = client
        .get(format!("https://api.theporndb.net/scenes/{id}"))
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json::<serde_json::Value>()
        .await
        .map_err(|error| error.to_string())?;
    let detail = detail.get("data").unwrap_or(&detail);
    let title = detail.get("title").and_then(|value| value.as_str());
    if !title.is_some_and(|value| strong_title_match(query, value)) {
        return Ok(None);
    }
    let genre = detail
        .get("tags")
        .and_then(|value| value.as_array())
        .map(|tags| tags.iter().filter_map(|tag| tag.get("name").and_then(|value| value.as_str())).take(10).collect::<Vec<_>>().join(", "))
        .filter(|value| !value.is_empty());
    Ok(Some(ProviderMatch {
        title: title.and_then(|value| non_empty_string(Some(value))),
        overview: detail.get("description").or_else(|| detail.get("details")).and_then(|value| value.as_str()).and_then(|value| non_empty_string(Some(value))),
        poster_path: detail.get("posters").and_then(|value| value.get("large")).or_else(|| detail.get("poster")).and_then(|value| value.as_str()).and_then(|value| non_empty_string(Some(value))),
        year: parse_year_prefix(detail.get("date").and_then(|value| value.as_str())),
        rating: None,
        genre,
        tmdb_id: None,
        imdb_id: detail.get("uuid").and_then(|value| value.as_str()).and_then(|value| non_empty_string(Some(value))),
    }))
}

async fn fetch_standard_metadata(
    client: &reqwest::Client,
    provider_keys: &HashMap<String, String>,
    query: &str,
    provider_errors: &mut Vec<String>,
    matches: &mut Vec<ProviderMatch>,
) {
    if let Some(key) = provider_keys.get("tmdb") {
        if task_progress::stop_requested() {
            return;
        }
        match tokio::time::timeout(
            std::time::Duration::from_secs(enrichment_jobs::PROVIDER_TIMEOUT_SECS),
            fetch_tmdb_metadata(client, key, query),
        )
        .await
        {
            Ok(Ok(Some(result))) => matches.push(result),
            Ok(Ok(None)) => {}
            Ok(Err(err)) => provider_errors.push(format!("tmdb/{query}: {err}")),
            Err(_) => provider_errors.push(format!(
                "tmdb/{query}: provider timed out after {}s",
                enrichment_jobs::PROVIDER_TIMEOUT_SECS
            )),
        }
    }
    if let Some(key) = provider_keys.get("omdb") {
        if task_progress::stop_requested() {
            return;
        }
        match tokio::time::timeout(
            std::time::Duration::from_secs(enrichment_jobs::PROVIDER_TIMEOUT_SECS),
            fetch_omdb_metadata(client, key, query),
        )
        .await
        {
            Ok(Ok(Some(result))) => matches.push(result),
            Ok(Ok(None)) => {}
            Ok(Err(err)) => provider_errors.push(format!("omdb/{query}: {err}")),
            Err(_) => provider_errors.push(format!(
                "omdb/{query}: provider timed out after {}s",
                enrichment_jobs::PROVIDER_TIMEOUT_SECS
            )),
        }
    }
}

async fn fetch_tmdb_metadata(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
) -> Result<Option<ProviderMatch>, String> {
    let encoded = percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC);
    let url = format!(
        "https://api.themoviedb.org/3/search/multi?api_key={api_key}&query={encoded}&include_adult=true&page=1"
    );
    let data = client
        .get(url)
        .send()
        .await
        .map_err(|err| err.to_string())?
        .json::<serde_json::Value>()
        .await
        .map_err(|err| err.to_string())?;
    let Some(first) = data
        .get("results")
        .and_then(|value| value.as_array())
        .and_then(|items| items.iter().find(|scene| scene.get("title").and_then(|value| value.as_str()).is_some_and(|title| strong_title_match(query, title))) )
    else {
        return Ok(None);
    };

    Ok(Some(ProviderMatch {
        title: non_empty_string(first.get("title").and_then(|value| value.as_str()))
            .or_else(|| non_empty_string(first.get("name").and_then(|value| value.as_str()))),
        overview: non_empty_string(first.get("overview").and_then(|value| value.as_str())),
        poster_path: first
            .get("poster_path")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(|poster| format!("https://image.tmdb.org/t/p/w500{poster}")),
        year: parse_year_prefix(first.get("release_date").and_then(|value| value.as_str()))
            .or_else(|| {
                parse_year_prefix(first.get("first_air_date").and_then(|value| value.as_str()))
            }),
        rating: first
            .get("vote_average")
            .and_then(|value| value.as_f64())
            .filter(|value| *value > 0.0),
        genre: None,
        tmdb_id: first
            .get("id")
            .and_then(|value| value.as_i64())
            .map(|id| id.to_string()),
        imdb_id: None,
    }))
}

async fn fetch_omdb_metadata(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
) -> Result<Option<ProviderMatch>, String> {
    let encoded = percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC);
    let url = format!("https://www.omdbapi.com/?apikey={api_key}&t={encoded}&plot=full");
    let data = client
        .get(url)
        .send()
        .await
        .map_err(|err| err.to_string())?
        .json::<serde_json::Value>()
        .await
        .map_err(|err| err.to_string())?;
    if data.get("Response").and_then(|value| value.as_str()) != Some("True") {
        return Ok(None);
    }

    Ok(Some(ProviderMatch {
        title: non_empty_string(data.get("Title").and_then(|value| value.as_str())),
        overview: non_empty_string(data.get("Plot").and_then(|value| value.as_str())),
        poster_path: non_empty_string(data.get("Poster").and_then(|value| value.as_str())),
        year: parse_year_prefix(data.get("Year").and_then(|value| value.as_str())),
        rating: data
            .get("imdbRating")
            .and_then(|value| value.as_str())
            .and_then(|rating| rating.parse::<f64>().ok())
            .filter(|value| *value > 0.0),
        genre: non_empty_string(data.get("Genre").and_then(|value| value.as_str())),
        tmdb_id: None,
        imdb_id: non_empty_string(data.get("imdbID").and_then(|value| value.as_str())),
    }))
}

async fn fetch_stashdb_metadata(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
) -> Result<Option<ProviderMatch>, String> {
    let body = serde_json::json!({
        "query": "query($title:String!){ queryScenes(input:{title:$title, per_page:1, page:1, direction:DESC, sort:DATE}) { scenes { title details release_date images { url width height } tags { name } urls { url } } } }",
        "variables": { "title": query }
    });

    let data = client
        .post("https://stashdb.org/graphql")
        .header("Content-Type", "application/json")
        .header("ApiKey", api_key)
        .json(&body)
        .send()
        .await
        .map_err(|err| err.to_string())?
        .json::<serde_json::Value>()
        .await
        .map_err(|err| err.to_string())?;
    if data.get("errors").is_some() {
        return Ok(None);
    }

    let Some(first) = data
        .get("data")
        .and_then(|value| value.get("queryScenes"))
        .and_then(|value| value.get("scenes"))
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
    else {
        return Ok(None);
    };

    let genre = first
        .get("tags")
        .and_then(|value| value.as_array())
        .map(|tags| {
            tags.iter()
                .filter_map(|tag| tag.get("name").and_then(|value| value.as_str()))
                .filter(|name| !name.trim().is_empty())
                .take(5)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|value| !value.trim().is_empty());

    Ok(Some(ProviderMatch {
        title: non_empty_string(first.get("title").and_then(|value| value.as_str())),
        overview: non_empty_string(first.get("details").and_then(|value| value.as_str())),
        poster_path: first
            .get("images")
            .and_then(|value| value.as_array())
            .and_then(|images| images.first())
            .and_then(|image| image.get("url"))
            .and_then(|value| value.as_str())
            .and_then(|value| non_empty_string(Some(value))),
        year: parse_year_prefix(first.get("release_date").and_then(|value| value.as_str())),
        rating: None,
        genre,
        tmdb_id: None,
        imdb_id: None,
    }))
}

fn merge_provider_matches(mut primary: ProviderMatch, secondary: ProviderMatch) -> ProviderMatch {
    if primary
        .title
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        primary.title = secondary.title;
    }
    if primary
        .overview
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        primary.overview = secondary.overview;
    }
    if primary
        .poster_path
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        primary.poster_path = secondary.poster_path;
    }
    if primary.year.is_none() {
        primary.year = secondary.year;
    }
    if primary.rating.is_none() {
        primary.rating = secondary.rating;
    }
    if primary
        .genre
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        primary.genre = secondary.genre;
    }
    if primary
        .tmdb_id
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        primary.tmdb_id = secondary.tmdb_id;
    }
    if primary
        .imdb_id
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        primary.imdb_id = secondary.imdb_id;
    }
    primary
}

fn local_display_title_match(item: &LibraryItemRecord) -> Option<ProviderMatch> {
    let title = normalize_filename_title(&item.file_path);
    if title.is_empty() || !should_update_title(&item.title, &title, &item.file_path) {
        return None;
    }

    Some(ProviderMatch {
        title: Some(title),
        ..ProviderMatch::default()
    })
}

fn local_embedded_title_match(embedded_title: Option<&str>) -> Option<ProviderMatch> {
    let title = embedded_title.and_then(clean_title_for_display)?;

    Some(ProviderMatch {
        title: Some(title),
        ..ProviderMatch::default()
    })
}

fn local_sidecar_artwork_match(item: &LibraryItemRecord) -> Option<ProviderMatch> {
    let poster_path = sidecar_poster_path_for_video(Path::new(&item.file_path))?;

    Some(ProviderMatch {
        poster_path: Some(poster_path.to_string_lossy().to_string()),
        ..ProviderMatch::default()
    })
}

fn build_metadata_update(
    item: &LibraryItemRecord,
    provider: &ProviderMatch,
    source_kind: &SourceKind,
) -> MetadataUpdate {
    let mut update = MetadataUpdate::default();

    if let Some(title) = provider.title.as_deref().and_then(clean_title_for_display) {
        if should_update_title(&item.title, &title, &item.file_path) {
            update.title = Some(title);
            update.changed_fields += 1;
        }
    }
    if should_update_optional_text(item.overview.as_deref(), provider.overview.as_deref()) {
        update.overview = provider
            .overview
            .as_deref()
            .and_then(|value| non_empty_string(Some(value)));
        update.changed_fields += usize::from(update.overview.is_some());
    }
    if should_update_optional_text(item.poster_path.as_deref(), provider.poster_path.as_deref()) {
        update.poster_path = provider
            .poster_path
            .as_deref()
            .and_then(|value| non_empty_string(Some(value)));
        update.changed_fields += usize::from(update.poster_path.is_some());
    }
    if item.year.is_none() && provider.year.is_some() {
        update.year = provider.year;
        update.changed_fields += 1;
    }
    if item.rating.is_none() && provider.rating.is_some() {
        update.rating = provider.rating;
        update.changed_fields += 1;
    }
    if should_update_optional_text(item.genre.as_deref(), provider.genre.as_deref()) {
        update.genre = provider
            .genre
            .as_deref()
            .and_then(|value| non_empty_string(Some(value)));
        update.changed_fields += usize::from(update.genre.is_some());
    }
    if should_update_optional_text(item.tmdb_id.as_deref(), provider.tmdb_id.as_deref()) {
        update.tmdb_id = provider
            .tmdb_id
            .as_deref()
            .and_then(|value| non_empty_string(Some(value)));
        update.changed_fields += usize::from(update.tmdb_id.is_some());
    }
    if should_update_optional_text(item.imdb_id.as_deref(), provider.imdb_id.as_deref()) {
        update.imdb_id = provider
            .imdb_id
            .as_deref()
            .and_then(|value| non_empty_string(Some(value)));
        update.changed_fields += usize::from(update.imdb_id.is_some());
    }
    if *source_kind == SourceKind::AdultVideo && !item.media_type.eq_ignore_ascii_case("adult") {
        update.media_type = Some("adult".to_string());
        update.changed_fields += 1;
    }

    update
}

const MAX_POSTER_BYTES: usize = 25 * 1024 * 1024;

fn poster_extension(url: &str, content_type: Option<&str>) -> &'static str {
    let content_type = content_type.unwrap_or_default().to_ascii_lowercase();
    if content_type.contains("png") {
        return "png";
    }
    if content_type.contains("webp") {
        return "webp";
    }
    if content_type.contains("gif") {
        return "gif";
    }
    let url_path = url.split('?').next().unwrap_or(url).to_ascii_lowercase();
    if url_path.ends_with(".png") {
        "png"
    } else if url_path.ends_with(".webp") {
        "webp"
    } else if url_path.ends_with(".gif") {
        "gif"
    } else {
        "jpg"
    }
}

fn valid_poster_payload(content_type: Option<&str>, bytes: &[u8]) -> bool {
    if bytes.is_empty() || bytes.len() > MAX_POSTER_BYTES {
        return false;
    }
    if content_type
        .map(|value| value.to_ascii_lowercase().starts_with("text/"))
        .unwrap_or(false)
    {
        return false;
    }
    let has_supported_signature = bytes.starts_with(&[0xFF, 0xD8, 0xFF])
        || bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || (bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP")
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a");
    if !has_supported_signature {
        return false;
    }
    // A file signature alone accepts truncated/corrupt payloads. Decode the
    // image and require a usable raster before it can become a media card.
    image::load_from_memory(bytes)
        .map(|image| image.width() >= 20 && image.height() >= 20)
        .unwrap_or(false)
}

fn write_poster_sidecar_bytes(
    video_path: &str,
    extension: &str,
    bytes: &[u8],
) -> Result<String, String> {
    if !valid_poster_payload(None, bytes) {
        return Err("poster payload is empty, too large, or not a supported image".to_string());
    }
    let video = Path::new(video_path);
    let parent = video.parent().ok_or("video file has no parent directory")?;
    let stem = video
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or("video file has no stem")?;
    let extension = match extension.to_ascii_lowercase().as_str() {
        "png" => "png",
        "webp" => "webp",
        "gif" => "gif",
        _ => "jpg",
    };
    let sidecar_path = parent.join(format!("{stem}-poster.{extension}"));
    if let Ok(existing_bytes) = std::fs::read(&sidecar_path) {
        if valid_poster_payload(None, &existing_bytes) {
            return Ok(sidecar_path.to_string_lossy().to_string());
        }
    }

    let temporary_path = parent.join(format!("{stem}-poster.{extension}.part"));
    let previous_path = parent.join(format!("{stem}-poster.{extension}.previous"));
    {
        let mut file = std::fs::File::create(&temporary_path)
            .map_err(|error| format!("poster create failed: {error}"))?;
        file.write_all(bytes)
            .map_err(|error| format!("poster write failed: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("poster sync failed: {error}"))?;
    }
    if sidecar_path.exists() {
        let _ = std::fs::remove_file(&previous_path);
        std::fs::rename(&sidecar_path, &previous_path)
            .map_err(|error| format!("poster backup before replacement failed: {error}"))?;
    }
    if let Err(error) = std::fs::rename(&temporary_path, &sidecar_path) {
        if previous_path.exists() {
            let _ = std::fs::rename(&previous_path, &sidecar_path);
        }
        return Err(format!("poster finalize failed: {error}"));
    }
    if previous_path.exists() {
        let _ = std::fs::remove_file(&previous_path);
    }
    Ok(sidecar_path.to_string_lossy().to_string())
}

/// Downloads a remote poster image URL to a verified local sidecar file next to the video.
pub(crate) async fn download_poster_to_sidecar(
    client: &reqwest::Client,
    url: &str,
    video_path: &str,
) -> Result<String, String> {
    let response = client
        .get(url)
        .header("User-Agent", "CinaVault/1.6.4")
        .send()
        .await
        .map_err(|error| format!("poster fetch failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("poster HTTP {}", response.status()));
    }
    if response
        .content_length()
        .map(|length| length > MAX_POSTER_BYTES as u64)
        .unwrap_or(false)
    {
        return Err(format!("poster exceeds {} bytes", MAX_POSTER_BYTES));
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let extension = poster_extension(url, content_type.as_deref());
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("poster read failed: {error}"))?;
    if !valid_poster_payload(content_type.as_deref(), &bytes) {
        return Err("poster response was not a supported image".to_string());
    }
    write_poster_sidecar_bytes(video_path, extension, &bytes)
}

/// Saves a provider poster next to the video, or, when that folder cannot be
/// written (a read-only drive, a NAS share, a protected Windows folder), into
/// CinaVault's own artwork cache so the poster is still saved locally.
async fn save_remote_poster(
    client: &reqwest::Client,
    url: &str,
    video_path: &str,
    app_data_dir: &Path,
    media_id: i64,
) -> Result<String, String> {
    let sidecar_error = match download_poster_to_sidecar(client, url, video_path).await {
        Ok(path) => return Ok(path),
        Err(error) => error,
    };
    crate::metadata_keyless::cache_remote_artwork(client, app_data_dir, media_id, "poster", url)
        .await
        .map(|cached| cached.path)
        .map_err(|cache_error| {
            format!("{sidecar_error}; app artwork cache also failed: {cache_error}")
        })
}

/// Writes a Kodi-compatible NFO sidecar XML file next to the video file.
#[allow(clippy::too_many_arguments)]
fn write_nfo_sidecar(
    video_path: &str,
    title: &str,
    year: Option<i32>,
    overview: Option<&str>,
    rating: Option<f64>,
    genre: Option<&str>,
    tmdb_id: Option<&str>,
    imdb_id: Option<&str>,
    poster_path: Option<&str>,
) -> Result<(), String> {
    let video = Path::new(video_path);
    let parent = video.parent().ok_or("video file has no parent directory")?;
    let stem = video
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("video file has no stem")?;

    let nfo_path = parent.join(format!("{stem}.nfo"));

    let escaped_title = xml_escape(title);
    let year_str = year.map(|y| y.to_string()).unwrap_or_default();
    let overview_str = overview.map(xml_escape).unwrap_or_default();
    let rating_str = rating.map(|r| format!("{r:.1}")).unwrap_or_default();
    let genre_str = genre.map(xml_escape).unwrap_or_default();
    let tmdb_str = tmdb_id.unwrap_or_default();
    let imdb_str = imdb_id.unwrap_or_default();
    let thumb_str = poster_path.unwrap_or_default();

    let nfo_content = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <movie>\n\
         \t<title>{escaped_title}</title>\n\
         \t<year>{year_str}</year>\n\
         \t<plot>{overview_str}</plot>\n\
         \t<rating>{rating_str}</rating>\n\
         \t<genre>{genre_str}</genre>\n\
         \t<uniqueid type=\"tmdb\">{tmdb_str}</uniqueid>\n\
         \t<uniqueid type=\"imdb\">{imdb_str}</uniqueid>\n\
         \t<thumb aspect=\"poster\">{thumb_str}</thumb>\n\
         </movie>\n"
    );

    std::fs::write(&nfo_path, nfo_content.as_bytes())
        .map_err(|e| format!("nfo write failed: {e}"))?;
    Ok(())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn should_update_optional_text(current: Option<&str>, incoming: Option<&str>) -> bool {
    let current_blank = current.map(|value| value.trim().is_empty()).unwrap_or(true);
    current_blank
        && incoming
            .map(|value| {
                let trimmed = value.trim();
                !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("N/A")
            })
            .unwrap_or(false)
}

fn is_video_library_item(item: &LibraryItemRecord) -> bool {
    matches!(
        item.media_type.to_lowercase().as_str(),
        "adult" | "movie" | "episode" | "video"
    ) && is_video_path(&item.file_path)
}

fn is_video_path(file_path: &str) -> bool {
    let lower = file_path.to_lowercase();
    [
        ".mp4", ".mkv", ".avi", ".mov", ".wmv", ".flv", ".webm", ".m4v", ".mpg", ".mpeg", ".ts",
        ".m2ts", ".vob", ".ogv", ".3gp", ".divx", ".rm", ".rmvb", ".asf",
    ]
    .iter()
    .any(|extension| lower.ends_with(extension))
}

fn extract_embedded_title(file_path: &str) -> Option<String> {
    let mut cmd = std::process::Command::new("ffprobe");
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
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }

    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

fn non_empty_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("N/A"))
        .map(str::to_string)
}

fn parse_year_prefix(value: Option<&str>) -> Option<i32> {
    let text = value?.trim();
    if text.len() < 4 {
        return None;
    }
    text[..4].parse::<i32>().ok()
}

fn looks_like_timestamp_only(value: &str) -> bool {
    let compact = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    Regex::new(r"^\d{8,14}$")
        .expect("timestamp regex should compile")
        .is_match(&compact)
}

fn is_low_quality_title(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("unknown")
        || looks_like_timestamp_only(trimmed)
        || trimmed.contains('_')
        || Regex::new(r"(?i)\b(480p|720p|1080p|2160p|x264|x265|webrip|bluray)\b")
            .expect("quality marker regex should compile")
            .is_match(trimmed)
}

fn clean_title_for_display(value: &str) -> Option<String> {
    let cleaned = value
        .replace(['\r', '\n', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|ch| ch == '.' || ch == ' ')
        .to_string();

    if cleaned.is_empty() || looks_like_timestamp_only(&cleaned) {
        None
    } else {
        Some(cleaned)
    }
}

fn push_unique_query(queries: &mut Vec<String>, value: Option<String>) {
    let Some(value) = value.and_then(|v| clean_title_for_display(&v)) else {
        return;
    };

    if !queries
        .iter()
        .any(|existing| strong_title_match(existing, &value))
    {
        queries.push(value);
    }
}

fn strong_title_match(left: &str, right: &str) -> bool {
    let left = canonical_title(left);
    let right = canonical_title(right);
    if left.is_empty() || right.is_empty() {
        return false;
    }
    if left == right {
        return true;
    }

    let left_tokens = left.split_whitespace().collect::<Vec<_>>();
    let right_tokens = right.split_whitespace().collect::<Vec<_>>();
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return false;
    }
    let common = left_tokens
        .iter()
        .filter(|token| right_tokens.contains(token))
        .count();
    let larger = left_tokens.len().max(right_tokens.len());
    common >= 2 && (common as f32 / larger as f32) >= 0.8
}

fn canonical_title(value: &str) -> String {
    let without_release_tokens = Regex::new(
        r"(?i)\b(480p|576p|720p|1080p|1440p|2160p|4k|8k|x264|x265|h264|h265|hevc|avc|aac|ac3|dts|web\s?dl|webrip|bluray|brrip|dvdrip|hdrip|proper|repack|extended|remux|group)\b",
    )
    .expect("release token regex should compile")
    .replace_all(value, " ")
    .to_string();

    without_release_tokens
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn sanitize_windows_filename(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => ' ',
            _ => ch,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|ch| ch == '.' || ch == ' ')
        .to_string();

    let upper = cleaned.to_ascii_uppercase();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if reserved.contains(&upper.as_str()) {
        String::new()
    } else {
        cleaned
    }
}

fn title_case(value: &str) -> String {
    value
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(_)
                    if word
                        .chars()
                        .all(|ch| ch.is_ascii_uppercase() || !ch.is_ascii_alphabetic()) =>
                {
                    word.to_string()
                }
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str().to_lowercase()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn should_update_title(current_title: &str, normalized_title: &str, file_path: &str) -> bool {
    if normalized_title.is_empty() {
        return false;
    }
    let current = current_title.trim();
    if current.is_empty() || current.eq_ignore_ascii_case("unknown") {
        return true;
    }
    if titles_match(current, normalized_title) {
        return false;
    }
    if is_low_quality_title(current) {
        return true;
    }
    let filename_title = Path::new(file_path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().replace(['.', '_', '-'], " "))
        .unwrap_or_default();
    current.eq_ignore_ascii_case(filename_title.trim())
        || current.contains('.')
        || current.contains('_')
}

fn titles_match(left: &str, right: &str) -> bool {
    left.trim().eq_ignore_ascii_case(right.trim())
}

fn push_sample(
    samples: &mut Vec<EnrichmentItemSummary>,
    id: i64,
    old_title: &str,
    new_title: &str,
    file_path: &str,
    action: &str,
) {
    if samples.len() >= 10 {
        return;
    }
    samples.push(EnrichmentItemSummary {
        id,
        old_title: old_title.to_string(),
        new_title: new_title.to_string(),
        file_path: file_path.to_string(),
        action: action.to_string(),
    });
}

#[derive(Debug, serde::Serialize)]
pub struct AdultMetadataReport {
    #[serde(rename = "type")]
    result_type: &'static str,
    status: &'static str,
    items_scanned: usize,
    metadata_items_enriched: usize,
    metadata_fields_updated: usize,
    posters_updated: usize,
    /// Posters promoted from an existing chapter frame when no provider
    /// poster existed (Issue 2 fallback; never overwrites a provider poster).
    posters_from_chapter_frames: usize,
    skipped_complete: usize,
    sidecars_written: usize,
    configured_adult_providers: Vec<String>,
    provider_count: usize,
    provider_errors: Vec<String>,
}

/// Dedicated command for gathering metadata specifically for adult media items.
/// Uses TPDB, StashDB, and any other configured adult providers.
///
/// Issue 1: this command no longer runs the gather inline. It claims the
/// single enrichment-job slot and returns an `EnrichmentJobStart`
/// immediately; the gather itself runs on a background task (see
/// `gather_adult_metadata_work` and `crate::enrichment_jobs`). Poll
/// `get_enrichment_job_status` for progress and the final report, which has
/// exactly the shape this command used to return. A second start while a
/// job runs returns `status: "busy"` with the running job's id.
#[tauri::command]
pub async fn gather_adult_metadata(
    app: AppHandle,
) -> Result<enrichment_jobs::EnrichmentJobStart, String> {
    crate::edition::ensure_adult_allowed()?;
    Ok(enrichment_jobs::start_adult_metadata_job(&app))
}

// ── Issue 2: shared per-item adult enrichment ─────────────────────────────
//
// The adult gather and the post-scan adult pass used to be two different
// implementations of the same job: the gather looped whole-library with
// TPDB/StashDB + the extra-adult batch, while post-scan fell back to the
// single-item check (verbatim title only, DB-only writes, no NFO, and a
// sidecar-only poster with no app-cache fallback). `enrich_adult_item` is
// the one implementation both paths now call, so a newly scanned adult
// item gets exactly the gather's treatment: embedded-title + library-title
// queries, cross-provider merging until title AND poster are found,
// `save_remote_poster` (sidecar, with the app artwork cache as fallback),
// a chapter-frame poster fallback, NFO + DB writes, and a durable attempt
// record. It never loops over the library itself: callers own iteration,
// progress, and the Issue 1 job guard, so this function cannot reintroduce
// an inline whole-library loop.

/// Terminal outcome of one `enrich_adult_item` call. Callers map outcomes
/// to their own counters; the attempt record is written here, not by the
/// caller, so the gather and post-scan cannot disagree about retry state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AdultItemOutcome {
    /// Not a video library item; nothing was attempted.
    SkippedNonVideo,
    /// The video file was not on disk; nothing was attempted.
    SkippedMissingFile,
    /// Already satisfies the Issue 1 skip-complete rule.
    SkippedComplete,
    /// No provider matched and no poster fallback applied.
    NoMatch,
    /// The remote phase exceeded `ITEM_TIMEOUT_SECS`; nothing was written.
    Timeout,
    /// A DB write failed after the NFO write (or the NFO path itself could
    /// not be evaluated); the error string is in the outcome.
    Failed(String),
    /// Metadata and/or a poster were saved.
    Enriched {
        /// Provider fields actually written (as in the old gather counts).
        fields_updated: usize,
        /// A new local poster was attached (provider poster, app-cache
        /// fallback, or chapter frame).
        poster_saved: bool,
        /// The saved poster came from a chapter frame, not a provider.
        poster_from_chapter_frame: bool,
        /// Still no poster after every fallback (retryable via attempts).
        poster_missing: bool,
        /// The NFO sidecar write succeeded.
        sidecar_written: bool,
    },
}

/// The keyed adult providers that have a key and are switched on, plus the
/// enabled keyless extra-adult providers. Shared by the gather and the
/// post-scan adult pass so both query the same provider set.
pub(crate) fn load_adult_provider_deps(
    db: &crate::db::Database,
) -> Result<
    (
        HashMap<String, String>,
        HashMap<String, String>,
    ),
    String,
> {
    let mut provider_keys = load_provider_keys(db)?;
    provider_keys.retain(|provider, _| {
        crate::metadata_ext::is_adult_provider_enabled(db, provider).unwrap_or(false)
    });
    // Every enabled adult provider beyond ThePornDB and StashDB — the
    // keyless ones (IAFD, PhoenixAdult, PGMA, Porn Site Nuxt) — runs as a
    // fallback when the keyed providers miss or are not configured.
    let extra_adult = crate::metadata_ext::enabled_extra_adult_provider_values(db)?;
    Ok((provider_keys, extra_adult))
}

/// One library row as a `LibraryItemRecord`, for the post-scan adult pass
/// (which starts from item ids, not from a whole-library query).
pub(crate) fn load_library_item_record(
    db: &crate::db::Database,
    id: i64,
) -> Option<LibraryItemRecord> {
    let mut stmt = db.conn.prepare(
        "SELECT mi.id, mi.title, mi.file_path, mi.media_type, mi.overview, \
         mi.poster_path, mi.year, mi.rating, mi.genre, mi.tmdb_id, mi.imdb_id, \
         ms.name, ms.path \
         FROM media_items mi \
         LEFT JOIN media_sources ms ON ms.id = mi.source_id \
         WHERE mi.id = ?1",
    ).ok()?;
    let mut rows = stmt.query_map(rusqlite::params![id], |row| {
        Ok(LibraryItemRecord {
            id: row.get(0)?,
            title: row.get(1)?,
            file_path: row.get(2)?,
            media_type: row.get(3)?,
            overview: row.get(4)?,
            poster_path: row.get(5)?,
            year: row.get(6)?,
            rating: row.get(7)?,
            genre: row.get(8)?,
            tmdb_id: row.get(9)?,
            imdb_id: row.get(10)?,
            source_name: row.get(11)?,
            source_path: row.get(12)?,
        })
    }).ok()?;
    rows.next()?.ok()
}

/// True when a provider match is usable as a complete match: it has both a
/// title and a poster. Title-only matches keep the search going.
fn provider_match_complete(provider: &ProviderMatch) -> bool {
    provider
        .title
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
        && provider
            .poster_path
            .as_deref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
}

/// The item's poster as stored today, when it is already a local file.
/// Remote URLs do not count: they still need the sidecar migration below.
fn local_poster_path(item: &LibraryItemRecord) -> Option<String> {
    item.poster_path.clone().filter(|path| {
        let trimmed = path.trim();
        !trimmed.is_empty()
            && !trimmed.starts_with("http://")
            && !trimmed.starts_with("https://")
    })
}

/// True when the chapter-frame fallback may run: the update brings no
/// poster and the item has no local poster of its own. A real provider
/// poster is never overwritten by a frame, and an existing local poster is
/// never replaced by one either.
fn needs_poster_fallback(
    item: &LibraryItemRecord,
    update_poster_path: Option<&str>,
) -> bool {
    update_poster_path
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
        && local_poster_path(item).is_none()
}

/// Promotes one existing chapter frame to the item's poster sidecar, as a
/// last resort when no provider poster exists. The middle frame is used:
/// the first frame is commonly a black frame or a title card, and the
/// middle frame is the most representative still available. The frame bytes
/// go through the same validated, atomic sidecar write as a downloaded
/// poster, and a valid sidecar that already exists on disk is kept instead
/// of being overwritten by the frame. Returns `None` when no usable frame
/// exists; it never generates frames — frame generation stays with the
/// chapter-thumbs step, and this only reuses frames that are already there.
fn promote_chapter_frame_poster(video_path: &str) -> Result<Option<String>, String> {
    let dir = crate::chapters::default_chapter_dir(video_path);
    if !std::path::Path::new(&dir).exists() {
        return Ok(None);
    }
    let mut frames: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .map_err(|error| format!("chapter frames unreadable: {error}"))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with("chapter_"))
                .unwrap_or(false)
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "jpg" | "png"))
                    .unwrap_or(false)
        })
        .collect();
    frames.sort();
    let Some(frame) = frames.get(frames.len() / 2).cloned() else {
        return Ok(None);
    };
    let extension = frame
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("jpg");
    let bytes = std::fs::read(&frame).map_err(|error| format!("chapter frame unreadable: {error}"))?;
    Ok(Some(write_poster_sidecar_bytes(
        video_path, extension, &bytes,
    )?))
}

/// Enriches one adult item: resolve, merge, poster, fallback, NFO, DB,
/// attempt record. See the section header for the contract shared by the
/// gather and post-scan. Provider errors are appended to `provider_errors`
/// (never swallowed); a DB write failure is additionally returned as
/// `AdultItemOutcome::Failed` so the caller can count it, and the attempt
/// record for it stays retryable.
pub(crate) async fn enrich_adult_item(
    state: &State<'_, AppState>,
    client: &reqwest::Client,
    provider_keys: &HashMap<String, String>,
    extra_adult: &HashMap<String, String>,
    item: &LibraryItemRecord,
    provider_errors: &mut Vec<String>,
) -> AdultItemOutcome {
    if !is_video_library_item(item) {
        record_adult_attempt(state, item, crate::adult_attempts::OUTCOME_SKIPPED_NON_VIDEO, None, provider_errors);
        return AdultItemOutcome::SkippedNonVideo;
    }
    if !Path::new(&item.file_path).exists() {
        record_adult_attempt(state, item, crate::adult_attempts::OUTCOME_SKIPPED_MISSING, None, provider_errors);
        return AdultItemOutcome::SkippedMissingFile;
    }
    if item_metadata_complete(item) {
        record_adult_attempt(state, item, crate::adult_attempts::OUTCOME_SKIPPED_COMPLETE, None, provider_errors);
        return AdultItemOutcome::SkippedComplete;
    }

    let embedded_title = extract_embedded_title_async(&item.file_path).await;
    let mut queries = build_query_candidates(item, embedded_title.clone());
    queries.truncate(enrichment_jobs::MAX_QUERY_CANDIDATES_PER_ITEM);

    // The whole remote phase for one item is bounded (Issue 1). Errors
    // collected inside the timeout are kept in a local vec and appended
    // afterwards, so the timeout future borrows a local instead of the
    // caller's vec across the await boundary.
    let mut remote_errors: Vec<String> = Vec::new();
    let remote_provider = match tokio::time::timeout(
        std::time::Duration::from_secs(enrichment_jobs::ITEM_TIMEOUT_SECS),
        async {
            let mut remote = resolve_provider_match(
                client,
                provider_keys,
                SourceKind::AdultVideo,
                &queries,
                &item.file_path,
                &mut remote_errors,
            )
            .await;

            // Issue 2: the extra-adult chain used to run only when the
            // keyed providers missed entirely, so a TPDB title-only match
            // suppressed a later Nuxt poster. It now also runs when the
            // match so far has no poster, and its fields are merged into
            // the existing match (first non-blank field wins) instead of
            // replacing it. The loop stops once title AND poster are both
            // present. IAFD, PhoenixAdult, and PGMA return no poster by
            // construction, so a merged match can still end posterless —
            // that is what the chapter-frame fallback below is for.
            if !extra_adult.is_empty()
                && remote
                    .as_ref()
                    .map(|provider| !provider_match_complete(provider))
                    .unwrap_or(true)
            {
                for query in &queries {
                    if task_progress::stop_requested() {
                        break;
                    }
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(
                            enrichment_jobs::PROVIDER_TIMEOUT_SECS,
                        ),
                        crate::metadata::fetch_adult_metadata_for_batch(
                            client,
                            extra_adult,
                            query,
                            &item.file_path,
                            &mut remote_errors,
                        ),
                    )
                    .await
                    {
                        Ok(Some((_provider, found))) => {
                            let found = ProviderMatch {
                                title: found.title,
                                overview: found.overview,
                                poster_path: found.poster_path,
                                year: found.year,
                                rating: found.rating,
                                genre: found.genre,
                                tmdb_id: found.tmdb_id,
                                imdb_id: found.imdb_id,
                            };
                            remote = Some(match remote {
                                Some(existing) => merge_provider_matches(existing, found),
                                None => found,
                            });
                            if remote
                                .as_ref()
                                .map(provider_match_complete)
                                .unwrap_or(false)
                            {
                                break;
                            }
                        }
                        Ok(None) => {}
                        Err(_) => {
                            remote_errors.push(format!(
                                "extra_adult/{query}: provider chain timed out after {}s",
                                enrichment_jobs::PROVIDER_TIMEOUT_SECS
                            ));
                        }
                    }
                }
            }

            remote
        },
    )
    .await
    {
        Ok(provider) => {
            provider_errors.extend(remote_errors);
            provider
        }
        Err(_) => {
            provider_errors.extend(remote_errors);
            provider_errors.push(format!(
                "item_timeout/{}: provider phase exceeded {}s; item will be retried on the next run",
                item.file_path,
                enrichment_jobs::ITEM_TIMEOUT_SECS
            ));
            record_adult_attempt(state, item, crate::adult_attempts::OUTCOME_TIMEOUT, None, provider_errors);
            return AdultItemOutcome::Timeout;
        }
    };

    let mut update = remote_provider
        .as_ref()
        .map(|provider| build_metadata_update(item, provider, &SourceKind::AdultVideo))
        .unwrap_or_default();

    // Issue 2: the gather used to download posters with the sidecar-only
    // helper, so a poster failed outright on a read-only folder or NAS
    // share. `save_remote_poster` keeps the sidecar first and falls back
    // to the app artwork cache, exactly like the library-enrichment loop.
    let mut poster_saved = false;
    let poster_candidate = update.poster_path.clone().or_else(|| {
        item.poster_path
            .clone()
            .filter(|path| path.starts_with("http://") || path.starts_with("https://"))
    });
    if let Some(remote_url) = poster_candidate {
        if remote_url.starts_with("http://") || remote_url.starts_with("https://") {
            match save_remote_poster(
                client,
                &remote_url,
                &item.file_path,
                &state.app_data_dir,
                item.id,
            )
            .await
            {
                Ok(local_path) => {
                    if update.poster_path.is_none() {
                        update.changed_fields += 1;
                    }
                    update.poster_path = Some(local_path);
                    poster_saved = true;
                }
                Err(err) => {
                    provider_errors.push(format!("poster/{}: {}", item.file_path, err));
                    // Issue 2 correction: a failed download must not leave
                    // the remote URL in the update. The old gather wrote it
                    // to the NFO and the DB anyway, which breaks the adult
                    // rule that only verified local posters are stored (the
                    // single-item check clears it on failure for the same
                    // reason). Clearing it also lets the chapter-frame
                    // fallback below run, and keeps skip-complete honest:
                    // a remote poster never counts as complete, so the next
                    // run retries the migration instead of believing the
                    // item already has a poster.
                    if update
                        .poster_path
                        .as_deref()
                        .map(|value| {
                            value.starts_with("http://") || value.starts_with("https://")
                        })
                        .unwrap_or(false)
                    {
                        update.poster_path = None;
                        update.changed_fields = update.changed_fields.saturating_sub(1);
                    }
                }
            }
        }
    }

    // Issue 2: chapter-frame fallback. Only when no provider poster was
    // saved and the item has no local poster; never overwrites either.
    let mut poster_from_chapter_frame = false;
    if needs_poster_fallback(item, update.poster_path.as_deref()) {
        match promote_chapter_frame_poster(&item.file_path) {
            Ok(Some(local_path)) => {
                if update.poster_path.is_none() {
                    update.changed_fields += 1;
                }
                update.poster_path = Some(local_path);
                poster_saved = true;
                poster_from_chapter_frame = true;
            }
            Ok(None) => {}
            Err(err) => {
                provider_errors.push(format!("chapter_frame/{}: {}", item.file_path, err));
            }
        }
    }

    let poster_missing =
        update.poster_path.is_none() && local_poster_path(item).is_none();

    let mut sidecar_written = false;
    if update.changed_fields > 0 || update.poster_path.is_some() {
        let nfo_title = update.title.as_deref().unwrap_or(&item.title);
        match write_nfo_sidecar(
            &item.file_path,
            nfo_title,
            update.year.or(item.year),
            update.overview.as_deref().or(item.overview.as_deref()),
            update.rating.or(item.rating),
            update.genre.as_deref().or(item.genre.as_deref()),
            update.tmdb_id.as_deref().or(item.tmdb_id.as_deref()),
            update.imdb_id.as_deref().or(item.imdb_id.as_deref()),
            update.poster_path.as_deref(),
        ) {
            Ok(()) => sidecar_written = true,
            Err(err) => {
                provider_errors.push(format!("nfo/{}: {}", item.file_path, err));
            }
        }
    }

    // Issue 2: a DB write failure used to be swallowed (`let _ =`), so a
    // failed item looked enriched and was never retried. It is now a
    // `Failed` outcome with a retryable attempt record; the NFO above was
    // already written, and the next run rewrites the DB row (skip-complete
    // cannot skip it, because the DB row is still incomplete).
    if update.changed_fields > 0 {
        let db_result = {
            let db = state.db.lock().map_err(|err| err.to_string());
            match db {
                Ok(db) => db.update_media_metadata_data(
                    &item.file_path,
                    update.title.as_deref(),
                    update.overview.as_deref(),
                    update.poster_path.as_deref(),
                    update.year,
                    update.rating,
                    update.genre.as_deref(),
                    update.tmdb_id.as_deref(),
                    update.imdb_id.as_deref(),
                    update.media_type.as_deref(),
                ),
                Err(err) => Err(err),
            }
        };
        if let Err(err) = db_result.map_err(|error| error.to_string()) {
            // The message is NOT pushed here: it travels in the Failed
            // outcome and the caller pushes it exactly once.
            let message = format!("db/{}: {}", item.file_path, err);
            record_adult_attempt(state, item, crate::adult_attempts::OUTCOME_FAILED, Some(&message), provider_errors);
            return AdultItemOutcome::Failed(message);
        }
    }

    let fields_updated = update.changed_fields;
    if remote_provider.is_none() && !poster_saved {
        record_adult_attempt(state, item, crate::adult_attempts::OUTCOME_NO_MATCH, None, provider_errors);
        return AdultItemOutcome::NoMatch;
    }
    let outcome_str = if poster_missing {
        crate::adult_attempts::OUTCOME_ENRICHED_POSTER_MISSING
    } else {
        crate::adult_attempts::OUTCOME_ENRICHED
    };
    record_adult_attempt(state, item, outcome_str, None, provider_errors);
    AdultItemOutcome::Enriched {
        fields_updated,
        poster_saved,
        poster_from_chapter_frame,
        poster_missing,
        sidecar_written,
    }
}

/// Best-effort attempt recording for one item. A recording failure never
/// fails the enrichment itself; it is appended to `provider_errors` so it
/// is visible in the job report instead of disappearing silently.
fn record_adult_attempt(
    state: &State<'_, AppState>,
    item: &LibraryItemRecord,
    outcome: &str,
    last_error: Option<&str>,
    provider_errors: &mut Vec<String>,
) {
    let result = state
        .db
        .lock()
        .map_err(|err| err.to_string())
        .and_then(|db| crate::adult_attempts::record_attempt(&db, item.id, &item.file_path, outcome, last_error));
    if let Err(err) = result {
        provider_errors.push(format!("attempt_record/{}: {}", item.file_path, err));
    }
}


/// Synchronous gather implementation. Runs inside a background job started
/// by `gather_adult_metadata` (or, for the one-shot library conversion, by
/// `convert_entire_library_to_adult`, which first checks the job guard).
pub(crate) async fn gather_adult_metadata_work(
    state: State<'_, AppState>,
) -> Result<AdultMetadataReport, String> {
    crate::edition::ensure_adult_allowed()?;
    let (items, provider_keys, extra_adult) = {
        let db = state.db.lock().map_err(|err| err.to_string())?;
        let mut stmt = db
            .conn
            .prepare(
                "SELECT mi.id, mi.title, mi.file_path, mi.media_type, mi.overview, \
                 mi.poster_path, mi.year, mi.rating, mi.genre, mi.tmdb_id, mi.imdb_id, \
                 ms.name, ms.path \
                 FROM media_items mi \
                 LEFT JOIN media_sources ms ON ms.id = mi.source_id \
                 WHERE mi.media_type = 'adult' \
                 ORDER BY date_added DESC",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(LibraryItemRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    file_path: row.get(2)?,
                    media_type: row.get(3)?,
                    overview: row.get(4)?,
                    poster_path: row.get(5)?,
                    year: row.get(6)?,
                    rating: row.get(7)?,
                    genre: row.get(8)?,
                    tmdb_id: row.get(9)?,
                    imdb_id: row.get(10)?,
                    source_name: row.get(11)?,
                    source_path: row.get(12)?,
                })
            })
            .map_err(|err| err.to_string())?;
        let items = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())?;
        // Issue 2: the post-scan adult pass loads the same provider set
        // through this helper, so the two paths cannot drift apart.
        let (provider_keys, extra_adult) = load_adult_provider_deps(&db)?;
        (items, provider_keys, extra_adult)
    };

    // Report every adapter the gather can actually call: the keyed providers
    // that have a key, plus the keyless adult providers that are switched on.
    let mut configured_adult_providers: Vec<String> = ["tpdb", "stashdb"]
        .iter()
        .filter(|&&p| provider_keys.contains_key(p))
        .map(|p| p.to_string())
        .collect();
    for provider in crate::metadata_ext::EXTRA_ADULT_PROVIDER_KEYS {
        if extra_adult.contains_key(*provider) {
            configured_adult_providers.push((*provider).to_string());
        }
    }

    let mut report = AdultMetadataReport {
        result_type: "adult_metadata_gather",
        status: "success",
        items_scanned: items.len(),
        metadata_items_enriched: 0,
        metadata_fields_updated: 0,
        posters_updated: 0,
        posters_from_chapter_frames: 0,
        skipped_complete: 0,
        sidecars_written: 0,
        provider_count: configured_adult_providers.len(),
        configured_adult_providers: configured_adult_providers.clone(),
        provider_errors: Vec::new(),
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|err| err.to_string())?;

    // Issue 1: previously this gather never started a MetadataTaskGuard, so
    // the UI sat on the frontend's local "Starting..." message for the whole
    // run. The guard also gives get_enrichment_job_status its live progress.
    let mut progress = task_progress::MetadataTaskGuard::start(
        "adult_metadata_gather",
        "Adult Metadata Gather",
        items.len(),
        "Preparing adult metadata gather",
    );
    let total_items = items.len();
    let mut stopped = false;
    for (index, item) in items.into_iter().enumerate() {
        if task_progress::stop_requested() {
            report
                .provider_errors
                .push("Stopped by user before processing the next item".to_string());
            stopped = true;
            break;
        }
        progress.update(
            index + 1,
            format!(
                "Gathering adult metadata for {} of {}",
                index + 1,
                total_items
            ),
        );
        // Issue 2: the whole per-item pipeline now lives in
        // `enrich_adult_item`, shared with the post-scan adult pass. The
        // gather only maps outcomes to report counters here.
        match enrich_adult_item(
            &state,
            &client,
            &provider_keys,
            &extra_adult,
            &item,
            &mut report.provider_errors,
        )
        .await
        {
            AdultItemOutcome::SkippedNonVideo
            | AdultItemOutcome::SkippedMissingFile
            | AdultItemOutcome::NoMatch
            | AdultItemOutcome::Timeout => {}
            AdultItemOutcome::SkippedComplete => {
                report.skipped_complete += 1;
            }
            AdultItemOutcome::Failed(error) => {
                report.provider_errors.push(error);
            }
            AdultItemOutcome::Enriched {
                fields_updated,
                poster_saved,
                poster_from_chapter_frame,
                poster_missing: _,
                sidecar_written,
            } => {
                if fields_updated > 0 {
                    report.metadata_items_enriched += 1;
                    report.metadata_fields_updated += fields_updated;
                }
                if poster_saved && poster_from_chapter_frame {
                    report.posters_from_chapter_frames += 1;
                } else if poster_saved {
                    report.posters_updated += 1;
                }
                if sidecar_written {
                    report.sidecars_written += 1;
                }
            }
        }
    }

    progress.finish(if stopped {
        "Adult metadata gather stopped by user".to_string()
    } else {
        format!(
            "Adult metadata gather complete: {} items enriched, {} skipped as complete, {} posters from chapter frames",
            report.metadata_items_enriched, report.skipped_complete, report.posters_from_chapter_frames
        )
    });

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::{
        build_metadata_update, build_query_candidates, classify_library_item,
        local_embedded_title_match, local_sidecar_artwork_match, normalize_filename_title,
        rename_confidence, safe_rename_target, strong_title_match, valid_poster_payload, write_poster_sidecar_bytes,
        EnrichmentMode, LibraryItemRecord, ProviderMatch, RenameTarget, SourceKind,
    };
    use std::fs;

    fn sample_item(title: &str, file_path: &str, source_name: Option<&str>) -> LibraryItemRecord {
        LibraryItemRecord {
            id: 1,
            title: title.to_string(),
            file_path: file_path.to_string(),
            media_type: "movie".to_string(),
            overview: None,
            poster_path: None,
            year: None,
            rating: None,
            genre: None,
            tmdb_id: None,
            imdb_id: None,
            source_name: source_name.map(str::to_string),
            source_path: None,
        }
    }

    #[test]
    fn normalizes_common_release_filename_into_clean_title() {
        assert_eq!(
            normalize_filename_title("My.Movie.2024.1080p.x264-GROUP.mkv"),
            "My Movie (2024)"
        );
    }

    #[test]
    fn timestamp_only_filename_is_not_renamed_without_metadata() {
        assert_eq!(normalize_filename_title("2024-08-31_141904.mp4"), "");
    }

    #[test]
    fn classifies_adult_sources_from_source_name_hints() {
        let item = sample_item(
            "2024-08-31 141904",
            r"E:\Personal Vids X\Media\2024-08-31_141904.mp4",
            Some("Personal Vids X"),
        );

        assert_eq!(classify_library_item(&item), SourceKind::AdultVideo);
    }

    #[test]
    fn builds_query_candidates_from_embedded_title_before_filename() {
        let item = sample_item(
            "2024-08-31 141904",
            r"E:\Videos\2024-08-31_141904.mp4",
            Some("General Video"),
        );

        let queries = build_query_candidates(&item, Some("Actual Scene Title".to_string()));

        assert_eq!(
            queries.first().map(String::as_str),
            Some("Actual Scene Title")
        );
    }

    #[test]
    fn embedded_title_fallback_enriches_metadata_when_provider_lookup_misses() {
        let item = sample_item(
            "2024-08-31 141904",
            r"E:\Videos\2024-08-31_141904.mp4",
            Some("General Video"),
        );

        let provider = local_embedded_title_match(Some("Actual Scene Title"))
            .expect("embedded title should produce a local metadata match");
        let update = build_metadata_update(&item, &provider, &SourceKind::StandardVideo);

        assert_eq!(provider.title.as_deref(), Some("Actual Scene Title"));
        assert_eq!(update.title.as_deref(), Some("Actual Scene Title"));
        assert_eq!(update.changed_fields, 1);
    }

    #[test]
    fn acquired_poster_is_validated_and_atomically_written_as_a_sidecar() {
        let dir =
            std::env::temp_dir().join(format!("cinavault-poster-write-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let video = dir.join("Movie.mp4");
        fs::write(&video, b"video").expect("video should be created");
        let mut png = Vec::new();
        image::DynamicImage::new_rgba8(20, 20)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("fixture PNG should encode");

        assert!(valid_poster_payload(Some("image/png"), &png));
        assert!(!valid_poster_payload(
            Some("text/html"),
            b"<html>error</html>"
        ));
        let poster = write_poster_sidecar_bytes(&video.to_string_lossy(), "png", &png)
            .expect("poster sidecar should be written");

        assert!(poster.ends_with("Movie-poster.png"));
        assert_eq!(fs::read(&poster).unwrap(), png);
        assert!(!dir.join("Movie-poster.png.part").exists());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn invalid_existing_poster_sidecar_is_replaced_with_verified_image_bytes() {
        let dir =
            std::env::temp_dir().join(format!("cinavault-poster-replace-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let video = dir.join("Movie.mp4");
        let stale_poster = dir.join("Movie-poster.jpg");
        let mut jpg = Vec::new();
        image::DynamicImage::new_rgb8(20, 20)
            .write_to(&mut std::io::Cursor::new(&mut jpg), image::ImageFormat::Jpeg)
            .expect("fixture JPEG should encode");
        fs::write(&video, b"video").expect("video should be created");
        fs::write(&stale_poster, b"<html>expired poster URL</html>")
            .expect("stale poster should be created");

        let poster = write_poster_sidecar_bytes(&video.to_string_lossy(), "jpg", &jpg)
            .expect("verified poster should replace invalid sidecar");

        assert_eq!(poster, stale_poster.to_string_lossy());
        assert_eq!(fs::read(&poster).unwrap(), jpg);
        assert!(!dir.join("Movie-poster.jpg.part").exists());
        assert!(!dir.join("Movie-poster.jpg.previous").exists());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn adult_provider_candidate_must_match_the_requested_title() {
        assert!(strong_title_match("Exact Scene Title", "Exact Scene Title"));
        assert!(!strong_title_match("Exact Scene Title", "Different Scene Title"));
    }

    #[test]
    fn truncated_image_signature_is_not_accepted_as_artwork() {
        assert!(!valid_poster_payload(Some("image/png"), b"\x89PNG\r\n\x1a\ntruncated"));
    }

    #[test]
    fn sidecar_artwork_fallback_populates_missing_posters() {
        let dir = std::env::temp_dir().join(format!("cinavault-sidecar-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let video = dir.join("Actual Scene Title.mp4");
        let poster = dir.join("Actual Scene Title-poster.jpg");
        fs::write(&video, b"video").expect("video should be created");
        fs::write(&poster, b"poster").expect("poster should be created");

        let item = sample_item(
            "Actual Scene Title",
            &video.to_string_lossy(),
            Some("General Video"),
        );

        let provider =
            local_sidecar_artwork_match(&item).expect("sidecar artwork should produce a match");
        let update = build_metadata_update(&item, &provider, &SourceKind::StandardVideo);

        assert_eq!(
            update.poster_path.as_deref(),
            Some(poster.to_string_lossy().as_ref())
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn allows_rename_for_balanced_confidence_when_provider_and_embedded_title_agree() {
        let item = sample_item(
            "2024-08-31 141904",
            r"E:\Videos\2024-08-31_141904.mp4",
            Some("General Video"),
        );
        let provider = ProviderMatch {
            title: Some("Actual Scene Title".to_string()),
            overview: None,
            poster_path: None,
            year: None,
            rating: None,
            genre: None,
            tmdb_id: Some("123".to_string()),
            imdb_id: None,
        };

        let confidence = rename_confidence(
            &item,
            Some("Actual Scene Title"),
            &provider,
            EnrichmentMode::MetadataAndRename,
        );

        assert!(confidence.allow_rename);
        assert_eq!(
            confidence.normalized_title.as_deref(),
            Some("Actual Scene Title")
        );
    }

    #[test]
    fn blocks_rename_when_provider_title_conflicts_with_embedded_title() {
        let item = sample_item(
            "2024-08-31 141904",
            r"E:\Videos\2024-08-31_141904.mp4",
            Some("General Video"),
        );
        let provider = ProviderMatch {
            title: Some("Different Provider Title".to_string()),
            overview: None,
            poster_path: None,
            year: None,
            rating: None,
            genre: None,
            tmdb_id: Some("123".to_string()),
            imdb_id: None,
        };

        let confidence = rename_confidence(
            &item,
            Some("Actual Scene Title"),
            &provider,
            EnrichmentMode::MetadataAndRename,
        );

        assert!(!confidence.allow_rename);
    }

    #[test]
    fn blocks_rename_when_stable_id_has_no_local_title_support() {
        let item = sample_item(
            "2024-08-31 141904",
            r"E:\Videos\2024-08-31_141904.mp4",
            Some("General Video"),
        );
        let provider = ProviderMatch {
            title: Some("Actual Scene Title".to_string()),
            overview: None,
            poster_path: None,
            year: None,
            rating: None,
            genre: None,
            tmdb_id: Some("123".to_string()),
            imdb_id: None,
        };

        let confidence =
            rename_confidence(&item, None, &provider, EnrichmentMode::MetadataAndRename);

        assert!(!confidence.allow_rename);
    }

    #[test]
    fn metadata_only_mode_updates_fields_without_allowing_rename() {
        let item = sample_item(
            "2024-08-31 141904",
            r"E:\Videos\2024-08-31_141904.mp4",
            Some("General Video"),
        );
        let provider = ProviderMatch {
            title: Some("Actual Scene Title".to_string()),
            overview: Some("Summary".to_string()),
            poster_path: None,
            year: Some(2024),
            rating: None,
            genre: None,
            tmdb_id: Some("123".to_string()),
            imdb_id: None,
        };

        let decision = rename_confidence(
            &item,
            Some("Actual Scene Title"),
            &provider,
            EnrichmentMode::MetadataOnly,
        );

        assert!(!decision.allow_rename);
    }

    #[test]
    fn safe_rename_target_blocks_existing_destination() {
        let dir =
            std::env::temp_dir().join(format!("cinavault-enrichment-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let source = dir.join("My.Movie.2024.1080p.mkv");
        let existing = dir.join("My Movie (2024).mkv");
        fs::write(&source, b"source").expect("source should be created");
        fs::write(&existing, b"existing").expect("existing target should be created");

        assert!(matches!(
            safe_rename_target(&source, "My Movie (2024)"),
            RenameTarget::Collision(_)
        ));

        let _ = fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod issue1_tests {
    use super::{item_metadata_complete, LibraryItemRecord};

    fn item_with(
        overview: Option<&str>,
        poster_path: Option<&str>,
        year: Option<i32>,
        genre: Option<&str>,
    ) -> LibraryItemRecord {
        LibraryItemRecord {
            id: 1,
            title: "Some Title".to_string(),
            file_path: "/media/some-title.mkv".to_string(),
            media_type: "adult".to_string(),
            overview: overview.map(str::to_string),
            poster_path: poster_path.map(str::to_string),
            year,
            rating: None,
            genre: genre.map(str::to_string),
            tmdb_id: None,
            imdb_id: None,
            source_name: None,
            source_path: None,
        }
    }

    #[test]
    fn complete_item_has_overview_local_poster_and_year() {
        assert!(item_metadata_complete(&item_with(
            Some("A synopsis"),
            Some("/media/some-title-poster.jpg"),
            Some(2024),
            None,
        )));
    }

    #[test]
    fn genre_substitutes_for_year_but_neither_does_not() {
        assert!(item_metadata_complete(&item_with(
            Some("A synopsis"),
            Some("/media/some-title-poster.jpg"),
            None,
            Some("Drama"),
        )));
        assert!(!item_metadata_complete(&item_with(
            Some("A synopsis"),
            Some("/media/some-title-poster.jpg"),
            None,
            None,
        )));
    }

    #[test]
    fn remote_poster_does_not_count_as_complete() {
        // Remote posters still need the sidecar migration the loops do.
        assert!(!item_metadata_complete(&item_with(
            Some("A synopsis"),
            Some("https://example.invalid/poster.jpg"),
            Some(2024),
            Some("Drama"),
        )));
    }

    #[test]
    fn blank_overview_or_n_a_overview_does_not_count_as_complete() {
        assert!(!item_metadata_complete(&item_with(
            None,
            Some("/media/some-title-poster.jpg"),
            Some(2024),
            Some("Drama"),
        )));
        assert!(!item_metadata_complete(&item_with(
            Some("N/A"),
            Some("/media/some-title-poster.jpg"),
            Some(2024),
            Some("Drama"),
        )));
    }
}

#[cfg(test)]
mod issue2_tests {
    use super::{
        local_poster_path, merge_provider_matches, needs_poster_fallback,
        promote_chapter_frame_poster, provider_match_complete, LibraryItemRecord, ProviderMatch,
    };
    use std::fs;

    fn item_with_poster(poster_path: Option<&str>) -> LibraryItemRecord {
        LibraryItemRecord {
            id: 1,
            title: "Some Scene".to_string(),
            file_path: "/media/some-scene.mp4".to_string(),
            media_type: "adult".to_string(),
            overview: None,
            poster_path: poster_path.map(str::to_string),
            year: None,
            rating: None,
            genre: None,
            tmdb_id: None,
            imdb_id: None,
            source_name: None,
            source_path: None,
        }
    }

    fn jpeg_bytes(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        image::DynamicImage::new_rgb8(width, height)
            .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Jpeg)
            .expect("fixture JPEG should encode");
        bytes
    }

    #[test]
    fn merging_fills_a_title_only_match_with_a_later_providers_poster() {
        let title_only = ProviderMatch {
            title: Some("Some Scene".to_string()),
            overview: Some("Synopsis".to_string()),
            ..ProviderMatch::default()
        };
        let poster_only = ProviderMatch {
            poster_path: Some("https://example.invalid/poster.jpg".to_string()),
            ..ProviderMatch::default()
        };
        let merged = merge_provider_matches(title_only, poster_only);
        assert!(provider_match_complete(&merged));
        assert_eq!(merged.title.as_deref(), Some("Some Scene"));
        assert_eq!(
            merged.poster_path.as_deref(),
            Some("https://example.invalid/poster.jpg")
        );
    }

    #[test]
    fn title_only_or_poster_only_matches_are_not_complete() {
        assert!(!provider_match_complete(&ProviderMatch {
            title: Some("Some Scene".to_string()),
            ..ProviderMatch::default()
        }));
        assert!(!provider_match_complete(&ProviderMatch {
            poster_path: Some("https://example.invalid/poster.jpg".to_string()),
            ..ProviderMatch::default()
        }));
        assert!(!provider_match_complete(&ProviderMatch::default()));
    }

    #[test]
    fn chapter_frame_fallback_runs_only_without_any_poster() {
        let no_poster = item_with_poster(None);
        assert!(needs_poster_fallback(&no_poster, None));
        assert!(needs_poster_fallback(&no_poster, Some("  ")));
        assert!(!needs_poster_fallback(&no_poster, Some("/media/poster.jpg")));

        let local_poster = item_with_poster(Some("/media/some-scene-poster.jpg"));
        assert!(!needs_poster_fallback(&local_poster, None));
        // A remote URL is not a local poster, so the fallback may still run
        // (it is what migrates the item to a local poster).
        let remote_poster = item_with_poster(Some("https://example.invalid/poster.jpg"));
        assert!(needs_poster_fallback(&remote_poster, None));
        assert_eq!(local_poster_path(&remote_poster), None);
        assert_eq!(
            local_poster_path(&local_poster).as_deref(),
            Some("/media/some-scene-poster.jpg")
        );
    }

    #[test]
    fn chapter_frame_fallback_promotes_the_middle_frame_as_a_sidecar() {
        let dir =
            std::env::temp_dir().join(format!("cinavault-chapter-poster-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let video = dir.join("Scene.mp4");
        fs::write(&video, b"video").expect("video should be created");
        let chapters = dir.join("Scene_chapters");
        fs::create_dir_all(&chapters).expect("chapters dir should be created");
        let first = jpeg_bytes(20, 20);
        let middle = jpeg_bytes(30, 30);
        let last = jpeg_bytes(40, 40);
        fs::write(chapters.join("chapter_0000.jpg"), &first).expect("frame should be written");
        fs::write(chapters.join("chapter_0001.jpg"), &middle).expect("frame should be written");
        fs::write(chapters.join("chapter_0002.jpg"), &last).expect("frame should be written");

        let poster = promote_chapter_frame_poster(&video.to_string_lossy())
            .expect("promotion should succeed")
            .expect("a middle frame should be promoted");
        assert!(poster.ends_with("Scene-poster.jpg"));
        assert_eq!(fs::read(&poster).unwrap(), middle);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn chapter_frame_fallback_returns_none_without_frames() {
        let dir =
            std::env::temp_dir().join(format!("cinavault-chapter-none-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let video = dir.join("Scene.mp4");
        fs::write(&video, b"video").expect("video should be created");

        assert_eq!(
            promote_chapter_frame_poster(&video.to_string_lossy())
                .expect("missing chapter dir should not be an error"),
            None
        );

        let _ = fs::remove_dir_all(dir);
    }
}
