use crate::error::{GvcError, Result};
use crate::utils::path_validator::PathValidator;
use jiff::Zoned;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// VersionControlAgent handles Git operations with hardened input validation.
pub struct VersionControlAgent {
    project_path: PathBuf,
}

impl VersionControlAgent {
    pub fn new<P: AsRef<Path>>(project_path: P) -> Result<Self> {
        let project_path = Self::validate_git_path(project_path.as_ref())?;
        Ok(Self { project_path })
    }

    /// Check if the working directory is clean
    pub fn is_working_directory_clean(&self) -> Result<bool> {
        let output = self.run_git(&["status", "--porcelain"])?;
        Self::ensure_success(&output, "git status")?;
        Ok(output.stdout.is_empty())
    }

    /// Create a new branch for the update
    pub fn create_update_branch(&self) -> Result<String> {
        let branch_name = self.create_safe_branch_name();
        let output = self.run_git(&["checkout", "-b", &branch_name])?;
        Self::ensure_success(&output, "git checkout -b")?;
        Ok(branch_name)
    }

    /// Stage the modified libs.versions.toml file
    pub fn stage_version_catalog(&self) -> Result<()> {
        let catalog_path = self.project_path.join("gradle/libs.versions.toml");
        PathValidator::validate_file_path(&catalog_path, &self.project_path).map_err(|err| {
            GvcError::GitOperation(format!("Refusing to stage unsafe path: {err}"))
        })?;

        let output = self.run_git(&["add", "gradle/libs.versions.toml"])?;
        Self::ensure_success(&output, "git add")?;
        Ok(())
    }

    /// Commit the changes with a standard message
    pub fn commit_updates(&self) -> Result<()> {
        let message = "chore(deps): update dependencies to latest versions";
        let output = self.run_git(&["commit", "-m", message])?;
        Self::ensure_success(&output, "git commit")?;
        Ok(())
    }

    /// Full workflow: create branch, stage, and commit
    pub fn commit_to_new_branch(&self) -> Result<String> {
        let branch_name = self.create_update_branch()?;
        self.stage_version_catalog()?;
        self.commit_updates()?;
        Ok(branch_name)
    }

    fn run_git(&self, args: &[&str]) -> Result<Output> {
        Command::new("git")
            .current_dir(&self.project_path)
            .args(args)
            .output()
            .map_err(|e| {
                GvcError::GitOperation(format!(
                    "Failed to execute git command '{}': {e}",
                    args.join(" ")
                ))
            })
    }

    fn ensure_success(output: &Output, command: &str) -> Result<()> {
        if output.status.success() {
            return Ok(());
        }

        Err(GvcError::GitOperation(format!(
            "{} failed: {}",
            command,
            String::from_utf8_lossy(&output.stderr)
        )))
    }

    fn validate_git_path(path: &Path) -> Result<PathBuf> {
        let dangerous = [';', '|', '&', '$', '`', '\n', '\r'];
        let path_str = path.to_string_lossy();
        if let Some(ch) = dangerous.iter().find(|c| path_str.contains(**c)) {
            return Err(GvcError::GitOperation(format!(
                "Path contains dangerous character: '{}'",
                ch
            )));
        }

        if !path.is_absolute() {
            return Err(GvcError::GitOperation(
                "Only absolute paths are allowed for Git operations".to_string(),
            ));
        }

        PathValidator::validate_project_path(path)
            .map_err(|err| GvcError::GitOperation(format!("Invalid Git path: {}", err)))
    }

    fn create_safe_branch_name(&self) -> String {
        let date = Zoned::now().strftime("%Y-%m-%d").to_string();
        let mut branch_name = format!("deps/update-{date}");

        branch_name = branch_name
            .chars()
            .map(|c| match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '/' => c,
                _ => '-',
            })
            .collect();

        if branch_name.len() > 50 {
            branch_name.truncate(50);
        }

        branch_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::{tempdir, tempdir_in};

    #[test]
    fn rejects_relative_paths() {
        let cwd = std::env::current_dir().unwrap();
        let temp = tempdir_in(&cwd).unwrap();
        let relative = PathBuf::from(temp.path().file_name().unwrap());
        assert!(VersionControlAgent::new(&relative).is_err());
    }

    #[test]
    fn rejects_dangerous_paths() {
        let dir = tempdir().unwrap();
        let dangerous = dir.path().join("sub;dir");
        fs::create_dir_all(&dangerous).unwrap();
        assert!(VersionControlAgent::new(dangerous).is_err());
    }

    #[test]
    fn creates_safe_branch() {
        let dir = tempdir().unwrap();
        let canonical = dir.path().canonicalize().unwrap();
        let agent = VersionControlAgent::new(&canonical).unwrap();
        let branch = agent.create_safe_branch_name();
        assert!(branch.starts_with("deps/update-"));
        assert!(
            branch
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/'))
        );
    }
}
