use crate::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateFile {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub size: u64,
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub key: String,
    pub count: usize,
    pub total_size: u64,
    pub files: Vec<DuplicateFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateScanResult {
    pub groups: Vec<DuplicateGroup>,
    pub total_wasted_bytes: u64,
    pub scanned_files: usize,
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn calculate_key_hash(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    bytes_to_hex(&hasher.finalize())
}

pub fn calculate_file_hash(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];

    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    Ok(bytes_to_hex(&hasher.finalize()))
}

#[tauri::command]
pub async fn find_duplicates(
    state: State<'_, AppState>,
    mode: Option<String>,
    similarity_threshold: Option<f64>,
) -> Result<DuplicateScanResult, String> {
    let _ = similarity_threshold;
    let scan_mode = mode.unwrap_or_else(|| "name_size".to_string());

    let db = match state.db.lock() {
        Ok(guard) => guard,
        Err(e) => return Err(format!("Failed to lock DB state: {}", e)),
    };

    let mut stmt = db
        .conn
        .prepare("SELECT id, path, title, file_size FROM media_items WHERE file_size > 0")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(DuplicateFile {
                id: row.get(0)?,
                path: row.get(1)?,
                name: row.get(2)?,
                size: row.get::<_, i64>(3).map(|s| s as u64)?,
                hash: None,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut all_files: Vec<DuplicateFile> = Vec::new();
    for item in rows.flatten() {
        all_files.push(item);
    }

    let scanned_files = all_files.len();
    let mut map: HashMap<String, Vec<DuplicateFile>> = HashMap::new();

    for file in all_files {
        let key = match scan_mode.as_str() {
            "size" => format!("{}", file.size),
            "name" => file.name.to_lowercase(),
            _ => format!("{}_{}", file.name.to_lowercase(), file.size),
        };
        map.entry(key).or_default().push(file);
    }

    let mut groups: Vec<DuplicateGroup> = Vec::new();
    let mut total_wasted_bytes: u64 = 0;

    for (key, files) in map.into_iter().filter(|(_, f)| f.len() > 1) {
        let count = files.len();
        let total_size: u64 = files.iter().map(|f| f.size).sum();
        let single_size = files.first().map(|f| f.size).unwrap_or(0);
        let wasted = single_size.saturating_mul((count - 1) as u64);
        total_wasted_bytes += wasted;

        groups.push(DuplicateGroup {
            key,
            count,
            total_size,
            files,
        });
    }

    Ok(DuplicateScanResult {
        groups,
        total_wasted_bytes,
        scanned_files,
    })
}

#[tauri::command]
pub async fn get_duplicate_groups(
    state: State<'_, AppState>,
    mode: Option<String>,
) -> Result<Vec<DuplicateGroup>, String> {
    let result = find_duplicates(state, mode, None).await?;
    Ok(result.groups)
}

#[tauri::command]
pub async fn remove_duplicate(state: State<'_, AppState>, item_id: i64) -> Result<bool, String> {
    let db = match state.db.lock() {
        Ok(guard) => guard,
        Err(e) => return Err(format!("Failed to lock DB state: {}", e)),
    };

    let source_path: Result<String, _> = db.conn.query_row(
        "SELECT path FROM media_items WHERE id = ?1",
        params![item_id],
        |row| row.get(0),
    );

    if let Ok(path) = source_path {
        // Keep the library row when the file could not be deleted, so the
        // copy stays visible and the caller sees the failure.
        remove_media_file(&PathBuf::from(&path))?;
    }

    db.conn
        .execute("DELETE FROM media_items WHERE id = ?1", params![item_id])
        .map_err(|e| e.to_string())?;

    Ok(true)
}

fn remove_media_file(path: &std::path::Path) -> Result<(), String> {
    if path.is_file() {
        std::fs::remove_file(path)
            .map_err(|e| format!("Failed to delete {}: {}", path.display(), e))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn quarantine(state: State<'_, AppState>, item_id: i64) -> Result<String, String> {
    let db = match state.db.lock() {
        Ok(guard) => guard,
        Err(e) => return Err(format!("Failed to lock DB state: {}", e)),
    };

    let source_path: String = db
        .conn
        .query_row(
            "SELECT path FROM media_items WHERE id = ?1",
            params![item_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Item {item_id} not found: {e}"))?;

    let source = PathBuf::from(&source_path);
    if !source.is_file() {
        return Err(format!("File does not exist: {}", source.display()));
    }

    let quarantine_dir = state.app_data_dir.join("quarantine");
    std::fs::create_dir_all(&quarantine_dir).map_err(|e| e.to_string())?;

    let file_name = source
        .file_name()
        .ok_or_else(|| "Missing file name".to_string())?;
    let dest = quarantine_dir.join(file_name);

    std::fs::rename(&source, &dest).map_err(|e| e.to_string())?;

    db.conn
        .execute(
            "UPDATE media_items SET path = ?1 WHERE id = ?2",
            params![dest.to_string_lossy().to_string(), item_id],
        )
        .map_err(|e| e.to_string())?;

    Ok(dest.to_string_lossy().to_string())
}

#[cfg(test)]
mod remove_media_file_tests {
    use super::remove_media_file;

    #[test]
    fn deletes_existing_file_and_tolerates_missing_one() {
        let path =
            std::env::temp_dir().join(format!("cinavault-remove-dup-{}.mkv", std::process::id()));
        std::fs::write(&path, b"x").unwrap();
        assert!(remove_media_file(&path).is_ok());
        assert!(!path.exists());
        assert!(remove_media_file(&path).is_ok());
    }
}
