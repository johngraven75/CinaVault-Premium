//! Photo portrait for the AI agent's head.
//!
//! The user picks a photo of themselves and marks both eyes and the mouth.
//! The photo is decoded, scaled to at most `MAX_EDGE` pixels and re-encoded
//! as JPEG (which also drops EXIF data such as GPS location), then kept with
//! the markers in `<app data>/agent-avatar/`. Nothing is bundled with the app
//! and nothing leaves the machine; the front end animates the photo locally.

use std::path::{Path, PathBuf};

use base64::Engine;
use image::imageops::FilterType;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const DIRECTORY: &str = "agent-avatar";
const PHOTO_FILE: &str = "portrait.jpg";
const CALIBRATION_FILE: &str = "calibration.json";
/// Longest edge of the stored photo; plenty for a panel a few hundred pixels tall.
const MAX_EDGE: u32 = 1280;
/// Largest photo accepted from the WebView, before decoding.
const MAX_UPLOAD_BYTES: usize = 12 * 1024 * 1024;
const JPEG_QUALITY: u8 = 90;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PortraitPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortraitCalibration {
    pub left_eye: PortraitPoint,
    pub right_eye: PortraitPoint,
    pub mouth: PortraitPoint,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAvatar {
    /// The stored photo as a `data:image/jpeg;base64,…` URL.
    pub data_url: String,
    pub calibration: PortraitCalibration,
}

/// Mirrors `calibrationError` in src/services/portraitAvatar.ts, measured in
/// square units so a portrait or landscape photo is judged the same way.
pub fn validate_calibration(cal: &PortraitCalibration, aspect: f64) -> Result<(), String> {
    let points = [cal.left_eye, cal.right_eye, cal.mouth];
    if points
        .iter()
        .any(|p| !p.x.is_finite() || !p.y.is_finite() || !(0.0..=1.0).contains(&p.x) || !(0.0..=1.0).contains(&p.y))
    {
        return Err("Keep the markers on the photo.".into());
    }
    if cal.left_eye.x >= cal.right_eye.x {
        return Err("The left eye marker must be left of the right eye marker.".into());
    }
    let ex = (cal.right_eye.x - cal.left_eye.x) * aspect;
    let ey = cal.right_eye.y - cal.left_eye.y;
    let eye_distance = ex.hypot(ey);
    if eye_distance < 0.04 {
        return Err("Move the eye markers apart, onto each eye.".into());
    }
    let (down_x, down_y) = (-ey / eye_distance, ex / eye_distance);
    let mid_x = (cal.left_eye.x + cal.right_eye.x) / 2.0;
    let mid_y = (cal.left_eye.y + cal.right_eye.y) / 2.0;
    let mouth_distance = (cal.mouth.x - mid_x) * aspect * down_x + (cal.mouth.y - mid_y) * down_y;
    if mouth_distance < eye_distance * 0.45 {
        return Err("Move the mouth marker down onto the mouth.".into());
    }
    if mouth_distance > eye_distance * 2.5 {
        return Err("The mouth marker is too far from the eyes.".into());
    }
    Ok(())
}

/// Decodes an uploaded photo, scales it down and re-encodes it as JPEG.
/// Returns the JPEG bytes and the photo's width/height ratio.
pub fn normalize_photo(bytes: &[u8]) -> Result<(Vec<u8>, f64), String> {
    if bytes.is_empty() {
        return Err("That photo is empty.".into());
    }
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err("Photos must be under 12 MB.".into());
    }
    let format = image::guess_format(bytes).map_err(|_| "Choose a JPEG, PNG or WebP photo.".to_string())?;
    if !matches!(format, image::ImageFormat::Jpeg | image::ImageFormat::Png | image::ImageFormat::WebP) {
        return Err("Choose a JPEG, PNG or WebP photo.".into());
    }
    let decoded = image::load_from_memory_with_format(bytes, format)
        .map_err(|error| format!("That photo could not be read: {error}"))?;
    if decoded.width() < 64 || decoded.height() < 64 {
        return Err("That photo is too small; use one at least 64 pixels across.".into());
    }
    let scaled = if decoded.width().max(decoded.height()) > MAX_EDGE {
        decoded.resize(MAX_EDGE, MAX_EDGE, FilterType::Lanczos3)
    } else {
        decoded
    };
    let rgb = scaled.to_rgb8();
    let aspect = f64::from(rgb.width()) / f64::from(rgb.height());
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY)
        .encode_image(&rgb)
        .map_err(|error| format!("The photo could not be saved: {error}"))?;
    Ok((jpeg, aspect))
}

fn directory(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("unable to resolve app data directory: {error}"))?
        .join(DIRECTORY);
    std::fs::create_dir_all(&dir).map_err(|error| format!("unable to create the portrait folder: {error}"))?;
    Ok(dir)
}

fn photo_aspect(path: &Path) -> Result<f64, String> {
    let (width, height) = image::image_dimensions(path).map_err(|error| format!("unable to read the portrait: {error}"))?;
    Ok(f64::from(width) / f64::from(height.max(1)))
}

fn read_avatar(dir: &Path) -> Result<Option<AgentAvatar>, String> {
    let photo = dir.join(PHOTO_FILE);
    let calibration = dir.join(CALIBRATION_FILE);
    if !photo.is_file() || !calibration.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(&photo).map_err(|error| format!("unable to read the portrait: {error}"))?;
    let text = std::fs::read_to_string(&calibration).map_err(|error| format!("unable to read the portrait markers: {error}"))?;
    let calibration: PortraitCalibration =
        serde_json::from_str(&text).map_err(|error| format!("the portrait markers are damaged: {error}"))?;
    Ok(Some(AgentAvatar {
        data_url: format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)),
        calibration,
    }))
}

fn write_calibration(dir: &Path, calibration: &PortraitCalibration) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(calibration).map_err(|error| error.to_string())?;
    crate::atomic_file::write_verified_atomic(&dir.join(CALIBRATION_FILE), &json)
}

/// The saved portrait, or `None` when the user hasn't set one up.
#[tauri::command]
pub fn agent_avatar_get(app: AppHandle) -> Result<Option<AgentAvatar>, String> {
    read_avatar(&directory(&app)?)
}

/// Stores a new photo (base64, without the `data:` prefix) and its markers.
#[tauri::command]
pub fn agent_avatar_save(app: AppHandle, photo: String, calibration: PortraitCalibration) -> Result<AgentAvatar, String> {
    // Base64 is a third larger than the bytes it carries.
    if photo.len() > MAX_UPLOAD_BYTES / 3 * 4 + 4 {
        return Err("Photos must be under 12 MB.".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(photo.trim())
        .map_err(|_| "That photo could not be read.".to_string())?;
    let (jpeg, aspect) = normalize_photo(&bytes)?;
    validate_calibration(&calibration, aspect)?;
    let dir = directory(&app)?;
    crate::atomic_file::write_verified_atomic(&dir.join(PHOTO_FILE), &jpeg)?;
    write_calibration(&dir, &calibration)?;
    read_avatar(&dir)?.ok_or_else(|| "The portrait could not be saved.".to_string())
}

/// Moves the eye and mouth markers on the photo already saved.
#[tauri::command]
pub fn agent_avatar_set_calibration(app: AppHandle, calibration: PortraitCalibration) -> Result<AgentAvatar, String> {
    let dir = directory(&app)?;
    let photo = dir.join(PHOTO_FILE);
    if !photo.is_file() {
        return Err("Choose a photo first.".into());
    }
    validate_calibration(&calibration, photo_aspect(&photo)?)?;
    write_calibration(&dir, &calibration)?;
    read_avatar(&dir)?.ok_or_else(|| "The portrait could not be saved.".to_string())
}

/// Deletes the photo and markers; the agent goes back to the hologram head.
#[tauri::command]
pub fn agent_avatar_clear(app: AppHandle) -> Result<(), String> {
    let dir = directory(&app)?;
    for name in [PHOTO_FILE, CALIBRATION_FILE] {
        let path = dir.join(name);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|error| format!("unable to remove the portrait: {error}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> PortraitPoint {
        PortraitPoint { x, y }
    }

    fn cal(left: (f64, f64), right: (f64, f64), mouth: (f64, f64)) -> PortraitCalibration {
        PortraitCalibration {
            left_eye: point(left.0, left.1),
            right_eye: point(right.0, right.1),
            mouth: point(mouth.0, mouth.1),
        }
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = image::RgbImage::from_fn(width, height, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    #[test]
    fn accepts_a_typical_portrait() {
        assert!(validate_calibration(&cal((0.4, 0.4), (0.6, 0.4), (0.5, 0.6)), 1.0).is_ok());
        // A tall full-body photo: eyes close together in x, scaled by the aspect.
        assert!(validate_calibration(&cal((0.425, 0.252), (0.575, 0.252), (0.5, 0.345)), 1000.0 / 1500.0).is_ok());
    }

    #[test]
    fn rejects_swapped_crowded_or_misplaced_markers() {
        assert!(validate_calibration(&cal((0.6, 0.4), (0.4, 0.4), (0.5, 0.6)), 1.0).unwrap_err().contains("left of"));
        assert!(validate_calibration(&cal((0.49, 0.4), (0.51, 0.4), (0.5, 0.6)), 1.0).unwrap_err().contains("apart"));
        assert!(validate_calibration(&cal((0.4, 0.4), (0.6, 0.4), (0.5, 0.41)), 1.0).unwrap_err().contains("down onto"));
        assert!(validate_calibration(&cal((0.4, 0.1), (0.6, 0.1), (0.5, 0.95)), 1.0).unwrap_err().contains("too far"));
        assert!(validate_calibration(&cal((0.4, 0.4), (1.2, 0.4), (0.5, 0.6)), 1.0).unwrap_err().contains("on the photo"));
        assert!(validate_calibration(&cal((f64::NAN, 0.4), (0.6, 0.4), (0.5, 0.6)), 1.0).is_err());
    }

    #[test]
    fn normalizes_large_photos_to_a_bounded_jpeg() {
        let (jpeg, aspect) = normalize_photo(&png(2000, 3000)).unwrap();
        assert_eq!(image::guess_format(&jpeg).unwrap(), image::ImageFormat::Jpeg);
        let decoded = image::load_from_memory(&jpeg).unwrap();
        assert_eq!(decoded.height(), MAX_EDGE);
        assert!((aspect - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn keeps_small_photos_at_their_size_and_rejects_junk() {
        let (jpeg, _) = normalize_photo(&png(320, 400)).unwrap();
        let decoded = image::load_from_memory(&jpeg).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (320, 400));
        assert!(normalize_photo(&[]).is_err());
        assert!(normalize_photo(b"not an image at all").is_err());
        assert!(normalize_photo(&png(32, 32)).unwrap_err().contains("too small"));
    }

    #[test]
    fn round_trips_a_saved_avatar() {
        let dir = std::env::temp_dir().join(format!("cv-avatar-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(read_avatar(&dir).unwrap().is_none());
        let (jpeg, _) = normalize_photo(&png(200, 200)).unwrap();
        std::fs::write(dir.join(PHOTO_FILE), &jpeg).unwrap();
        let calibration = cal((0.4, 0.4), (0.6, 0.4), (0.5, 0.6));
        write_calibration(&dir, &calibration).unwrap();
        let avatar = read_avatar(&dir).unwrap().unwrap();
        assert!(avatar.data_url.starts_with("data:image/jpeg;base64,"));
        assert_eq!(avatar.calibration, calibration);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
