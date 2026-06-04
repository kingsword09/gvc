mod add_resolver;
mod presenter;

use crate::agents::{
    AddTargetKind, CatalogEditor, DependencyUpdater, ProjectScannerAgent, VersionControlAgent,
};
use crate::error::{GvcError, Result};
use crate::gradle::GradleConfigParser;
use crate::utils::path_validator::PathValidator;
use add_resolver::resolve_add_coordinate;
use colored::Colorize;
use presenter::{
    print_add_result, print_available_updates, print_dependencies, print_repositories,
    print_update_report,
};
use std::path::Path;

/// Add a new dependency or plugin entry to the version catalog
pub fn execute_add<P: AsRef<Path>>(
    project_path: P,
    plugin_flag: bool,
    _library_flag: bool,
    coordinate: &str,
    alias_override: Option<&str>,
    version_alias_override: Option<&str>,
    stable_only: bool,
) -> Result<()> {
    let project_path = PathValidator::validate_project_path(project_path)?;
    println!(
        "{}",
        "Adding entry to Gradle version catalog...".cyan().bold()
    );

    println!("\n{}", "1. Validating project structure...".yellow());
    let scanner = ProjectScannerAgent::new(&project_path);
    let project_info = scanner.validate()?;
    println!("{}", "✓ Project structure is valid".green());

    let target = resolve_add_target(plugin_flag, coordinate)?;

    println!(
        "\n{}",
        "2. Reading Gradle repository configuration...".yellow()
    );
    let gradle_parser = GradleConfigParser::new(&project_path);
    let gradle_config = gradle_parser.parse()?;
    print_repositories(&gradle_config.repositories);

    println!(
        "\n{}",
        "3. Validating coordinate against remote repositories...".yellow()
    );
    let resolved_coordinate =
        resolve_add_coordinate(target, coordinate, &gradle_config.repositories, stable_only)?;

    println!("\n{}", "4. Writing to version catalog...".yellow());
    let editor = CatalogEditor::new(&project_info.toml_path);
    let result = match target {
        AddTargetKind::Library => {
            editor.add_library(&resolved_coordinate, alias_override, version_alias_override)
        }
        AddTargetKind::Plugin => {
            editor.add_plugin(&resolved_coordinate, alias_override, version_alias_override)
        }
    }?;

    print_add_result(&result);

    println!("\n{}", "✨ Entry added successfully!".green().bold());
    Ok(())
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
    interactive: bool,
    filter: Option<String>,
    stable_only: bool,
    no_git: bool,
) -> Result<()> {
    let project_path = PathValidator::validate_project_path(project_path)?;
    println!("{}", "Starting dependency update process...".cyan().bold());

    println!("\n{}", "1. Validating project structure...".yellow());
    let scanner = ProjectScannerAgent::new(&project_path);
    let project_info = scanner.validate()?;
    println!("{}", "✓ Project structure is valid".green());

    if project_info.has_git && !no_git {
        println!("\n{}", "2. Checking Git status...".yellow());
        let git_agent = VersionControlAgent::new(&project_path)?;

        if !git_agent.is_working_directory_clean()? {
            println!(
                "{}",
                "⚠ Warning: Working directory has uncommitted changes".red()
            );
            println!("Please commit or stash your changes before proceeding.");
            return Ok(());
        }
        println!("{}", "✓ Working directory is clean".green());
    } else if !no_git {
        println!(
            "\n{}",
            "2. Git repository not detected, skipping Git checks".yellow()
        );
    }

    println!(
        "\n{}",
        "3. Reading Gradle repository configuration...".yellow()
    );
    let gradle_parser = GradleConfigParser::new(&project_path);
    let gradle_config = gradle_parser.parse()?;
    print_repositories(&gradle_config.repositories);

    println!("\n{}", "4. Updating dependencies...".yellow());
    let updater = DependencyUpdater::with_repositories(gradle_config.repositories)?;
    let Some(report) = update_catalog(
        &updater,
        &project_info.toml_path,
        stable_only,
        interactive,
        filter.as_deref(),
    )?
    else {
        return Ok(());
    };

    println!("{}", "✓ Update completed".green());
    print_update_report(&report);

    if project_info.has_git && !no_git && !report.is_empty() {
        println!("\n{}", "5. Creating Git commit...".yellow());
        let git_agent = VersionControlAgent::new(&project_path)?;
        let branch_name = git_agent.commit_to_new_branch()?;
        println!(
            "{}",
            format!("✓ Changes committed to branch: {}", branch_name).green()
        );
    } else if report.is_empty() {
        println!("\n{}", "No updates were applied".yellow());
    }

    println!(
        "\n{}",
        "✨ Update process completed successfully!".green().bold()
    );
    Ok(())
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
            println!("\n{}", "Update cancelled by user.".yellow());
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

/// Execute the check workflow (dry-run)
pub fn execute_check<P: AsRef<Path>>(project_path: P, stable_only: bool) -> Result<()> {
    let project_path = PathValidator::validate_project_path(project_path)?;
    let version_channel = if stable_only { "stable" } else { "all" };
    println!(
        "{}",
        format!(
            "Checking for available updates ({} versions)...",
            version_channel
        )
        .cyan()
        .bold()
    );

    println!("\n{}", "1. Validating project structure...".yellow());
    let scanner = ProjectScannerAgent::new(&project_path);
    let project_info = scanner.validate()?;
    println!("{}", "✓ Project structure is valid".green());

    println!(
        "\n{}",
        "2. Reading Gradle repository configuration...".yellow()
    );
    let gradle_parser = GradleConfigParser::new(&project_path);
    let gradle_config = gradle_parser.parse()?;
    print_repositories(&gradle_config.repositories);

    println!("\n{}", "3. Checking for available updates...".yellow());
    let updater = DependencyUpdater::with_repositories(gradle_config.repositories)?;
    let report = updater.check_for_updates(&project_info.toml_path, stable_only)?;

    println!("{}", "✓ Check completed".green());
    print_available_updates(&report, stable_only);

    Ok(())
}

/// Execute the list workflow - display all dependencies
pub fn execute_list<P: AsRef<Path>>(project_path: P) -> Result<()> {
    let project_path = PathValidator::validate_project_path(project_path)?;
    println!(
        "{}",
        "Listing dependencies in version catalog...".cyan().bold()
    );

    println!("\n{}", "1. Validating project structure...".yellow());
    let scanner = ProjectScannerAgent::new(&project_path);
    let project_info = scanner.validate()?;
    println!("{}", "✓ Project structure is valid".green());

    println!("\n{}", "2. Reading version catalog...".yellow());
    let content = std::fs::read_to_string(&project_info.toml_path)
        .map_err(|e| GvcError::TomlParsing(format!("Failed to read catalog: {}", e)))?;

    let doc = content
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| GvcError::TomlParsing(format!("Failed to parse TOML: {}", e)))?;

    println!("{}", "✓ Catalog loaded".green());
    print_dependencies(&doc);

    Ok(())
}
