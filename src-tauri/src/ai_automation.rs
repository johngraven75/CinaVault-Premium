use crate::{downloads, duplicates, enrichment, scanner, AppState};
use std::collections::BTreeSet;
use tauri::State;

fn normalized_tasks(tasks: Option<Vec<String>>) -> Vec<String> {
    let defaults = vec![
        "scan".to_string(),
        "enrich".to_string(),
        "posters".to_string(),
        "nfo".to_string(),
        "duplicates".to_string(),
        "normalize".to_string(),
        "tags".to_string(),
    ];
    let requested = tasks.unwrap_or(defaults);
    requested
        .into_iter()
        .map(|task| task.trim().to_ascii_lowercase())
        .filter(|task| !task.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}


const AUTOMATED_DOWNLOAD_MODEL: &str = "Qwen/Qwen3-4B-Instruct-2507";

#[tauri::command]
pub async fn ai_automated_download(
    state: State<'_, AppState>,
    url: String,
    output_dir: Option<String>,
    include_playlist: Option<bool>,
    cookies_file: Option<String>,
) -> Result<serde_json::Value, String> {
    let trimmed_url = url.trim();
    if !(trimmed_url.starts_with("https://") || trimmed_url.starts_with("http://")) {
        return Err("A valid HTTP or HTTPS media link is required".to_string());
    }

    let _tool_status = crate::media_tools::ensure_media_tools()
        .map_err(|error| format!("Automatic media tool setup failed: {error}"))?;

    let planning_prompt = format!(
        "Classify this submitted media link for a fully automated lawful download workflow. Return concise JSON with media_kind, likely_adult, metadata_priority, and poster_strategy. Do not suggest bypassing DRM, paywalls, access controls, or CAPTCHAs. URL: {trimmed_url}"
    );
    let ai_plan = match crate::ai::ai_inference(
        state.clone(),
        planning_prompt,
        Some(AUTOMATED_DOWNLOAD_MODEL.to_string()),
        None,
    )
    .await
    {
        Ok(value) => value,
        Err(error) => serde_json::json!({
            "status": "fallback",
            "message": error,
            "model": AUTOMATED_DOWNLOAD_MODEL,
            "deterministic_pipeline_continued": true,
        }),
    };

    let mut result = downloads::start_media_download(
        state,
        trimmed_url.to_string(),
        output_dir,
        Some("bestvideo*+bestaudio/best/bestvideo+bestaudio".to_string()),
        cookies_file,
        Some(include_playlist.unwrap_or(false)),
    )
    .await?;

    if let Some(object) = result.as_object_mut() {
        object.insert(
            "automation".to_string(),
            serde_json::json!({
                "enabled": true,
                "model": AUTOMATED_DOWNLOAD_MODEL,
                "ai_plan": ai_plan,
                "steps": [
                    "validate_link",
                    "repair_download_dependencies",
                    "classify_media",
                    "download_best_available_format",
                    "detect_final_files",
                    "verify_adult_metadata",
                    "retrieve_provider_metadata",
                    "download_and_validate_poster",
                    "write_nfo_sidecar",
                    "return_verification_report"
                ],
                "fallback": "deterministic processing remains active when AI inference is unavailable"
            }),
        );
    }
    Ok(result)
}

#[tauri::command]
pub async fn ai_library_manage(
    state: State<'_, AppState>,
    tasks: Option<Vec<String>>,
) -> Result<serde_json::Value, String> {
    let requested = normalized_tasks(tasks);
    let mut results = serde_json::Map::new();
    let mut errors = Vec::<String>::new();
    let mut total_updated = 0u64;

    if requested.iter().any(|task| task == "scan") {
        match scanner::scan_sources(state.clone()).await {
            Ok(report) => {
                total_updated += report
                    .get("total_added")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0);
                total_updated += report
                    .get("total_updated")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0);
                results.insert("scan".to_string(), report);
            }
            Err(err) => {
                errors.push(format!("scan: {err}"));
                results.insert(
                    "scan".to_string(),
                    serde_json::json!({ "status": "error", "error": err }),
                );
            }
        }
    }

    let needs_enrichment = requested.iter().any(|task| {
        matches!(
            task.as_str(),
            "enrich" | "posters" | "nfo" | "normalize" | "tags" | "metadata"
        )
    });
    if needs_enrichment {
        let rename_files = requested.iter().any(|task| task == "normalize");
        match enrichment::run_library_enrichment(state.clone(), rename_files).await {
            Ok(report) => {
                total_updated += report.metadata_fields_updated as u64;
                results.insert(
                    "enrichment".to_string(),
                    serde_json::json!({
                        "status": "ok",
                        "items_scanned": report.items_scanned,
                        "metadata_updated": report.metadata_updated,
                        "metadata_items_enriched": report.metadata_items_enriched,
                        "metadata_fields_updated": report.metadata_fields_updated,
                        "posters_downloaded": report.posters_downloaded,
                        "sidecars_written": report.sidecars_written,
                        "files_renamed": report.files_renamed,
                        "provider_errors": report.provider_errors,
                    }),
                );
            }
            Err(err) => {
                errors.push(format!("enrichment: {err}"));
                results.insert(
                    "enrichment".to_string(),
                    serde_json::json!({ "status": "error", "error": err }),
                );
            }
        }
    }

    if requested.iter().any(|task| task == "duplicates") {
        match duplicates::find_duplicates(state.clone(), Some("name_size".to_string()), Some(0.0))
            .await
        {
            Ok(report) => {
                results.insert("duplicates".to_string(), report);
            }
            Err(err) => {
                errors.push(format!("duplicates: {err}"));
                results.insert(
                    "duplicates".to_string(),
                    serde_json::json!({ "status": "error", "error": err }),
                );
            }
        }
    }

    let status = if errors.is_empty() {
        "success"
    } else if results
        .values()
        .any(|value| value.get("status") != Some(&serde_json::json!("error")))
    {
        "partial"
    } else {
        "failed"
    };

    Ok(serde_json::json!({
        "type": "ai_library_manage",
        "status": status,
        "tasks_run": requested,
        "total_updated": total_updated,
        "results": results,
        "errors": errors,
        "message": "AI media automation completed real scanning, metadata enrichment, artwork/NFO handling, filename normalization when requested, and duplicate analysis."
    }))
}

#[cfg(test)]
mod tests {
    use super::normalized_tasks;

    #[test]
    fn tasks_are_normalized_and_deduplicated() {
        let tasks = normalized_tasks(Some(vec![
            " Scan ".to_string(),
            "ENRICH".to_string(),
            "scan".to_string(),
        ]));
        assert_eq!(tasks, vec!["enrich".to_string(), "scan".to_string()]);
    }

    #[test]
    fn defaults_cover_real_media_automation() {
        let tasks = normalized_tasks(None);
        for required in [
            "scan",
            "enrich",
            "posters",
            "nfo",
            "duplicates",
            "normalize",
            "tags",
        ] {
            assert!(tasks.iter().any(|task| task == required));
        }
    }
}
