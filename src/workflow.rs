mod add_resolver;
mod presenter;

use crate::agents::{
    AddTargetKind, CatalogAuditor, CatalogEditor, CatalogExplainer, DependencyUpdater,
    KotlinDoctor, ProjectScannerAgent, VersionControlAgent,
};
use crate::cli::OutputFormat;
use crate::error::{GvcError, Result};
use crate::gradle::GradleConfigParser;
use crate::utils::path_validator::PathValidator;
use add_resolver::resolve_add_coordinate;
use colored::Colorize;
use presenter::{
    add_target_json, dependencies_json, doctor_json, findings_json, print_add_result,
    print_audit_report, print_available_updates, print_dependencies, print_doctor_report,
    print_json, print_outdated_report, print_repositories, print_update_report, print_why_report,
    updates_json, why_json,
};
use regex::Regex;
use serde_json::json;
use std::path::Path;

#[derive(Clone, Copy, Debug)]
pub struct RunOptions<'a> {
    pub catalog_path: Option<&'a str>,
    pub output_format: OutputFormat,
}

impl RunOptions<'_> {
    fn is_json(&self) -> bool {
        self.output_format == OutputFormat::Json
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkflowStatus {
    Success,
    UpdatesAvailable,
    IssuesFound,
}

#[derive(Clone, Copy, Debug)]
pub struct AddOptions<'a> {
    pub plugin_flag: bool,
    pub coordinate: &'a str,
    pub alias_override: Option<&'a str>,
    pub version_alias_override: Option<&'a str>,
    pub stable_only: bool,
    pub allow_version_alias_update: bool,
}

#[derive(Clone, Debug)]
pub struct UpdateOptions {
    pub interactive: bool,
    pub target_filter: Option<String>,
    pub stable_only: bool,
    pub no_git: bool,
    pub dry_run: bool,
    pub explicit_apply: bool,
}

impl WorkflowStatus {
    pub fn exit_code(self) -> i32 {
        match self {
            Self::Success => 0,
            Self::UpdatesAvailable | Self::IssuesFound => 2,
        }
    }
}

fn validate_project(
    project_path: &Path,
    options: &RunOptions<'_>,
) -> Result<crate::agents::project_scanner::ProjectInfo> {
    let scanner = ProjectScannerAgent::new(project_path);
    match options.catalog_path {
        Some(catalog_path) => scanner.validate_with_catalog(Some(Path::new(catalog_path))),
        None => scanner.validate(),
    }
}

fn load_catalog_document(catalog_path: &Path) -> Result<toml_edit::DocumentMut> {
    let content = std::fs::read_to_string(catalog_path)
        .map_err(|e| GvcError::TomlParsing(format!("Failed to read catalog: {}", e)))?;

    content
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| GvcError::TomlParsing(format!("Failed to parse TOML: {}", e)))
}

/// Add a new dependency or plugin entry to the version catalog
pub fn execute_add<P: AsRef<Path>>(
    project_path: P,
    run_options: RunOptions<'_>,
    add_options: AddOptions<'_>,
) -> Result<WorkflowStatus> {
    let project_path = PathValidator::validate_project_path(project_path)?;
    crate::outln!(
        "{}",
        "Adding entry to Gradle version catalog...".cyan().bold()
    );

    crate::outln!("\n{}", "1. Validating project structure...".yellow());
    let project_info = validate_project(&project_path, &run_options)?;
    crate::outln!("{}", "✓ Project structure is valid".green());

    let target = resolve_add_target(add_options.plugin_flag, add_options.coordinate)?;

    crate::outln!(
        "\n{}",
        "2. Reading Gradle repository configuration...".yellow()
    );
    let gradle_parser = GradleConfigParser::new(&project_path);
    let gradle_config = gradle_parser.parse()?;
    print_repositories(&gradle_config.repositories);

    crate::outln!(
        "\n{}",
        "3. Validating coordinate against remote repositories...".yellow()
    );
    let resolved_coordinate = resolve_add_coordinate(
        target,
        add_options.coordinate,
        &gradle_config.repositories,
        add_options.stable_only,
    )?;

    crate::outln!("\n{}", "4. Writing to version catalog...".yellow());
    let editor = CatalogEditor::new(&project_info.toml_path);
    let result = match target {
        AddTargetKind::Library => editor.add_library(
            &resolved_coordinate,
            add_options.alias_override,
            add_options.version_alias_override,
            add_options.allow_version_alias_update,
        ),
        AddTargetKind::Plugin => editor.add_plugin(
            &resolved_coordinate,
            add_options.alias_override,
            add_options.version_alias_override,
            add_options.allow_version_alias_update,
        ),
    }?;

    print_add_result(&result);

    if run_options.is_json() {
        print_json(&json!({
            "status": "added",
            "command": "add",
            "catalog": project_info.toml_path.display().to_string(),
            "target": add_target_json(result.target),
            "alias": result.alias,
            "version_alias": result.version_alias,
        }))?;
    }

    crate::outln!("\n{}", "✨ Entry added successfully!".green().bold());
    Ok(WorkflowStatus::Success)
}

fn resolve_add_target(plugin_flag: bool, coordinate: &str) -> Result<AddTargetKind> {
    if coordinate.trim().is_empty() {
        return Err(GvcError::ProjectValidation(
            "Coordinate is required. Example: gvc add group:artifact:version".into(),
        ));
    }

    if plugin_flag {
        Ok(AddTargetKind::Plugin)
    } else {
        Ok(AddTargetKind::Library)
    }
}

/// Execute the update workflow
pub fn execute_update<P: AsRef<Path>>(
    project_path: P,
    run_options: RunOptions<'_>,
    update_options: UpdateOptions,
) -> Result<WorkflowStatus> {
    if update_options.interactive && crate::utils::output::is_quiet() {
        return Err(GvcError::ProjectValidation(
            "--interactive cannot be combined with --format json or --quiet".into(),
        ));
    }

    let project_path = PathValidator::validate_project_path(project_path)?;
    let mode = if update_options.dry_run {
        "dry-run"
    } else {
        "apply"
    };
    crate::outln!(
        "{}",
        format!("Starting dependency update process ({mode})...")
            .cyan()
            .bold()
    );

    crate::outln!("\n{}", "1. Validating project structure...".yellow());
    let project_info = validate_project(&project_path, &run_options)?;
    crate::outln!("{}", "✓ Project structure is valid".green());

    if project_info.has_git && !update_options.no_git && !update_options.dry_run {
        crate::outln!("\n{}", "2. Checking Git status...".yellow());
        let git_agent = VersionControlAgent::new(&project_path)?;

        if !git_agent.is_working_directory_clean()? {
            crate::outln!(
                "{}",
                "⚠ Warning: Working directory has uncommitted changes".red()
            );
            return Err(GvcError::ProjectValidation(
                "Working directory has uncommitted changes. Commit or stash them, or re-run with --no-git.".into(),
            ));
        }
        crate::outln!("{}", "✓ Working directory is clean".green());
    } else if !update_options.no_git && !update_options.dry_run {
        crate::outln!(
            "\n{}",
            "2. Git repository not detected, skipping Git checks".yellow()
        );
    }

    crate::outln!(
        "\n{}",
        "3. Reading Gradle repository configuration...".yellow()
    );
    let gradle_parser = GradleConfigParser::new(&project_path);
    let gradle_config = gradle_parser.parse()?;
    print_repositories(&gradle_config.repositories);

    crate::outln!(
        "\n{}",
        if update_options.dry_run {
            "4. Checking dependencies..."
        } else {
            "4. Updating dependencies..."
        }
        .yellow()
    );
    let updater = DependencyUpdater::with_repositories(gradle_config.repositories)?;
    let report = if update_options.dry_run {
        preview_updates(
            &updater,
            &project_info.toml_path,
            update_options.stable_only,
            update_options.target_filter.as_deref(),
        )?
    } else {
        let Some(report) = update_catalog(
            &updater,
            &project_info.toml_path,
            update_options.stable_only,
            update_options.interactive,
            update_options.target_filter.as_deref(),
        )?
        else {
            return Ok(WorkflowStatus::Success);
        };
        report
    };

    crate::outln!(
        "{}",
        if update_options.dry_run {
            "✓ Dry run completed"
        } else {
            "✓ Update completed"
        }
        .green()
    );
    if update_options.dry_run {
        print_available_updates(&report, update_options.stable_only);
    } else {
        print_update_report(&report);
    }

    let mut branch_name = None;
    if project_info.has_git
        && !update_options.no_git
        && !update_options.dry_run
        && !report.is_empty()
    {
        crate::outln!("\n{}", "5. Creating Git commit...".yellow());
        let git_agent = VersionControlAgent::new(&project_path)?;
        let created_branch = git_agent.commit_to_new_branch()?;
        crate::outln!(
            "{}",
            format!("✓ Changes committed to branch: {}", created_branch).green()
        );
        branch_name = Some(created_branch);
    } else if report.is_empty() {
        crate::outln!(
            "\n{}",
            if update_options.dry_run {
                "No updates were found"
            } else {
                "No updates were applied"
            }
            .yellow()
        );
    }

    if run_options.is_json() {
        print_json(&json!({
            "status": if report.is_empty() {
                "up_to_date"
            } else if update_options.dry_run {
                "updates_available"
            } else {
                "updated"
            },
            "command": "update",
            "mode": mode,
            "catalog": project_info.toml_path.display().to_string(),
            "stable_only": update_options.stable_only,
            "target": update_options.target_filter,
            "explicit_apply": update_options.explicit_apply,
            "git_branch": branch_name,
            "updates": updates_json(&report),
        }))?;
    }

    crate::outln!(
        "\n{}",
        "✨ Update process completed successfully!".green().bold()
    );
    Ok(WorkflowStatus::Success)
}

fn preview_updates(
    updater: &DependencyUpdater,
    toml_path: &Path,
    stable_only: bool,
    target_filter: Option<&str>,
) -> Result<crate::agents::UpdateReport> {
    let report = updater.check_for_updates(toml_path, stable_only)?;
    if let Some(pattern) = target_filter {
        filter_update_report(report, pattern)
    } else {
        Ok(report)
    }
}

fn update_catalog(
    updater: &DependencyUpdater,
    toml_path: &Path,
    stable_only: bool,
    interactive: bool,
    filter: Option<&str>,
) -> Result<Option<crate::agents::UpdateReport>> {
    let result = match filter {
        Some(pattern) => {
            updater.update_targeted_dependency(toml_path, stable_only, interactive, pattern)
        }
        None => updater.update_version_catalog(toml_path, stable_only, interactive),
    };

    match result {
        Ok(report) => Ok(Some(report)),
        Err(GvcError::UserCancelled) => {
            crate::outln!("\n{}", "Update cancelled by user.".yellow());
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn filter_update_report(
    report: crate::agents::UpdateReport,
    pattern: &str,
) -> Result<crate::agents::UpdateReport> {
    let matcher = AliasMatcher::new(pattern)?;
    let mut filtered = crate::agents::UpdateReport::new();

    for (alias, (old, new)) in report.version_updates {
        if matcher.matches(&alias) {
            filtered.add_version_update(alias, old, new);
        }
    }

    for (alias, (old, new)) in report.library_updates {
        if matcher.matches(&alias) {
            filtered.add_library_update(alias, old, new);
        }
    }

    for (alias, (old, new)) in report.plugin_updates {
        if matcher.matches(&alias) {
            filtered.add_plugin_update(alias, old, new);
        }
    }

    if filtered.is_empty() {
        crate::outln!(
            "{}",
            format!("No updates matched target pattern '{}'.", pattern).yellow()
        );
    }

    Ok(filtered)
}

#[derive(Debug)]
struct AliasMatcher {
    regex: Regex,
}

impl AliasMatcher {
    fn new(pattern: &str) -> Result<Self> {
        let trimmed = pattern.trim();
        if trimmed.is_empty() {
            return Err(GvcError::ProjectValidation(
                "Target pattern cannot be empty".into(),
            ));
        }

        let adjusted = if trimmed.contains(['*', '?']) {
            trimmed.to_string()
        } else {
            format!("*{}*", trimmed)
        };

        Ok(Self {
            regex: compile_glob(&adjusted)?,
        })
    }

    fn matches(&self, alias: &str) -> bool {
        self.regex.is_match(alias)
    }
}

fn compile_glob(pattern: &str) -> Result<Regex> {
    let mut regex = String::from("(?i)^");
    for ch in pattern.chars() {
        match ch {
            '*' => regex.push_str(".*"),
            '?' => regex.push('.'),
            '.' | '+' | '(' | ')' | '|' | '^' | '$' | '{' | '}' | '[' | ']' | '\\' => {
                regex.push('\\');
                regex.push(ch);
            }
            _ => regex.push(ch),
        }
    }
    regex.push('$');

    Regex::new(&regex).map_err(|e| {
        GvcError::ProjectValidation(format!("Invalid target pattern '{}': {}", pattern, e))
    })
}

fn collect_available_updates(
    project_path: &Path,
    options: &RunOptions<'_>,
    stable_only: bool,
) -> Result<(
    crate::agents::project_scanner::ProjectInfo,
    crate::agents::UpdateReport,
)> {
    crate::outln!("\n{}", "1. Validating project structure...".yellow());
    let project_info = validate_project(project_path, options)?;
    crate::outln!("{}", "✓ Project structure is valid".green());

    crate::outln!(
        "\n{}",
        "2. Reading Gradle repository configuration...".yellow()
    );
    let gradle_parser = GradleConfigParser::new(project_path);
    let gradle_config = gradle_parser.parse()?;
    print_repositories(&gradle_config.repositories);

    crate::outln!("\n{}", "3. Checking for available updates...".yellow());
    let updater = DependencyUpdater::with_repositories(gradle_config.repositories)?;
    let report = updater.check_for_updates(&project_info.toml_path, stable_only)?;

    Ok((project_info, report))
}

/// Execute the check workflow (dry-run)
pub fn execute_check<P: AsRef<Path>>(
    project_path: P,
    options: RunOptions<'_>,
    stable_only: bool,
    fail_on_updates: bool,
) -> Result<WorkflowStatus> {
    let project_path = PathValidator::validate_project_path(project_path)?;
    let version_channel = if stable_only { "stable" } else { "all" };
    crate::outln!(
        "{}",
        format!(
            "Checking for available updates ({} versions)...",
            version_channel
        )
        .cyan()
        .bold()
    );

    let (project_info, report) = collect_available_updates(&project_path, &options, stable_only)?;

    crate::outln!("{}", "✓ Check completed".green());
    print_available_updates(&report, stable_only);

    if options.is_json() {
        print_json(&json!({
            "status": if report.is_empty() { "up_to_date" } else { "updates_available" },
            "command": "check",
            "catalog": project_info.toml_path.display().to_string(),
            "stable_only": stable_only,
            "fail_on_updates": fail_on_updates,
            "updates": updates_json(&report),
            "apply_command": if stable_only { "gvc update" } else { "gvc update --no-stable-only" },
        }))?;
    }

    if fail_on_updates && !report.is_empty() {
        Ok(WorkflowStatus::UpdatesAvailable)
    } else {
        Ok(WorkflowStatus::Success)
    }
}

/// Execute the outdated workflow - package-manager style update view.
pub fn execute_outdated<P: AsRef<Path>>(
    project_path: P,
    options: RunOptions<'_>,
    stable_only: bool,
    fail_on_updates: bool,
) -> Result<WorkflowStatus> {
    let project_path = PathValidator::validate_project_path(project_path)?;
    let version_channel = if stable_only { "stable" } else { "all" };
    crate::outln!(
        "{}",
        format!(
            "Checking outdated catalog entries ({} versions)...",
            version_channel
        )
        .cyan()
        .bold()
    );

    let (project_info, report) = collect_available_updates(&project_path, &options, stable_only)?;

    crate::outln!("{}", "✓ Outdated check completed".green());
    print_outdated_report(&report, stable_only);

    if options.is_json() {
        print_json(&json!({
            "status": if report.is_empty() { "up_to_date" } else { "updates_available" },
            "command": "outdated",
            "catalog": project_info.toml_path.display().to_string(),
            "stable_only": stable_only,
            "fail_on_updates": fail_on_updates,
            "outdated": updates_json(&report),
            "update_command": if stable_only { "gvc update" } else { "gvc update --no-stable-only" },
        }))?;
    }

    if fail_on_updates && !report.is_empty() {
        Ok(WorkflowStatus::UpdatesAvailable)
    } else {
        Ok(WorkflowStatus::Success)
    }
}

/// Execute the list workflow - display all dependencies
pub fn execute_list<P: AsRef<Path>>(
    project_path: P,
    options: RunOptions<'_>,
) -> Result<WorkflowStatus> {
    let project_path = PathValidator::validate_project_path(project_path)?;
    crate::outln!(
        "{}",
        "Listing dependencies in version catalog...".cyan().bold()
    );

    crate::outln!("\n{}", "1. Validating project structure...".yellow());
    let project_info = validate_project(&project_path, &options)?;
    crate::outln!("{}", "✓ Project structure is valid".green());

    crate::outln!("\n{}", "2. Reading version catalog...".yellow());
    let doc = load_catalog_document(&project_info.toml_path)?;

    crate::outln!("{}", "✓ Catalog loaded".green());
    print_dependencies(&doc);

    if options.is_json() {
        let catalog = dependencies_json(&doc);
        print_json(&json!({
            "status": "ok",
            "command": "list",
            "catalog": project_info.toml_path.display().to_string(),
            "data": catalog,
        }))?;
    }

    Ok(WorkflowStatus::Success)
}

/// Explain a catalog entry by alias or coordinate.
pub fn execute_why<P: AsRef<Path>>(
    project_path: P,
    options: RunOptions<'_>,
    query: &str,
) -> Result<WorkflowStatus> {
    let project_path = PathValidator::validate_project_path(project_path)?;
    crate::outln!("{}", "Explaining version catalog entry...".cyan().bold());

    crate::outln!("\n{}", "1. Validating project structure...".yellow());
    let project_info = validate_project(&project_path, &options)?;
    crate::outln!("{}", "✓ Project structure is valid".green());

    crate::outln!("\n{}", "2. Reading version catalog...".yellow());
    let doc = load_catalog_document(&project_info.toml_path)?;
    crate::outln!("{}", "✓ Catalog loaded".green());

    crate::outln!("\n{}", "3. Explaining catalog entry...".yellow());
    let report = CatalogExplainer::explain(&doc, query)?;
    print_why_report(&report);

    if options.is_json() {
        print_json(&json!({
            "status": "ok",
            "command": "why",
            "catalog": project_info.toml_path.display().to_string(),
            "why": why_json(&report)?,
        }))?;
    }

    Ok(WorkflowStatus::Success)
}

/// Execute the catalog audit workflow.
pub fn execute_audit<P: AsRef<Path>>(
    project_path: P,
    options: RunOptions<'_>,
    fail_on_issues: bool,
) -> Result<WorkflowStatus> {
    let project_path = PathValidator::validate_project_path(project_path)?;
    crate::outln!("{}", "Auditing Gradle version catalog...".cyan().bold());

    crate::outln!("\n{}", "1. Validating project structure...".yellow());
    let project_info = validate_project(&project_path, &options)?;
    crate::outln!("{}", "✓ Project structure is valid".green());

    crate::outln!("\n{}", "2. Reading version catalog...".yellow());
    let doc = load_catalog_document(&project_info.toml_path)?;
    crate::outln!("{}", "✓ Catalog loaded".green());

    crate::outln!("\n{}", "3. Checking catalog quality...".yellow());
    let report = CatalogAuditor::analyze(&doc);
    print_audit_report(&report);

    if options.is_json() {
        print_json(&json!({
            "status": if report.has_issues() { "issues_found" } else { "ok" },
            "command": "audit",
            "catalog": project_info.toml_path.display().to_string(),
            "fail_on_issues": fail_on_issues,
            "audit": findings_json(&report)?,
        }))?;
    }

    if fail_on_issues && report.has_issues() {
        Ok(WorkflowStatus::IssuesFound)
    } else {
        Ok(WorkflowStatus::Success)
    }
}

/// Execute the Kotlin/Android catalog diagnostic workflow.
pub fn execute_doctor<P: AsRef<Path>>(
    project_path: P,
    options: RunOptions<'_>,
    fail_on_issues: bool,
) -> Result<WorkflowStatus> {
    let project_path = PathValidator::validate_project_path(project_path)?;
    crate::outln!(
        "{}",
        "Running Kotlin/Android catalog doctor...".cyan().bold()
    );

    crate::outln!("\n{}", "1. Validating project structure...".yellow());
    let project_info = validate_project(&project_path, &options)?;
    crate::outln!("{}", "✓ Project structure is valid".green());

    crate::outln!("\n{}", "2. Reading version catalog...".yellow());
    let doc = load_catalog_document(&project_info.toml_path)?;
    crate::outln!("{}", "✓ Catalog loaded".green());

    crate::outln!("\n{}", "3. Checking Kotlin/Android consistency...".yellow());
    let report = KotlinDoctor::analyze(&doc);
    print_doctor_report(&report);

    if options.is_json() {
        print_json(&json!({
            "status": if report.has_issues() { "issues_found" } else { "ok" },
            "command": "doctor",
            "catalog": project_info.toml_path.display().to_string(),
            "fail_on_issues": fail_on_issues,
            "doctor": doctor_json(&report)?,
        }))?;
    }

    if fail_on_issues && report.has_issues() {
        Ok(WorkflowStatus::IssuesFound)
    } else {
        Ok(WorkflowStatus::Success)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::UpdateReport;
    use std::fs;

    #[test]
    fn filter_update_report_matches_alias_case_insensitively() {
        let mut report = UpdateReport::new();
        report.add_version_update("kotlin".into(), "1.9.0".into(), "2.0.0".into());
        report.add_library_update("okhttp".into(), "4.11.0".into(), "4.12.0".into());
        report.add_plugin_update("ksp".into(), "1.0.0".into(), "1.1.0".into());

        let filtered = filter_update_report(report, "KOT").unwrap();

        assert_eq!(filtered.total_updates(), 1);
        assert!(filtered.version_updates.contains_key("kotlin"));
    }

    #[test]
    fn filter_update_report_accepts_globs() {
        let mut report = UpdateReport::new();
        report.add_library_update("androidx-core".into(), "1.0.0".into(), "1.1.0".into());
        report.add_library_update("androidx-appcompat".into(), "1.0.0".into(), "1.1.0".into());
        report.add_library_update("okhttp".into(), "4.0.0".into(), "4.1.0".into());

        let filtered = filter_update_report(report, "androidx-*").unwrap();

        assert_eq!(filtered.total_updates(), 2);
        assert!(filtered.library_updates.contains_key("androidx-core"));
        assert!(filtered.library_updates.contains_key("androidx-appcompat"));
    }

    #[test]
    fn alias_matcher_rejects_empty_pattern() {
        let err = AliasMatcher::new("   ").unwrap_err();
        assert!(matches!(err, GvcError::ProjectValidation(_)));
    }

    #[test]
    fn execute_audit_returns_issues_found_when_fail_on_issues() {
        crate::utils::output::init(true);
        let project = tempfile::tempdir().unwrap();
        fs::write(project.path().join("gradlew"), "").unwrap();
        fs::create_dir(project.path().join("gradle")).unwrap();
        fs::write(
            project.path().join("gradle/libs.versions.toml"),
            r#"
[libraries]
core = { module = "androidx.core:core-ktx", version = "1.12.0" }
coreAgain = { group = "androidx.core", name = "core-ktx", version = "1.12.0" }
"#,
        )
        .unwrap();

        let status = execute_audit(
            project.path(),
            RunOptions {
                catalog_path: None,
                output_format: OutputFormat::Text,
            },
            true,
        )
        .unwrap();

        assert_eq!(status, WorkflowStatus::IssuesFound);
    }

    #[test]
    fn execute_why_explains_existing_alias() {
        crate::utils::output::init(true);
        let project = tempfile::tempdir().unwrap();
        fs::write(project.path().join("gradlew"), "").unwrap();
        fs::create_dir(project.path().join("gradle")).unwrap();
        fs::write(
            project.path().join("gradle/libs.versions.toml"),
            r#"
[versions]
core = "1.12.0"

[libraries]
androidxCore = { module = "androidx.core:core-ktx", version = { ref = "core" } }
"#,
        )
        .unwrap();

        let status = execute_why(
            project.path(),
            RunOptions {
                catalog_path: None,
                output_format: OutputFormat::Json,
            },
            "androidxCore",
        )
        .unwrap();

        assert_eq!(status, WorkflowStatus::Success);
    }
}
