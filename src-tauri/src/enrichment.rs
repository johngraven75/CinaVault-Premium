use crate::AppState;
use regex::Regex;
use rusqlite::params;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::State;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameTarget {
    Ready(PathBuf, String),
    Collision(PathBuf),
    Invalid(String),
    Unchanged(PathBuf),
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
    result_type: &'static str,
    mode: &'static str,
    items_scanned: usize,
    metadata_updated: usize,
    files_renamed: usize,
    rename_collisions_skipped: usize,
    rename_failures: usize,
    missing_files_skipped: usize,
    low_confidence_skipped: usize,
    samples: Vec<EnrichmentItemSummary>,
}

#[derive(Debug)]
struct CandidateItem {
    id: i64,
    title: String,
    file_path: String,
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
    text = Regex::new(r"(?i)\[[^\]]*\]|\([^\)]*(1080p|720p|2160p|x264|x265|hevc|aac|web[- ]?dl|bluray)[^\)]*\)")
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
pub fn run_library_enrichment(
    state: State<AppState>,
    rename_files: bool,
) -> Result<LibraryEnrichmentReport, String> {
    let items = {
        let db = state.db.lock().map_err(|err| err.to_string())?;
        let mut stmt = db
            .conn
            .prepare(
                "SELECT id, title, file_path
                 FROM media_items
                 WHERE media_type IN ('movie', 'episode', 'video')
                 ORDER BY date_added DESC",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(CandidateItem {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    file_path: row.get(2)?,
                })
            })
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())?
    };

    let mut report = LibraryEnrichmentReport {
        result_type: "library_enrichment",
        mode: if rename_files { "metadata_and_filename_normalization" } else { "metadata_only" },
        items_scanned: items.len(),
        metadata_updated: 0,
        files_renamed: 0,
        rename_collisions_skipped: 0,
        rename_failures: 0,
        missing_files_skipped: 0,
        low_confidence_skipped: 0,
        samples: Vec::new(),
    };

    for item in items {
        let normalized = normalize_filename_title(&item.file_path);
        if normalized.is_empty() {
            report.low_confidence_skipped += 1;
            continue;
        }

        let title_needs_update = should_update_title(&item.title, &normalized, &item.file_path);
        if title_needs_update {
            let db = state.db.lock().map_err(|err| err.to_string())?;
            db.conn
                .execute(
                    "UPDATE media_items SET title = ?1 WHERE id = ?2",
                    params![normalized, item.id],
                )
                .map_err(|err| err.to_string())?;
            report.metadata_updated += 1;
            push_sample(
                &mut report.samples,
                item.id,
                &item.title,
                &normalized,
                &item.file_path,
                "metadata_updated",
            );
        }

        if !rename_files {
            continue;
        }

        let source = Path::new(&item.file_path);
        if !source.exists() {
            report.missing_files_skipped += 1;
            continue;
        }
        if !title_needs_update && !titles_match(&item.title, &normalized) {
            report.low_confidence_skipped += 1;
            continue;
        }

        match safe_rename_target(source, &normalized) {
            RenameTarget::Ready(target, cleaned_title) => {
                match std::fs::rename(source, &target) {
                    Ok(()) => {
                        let new_path = target.to_string_lossy().to_string();
                        let db = state.db.lock().map_err(|err| err.to_string())?;
                        db.conn
                            .execute(
                                "UPDATE media_items SET file_path = ?1, title = ?2 WHERE id = ?3",
                                params![new_path, cleaned_title, item.id],
                            )
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
                }
            }
            RenameTarget::Collision(_) => report.rename_collisions_skipped += 1,
            RenameTarget::Invalid(_) | RenameTarget::Unchanged(_) => {}
        }
    }

    Ok(report)
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

fn sanitize_windows_filename(value: &str) -> String {
    value
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
        .to_string()
}

fn title_case(value: &str) -> String {
    value
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(_) if word.chars().all(|ch| ch.is_ascii_uppercase() || !ch.is_ascii_alphabetic()) => word.to_string(),
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
    let filename_title = Path::new(file_path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().replace(['.', '_', '-'], " "))
        .unwrap_or_default();
    current.eq_ignore_ascii_case(filename_title.trim()) || current.contains('.') || current.contains('_')
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

#[cfg(test)]
mod tests {
    use super::{normalize_filename_title, safe_rename_target, RenameTarget};
    use std::fs;

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
    fn safe_rename_target_blocks_existing_destination() {
        let dir = std::env::temp_dir().join(format!("cinavault-enrichment-{}", uuid::Uuid::new_v4()));
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
