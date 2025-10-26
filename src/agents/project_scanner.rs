use crate::error::{GvcError, Result};
use std::path::{Path, PathBuf};

/// ProjectScannerAgent validates the project structure
pub struct ProjectScannerAgent {
    project_path: PathBuf,
}

impl ProjectScannerAgent {
    pub fn new<P: AsRef<Path>>(project_path: P) -> Self {
        Self {
            project_path: project_path.as_ref().to_path_buf(),
        }
    }

    /// Validates the project structure
    pub fn validate(&self) -> Result<ProjectInfo> {
        // Check for Gradle wrapper
        let gradlew_exists = self.project_path.join("gradlew").exists()
            || self.project_path.join("gradlew.bat").exists();

        if !gradlew_exists {
            return Err(GvcError::ProjectValidation(
                "Gradle wrapper (gradlew or gradlew.bat) not found".to_string(),
            ));
        }

        // Check for libs.versions.toml
        let toml_path = self.project_path.join("gradle/libs.versions.toml");
        if !toml_path.exists() {
            return Err(GvcError::ProjectValidation(
                "gradle/libs.versions.toml not found".to_string(),
            ));
        }

        // Check for Git repository
        let git_dir = self.project_path.join(".git");
        let is_git_repo = git_dir.exists() && git_dir.is_dir();

        Ok(ProjectInfo {
            project_path: self.project_path.clone(),
            toml_path,
            has_git: is_git_repo,
            gradlew_path: if cfg!(target_os = "windows") {
                self.project_path.join("gradlew.bat")
            } else {
                self.project_path.join("gradlew")
            },
        })
    }
}

#[derive(Debug, Clone)]
pub struct ProjectInfo {
    #[allow(dead_code)]
    pub project_path: PathBuf,
    pub toml_path: PathBuf,
    pub has_git: bool,
    #[allow(dead_code)]
    pub gradlew_path: PathBuf,
}
