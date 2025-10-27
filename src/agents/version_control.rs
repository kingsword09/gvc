use crate::error::{GvcError, Result};
use jiff::Zoned;
use std::path::Path;
use std::process::Command;

/// VersionControlAgent handles Git operations
pub struct VersionControlAgent {
    project_path: std::path::PathBuf,
}

impl VersionControlAgent {
    pub fn new<P: AsRef<Path>>(project_path: P) -> Self {
        Self {
            project_path: project_path.as_ref().to_path_buf(),
        }
    }

    /// Check if the working directory is clean
    pub fn is_working_directory_clean(&self) -> Result<bool> {
        let output = Command::new("git")
            .current_dir(&self.project_path)
            .args(["status", "--porcelain"])
            .output()
            .map_err(|e| GvcError::GitOperation(format!("Failed to check git status: {}", e)))?;

        if !output.status.success() {
            return Err(GvcError::GitOperation(
                "Failed to check git status".to_string(),
            ));
        }

        Ok(output.stdout.is_empty())
    }

    /// Create a new branch for the update
    pub fn create_update_branch(&self) -> Result<String> {
        let date = Zoned::now().strftime("%Y-%m-%d").to_string();
        let branch_name = format!("deps/update-{}", date);

        // Create and checkout the branch
        let output = Command::new("git")
            .current_dir(&self.project_path)
            .args(["checkout", "-b", &branch_name])
            .output()
            .map_err(|e| GvcError::GitOperation(format!("Failed to create branch: {}", e)))?;

        if !output.status.success() {
            return Err(GvcError::GitOperation(format!(
                "Failed to create branch: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(branch_name)
    }

    /// Stage the modified libs.versions.toml file
    pub fn stage_version_catalog(&self) -> Result<()> {
        let output = Command::new("git")
            .current_dir(&self.project_path)
            .args(["add", "gradle/libs.versions.toml"])
            .output()
            .map_err(|e| GvcError::GitOperation(format!("Failed to stage file: {}", e)))?;

        if !output.status.success() {
            return Err(GvcError::GitOperation(format!(
                "Failed to stage file: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Commit the changes with a standard message
    pub fn commit_updates(&self) -> Result<()> {
        let message = "chore(deps): update dependencies to latest versions";

        let output = Command::new("git")
            .current_dir(&self.project_path)
            .args(["commit", "-m", message])
            .output()
            .map_err(|e| GvcError::GitOperation(format!("Failed to commit: {}", e)))?;

        if !output.status.success() {
            return Err(GvcError::GitOperation(format!(
                "Failed to commit: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Full workflow: create branch, stage, and commit
    pub fn commit_to_new_branch(&self) -> Result<String> {
        let branch_name = self.create_update_branch()?;
        self.stage_version_catalog()?;
        self.commit_updates()?;
        Ok(branch_name)
    }
}
