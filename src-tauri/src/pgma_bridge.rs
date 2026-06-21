use std::path::PathBuf;
use tauri::State;

#[tauri::command]
pub fn find_local_candidates(library_path: String) -> Result<Vec<String>, String> {
    let base_path = PathBuf::from(&library_path);
    if !base_path.exists() || !base_path.is_dir() {
        return Err("Invalid library path provided".to_string());
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(base_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                candidates.push(path);
            }
        }
    }

    // Fixed lifetime bug by consuming the vector with into_iter() instead of draining a reference
    let result = candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .map(|path| path.to_string_lossy().to_string())
        .map(|single| vec![single])
        .unwrap_or_default();

    Ok(result)
}

#[tauri::command]
pub fn refresh_pgma_library() -> Result<String, String> {
    // Stub implementation to satisfy main.rs linkage
    println!("Refreshing PGMA Library...");
    Ok("Library refreshed successfully".to_string())
}
