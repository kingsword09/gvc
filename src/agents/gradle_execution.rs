use crate::error::{GvcError, Result};
use std::path::Path;
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};

/// GradleExecutionAgent executes Gradle commands
pub struct GradleExecutionAgent {
    gradlew_path: std::path::PathBuf,
    project_path: std::path::PathBuf,
}

impl GradleExecutionAgent {
    pub fn new<P: AsRef<Path>>(gradlew_path: P, project_path: P) -> Self {
        Self {
            gradlew_path: gradlew_path.as_ref().to_path_buf(),
            project_path: project_path.as_ref().to_path_buf(),
        }
    }

    /// Execute version catalog update command
    pub fn execute_version_catalog_update(&self, interactive: bool, stable_only: bool) -> Result<()> {
        let mut args = vec!["nl.littlerobots.version-catalog-update:versionCatalogUpdate"];
        
        if interactive {
            args.push("--interactive");
        }
        if stable_only {
            args.push("--stable-only");
        }

        self.execute_gradle_command(&args)
    }

    /// Execute version catalog apply updates (for interactive mode)
    pub fn execute_apply_updates(&self) -> Result<()> {
        self.execute_gradle_command(&["nl.littlerobots.version-catalog-update:versionCatalogApplyUpdates"])
    }

    /// Execute dependency updates check
    pub fn execute_dependency_updates_check(&self) -> Result<String> {
        let args = vec!["dependencyUpdates", "-DoutputFormatter=json"];
        self.execute_gradle_command(&args)?;

        // Read the JSON report
        let report_path = self.project_path.join("build/dependencyUpdates/report.json");
        if report_path.exists() {
            std::fs::read_to_string(report_path)
                .map_err(|e| GvcError::GradleExecution(format!("Failed to read report: {}", e)))
        } else {
            Err(GvcError::GradleExecution(
                "Dependency updates report not found".to_string(),
            ))
        }
    }

    /// Execute a Gradle command with live output streaming
    fn execute_gradle_command(&self, args: &[&str]) -> Result<()> {
        println!("Executing: {} {}", self.gradlew_path.display(), args.join(" "));

        let mut command = Command::new(&self.gradlew_path);
        command
            .current_dir(&self.project_path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|e| GvcError::GradleExecution(format!("Failed to spawn process: {}", e)))?;

        // Stream stdout
        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    println!("{}", line);
                }
            }
        }

        // Wait for completion
        let status = child
            .wait()
            .map_err(|e| GvcError::GradleExecution(format!("Failed to wait for process: {}", e)))?;

        if !status.success() {
            return Err(GvcError::GradleExecution(format!(
                "Gradle command failed with exit code: {}",
                status.code().unwrap_or(-1)
            )));
        }

        Ok(())
    }
}
