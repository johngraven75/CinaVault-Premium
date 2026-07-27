use reqwest::header::CONTENT_TYPE;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const MAX_ARTWORK_BYTES: usize = 25 * 1024 * 1024;
static CACHE_ROOT: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct CachedArtwork {
    pub path: String,
    pub mime_type: String,
    pub byte_length: usize,
    pub sha256: String,
    pub source_provider: String,
}

pub fn configure(cache_root: PathBuf) {
    let _ = std::fs::create_dir_all(&cache_root);
    let _ = CACHE_ROOT.set(cache_root);
}

fn configured_root() -> Result<PathBuf, String> {
    CACHE_ROOT
        .get()
        .cloned()
        .ok_or_else(|| "Artwork cache root is not initialized".to_string())
}

fn image_type(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(("image/jpeg", "jpg"));
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(("image/png", "png"));
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some(("image/webp", "webp"));
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(("image/gif", "gif"));
    }
    None
}

pub fn validate_image_bytes(bytes: &[u8]) -> Result<(&'static str, &'static str), String> {
    if bytes.is_empty() {
        return Err("Artwork response was empty".to_string());
    }
    if bytes.len() > MAX_ARTWORK_BYTES {
        return Err(format!("Artwork exceeds the {} byte limit", MAX_ARTWORK_BYTES));
    }
    image_type(bytes).ok_or_else(|| "Artwork response is not a supported image".to_string())
}

fn cache_key(media_identity: &str, kind: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"cinavault-artwork-cache-v1");
    hasher.update(media_identity.as_bytes());
    hasher.update(b"\0");
    hasher.update(kind.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn cache_path(root: &Path, media_identity: &str, kind: &str, extension: &str) -> PathBuf {
    root.join(format!("{}-{kind}.{extension}", cache_key(media_identity, kind)))
}

pub async fn download_to_cache(
    client: &reqwest::Client,
    source_url: &str,
    media_identity: &str,
    kind: &str,
    source_provider: &str,
) -> Result<CachedArtwork, String> {
    if !source_url.starts_with("https://") {
        return Err("Artwork providers must use HTTPS".to_string());
    }
    let root = configured_root()?;
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;

    let response = client
        .get(source_url)
        .header("Accept", "image/avif,image/webp,image/apng,image/*,*/*;q=0.8")
        .header("User-Agent", "CinaVault/2.0")
        .send()
        .await
        .map_err(|error| format!("Artwork fetch failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Artwork fetch failed: {error}"))?;

    if response
        .content_length()
        .is_some_and(|length| length > MAX_ARTWORK_BYTES as u64)
    {
        return Err("Artwork response exceeds the configured limit".to_string());
    }

    let advertised_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or(value).trim().to_ascii_lowercase());
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("Artwork read failed: {error}"))?;
    let (mime_type, extension) = validate_image_bytes(&bytes)?;
    if advertised_type
        .as_deref()
        .is_some_and(|value| !value.starts_with("image/"))
    {
        return Err("Artwork server returned a non-image content type".to_string());
    }

    let final_path = cache_path(&root, media_identity, kind, extension);
    let temporary_path = final_path.with_extension(format!("{extension}.part"));
    std::fs::write(&temporary_path, &bytes)
        .map_err(|error| format!("Artwork cache write failed: {error}"))?;
    std::fs::rename(&temporary_path, &final_path)
        .map_err(|error| format!("Artwork cache finalize failed: {error}"))?;

    let sha256 = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();

    Ok(CachedArtwork {
        path: final_path.to_string_lossy().to_string(),
        mime_type: mime_type.to_string(),
        byte_length: bytes.len(),
        sha256,
        source_provider: source_provider.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{cache_key, validate_image_bytes};

    #[test]
    fn image_validator_accepts_real_signatures() {
        assert_eq!(validate_image_bytes(&[0xFF, 0xD8, 0xFF, 0x00]).unwrap().0, "image/jpeg");
        assert_eq!(validate_image_bytes(b"\x89PNG\r\n\x1a\nrest").unwrap().0, "image/png");
        assert_eq!(validate_image_bytes(b"RIFF0000WEBPrest").unwrap().0, "image/webp");
    }

    #[test]
    fn image_validator_rejects_html_and_empty_payloads() {
        assert!(validate_image_bytes(b"").is_err());
        assert!(validate_image_bytes(b"<html>not artwork</html>").is_err());
    }

    #[test]
    fn cache_keys_are_stable_and_do_not_expose_media_paths() {
        let key = cache_key(r"C:\Movies\Example.mkv", "poster");
        assert_eq!(key.len(), 64);
        assert!(!key.contains("Movies"));
        assert_eq!(key, cache_key(r"C:\Movies\Example.mkv", "poster"));
    }
}
