use crate::agents::{AddResult, AddTargetKind, UpdateReport};
use crate::gradle::Repository;
use crate::maven::version::Version;
use crate::utils::toml::TomlUtils;
use colored::Colorize;
use std::collections::HashMap;
use toml_edit::DocumentMut;

pub(super) fn print_repositories(repositories: &[Repository]) {
    println!("   Found {} repositories:", repositories.len());
    for repo in repositories {
        println!("   • {} ({})", repo.name.bright_cyan(), repo.url.dimmed());
    }
}

pub(super) fn print_add_result(result: &AddResult) {
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

pub(super) fn print_available_updates(report: &UpdateReport, stable_only: bool) {
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
            let stability = if Version::parse(new).is_stable() {
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

pub(super) fn print_dependencies(doc: &DocumentMut) {
    let version_refs = collect_version_refs(doc);

    println!("\n{}", "📦 Dependencies:".cyan().bold());
    print_libraries(doc, &version_refs);
    print_plugins(doc, &version_refs);
    print_summary(doc);
}

pub(super) fn print_update_report(report: &UpdateReport) {
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

fn print_libraries(doc: &DocumentMut, version_refs: &HashMap<String, String>) {
    let Some(libraries) = doc.get("libraries").and_then(|v| v.as_table()) else {
        return;
    };
    if libraries.is_empty() {
        return;
    }

    println!("\n{}", "Libraries:".yellow().bold());
    let mut lib_list: Vec<_> = libraries.iter().collect();
    lib_list.sort_by_key(|(key, _)| *key);

    for (name, value) in lib_list {
        let Some(details) = TomlUtils::extract_library_details(value) else {
            println!("  {} {}", name.yellow(), "(coordinate unknown)".dimmed());
            continue;
        };

        let coordinate = format!("{}:{}", details.group, details.artifact);
        match resolve_version(details.version, details.version_ref, version_refs) {
            Some(version) => println!("  {}", format!("{}:{}", coordinate, version).cyan()),
            None => println!("  {} {}", coordinate.cyan(), "(version unknown)".dimmed()),
        }
    }
}

fn print_plugins(doc: &DocumentMut, version_refs: &HashMap<String, String>) {
    let Some(plugins) = doc.get("plugins").and_then(|v| v.as_table()) else {
        return;
    };
    if plugins.is_empty() {
        return;
    }

    println!("\n{}", "Plugins:".yellow().bold());
    let mut plugin_list: Vec<_> = plugins.iter().collect();
    plugin_list.sort_by_key(|(key, _)| *key);

    for (name, value) in plugin_list {
        let Some(details) = TomlUtils::extract_plugin_details(name, value) else {
            println!("  {} {}", name.yellow(), "(plugin unknown)".dimmed());
            continue;
        };

        match resolve_version(details.version, details.version_ref, version_refs) {
            Some(version) => println!("  {}", format!("{}:{}", details.id, version).magenta()),
            None => println!(
                "  {} {}",
                details.id.magenta(),
                "(version unknown)".dimmed()
            ),
        }
    }
}

fn print_summary(doc: &DocumentMut) {
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

fn collect_version_refs(doc: &DocumentMut) -> HashMap<String, String> {
    doc.get("versions")
        .and_then(|v| v.as_table())
        .map(|versions| {
            versions
                .iter()
                .filter_map(|(name, value)| {
                    value
                        .as_str()
                        .map(|version| (name.to_string(), version.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn resolve_version(
    version: Option<String>,
    version_ref: Option<String>,
    version_refs: &HashMap<String, String>,
) -> Option<String> {
    version.or_else(|| {
        version_ref.map(|ref_name| {
            version_refs
                .get(&ref_name)
                .cloned()
                .unwrap_or_else(|| format!("${{{}}}", ref_name))
        })
    })
}
