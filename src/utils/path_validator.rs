use crate::error::{GvcError, Result};
use std::path::{Path, PathBuf};

/// Provides safe path validation helpers to avoid traversal and host intrusion.
pub struct PathValidator;

impl PathValidator {
    /// Validates and canonicalises an incoming project path.
    pub fn validate_project_path(path: impl AsRef<Path>) -> Result<PathBuf> {
        let path = path.as_ref();

        let canonical = path.canonicalize().map_err(|e| {
            GvcError::ProjectValidation(format!("Invalid path '{}': {e}", path.display()))
        })?;

        if !canonical.is_dir() {
            return Err(GvcError::ProjectValidation(format!(
                "Path '{}' is not a directory",
                canonical.display()
            )));
        }

        const FORBIDDEN: &[&str] = &["/etc", "/sys", "/proc", "/dev", "/boot"];

        for forbidden in FORBIDDEN {
            let forbidden_path = Path::new(forbidden);

            if path.starts_with(forbidden_path) || canonical.starts_with(forbidden_path) {
                return Err(GvcError::ProjectValidation(format!(
                    "Access to system directory '{}' is not allowed",
                    forbidden
                )));
            }

            if let Ok(canonical_forbidden) = forbidden_path.canonicalize() {
                if canonical.starts_with(&canonical_forbidden) {
                    return Err(GvcError::ProjectValidation(format!(
                        "Access to system directory '{}' is not allowed",
                        forbidden
                    )));
                }
            }
        }

        Ok(canonical)
    }

    /// Ensures the file path resides inside the provided base directory.
    pub fn validate_file_path(
        file_path: impl AsRef<Path>,
        base_dir: impl AsRef<Path>,
    ) -> Result<PathBuf> {
        let file_path = file_path.as_ref();
        let base_dir = base_dir.as_ref();

        let canonical_file = file_path.canonicalize().map_err(|e| {
            GvcError::ProjectValidation(format!("Invalid file path '{}': {e}", file_path.display()))
        })?;

        let canonical_base = base_dir.canonicalize().map_err(|e| {
            GvcError::ProjectValidation(format!(
                "Invalid base directory '{}': {e}",
                base_dir.display()
            ))
        })?;

        if !canonical_file.starts_with(&canonical_base) {
            return Err(GvcError::ProjectValidation(
                "File path is outside the allowed directory".to_string(),
            ));
        }

        Ok(canonical_file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn validate_project_path_accepts_directory() {
        let dir = tempdir().unwrap();
        assert!(PathValidator::validate_project_path(dir.path()).is_ok());
    }

    #[test]
    fn validate_project_path_rejects_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("file.txt");
        fs::write(&file_path, "test").unwrap();
        let err = PathValidator::validate_project_path(&file_path).unwrap_err();
        assert!(matches!(err, GvcError::ProjectValidation(_)));
    }

    #[test]
    fn validate_project_path_rejects_system_directory() {
        assert!(PathValidator::validate_project_path("/etc").is_err());
    }

    #[test]
    fn validate_file_path_rejects_traversal() {
        let dir = tempdir().unwrap();
        let base = dir.path();
        let outside = Path::new("/tmp");
        let result = PathValidator::validate_file_path(outside, base);
        assert!(result.is_err());
    }
}
