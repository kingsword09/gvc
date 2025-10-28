use crate::agents::catalog_editor::{parse_library_coordinate, parse_plugin_coordinate};
use crate::agents::{
    AddResult, AddTargetKind, CatalogEditor, DependencyUpdater, ProjectScannerAgent, UpdateReport,
    VersionControlAgent,
};
use crate::error::{GvcError, Result};
use crate::gradle::{GradleConfigParser, Repository};
use crate::maven::{MavenRepository, PluginPortalClient};
use colored::Colorize;
use std::path::Path;

/// Add a new dependency or plugin entry to the version catalog
pub fn execute_add<P: AsRef<Path>>(
    project_path: P,
    plugin_flag: bool,
    _library_flag: bool,
    coordinate: &str,
    alias_override: Option<&str>,
    version_alias_override: Option<&str>,
) -> Result<()> {
    let project_path = project_path.as_ref();
    println!(
        "{}",
        "Adding entry to Gradle version catalog...".cyan().bold()
    );

    println!("\n{}", "1. Validating project structure...".yellow());
    let scanner = ProjectScannerAgent::new(project_path);
    let project_info = scanner.validate()?;
    println!("{}", "✓ Project structure is valid".green());

    let (target, coordinate) = resolve_add_target(plugin_flag, coordinate)?;

    println!(
        "\n{}",
        "2. Reading Gradle repository configuration...".yellow()
    );
    let gradle_parser = GradleConfigParser::new(project_path);
    let gradle_config = gradle_parser.parse()?;
    println!(
        "   Found {} repositories:",
        gradle_config.repositories.len()
    );
    for repo in &gradle_config.repositories {
        println!("   • {} ({})", repo.name.bright_cyan(), repo.url.dimmed());
    }

    println!(
        "\n{}",
        "3. Validating coordinate against remote repositories...".yellow()
    );

    let repositories = gradle_config.repositories.clone();

    match target {
        AddTargetKind::Library => {
            let (group, artifact, version) = parse_library_coordinate(coordinate)?;
            verify_library_version(&repositories, &group, &artifact, &version)?;
        }
        AddTargetKind::Plugin => {
            let (plugin_id, version) = parse_plugin_coordinate(coordinate)?;
            verify_plugin_version(&plugin_id, &version)?;
        }
    }

    println!("\n{}", "4. Writing to version catalog...".yellow());
    let editor = CatalogEditor::new(&project_info.toml_path);

    let result = match target {
        AddTargetKind::Library => {
            editor.add_library(coordinate, alias_override, version_alias_override)
        }
        AddTargetKind::Plugin => {
            editor.add_plugin(coordinate, alias_override, version_alias_override)
        }
    }?;

    print_add_result(&result);

    println!("\n{}", "✨ Entry added successfully!".green().bold());

    Ok(())
}

fn resolve_add_target(plugin_flag: bool, coordinate: &str) -> Result<(AddTargetKind, &str)> {
    if coordinate.trim().is_empty() {
        return Err(GvcError::ProjectValidation(
            "Coordinate is required. Example: gvc add group:artifact:version".into(),
        ));
    }

    let target = if plugin_flag {
        AddTargetKind::Plugin
    } else {
        AddTargetKind::Library
    };

    Ok((target, coordinate))
}

fn print_add_result(result: &AddResult) {
    match result.target {
        AddTargetKind::Library => {
            println!(
                "{}",
                format!(
                    "✓ Library '{}' added with version alias '{}'",
                    result.alias, result.version_alias
                )
                .green()
            );
        }
        AddTargetKind::Plugin => {
            println!(
                "{}",
                format!(
                    "✓ Plugin '{}' added with version alias '{}'",
                    result.alias, result.version_alias
                )
                .green()
            );
        }
    }
}

fn verify_library_version(
    repositories: &[Repository],
    group: &str,
    artifact: &str,
    version: &str,
) -> Result<()> {
    let repo = MavenRepository::with_repositories(repositories.to_vec())?;
    let available_versions = repo.fetch_available_versions(group, artifact)?;

    if available_versions.iter().any(|v| v == version) {
        println!("   {}", format!("✓ {group}:{artifact} @ {version}").green());
        Ok(())
    } else {
        Err(GvcError::ProjectValidation(format!(
            "Version '{}' for '{}:{}' not found in configured repositories",
            version, group, artifact
        )))
    }
}

fn verify_plugin_version(plugin_id: &str, version: &str) -> Result<()> {
    let client = PluginPortalClient::new()?;
    let available_versions = client.fetch_available_plugin_versions(plugin_id)?;

    if available_versions.iter().any(|v| v == version) {
        println!("   {}", format!("✓ plugin {plugin_id} @ {version}").green());
        Ok(())
    } else {
        Err(GvcError::ProjectValidation(format!(
            "Version '{}' for plugin '{}' not found on Gradle Plugin Portal",
            version, plugin_id
        )))
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
    let project_path = project_path.as_ref();
    println!("{}", "Starting dependency update process...".cyan().bold());

    // Step 1: Validate project structure
    println!("\n{}", "1. Validating project structure...".yellow());
    let scanner = ProjectScannerAgent::new(project_path);
    let project_info = scanner.validate()?;
    println!("{}", "✓ Project structure is valid".green());

    // Step 2: Check Git status (if Git is available and not disabled)
    if project_info.has_git && !no_git {
        println!("\n{}", "2. Checking Git status...".yellow());
        let git_agent = VersionControlAgent::new(project_path);

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

    // Step 3: Read Gradle repository configuration
    println!(
        "\n{}",
        "3. Reading Gradle repository configuration...".yellow()
    );
    let gradle_parser = GradleConfigParser::new(project_path);
    let gradle_config = gradle_parser.parse()?;

    println!(
        "   Found {} repositories:",
        gradle_config.repositories.len()
    );
    for repo in &gradle_config.repositories {
        println!("   • {} ({})", repo.name.bright_cyan(), repo.url.dimmed());
    }

    // Step 4: Update dependencies
    println!("\n{}", "4. Updating dependencies...".yellow());
    let updater = DependencyUpdater::with_repositories(gradle_config.repositories)?;

    let report = match filter {
        Some(pattern) => match updater.update_targeted_dependency(
            &project_info.toml_path,
            stable_only,
            interactive,
            &pattern,
        ) {
            Ok(report) => report,
            Err(GvcError::UserCancelled) => {
                println!("\n{}", "Update cancelled by user.".yellow());
                return Ok(());
            }
            Err(e) => return Err(e),
        },
        None => {
            match updater.update_version_catalog(&project_info.toml_path, stable_only, interactive)
            {
                Ok(report) => report,
                Err(GvcError::UserCancelled) => {
                    println!("\n{}", "Update cancelled by user.".yellow());
                    return Ok(());
                }
                Err(e) => return Err(e),
            }
        }
    };

    println!("{}", "✓ Update completed".green());

    // Step 5: Display summary
    print_update_report(&report);

    // Step 6: Git operations (if enabled)
    if project_info.has_git && !no_git && !report.is_empty() {
        println!("\n{}", "5. Creating Git commit...".yellow());
        let git_agent = VersionControlAgent::new(project_path);
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

/// Execute the check workflow (dry-run)
pub fn execute_check<P: AsRef<Path>>(project_path: P, stable_only: bool) -> Result<()> {
    let project_path = project_path.as_ref();
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

    // Step 1: Validate project structure
    println!("\n{}", "1. Validating project structure...".yellow());
    let scanner = ProjectScannerAgent::new(project_path);
    let project_info = scanner.validate()?;
    println!("{}", "✓ Project structure is valid".green());

    // Step 2: Read Gradle repository configuration
    println!(
        "\n{}",
        "2. Reading Gradle repository configuration...".yellow()
    );
    let gradle_parser = GradleConfigParser::new(project_path);
    let gradle_config = gradle_parser.parse()?;

    println!(
        "   Found {} repositories:",
        gradle_config.repositories.len()
    );
    for repo in &gradle_config.repositories {
        println!("   • {} ({})", repo.name.bright_cyan(), repo.url.dimmed());
    }

    // Step 3: Check for updates without modifying the file
    println!("\n{}", "3. Checking for available updates...".yellow());

    let updater = DependencyUpdater::with_repositories(gradle_config.repositories)?;

    // 读取当前的TOML但不写回
    let report = updater.check_for_updates(&project_info.toml_path, stable_only)?;

    println!("{}", "✓ Check completed".green());

    // Step 4: Display available updates
    print_available_updates(&report, stable_only);

    Ok(())
}

fn print_available_updates(report: &UpdateReport, stable_only: bool) {
    if report.is_empty() {
        println!("\n{}", "✨ All dependencies are up to date!".green().bold());
        return;
    }

    println!("\n{}", "📦 Available Updates:".cyan().bold());
    println!(
        "{}",
        format!("Found {} update(s)", report.total_updates()).yellow()
    );

    if stable_only {
        println!("{}", "   (showing stable versions only)".dimmed());
    } else {
        println!(
            "{}",
            "   (showing all versions including pre-releases)".dimmed()
        );
    }

    if !report.version_updates.is_empty() {
        println!("\n{}:", "Version updates".cyan().bold());
        for (name, (old, new)) in &report.version_updates {
            println!(
                "  • {} {} → {}",
                name.white().bold(),
                old.red(),
                new.green().bold()
            );
        }
    }

    if !report.library_updates.is_empty() {
        println!("\n{}:", "Library updates".cyan().bold());
        for (name, (old, new)) in &report.library_updates {
            let stability = if is_stable_version(new) {
                "stable".green()
            } else {
                "pre-release".yellow()
            };
            println!(
                "  • {} {} → {} ({})",
                name.white().bold(),
                old.dimmed(),
                new.green().bold(),
                stability
            );
        }
    }

    if !report.plugin_updates.is_empty() {
        println!("\n{}:", "Plugin updates".cyan().bold());
        for (name, (old, new)) in &report.plugin_updates {
            println!(
                "  • {} {} → {}",
                name.white().bold(),
                old.red(),
                new.green().bold()
            );
        }
    }

    println!("\n{}", "To apply these updates, run:".dimmed());
    if stable_only {
        println!("  {}", "gvc update --stable-only".cyan());
    } else {
        println!("  {}", "gvc update".cyan());
    }
}

fn is_stable_version(version: &str) -> bool {
    let lower = version.to_lowercase();
    !lower.contains("alpha")
        && !lower.contains("beta")
        && !lower.contains("rc")
        && !lower.contains("snapshot")
        && !lower.contains("dev")
}

/// Execute the list workflow - display all dependencies
pub fn execute_list<P: AsRef<Path>>(project_path: P) -> Result<()> {
    let project_path = project_path.as_ref();
    println!(
        "{}",
        "Listing dependencies in version catalog...".cyan().bold()
    );

    // Step 1: Validate project structure
    println!("\n{}", "1. Validating project structure...".yellow());
    let scanner = ProjectScannerAgent::new(project_path);
    let project_info = scanner.validate()?;
    println!("{}", "✓ Project structure is valid".green());

    // Step 2: Parse TOML file
    println!("\n{}", "2. Reading version catalog...".yellow());
    let content = std::fs::read_to_string(&project_info.toml_path).map_err(|e| {
        crate::error::GvcError::TomlParsing(format!("Failed to read catalog: {}", e))
    })?;

    let doc = content
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| crate::error::GvcError::TomlParsing(format!("Failed to parse TOML: {}", e)))?;

    println!("{}", "✓ Catalog loaded".green());

    // Step 3: Display dependencies
    print_dependencies(&doc);

    Ok(())
}

fn print_dependencies(doc: &toml_edit::DocumentMut) {
    use crate::maven::parse_maven_coordinate;
    use std::collections::HashMap;

    println!("\n{}", "📦 Dependencies:".cyan().bold());

    // First, collect all version references
    let mut version_refs = HashMap::new();
    if let Some(versions) = doc.get("versions").and_then(|v| v.as_table()) {
        for (name, value) in versions.iter() {
            if let Some(version_str) = value.as_str() {
                version_refs.insert(name.to_string(), version_str.to_string());
            }
        }
    }

    // Display [libraries] section in Maven coordinate format
    if let Some(libraries) = doc.get("libraries").and_then(|v| v.as_table()) {
        if !libraries.is_empty() {
            println!("\n{}", "Libraries:".yellow().bold());
            let mut lib_list: Vec<_> = libraries.iter().collect();
            lib_list.sort_by_key(|(k, _)| *k);

            for (name, value) in lib_list {
                let mut coordinate = String::new();
                let mut version_str = String::new();

                // Parse the library specification
                if let Some(str_value) = value.as_str() {
                    // Format 1: "group:artifact:version"
                    if let Some((group, artifact, version)) = parse_maven_coordinate(str_value) {
                        coordinate = format!("{}:{}", group, artifact);
                        if let Some(v) = version {
                            version_str = v.to_string();
                        }
                    }
                } else if let Some(inline_table) = value.as_inline_table() {
                    // Inline table format: { group = "...", name = "...", version.ref = "..." }
                    if let Some(module) = inline_table.get("module").and_then(|v| v.as_str()) {
                        if let Some((group, artifact, _)) = parse_maven_coordinate(module) {
                            coordinate = format!("{}:{}", group, artifact);
                        }
                    } else if let Some(group) = inline_table.get("group").and_then(|v| v.as_str()) {
                        if let Some(artifact) = inline_table.get("name").and_then(|v| v.as_str()) {
                            coordinate = format!("{}:{}", group, artifact);
                        }
                    }

                    // Get version
                    if let Some(version) = inline_table.get("version") {
                        if let Some(v) = version.as_str() {
                            version_str = v.to_string();
                        } else if let Some(version_ref) = version.as_inline_table() {
                            if let Some(ref_name) = version_ref.get("ref").and_then(|v| v.as_str())
                            {
                                if let Some(resolved) = version_refs.get(ref_name) {
                                    version_str = resolved.clone();
                                } else {
                                    version_str = format!("${{{}}}", ref_name);
                                }
                            }
                        }
                    }
                } else if let Some(table) = value.as_table() {
                    // Regular table format
                    if let Some(module) = table.get("module").and_then(|v| v.as_str()) {
                        if let Some((group, artifact, _)) = parse_maven_coordinate(module) {
                            coordinate = format!("{}:{}", group, artifact);
                        }
                    } else if let Some(group) = table.get("group").and_then(|v| v.as_str()) {
                        if let Some(artifact) = table.get("name").and_then(|v| v.as_str()) {
                            coordinate = format!("{}:{}", group, artifact);
                        }
                    }

                    // Get version
                    if let Some(version) = table.get("version") {
                        if let Some(v) = version.as_str() {
                            version_str = v.to_string();
                        } else if let Some(version_ref) = version.as_table() {
                            if let Some(ref_name) = version_ref.get("ref").and_then(|v| v.as_str())
                            {
                                if let Some(resolved) = version_refs.get(ref_name) {
                                    version_str = resolved.clone();
                                } else {
                                    version_str = format!("${{{}}}", ref_name);
                                }
                            }
                        } else if let Some(version_ref) = version.as_inline_table() {
                            if let Some(ref_name) = version_ref.get("ref").and_then(|v| v.as_str())
                            {
                                if let Some(resolved) = version_refs.get(ref_name) {
                                    version_str = resolved.clone();
                                } else {
                                    version_str = format!("${{{}}}", ref_name);
                                }
                            }
                        }
                    }
                }

                if !coordinate.is_empty() && !version_str.is_empty() {
                    println!("  {}", format!("{}:{}", coordinate, version_str).cyan());
                } else if !coordinate.is_empty() {
                    println!("  {} {}", coordinate.cyan(), "(version unknown)".dimmed());
                } else {
                    println!("  {} {}", name.yellow(), "(coordinate unknown)".dimmed());
                }
            }
        }
    }

    // Display [plugins] section
    if let Some(plugins) = doc.get("plugins").and_then(|v| v.as_table()) {
        if !plugins.is_empty() {
            println!("\n{}", "Plugins:".yellow().bold());
            let mut plugin_list: Vec<_> = plugins.iter().collect();
            plugin_list.sort_by_key(|(k, _)| *k);

            for (name, value) in plugin_list {
                let mut plugin_id = String::new();
                let mut version_str = String::new();

                if let Some(str_value) = value.as_str() {
                    plugin_id = name.to_string();
                    version_str = str_value.to_string();
                } else if let Some(table) = value.as_table() {
                    if let Some(id) = table.get("id").and_then(|v| v.as_str()) {
                        plugin_id = id.to_string();
                    } else {
                        plugin_id = name.to_string();
                    }

                    if let Some(version) = table.get("version") {
                        if let Some(v) = version.as_str() {
                            version_str = v.to_string();
                        } else if let Some(version_ref) = version.as_table() {
                            if let Some(ref_name) = version_ref.get("ref").and_then(|v| v.as_str())
                            {
                                // Resolve version reference
                                if let Some(resolved) = version_refs.get(ref_name) {
                                    version_str = resolved.clone();
                                } else {
                                    version_str = format!("${{{}}}", ref_name);
                                }
                            }
                        }
                    }
                }

                if !version_str.is_empty() {
                    println!("  {}", format!("{}:{}", plugin_id, version_str).magenta());
                } else {
                    println!("  {} {}", plugin_id.magenta(), "(version unknown)".dimmed());
                }
            }
        }
    }

    // Summary
    let library_count = doc
        .get("libraries")
        .and_then(|v| v.as_table())
        .map(|t| t.len())
        .unwrap_or(0);
    let plugin_count = doc
        .get("plugins")
        .and_then(|v| v.as_table())
        .map(|t| t.len())
        .unwrap_or(0);

    println!("\n{}", "Summary:".cyan().bold());
    println!("  {} libraries", library_count.to_string().yellow());
    println!("  {} plugins", plugin_count.to_string().yellow());
}

fn print_update_report(report: &UpdateReport) {
    if report.is_empty() {
        println!("\n{}", "No updates were found".yellow());
        return;
    }

    println!("\n{}", "Update Summary:".cyan().bold());
    println!(
        "{}",
        format!("Total updates: {}", report.total_updates()).green()
    );

    if !report.version_updates.is_empty() {
        println!("\n{}:", "Version updates".cyan());
        for (name, (old, new)) in &report.version_updates {
            println!(
                "  • {} {} → {}",
                name.white().bold(),
                old.red(),
                new.green()
            );
        }
    }

    if !report.library_updates.is_empty() {
        println!("\n{}:", "Library updates".cyan());
        for (name, (old, new)) in &report.library_updates {
            println!(
                "  • {} {} → {}",
                name.white().bold(),
                old.red(),
                new.green()
            );
        }
    }

    if !report.plugin_updates.is_empty() {
        println!("\n{}:", "Plugin updates".cyan());
        for (name, (old, new)) in &report.plugin_updates {
            println!(
                "  • {} {} → {}",
                name.white().bold(),
                old.red(),
                new.green()
            );
        }
    }
}
