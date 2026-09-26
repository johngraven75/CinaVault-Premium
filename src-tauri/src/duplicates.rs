use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DuplicateGroup {
    pub hash: String,
    pub size: u64,
    pub paths: Vec<PathBuf>,
}

pub fn scan_directory_for_duplicates(dir: &Path) -> Result<Vec<DuplicateGroup>, String> {
    let mut size_map: HashMap<u64, Vec<PathBuf>> = HashMap::new();

    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Ok(meta) = entry.metadata() {
                if meta.len() > 0 {
                    size_map.entry(meta.len()).or_default().push(entry.path().to_path_buf());
                }
            }
        }
    }

    let mut duplicate_groups: Vec<DuplicateGroup> = Vec::new();

    for (size, paths) in size_map.into_iter().filter(|(_, p)| p.len() > 1) {
        let mut hash_map: HashMap<String, Vec<PathBuf>> = HashMap::new();

        for path in paths {
            if let Ok(hash) = calculate_file_hash(&path) {
                hash_map.entry(hash).or_default().push(path);
            }
        }

        for (hash, group_paths) in hash_map.into_iter().filter(|(_, p)| p.len() > 1) {
            duplicate_groups.push(DuplicateGroup {
                hash,
                size,
                paths: group_paths,
            });
        }
    }

    Ok(duplicate_groups)
}

pub fn calculate_key_hash(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
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

    Ok(hex::encode(hasher.finalize()))
}
