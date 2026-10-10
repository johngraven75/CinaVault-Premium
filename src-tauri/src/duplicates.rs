use crate::db::{Database, MediaItem};
use crate::library_unify::{content_fingerprint, group_items};
use crate::AppState;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
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

fn file_name_of(path: &str) -> String {
    path.rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or(path)
        .to_string()
}

fn to_duplicate_file(item: &MediaItem) -> DuplicateFile {
    DuplicateFile {
        id: item.id.unwrap_or_default(),
        path: item.file_path.clone(),
        name: if item.title.trim().is_empty() {
            file_name_of(&item.file_path)
        } else {
            item.title.clone()
        },
        size: item.file_size.unwrap_or(0).max(0) as u64,
        hash: None,
    }
}

/// Keys files into candidate groups for every mode except `work`.
fn keyed_groups(
    mode: &str,
    files: Vec<DuplicateFile>,
) -> Result<HashMap<String, Vec<DuplicateFile>>, String> {
    let mut map: HashMap<String, Vec<DuplicateFile>> = HashMap::new();
    match mode {
        "content" => {
            // Only files that share a size can share content; fingerprint those.
            let mut by_size: HashMap<u64, Vec<DuplicateFile>> = HashMap::new();
            for file in files.into_iter().filter(|file| file.size > 0) {
                by_size.entry(file.size).or_default().push(file);
            }
            for (_, same_size) in by_size.into_iter().filter(|(_, f)| f.len() > 1) {
                for mut file in same_size {
                    match content_fingerprint(Path::new(&file.path)) {
                        Ok(fingerprint) => {
                            file.hash = Some(fingerprint.clone());
                            map.entry(format!("content:{fingerprint}"))
                                .or_default()
                                .push(file);
                        }
                        Err(error) => {
                            log::debug!("Skipping unreadable duplicate candidate: {error}")
                        }
                    }
                }
            }
        }
        _ => {
            for file in files {
                let key = match mode {
                    "size" if file.size > 0 => format!("size:{}", file.size),
                    "size" => continue,
                    "name" => format!("name:{}", file.name.trim().to_lowercase()),
                    _ if file.size > 0 => {
                        format!(
                            "name_size:{}_{}",
                            file.name.trim().to_lowercase(),
                            file.size
                        )
                    }
                    _ => continue,
                };
                map.entry(key).or_default().push(file);
            }
        }
    }
    Ok(map)
}

/// Candidate groups with more than one copy, for any scan mode. Blocking.
fn raw_duplicate_groups(
    mode: &str,
    items: Vec<MediaItem>,
) -> Result<Vec<(String, Vec<DuplicateFile>)>, String> {
    if mode == "work" {
        return Ok(group_items(items)
            .into_iter()
            .filter(|entry| entry.copy_count > 1)
            .map(|entry| {
                let files = entry
                    .copies
                    .iter()
                    .map(|copy| DuplicateFile {
                        id: copy.id,
                        path: copy.file_path.clone(),
                        name: entry.primary.title.clone(),
                        size: copy.file_size.unwrap_or(0).max(0) as u64,
                        hash: None,
                    })
                    .collect();
                (entry.work_key, files)
            })
            .collect());
    }
    let files: Vec<DuplicateFile> = items.iter().map(to_duplicate_file).collect();
    Ok(keyed_groups(mode, files)?
        .into_iter()
        .filter(|(_, files)| files.len() > 1)
        .collect())
}

/// Group library copies that duplicate each other.
///
/// Modes: `name_size` (default: same title and byte size), `size`, `name`,
/// `work` (same unified-library work, e.g. a 4K and a 1080p copy of one film),
/// and `content` (same size plus SHA-256 of the first/middle/last 4 MiB).
#[tauri::command]
pub async fn find_duplicates(
    state: State<'_, AppState>,
    mode: Option<String>,
) -> Result<DuplicateScanResult, String> {
    let scan_mode = mode
        .map(|mode| mode.trim().to_ascii_lowercase())
        .filter(|mode| !mode.is_empty())
        .unwrap_or_else(|| "name_size".to_string());
    if !["name_size", "size", "name", "work", "content"].contains(&scan_mode.as_str()) {
        return Err(format!(
            "Unknown duplicate mode '{scan_mode}'. Use name_size, size, name, work or content."
        ));
    }

    let items = {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("Failed to lock DB state: {}", e))?;
        db.get_media_items_data(None, None, None)
            .map_err(|e| e.to_string())?
    };
    let scanned_files = items.len();

    // Grouping is CPU-bound (normalization and union-find for `work`, file
    // fingerprinting for `content`): keep it off the async executor.
    let mode = scan_mode.clone();
    let raw_groups = tokio::task::spawn_blocking(move || raw_duplicate_groups(&mode, items))
        .await
        .map_err(|e| format!("Duplicate scan failed: {e}"))??;

    let mut groups: Vec<DuplicateGroup> = Vec::new();
    let mut total_wasted_bytes: u64 = 0;
    for (key, files) in raw_groups {
        let count = files.len();
        let total_size: u64 = files.iter().map(|f| f.size).sum();
        // Keeping the largest copy, everything else is reclaimable.
        let kept = files.iter().map(|f| f.size).max().unwrap_or(0);
        total_wasted_bytes += total_size.saturating_sub(kept);
        groups.push(DuplicateGroup {
            key,
            count,
            total_size,
            files,
        });
    }
    groups.sort_by(|a, b| b.total_size.cmp(&a.total_size).then(a.key.cmp(&b.key)));

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
    let result = find_duplicates(state, mode).await?;
    Ok(result.groups)
}

#[tauri::command]
pub async fn remove_duplicate(state: State<'_, AppState>, item_id: i64) -> Result<bool, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("Failed to lock DB state: {}", e))?;
    remove_duplicate_in(&db, item_id)
}

fn remove_duplicate_in(db: &Database, item_id: i64) -> Result<bool, String> {
    let source_path: Option<String> = db
        .conn
        .query_row(
            "SELECT file_path FROM media_items WHERE id = ?1",
            params![item_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if let Some(path) = source_path {
        // Keep the library row when the file could not be deleted, so the
        // copy stays visible and the caller sees the failure.
        remove_media_file(&PathBuf::from(&path))?;
    }

    db.conn
        .execute("DELETE FROM media_items WHERE id = ?1", params![item_id])
        .map_err(|e| e.to_string())?;

    Ok(true)
}

/// Rename, falling back to copy + delete when the quarantine folder is on a
/// different volume than the media file. `dest` must be a path the caller
/// owns (see [`reserve_quarantine_path`]): `rename` replaces an existing
/// destination on every platform std supports.
fn move_file(source: &Path, dest: &Path) -> Result<(), String> {
    if std::fs::rename(source, dest).is_ok() {
        return Ok(());
    }
    std::fs::copy(source, dest)
        .map_err(|e| format!("Failed to move {} to quarantine: {e}", source.display()))?;
    std::fs::remove_file(source).map_err(|e| {
        let _ = std::fs::remove_file(dest);
        format!(
            "Failed to remove {} after copying to quarantine: {e}",
            source.display()
        )
    })
}

/// Atomically claim a quarantine path that no other file uses: `<name>`, then
/// `<id>-<name>`, then `<id>-<n>-<name>`. The empty placeholder created here
/// (`create_new`, so an existing file is never touched) is what the move then
/// replaces, so an earlier quarantined copy can never be overwritten.
fn reserve_quarantine_path(
    quarantine_dir: &Path,
    file_name: &std::ffi::OsStr,
    item_id: i64,
) -> Result<PathBuf, String> {
    let name = file_name.to_string_lossy();
    let candidates = std::iter::once(name.to_string())
        .chain(std::iter::once(format!("{item_id}-{name}")))
        .chain((2..10_000u32).map(|n| format!("{item_id}-{n}-{name}")));
    for candidate in candidates {
        let path = quarantine_dir.join(candidate);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(_) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(format!(
                    "Unable to create quarantine file {}: {e}",
                    path.display()
                ))
            }
        }
    }
    Err(format!(
        "No free quarantine file name for {name} in {}",
        quarantine_dir.display()
    ))
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
    let db = state
        .db
        .lock()
        .map_err(|e| format!("Failed to lock DB state: {}", e))?;
    quarantine_in(&db, &state.app_data_dir.join("quarantine"), item_id)
}

fn quarantine_in(db: &Database, quarantine_dir: &Path, item_id: i64) -> Result<String, String> {
    let source_path: String = db
        .conn
        .query_row(
            "SELECT file_path FROM media_items WHERE id = ?1",
            params![item_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Item {item_id} not found: {e}"))?;

    let source = PathBuf::from(&source_path);
    if !source.is_file() {
        return Err(format!("File does not exist: {}", source.display()));
    }

    std::fs::create_dir_all(quarantine_dir).map_err(|e| e.to_string())?;

    let file_name = source
        .file_name()
        .ok_or_else(|| "Missing file name".to_string())?;
    let dest = reserve_quarantine_path(quarantine_dir, file_name, item_id)?;

    if let Err(error) = move_file(&source, &dest) {
        let _ = std::fs::remove_file(&dest);
        return Err(error);
    }

    if let Err(error) = db.conn.execute(
        "UPDATE media_items SET file_path = ?1 WHERE id = ?2",
        params![dest.to_string_lossy().to_string(), item_id],
    ) {
        // The library row still points at the original path: put the file
        // back there so the row and the file agree again.
        return Err(match move_file(&dest, &source) {
            Ok(()) => format!("Quarantine cancelled; library update failed: {error}"),
            Err(restore) => format!(
                "Library update failed ({error}) and the file could not be restored; it is at {}: {restore}",
                dest.display()
            ),
        });
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library_unify::test_item;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cinavault-dup-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn file(id: i64, name: &str, path: &str, size: u64) -> DuplicateFile {
        DuplicateFile {
            id,
            path: path.into(),
            name: name.into(),
            size,
            hash: None,
        }
    }

    #[test]
    fn name_size_size_and_name_modes() {
        let files = vec![
            file(1, "Movie", "/a/m.mkv", 10),
            file(2, "movie", "/b/m.mkv", 10),
            file(3, "Other", "/c/o.mkv", 10),
            file(4, "Movie", "/d/m.mkv", 20),
        ];
        let groups = keyed_groups("name_size", files.clone()).unwrap();
        assert_eq!(groups["name_size:movie_10"].len(), 2);
        let groups = keyed_groups("size", files.clone()).unwrap();
        assert_eq!(groups["size:10"].len(), 3);
        let groups = keyed_groups("name", files).unwrap();
        assert_eq!(groups["name:movie"].len(), 3);
    }

    #[test]
    fn content_mode_matches_identical_bytes() {
        let dir = temp_dir();
        let paths: Vec<PathBuf> = ["a.mkv", "b.mkv", "c.mkv"]
            .iter()
            .map(|name| dir.join(name))
            .collect();
        std::fs::write(&paths[0], b"0123456789").unwrap();
        std::fs::write(&paths[1], b"0123456789").unwrap();
        std::fs::write(&paths[2], b"9876543210").unwrap();
        let files = paths
            .iter()
            .enumerate()
            .map(|(i, p)| file(i as i64, "x", p.to_str().unwrap(), 10))
            .collect();
        let groups: Vec<_> = keyed_groups("content", files)
            .unwrap()
            .into_iter()
            .filter(|(_, f)| f.len() > 1)
            .collect();
        assert_eq!(groups.len(), 1);
        let ids: Vec<i64> = groups[0].1.iter().map(|f| f.id).collect();
        assert!(ids.contains(&0) && ids.contains(&1));
        assert!(groups[0].1[0].hash.is_some());
        std::fs::remove_dir_all(dir).ok();
    }

    fn add_item(db: &Database, path: &Path) -> i64 {
        let mut item = test_item(0, "Movie", path.to_str().unwrap(), "movie");
        item.id = None;
        db.add_media_item_data(&item).unwrap()
    }

    #[test]
    fn quarantine_never_overwrites_an_earlier_quarantined_copy() {
        let dir = temp_dir();
        let db = Database::new(dir.join("lib.db").to_str().unwrap()).unwrap();
        let quarantine_dir = dir.join("quarantine");
        std::fs::create_dir_all(&quarantine_dir).unwrap();
        std::fs::create_dir_all(dir.join("lib")).unwrap();
        let media = dir.join("lib").join("Movie.mkv");
        std::fs::write(&media, b"new copy").unwrap();
        let id = add_item(&db, &media);

        // Both the plain name and the id-prefixed name are already taken.
        let plain = quarantine_dir.join("Movie.mkv");
        let prefixed = quarantine_dir.join(format!("{id}-Movie.mkv"));
        std::fs::write(&plain, b"first").unwrap();
        std::fs::write(&prefixed, b"second").unwrap();

        let moved = PathBuf::from(quarantine_in(&db, &quarantine_dir, id).unwrap());
        assert_ne!(moved, plain);
        assert_ne!(moved, prefixed);
        assert_eq!(std::fs::read(&plain).unwrap(), b"first");
        assert_eq!(std::fs::read(&prefixed).unwrap(), b"second");
        assert_eq!(std::fs::read(&moved).unwrap(), b"new copy");
        assert!(!media.exists());
        drop(db);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn quarantine_restores_the_file_when_the_library_update_fails() {
        let dir = temp_dir();
        let db = Database::new(dir.join("lib.db").to_str().unwrap()).unwrap();
        let quarantine_dir = dir.join("quarantine");
        let media = dir.join("Movie.mkv");
        std::fs::write(&media, b"x").unwrap();
        let id = add_item(&db, &media);
        db.conn
            .execute_batch(
                "CREATE TRIGGER block_path_update BEFORE UPDATE OF file_path ON media_items \
                 BEGIN SELECT RAISE(ABORT, 'simulated write failure'); END;",
            )
            .unwrap();

        let error = quarantine_in(&db, &quarantine_dir, id).unwrap_err();
        assert!(error.contains("simulated write failure"), "{error}");
        assert_eq!(std::fs::read(&media).unwrap(), b"x");
        assert_eq!(std::fs::read_dir(&quarantine_dir).unwrap().count(), 0);
        let stored: String = db
            .conn
            .query_row(
                "SELECT file_path FROM media_items WHERE id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(Path::new(&stored), media.as_path());
        drop(db);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn work_mode_groups_copies_of_one_work() {
        let mut a = test_item(1, "Heat", "/a/Heat.1995.2160p.mkv", "movie");
        a.tmdb_id = Some("949".into());
        a.file_size = Some(20);
        let mut b = test_item(2, "Heat", "/b/Heat.1995.1080p.mkv", "movie");
        b.tmdb_id = Some("949".into());
        b.file_size = Some(10);
        let other = test_item(3, "Ronin", "/c/Ronin.mkv", "movie");
        let groups = raw_duplicate_groups("work", vec![a, b, other]).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, "tmdb:949");
        assert_eq!(groups[0].1.len(), 2);
    }

    #[test]
    fn quarantine_and_remove_use_file_path_column() {
        let dir = temp_dir();
        let db = Database::new(dir.join("lib.db").to_str().unwrap()).unwrap();
        let media = dir.join("Movie.mkv");
        std::fs::write(&media, b"x").unwrap();
        let mut item = test_item(0, "Movie", media.to_str().unwrap(), "movie");
        item.id = None;
        let id = db.add_media_item_data(&item).unwrap();

        let moved = quarantine_in(&db, &dir.join("quarantine"), id).unwrap();
        assert!(!media.exists());
        assert!(Path::new(&moved).is_file());
        let stored: String = db
            .conn
            .query_row(
                "SELECT file_path FROM media_items WHERE id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored, moved);

        assert!(remove_duplicate_in(&db, id).unwrap());
        assert!(!Path::new(&moved).exists());
        let remaining: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM media_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 0);
        drop(db);
        std::fs::remove_dir_all(dir).ok();
    }
}
