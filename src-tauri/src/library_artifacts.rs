use std::path::{Path, PathBuf};

const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "svg"];
const POSTER_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp"];
const MEDIA_EXTS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "ts", "m2ts",
    "vob", "ogv", "3gp", "divx", "rm", "rmvb", "asf", "mp3", "flac", "aac", "ogg", "wma",
    "wav", "m4a", "opus", "alac", "aiff",
];
const EXACT_ARTWORK_STEMS: &[&str] = &[
    "poster",
    "cover",
    "folder",
    "default",
    "fanart",
    "backdrop",
    "landscape",
    "banner",
    "clearart",
    "clearlogo",
    "logo",
    "disc",
    "cdart",
    "thumb",
    "thumbnail",
    "scene-poster",
    "metadata-poster",
];
const ARTWORK_SUFFIXES: &[&str] = &[
    "-poster", "_poster", ".poster",
    "-cover", "_cover", ".cover",
    "-folder", "_folder", ".folder",
    "-fanart", "_fanart", ".fanart",
    "-backdrop", "_backdrop", ".backdrop",
    "-landscape", "_landscape", ".landscape",
    "-banner", "_banner", ".banner",
    "-clearart", "_clearart", ".clearart",
    "-clearlogo", "_clearlogo", ".clearlogo",
    "-logo", "_logo", ".logo",
    "-disc", "_disc", ".disc",
    "-cdart", "_cdart", ".cdart",
    "-thumb", "_thumb", ".thumb",
    "-thumbnail", "_thumbnail", ".thumbnail",
    "-metadata-poster", "_metadata_poster", ".metadata-poster",
];

fn extension_lower(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

fn stem_text(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.trim().to_ascii_lowercase())
        .unwrap_or_default()
}

fn is_image_path(path: &Path) -> bool {
    extension_lower(path)
        .map(|ext| IMAGE_EXTS.contains(&ext.as_str()))
        .unwrap_or(false)
}

fn sibling_media_file_with_stem(path: &Path, stem: &str) -> Option<PathBuf> {
    let parent = path.parent()?;
    for ext in MEDIA_EXTS {
        let candidate = parent.join(format!("{stem}.{ext}"));
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

pub fn is_generated_chapter_image_path(path: &Path) -> bool {
    let path_lower = path.to_string_lossy().replace('/', "\\").to_lowercase();
    path_lower.contains("_chapters\\chapter_")
        || path_lower.contains("cinavault\\embedded-posters\\")
        || path_lower.contains("cinavault/embedded-posters/")
}

pub fn media_file_for_artwork_image(path: &Path) -> Option<PathBuf> {
    if !is_image_path(path) {
        return None;
    }
    let stem = stem_text(path);
    if stem.is_empty() {
        return None;
    }

    if let Some(media_file) = sibling_media_file_with_stem(path, &stem) {
        return Some(media_file);
    }

    for suffix in ARTWORK_SUFFIXES {
        if let Some(media_stem) = stem.strip_suffix(suffix) {
            if !media_stem.trim().is_empty() {
                if let Some(media_file) = sibling_media_file_with_stem(path, media_stem) {
                    return Some(media_file);
                }
            }
        }
    }

    None
}

pub fn is_sidecar_artwork_image(path: &Path) -> bool {
    if !is_image_path(path) {
        return false;
    }

    let stem = stem_text(path);
    if EXACT_ARTWORK_STEMS.contains(&stem.as_str()) {
        return true;
    }

    if ARTWORK_SUFFIXES.iter().any(|suffix| stem.ends_with(suffix)) {
        return true;
    }

    media_file_for_artwork_image(path).is_some()
}

pub fn sidecar_poster_path_for_video(video_path: &Path) -> Option<PathBuf> {
    let parent = video_path.parent()?;
    let stem = video_path.file_stem()?.to_str()?.trim();
    if stem.is_empty() {
        return None;
    }

    for ext in POSTER_EXTS {
        let candidate = parent.join(format!("{stem}.{ext}"));
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let stem_candidates = [
        "-poster", "_poster", ".poster",
        "-cover", "_cover", ".cover",
        "-folder", "_folder", ".folder",
        "-fanart", "_fanart", ".fanart",
        "-backdrop", "_backdrop", ".backdrop",
        "-landscape", "_landscape", ".landscape",
        "-banner", "_banner", ".banner",
        "-thumb", "_thumb", ".thumb",
        "-thumbnail", "_thumbnail", ".thumbnail",
        "-metadata-poster", "_metadata_poster", ".metadata-poster",
    ];
    for suffix in stem_candidates {
        for ext in POSTER_EXTS {
            let candidate = parent.join(format!("{stem}{suffix}.{ext}"));
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    let generic_candidates = ["poster", "cover", "folder", "fanart", "backdrop", "landscape", "banner", "thumb", "thumbnail"];
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

#[cfg(test)]
mod tests {
    use super::{is_sidecar_artwork_image, media_file_for_artwork_image, sidecar_poster_path_for_video};
    use std::fs;
    use std::path::Path;

    fn unique_test_dir(name: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos();
        std::env::temp_dir().join(format!("cinavault-{name}-{nonce}"))
    }

    #[test]
    fn exact_stem_image_next_to_video_is_artwork_not_photo_media() {
        let dir = unique_test_dir("exact-stem-poster");
        fs::create_dir_all(&dir).expect("test dir should be created");
        let video = dir.join("Scene Name.mkv");
        let poster = dir.join("Scene Name.jpg");
        fs::write(&video, b"video").expect("video marker should be written");
        fs::write(&poster, b"poster").expect("poster marker should be written");

        assert!(is_sidecar_artwork_image(&poster));
        assert_eq!(media_file_for_artwork_image(&poster).as_deref(), Some(video.as_path()));
        assert_eq!(sidecar_poster_path_for_video(&video).as_deref(), Some(poster.as_path()));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn regular_photo_without_media_sibling_is_not_artwork() {
        let photo = Path::new(r"E:\Photos\Vacation\beach-day.jpg");
        assert!(!is_sidecar_artwork_image(photo));
    }
}
