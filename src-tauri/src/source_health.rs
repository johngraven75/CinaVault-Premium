use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct SourcePathHealth {
    pub path: String,
    pub source_type: String,
    pub exists: bool,
    pub readable: bool,
    pub expected_kind: bool,
    pub status: &'static str,
    pub message: String,
}

fn test_directory_read(path: &Path) -> Result<(), String> {
    let mut entries = std::fs::read_dir(path).map_err(|error| error.to_string())?;
    // Force at least one iterator operation so delayed permission/device errors surface.
    if let Some(entry) = entries.next() {
        entry.map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn validate_source_path(path: String, source_type: String) -> SourcePathHealth {
    let trimmed = path.trim().to_string();
    let candidate = Path::new(&trimmed);
    let exists = candidate.exists();
    let expected_kind = match source_type.as_str() {
        "file" => candidate.is_file(),
        _ => candidate.is_dir(),
    };

    let read_result = if !exists {
        Err("path does not exist or the external drive is disconnected".to_string())
    } else if !expected_kind {
        Err(match source_type.as_str() {
            "file" => "source type is File but the selected path is not a file".to_string(),
            _ => "source type is Folder/Drive but the selected path is not a directory".to_string(),
        })
    } else if source_type == "file" {
        std::fs::File::open(candidate)
            .map(|_| ())
            .map_err(|error| error.to_string())
    } else {
        test_directory_read(candidate)
    };

    match read_result {
        Ok(()) => SourcePathHealth {
            path: trimmed,
            source_type,
            exists,
            readable: true,
            expected_kind,
            status: "ready",
            message: "Source is connected, readable, and ready to scan".to_string(),
        },
        Err(message) => SourcePathHealth {
            path: trimmed,
            source_type,
            exists,
            readable: false,
            expected_kind,
            status: "unavailable",
            message,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::validate_source_path;

    #[test]
    fn missing_external_path_is_reported_as_unavailable() {
        let result = validate_source_path(
            "Z:\\definitely-not-a-real-cinavault-drive".to_string(),
            "drive".to_string(),
        );
        assert_eq!(result.status, "unavailable");
        assert!(!result.readable);
    }
}
