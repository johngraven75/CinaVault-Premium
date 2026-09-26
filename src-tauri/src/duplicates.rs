use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::State;
use walkdir::WalkDir;
use crate::AppState;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DuplicateFile {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub hash: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DuplicateGroup {
    pub key: String,
    pub count: usize,
    pub total_size: u64,
    pub files: Vec<DuplicateFile>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];

    loop {
        let count = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    Ok(bytes_to_hex(&hasher.finalize()))
}

#[tauri::command]
pub fn find_duplicates(
    state: State<'_, Arc<AppState>>,
    mode: Option<String>,
    similarity_threshold: Option<f64>,
) -> Result<DuplicateScanResult, String> {
    let _ = similarity_threshold;
    let scan_mode = mode.unwrap_or_else(|| "name_size".to_string());

    let db = match state.db.lock() {
        Ok(guard) => guard,
        Err(e) => return Err(format!("Failed to lock DB state: {}", e)),
    };

    let conn = db.get_connection();
    let mut stmt = conn
        .prepare("SELECT path, name, size FROM media_items WHERE size > 0")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(DuplicateFile {
                path: row.get::<_, String>(0)?,
                name: row.get::<_, String>(1)?,
                size: row.get::<_, i64>(2).map(|s| s as u64)?,
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
