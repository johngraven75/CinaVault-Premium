//! Shared error type for Tauri-facing code.
//!
//! Commands in this crate return `Result<T, String>`, which is what the
//! frontend receives. `AppError` gives new code a typed error whose text is
//! safe to show: the user sees a plain message and a suggested fix, while the
//! full detail (paths, SQL, provider responses) goes to the log only.
//! `From<AppError> for String` lets `?` convert it inside existing commands.

use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum AppError {
    Io(std::io::Error),
    Db(String),
    Network(String),
    PathNotAllowed(String),
    InvalidInput(String),
    NasUnreachable(String),
    Cancelled,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    /// Message that is safe to show in the UI. Never includes paths, SQL or
    /// provider output.
    pub fn user_message(&self) -> String {
        match self {
            AppError::Io(_) => "A file operation failed.".to_string(),
            AppError::Db(_) => "The library database reported a problem.".to_string(),
            AppError::Network(_) => "A network request failed.".to_string(),
            AppError::PathNotAllowed(_) => {
                "That location isn't in your allowed library folders.".to_string()
            }
            AppError::InvalidInput(message) => message.clone(),
            AppError::NasUnreachable(_) => "Can't reach the NAS.".to_string(),
            AppError::Cancelled => "Cancelled.".to_string(),
        }
    }

    /// Suggested next step for the user, when there is a useful one.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            AppError::Io(_) => {
                Some("Check that the drive is connected and writable, then try again.")
            }
            AppError::Db(_) => Some("Restart CinaVault; if it keeps happening, check the logs."),
            AppError::Network(_) => Some("Check your internet connection and try again."),
            AppError::PathNotAllowed(_) => Some("Add the folder under Sources first."),
            AppError::NasUnreachable(_) => Some("Check your network connection or credentials."),
            AppError::InvalidInput(_) | AppError::Cancelled => None,
        }
    }

    /// Full technical detail for the log. Do not send this to the frontend.
    pub fn detail(&self) -> String {
        match self {
            AppError::Io(error) => format!("io: {error}"),
            AppError::Db(message) => format!("db: {message}"),
            AppError::Network(message) => format!("network: {message}"),
            AppError::PathNotAllowed(path) => format!("path not allowed: {path}"),
            AppError::InvalidInput(message) => format!("invalid input: {message}"),
            AppError::NasUnreachable(message) => format!("nas unreachable: {message}"),
            AppError::Cancelled => "cancelled".to_string(),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.hint() {
            Some(hint) => write!(f, "{} {}", self.user_message(), hint),
            None => write!(f, "{}", self.user_message()),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        AppError::Io(error)
    }
}

impl From<AppError> for String {
    fn from(error: AppError) -> Self {
        log::error!("command failed: {}", error.detail());
        error.to_string()
    }
}

/// Resolve `requested` and confirm it stays inside one of `roots`.
///
/// The path is canonicalized first, so `..` segments and symlinks that point
/// outside the allowed folders are rejected. For a path that does not exist
/// yet (a new destination) the parent directory is resolved instead and the
/// file name is re-attached, so a destination can still be validated.
pub fn confine_to_roots(requested: &Path, roots: &[PathBuf]) -> AppResult<PathBuf> {
    let denied = || AppError::PathNotAllowed(requested.display().to_string());
    let has_parent_segment = requested
        .components()
        .any(|part| matches!(part, std::path::Component::ParentDir));
    let resolved = match requested.canonicalize() {
        Ok(path) => path,
        Err(_) => {
            if has_parent_segment {
                return Err(denied());
            }
            let parent = requested.parent().ok_or_else(denied)?;
            let name = requested.file_name().ok_or_else(denied)?;
            parent.canonicalize().map_err(|_| denied())?.join(name)
        }
    };
    let allowed = roots
        .iter()
        .filter_map(|root| root.canonicalize().ok())
        .any(|root| resolved.starts_with(&root));
    if allowed {
        Ok(resolved)
    } else {
        Err(denied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cinavault-app-error-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    #[test]
    fn user_text_never_contains_internal_detail() {
        let error = AppError::Db("UNIQUE constraint failed: media_items.file_path".to_string());
        assert!(!error.to_string().contains("media_items"));
        assert!(error.detail().contains("media_items"));
        let text: String = AppError::NasUnreachable("smb://10.0.0.5/share".to_string()).into();
        assert!(text.contains("Can't reach the NAS."));
        assert!(!text.contains("10.0.0.5"));
        assert!(text.contains("credentials"));
    }

    #[test]
    fn invalid_input_passes_its_message_through() {
        let error = AppError::InvalidInput("Name is required".to_string());
        assert_eq!(error.to_string(), "Name is required");
        assert!(error.hint().is_none());
    }

    #[test]
    fn allows_paths_inside_a_root_and_new_destinations() {
        let root = scratch("inside");
        let file = root.join("movie.mkv");
        fs::write(&file, b"x").expect("write file");
        assert!(confine_to_roots(&file, &[root.clone()]).is_ok());
        let new_file = root.join("quarantine-copy.mkv");
        assert!(confine_to_roots(&new_file, &[root.clone()]).is_ok());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_traversal_and_foreign_roots() {
        let root = scratch("root");
        let other = scratch("other");
        let outside = other.join("secret.txt");
        fs::write(&outside, b"x").expect("write file");
        assert!(matches!(
            confine_to_roots(&outside, &[root.clone()]),
            Err(AppError::PathNotAllowed(_))
        ));
        let traversal = root
            .join("..")
            .join(other.file_name().expect("name"))
            .join("secret.txt");
        assert!(matches!(
            confine_to_roots(&traversal, &[root.clone()]),
            Err(AppError::PathNotAllowed(_))
        ));
        let missing_traversal = root.join("..").join("not-created.txt");
        assert!(matches!(
            confine_to_roots(&missing_traversal, &[root.clone()]),
            Err(AppError::PathNotAllowed(_))
        ));
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&other);
    }
}
