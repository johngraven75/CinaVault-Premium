// CinaVault Premium — Duplicate Finder Module
use crate::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use tauri::State;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DuplicateGroup {
    pub id: i64,
    pub group_hash: String,
    pub items: Vec<DuplicateItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DuplicateItem {
    pub id: i64,
    pub media_id: i64,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub title: Option<String>,
}

#[tauri::command]
pub async fn find_duplicates(
    state: State<'_, AppState>,
    match_by: Option<String>,
    tolerance_mb: Option<f64>,
) -> Result<serde_json::Value, String> {
    let match_rule = match_by.unwrap_or_else(|| "name_size".to_string());
    let tolerance = tolerance_mb.unwrap_or(0.0) * 1_048_576.0; // Convert MB to bytes

    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Clear previous results
    db.conn
        .execute("DELETE FROM duplicate_items", [])
        .map_err(|e| e.to_string())?;
    db.conn
        .execute("DELETE FROM duplicate_groups", [])
        .map_err(|e| e.to_string())?;

    // Get all media items
    let mut stmt = db
        .conn
        .prepare("SELECT id, title, file_path, file_size FROM media_items ORDER BY title")
        .map_err(|e| e.to_string())?;

    let items: Vec<(i64, String, String, Option<i64>)> = stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut groups: HashMap<String, Vec<(i64, String, Option<i64>)>> = HashMap::new();

    for (id, title, path, size) in &items {
        let key = match match_rule.as_str() {
            "name" => title.to_lowercase().trim().to_string(),
            "size" => {
                if let Some(s) = size {
                    format!("size_{}", s)
                } else {
                    continue;
                }
            }
            "hash" => {
                // Partial hash (first 1MB)
                match partial_hash(path) {
                    Ok(h) => h,
                    Err(_) => continue,
                }
            }
            _ => {
                // name_size (default)
                let name_key = title.to_lowercase().trim().to_string();
                let size_key = size.unwrap_or(0);
                format!("{}_{}", name_key, size_key)
            }
        };

        groups
            .entry(key)
            .or_default()
            .push((*id, path.clone(), *size));
    }

    // Filter to groups with 2+ items (actual duplicates)
    let now = chrono::Utc::now().to_rfc3339();
    let mut total_groups = 0u64;
    let mut total_items = 0u64;

    for (key, group_items) in &groups {
        if group_items.len() < 2 {
            continue;
        }

        // Check size tolerance for name-based matches
        if match_rule == "name_size" || match_rule == "name" {
            if tolerance > 0.0 {
                let sizes: Vec<i64> = group_items.iter().filter_map(|(_, _, s)| *s).collect();
                if sizes.len() >= 2 {
                    let max = *sizes.iter().max().unwrap_or(&0);
                    let min = *sizes.iter().min().unwrap_or(&0);
                    if (max - min) as f64 > tolerance {
                        continue; // Size difference exceeds tolerance
                    }
                }
            }
        }

        let hash = format!("{:x}", Sha256::digest(key.as_bytes()));
        db.conn
            .execute(
                "INSERT INTO duplicate_groups (group_hash, created_at) VALUES (?1, ?2)",
                params![hash, now],
            )
            .map_err(|e| e.to_string())?;
        let group_id = db.conn.last_insert_rowid();
        total_groups += 1;

        for (media_id, path, size) in group_items {
            db.conn.execute(
                "INSERT INTO duplicate_items (group_id, media_id, file_path, file_size) VALUES (?1, ?2, ?3, ?4)",
                params![group_id, media_id, path, size],
            ).map_err(|e| e.to_string())?;
            total_items += 1;
        }
    }

    Ok(serde_json::json!({
        "groups_found": total_groups,
        "total_duplicates": total_items,
        "match_rule": match_rule,
    }))
}

#[tauri::command]
pub fn get_duplicate_groups(state: State<AppState>) -> Result<Vec<DuplicateGroup>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let mut group_stmt = db
        .conn
        .prepare("SELECT id, group_hash FROM duplicate_groups ORDER BY id")
        .map_err(|e| e.to_string())?;

    let groups: Vec<(i64, String)> = group_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut result = Vec::new();

    for (gid, hash) in groups {
        let mut item_stmt = db
            .conn
            .prepare(
                "SELECT di.id, di.media_id, di.file_path, di.file_size, mi.title \
             FROM duplicate_items di LEFT JOIN media_items mi ON di.media_id = mi.id \
             WHERE di.group_id = ?1",
            )
            .map_err(|e| e.to_string())?;

        let items: Vec<DuplicateItem> = item_stmt
            .query_map(params![gid], |row| {
                Ok(DuplicateItem {
                    id: row.get(0)?,
                    media_id: row.get(1)?,
                    file_path: row.get(2)?,
                    file_size: row.get(3)?,
                    title: row.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        result.push(DuplicateGroup {
            id: gid,
            group_hash: hash,
            items,
        });
    }

    Ok(result)
}

#[tauri::command]
pub fn remove_duplicate(
    state: State<AppState>,
    item_id: i64,
    delete_file: bool,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    if delete_file {
        let mut stmt = db
            .conn
            .prepare("SELECT file_path FROM duplicate_items WHERE id = ?1")
            .map_err(|e| e.to_string())?;
        if let Ok(path) = stmt.query_row(params![item_id], |row| row.get::<_, String>(0)) {
            let _ = std::fs::remove_file(&path); // Best effort delete
        }
    }

    // Remove from DB
    let mut stmt = db
        .conn
        .prepare("SELECT media_id FROM duplicate_items WHERE id = ?1")
        .map_err(|e| e.to_string())?;
    if let Ok(media_id) = stmt.query_row(params![item_id], |row| row.get::<_, i64>(0)) {
        db.conn
            .execute("DELETE FROM media_items WHERE id = ?1", params![media_id])
            .map_err(|e| e.to_string())?;
    }
    db.conn
        .execute(
            "DELETE FROM duplicate_items WHERE id = ?1",
            params![item_id],
        )
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn partial_hash(path: &str) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut buffer = vec![0u8; 1_048_576]; // 1MB
    let bytes_read = file.read(&mut buffer).map_err(|e| e.to_string())?;
    buffer.truncate(bytes_read);
    let hash = Sha256::digest(&buffer);
    Ok(format!("{:x}", hash))
}
