// CinaVault Premium — Chapter Thumbnail Generation
use serde::{Deserialize, Serialize};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn command_output(cmd: &mut Command) -> Result<std::process::Output, std::io::Error> {
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.output()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChapterThumb {
    pub timestamp: f64,
    pub path: String,
    pub label: String,
}

const INTERVAL_FILE: &str = ".interval";

/// Thumbnail folder for a video: `<folder>/<name>_chapters`.
pub fn default_chapter_dir(file_path: &str) -> String {
    let p = Path::new(file_path);
    let parent = p.parent().unwrap_or(Path::new("."));
    let stem = p.file_stem().unwrap_or_default().to_string_lossy();
    parent
        .join(format!("{}_chapters", stem))
        .to_string_lossy()
        .to_string()
}

fn tool_path(name: &str) -> String {
    crate::media_tools::resolve_executable(name)
        .to_string_lossy()
        .to_string()
}

/// One frame every `interval` seconds into `out_dir`, via ffprobe + ffmpeg.
pub fn generate_thumbs_blocking(
    file_path: &str,
    out_dir: &str,
    interval: u64,
    ffmpeg: &str,
) -> Result<Vec<ChapterThumb>, String> {
    let interval = interval.max(10);
    let ffprobe_out = command_output(Command::new(tool_path("ffprobe")).args([
        "-v",
        "error",
        "-show_entries",
        "format=duration",
        "-of",
        "csv=p=0",
        file_path,
    ]))
    .map_err(|e| format!("ffprobe failed: {}", e))?;

    let duration_str = String::from_utf8_lossy(&ffprobe_out.stdout);
    let duration: f64 = duration_str.trim().parse().unwrap_or(0.0);

    if duration <= 0.0 {
        return Err("Could not determine video duration".into());
    }

    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    // get_chapter_thumbs reads this back to label the frames.
    std::fs::write(Path::new(out_dir).join(INTERVAL_FILE), interval.to_string()).ok();

    let mut thumbs = Vec::new();
    let mut t = 0.0f64;
    let mut idx = 0;

    while t < duration {
        let out_path = Path::new(out_dir)
            .join(format!("chapter_{:04}.jpg", idx))
            .to_string_lossy()
            .to_string();
        let timestamp = format!("{:.2}", t);

        let result = command_output(Command::new(ffmpeg).args([
            "-ss", &timestamp, "-i", file_path, "-vframes", "1", "-q:v", "3", "-y", &out_path,
        ]));

        if let Ok(output) = result {
            if output.status.success() {
                thumbs.push(ChapterThumb {
                    timestamp: t,
                    path: out_path,
                    label: time_label(t),
                });
            }
        }

        t += interval as f64;
        idx += 1;
    }

    Ok(thumbs)
}

fn time_label(t: f64) -> String {
    let hours = (t as u64) / 3600;
    let mins = ((t as u64) % 3600) / 60;
    let secs = (t as u64) % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, secs)
}

fn has_thumbs(dir: &str) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .any(|entry| entry.file_name().to_string_lossy().starts_with("chapter_"))
        })
        .unwrap_or(false)
}

/// Interval for automatic generation, from the switch config (minutes).
pub fn configured_interval_secs(db: &crate::db::Database) -> u64 {
    crate::feature_flags::config(db, "chapter_thumbs")
        .get("intervalMinutes")
        .and_then(serde_json::Value::as_f64)
        .filter(|minutes| minutes.is_finite() && *minutes > 0.0)
        .map(|minutes| (minutes * 60.0).round() as u64)
        .unwrap_or(300)
        .clamp(30, 3600)
}

/// After a scan: thumbnails for each new video that has none yet.
pub async fn generate_for_new_items(
    items: Vec<(i64, String)>,
    interval: u64,
    progress: &mut crate::task_progress::MetadataTaskGuard,
    offset: usize,
) -> serde_json::Value {
    let mut generated = 0usize;
    let mut skipped = 0usize;
    let mut errors = Vec::new();
    let ffmpeg = tool_path("ffmpeg");
    for (index, (_, file_path)) in items.iter().enumerate() {
        if crate::task_progress::stop_requested() {
            break;
        }
        progress.update(offset + index, format!("Chapter thumbnails: {file_path}"));
        let dir = default_chapter_dir(file_path);
        if has_thumbs(&dir) || !Path::new(file_path).is_file() {
            skipped += 1;
            continue;
        }
        let (path, out, ff) = (file_path.clone(), dir.clone(), ffmpeg.clone());
        let result = tokio::task::spawn_blocking(move || {
            generate_thumbs_blocking(&path, &out, interval, &ff)
        })
        .await
        .map_err(|error| error.to_string())
        .and_then(|result| result);
        match result {
            Ok(thumbs) if !thumbs.is_empty() => generated += 1,
            Ok(_) => errors.push(format!("{file_path}: no frames could be extracted")),
            Err(error) => errors.push(format!("{file_path}: {error}")),
        }
    }
    serde_json::json!({
        "generated": generated,
        "skipped": skipped,
        "errors": errors.into_iter().take(20).collect::<Vec<_>>(),
    })
}

#[tauri::command]
pub async fn generate_chapter_thumbs(
    file_path: String,
    output_dir: Option<String>,
    interval_secs: Option<u64>,
    ffmpeg_path: Option<String>,
) -> Result<Vec<ChapterThumb>, String> {
    let ffmpeg = ffmpeg_path.unwrap_or_else(|| tool_path("ffmpeg"));
    let interval = interval_secs.unwrap_or(300); // 5 minutes default
    let out_dir = output_dir.unwrap_or_else(|| default_chapter_dir(&file_path));
    tokio::task::spawn_blocking(move || {
        generate_thumbs_blocking(&file_path, &out_dir, interval, &ffmpeg)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn get_chapter_thumbs(chapter_dir: String) -> Result<Vec<ChapterThumb>, String> {
    let dir = Path::new(&chapter_dir);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut thumbs = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "jpg" || ext == "png")
                .unwrap_or(false)
        })
        .collect();

    entries.sort_by_key(|e| e.file_name());

    let interval = std::fs::read_to_string(dir.join(INTERVAL_FILE))
        .ok()
        .and_then(|raw| raw.trim().parse::<f64>().ok())
        .filter(|value| *value > 0.0)
        .unwrap_or(300.0);
    for (i, entry) in entries.iter().enumerate() {
        let timestamp = (i as f64) * interval;
        thumbs.push(ChapterThumb {
            timestamp,
            path: entry.path().to_string_lossy().to_string(),
            label: time_label(timestamp),
        });
    }

    Ok(thumbs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_folder_sits_next_to_the_video() {
        let dir = default_chapter_dir("/media/Heat (1995).mkv");
        assert_eq!(Path::new(&dir), Path::new("/media/Heat (1995)_chapters"));
        assert!(crate::library_artifacts::is_generated_chapter_image_path(
            &Path::new(&dir).join("chapter_0000.jpg")
        ));
        assert_eq!(time_label(3725.0), "01:02:05");
    }

    #[test]
    fn interval_comes_from_the_switch_config() {
        let db = crate::db::Database::new(":memory:").unwrap();
        assert_eq!(configured_interval_secs(&db), 300);
        db.set_feature_setting_data("chapter_thumbs", true, r#"{"intervalMinutes":2}"#)
            .unwrap();
        assert_eq!(configured_interval_secs(&db), 120);
        db.set_feature_setting_data("chapter_thumbs", true, r#"{"intervalMinutes":0.1}"#)
            .unwrap();
        assert_eq!(configured_interval_secs(&db), 30);
    }
}
