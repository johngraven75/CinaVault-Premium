use std::path::{Path, PathBuf};

const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "svg"];
const POSTER_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp"];
const EXACT_ARTWORK_STEMS: &[&str] = &[
    "poster",
    "cover",
    "folder",
    "default",
    "fanart",
    "backdrop",
    "landscape",
    "banner",
    "thumb",
    "thumbnail",
];
const ARTWORK_SUFFIXES: &[&str] = &[
    "-poster",
    "_poster",
    ".poster",
    "-cover",
    "_cover",
    ".cover",
    "-folder",
    "_folder",
    ".folder",
    "-fanart",
    "_fanart",
    ".fanart",
    "-backdrop",
    "_backdrop",
    ".backdrop",
    "-landscape",
    "_landscape",
    ".landscape",
    "-banner",
    "_banner",
    ".banner",
    "-thumb",
    "_thumb",
    ".thumb",
    "-thumbnail",
    "_thumbnail",
    ".thumbnail",
];
const VIDEO_EXTS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "ts", "m2ts", "vob",
    "ogv", "3gp", "divx", "rm", "rmvb", "asf",
];
const METADATA_POSTER_KEYS: &[&str] = &[
    "poster_path",
    "poster",
    "poster_url",
    "image_url",
    "image",
    "cover_path",
    "cover",
    "thumbnail_path",
    "thumbnail",
    "backdrop_path",
];

fn extension_lower(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

pub fn is_generated_chapter_image_path(path: &Path) -> bool {
    let path_lower = path.to_string_lossy().replace('/', "\\").to_lowercase();
    path_lower.contains("_chapters\\chapter_")
}

pub fn is_sidecar_artwork_image(path: &Path) -> bool {
    let Some(ext) = extension_lower(path) else {
        return false;
    };
    if !IMAGE_EXTS.contains(&ext.as_str()) {
        return false;
    }

    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.trim().to_ascii_lowercase())
        .unwrap_or_default();

    EXACT_ARTWORK_STEMS.contains(&stem.as_str())
        || ARTWORK_SUFFIXES.iter().any(|suffix| stem.ends_with(suffix))
}

pub fn is_artwork_image_for_nearby_media(path: &Path) -> bool {
    let Some(ext) = extension_lower(path) else {
        return false;
    };
    if !IMAGE_EXTS.contains(&ext.as_str()) {
        return false;
    }

    let Some(parent) = path.parent() else {
        return false;
    };
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    let stem = stem.trim();
    if stem.is_empty() {
        return false;
    }

    let media_stem = ARTWORK_SUFFIXES
        .iter()
        .find_map(|suffix| stem.strip_suffix(suffix))
        .unwrap_or(stem)
        .trim();
    if media_stem.is_empty() {
        return false;
    }

    VIDEO_EXTS
        .iter()
        .any(|media_ext| parent.join(format!("{media_stem}.{media_ext}")).exists())
}

fn metadata_sidecar_path_for_media(media_path: &Path) -> Option<PathBuf> {
    let parent = media_path.parent()?;
    let stem = media_path.file_stem()?.to_str()?.trim();
    if stem.is_empty() {
        return None;
    }
    Some(parent.join(format!("{stem}.cinavault.json")))
}

fn is_passthrough_artwork_path(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("data:")
        || lower.starts_with("asset:")
}

fn existing_path_string(path: PathBuf) -> String {
    let value = std::fs::canonicalize(&path)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    if let Some(stripped) = value.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{stripped}")
    } else if let Some(stripped) = value.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        value
    }
}

fn metadata_poster_path_for_media(media_path: &Path) -> Option<String> {
    let sidecar_path = metadata_sidecar_path_for_media(media_path)?;
    let parent = media_path.parent()?;
    let body = std::fs::read_to_string(sidecar_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&body).ok()?;

    for key in METADATA_POSTER_KEYS {
        let Some(raw) = value.get(key).and_then(|value| value.as_str()) else {
            continue;
        };
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        if is_passthrough_artwork_path(raw) {
            return Some(raw.to_string());
        }

        let candidate = Path::new(raw);
        if candidate.is_absolute() && candidate.exists() {
            return Some(existing_path_string(candidate.to_path_buf()));
        }

        let relative_candidate = parent.join(candidate);
        if relative_candidate.exists() {
            return Some(existing_path_string(relative_candidate));
        }
    }

    None
}

fn local_poster_path_for_video(video_path: &Path) -> Option<PathBuf> {
    let parent = video_path.parent()?;
    let stem = video_path.file_stem()?.to_str()?.trim();
    if stem.is_empty() {
        return None;
    }

    let stem_candidates = [
        "-poster",
        "_poster",
        ".poster",
        "-cover",
        "_cover",
        ".cover",
        "-folder",
        "_folder",
        ".folder",
        "-thumb",
        "_thumb",
        ".thumb",
        "-thumbnail",
        "_thumbnail",
        ".thumbnail",
        "-fanart",
        "_fanart",
        ".fanart",
        "-backdrop",
        "_backdrop",
        ".backdrop",
    ];
    for suffix in stem_candidates {
        for ext in POSTER_EXTS {
            let candidate = parent.join(format!("{stem}{suffix}.{ext}"));
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    for ext in POSTER_EXTS {
        let candidate = parent.join(format!("{stem}.{ext}"));
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let generic_candidates = [
        "poster",
        "cover",
        "folder",
        "default",
        "thumb",
        "thumbnail",
        "fanart",
        "backdrop",
    ];
    for name in generic_candidates {
        for ext in POSTER_EXTS {
            let candidate = parent.join(format!("{name}.{ext}"));
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    None
}

pub fn available_poster_path_for_media(media_path: &Path) -> Option<String> {
    metadata_poster_path_for_media(media_path).or_else(|| {
        sidecar_poster_path_for_video(media_path).map(|path| path.to_string_lossy().to_string())
    })
}

pub fn sidecar_poster_path_for_video(video_path: &Path) -> Option<PathBuf> {
    local_poster_path_for_video(video_path)
}

#[cfg(test)]
mod tests {
    use super::available_poster_path_for_media;
    use std::fs;

    #[test]
    fn finds_same_stem_poster_image_next_to_video() {
        let dir = std::env::temp_dir().join(format!(
            "cinavault-artifacts-same-stem-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).expect("test dir should be created");
        let video_path = dir.join("Scene Name.mp4");
        let poster_path = dir.join("Scene Name.jpg");
        fs::write(&video_path, b"video").expect("video should exist");
        fs::write(&poster_path, b"poster").expect("poster should exist");

        assert_eq!(
            available_poster_path_for_media(&video_path).as_deref(),
            Some(poster_path.to_string_lossy().as_ref())
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn finds_poster_from_cinavault_metadata_sidecar() {
        let dir = std::env::temp_dir().join(format!(
            "cinavault-artifacts-sidecar-json-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).expect("test dir should be created");
        let video_path = dir.join("Scene Name.mp4");
        let poster_path = dir.join("artwork").join("poster.webp");
        fs::create_dir_all(poster_path.parent().unwrap()).expect("artwork dir should exist");
        fs::write(&video_path, b"video").expect("video should exist");
        fs::write(&poster_path, b"poster").expect("poster should exist");
        fs::write(
            dir.join("Scene Name.cinavault.json"),
            r#"{"poster_path":"artwork/poster.webp"}"#,
        )
        .expect("metadata sidecar should exist");

        assert_eq!(
            available_poster_path_for_media(&video_path).as_deref(),
            Some(poster_path.to_string_lossy().as_ref())
        );

        let _ = fs::remove_dir_all(dir);
    }
}
