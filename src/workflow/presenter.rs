use crate::agents::{AddResult, AddTargetKind, DoctorReport, DoctorSeverity, UpdateReport};
use crate::error::{GvcError, Result};
use crate::gradle::Repository;
use crate::maven::version::Version;
use crate::utils::toml::TomlUtils;
use colored::Colorize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use toml_edit::DocumentMut;

pub(super) fn print_json(value: &Value) -> Result<()> {
    let rendered = serde_json::to_string_pretty(value)
        .map_err(|e| GvcError::TomlParsing(format!("Failed to serialize JSON output: {e}")))?;
    println!("{rendered}");
    Ok(())
}

pub(super) fn updates_json(report: &UpdateReport) -> Value {
    json!({
        "total": report.total_updates(),
        "versions": update_map_json(&report.version_updates),
        "libraries": update_map_json(&report.library_updates),
        "plugins": update_map_json(&report.plugin_updates),
    })
}

pub(super) fn dependencies_json(doc: &DocumentMut) -> Value {
    let version_refs = collect_version_refs(doc);
    let libraries = collect_libraries_json(doc, &version_refs);
    let plugins = collect_plugins_json(doc, &version_refs);

    json!({
        "libraries": libraries,
        "plugins": plugins,
        "summary": {
            "libraries": libraries.len(),
            "plugins": plugins.len(),
        }
    })
}

pub(super) fn findings_json(report: &DoctorReport) -> Result<Value> {
    let findings = serde_json::to_value(&report.findings)
        .map_err(|e| GvcError::TomlParsing(format!("Failed to serialize doctor findings: {e}")))?;

    Ok(json!({
        "summary": {
            "total": report.total(),
            "errors": report.errors(),
            "warnings": report.warnings(),
            "info": report.infos(),
        },
        "findings": findings,
    }))
}

pub(super) fn doctor_json(report: &DoctorReport) -> Result<Value> {
    findings_json(report)
}

pub(super) fn add_target_json(target: AddTargetKind) -> &'static str {
    match target {
        AddTargetKind::Library => "library",
        AddTargetKind::Plugin => "plugin",
    }
}

pub(super) fn print_doctor_report(report: &DoctorReport) {
    print_findings_report(
        "Kotlin/Android Doctor:",
        "✓ No Kotlin/Android catalog issues found",
        report,
    );
}

pub(super) fn print_audit_report(report: &DoctorReport) {
    print_findings_report(
        "Catalog Audit:",
        "✓ No catalog quality issues found",
        report,
    );
}

fn print_findings_report(title: &str, empty_message: &str, report: &DoctorReport) {
    crate::outln!("\n{}", title.cyan().bold());

    if report.total() == 0 {
        crate::outln!("{}", empty_message.green());
    } else {
        crate::outln!(
            "{}",
            format!(
                "{} finding(s): {} error(s), {} warning(s), {} info(s)",
                report.total(),
                report.errors(),
                report.warnings(),
                report.infos()
            )
            .yellow()
        );
    }

    for finding in &report.findings {
        crate::outln!(
            "\n{} {}",
            severity_label(&finding.severity),
            finding.code.white().bold()
        );
        crate::outln!("  {}", finding.message);
        crate::outln!("  Recommendation: {}", finding.recommendation.dimmed());
        for evidence in &finding.evidence {
            crate::outln!("  - {}", evidence);
        }
    }
}

pub(super) fn print_repositories(repositories: &[Repository]) {
    crate::outln!("   Found {} repositories:", repositories.len());
    for repo in repositories {
        crate::outln!("   • {} ({})", repo.name.bright_cyan(), repo.url.dimmed());
    }
}

pub(super) fn print_add_result(result: &AddResult) {
    match result.target {
        AddTargetKind::Library => {
            crate::outln!(
                "{}",
                format!(
                    "✓ Library '{}' added with version alias '{}'",
                    result.alias, result.version_alias
                )
                .green()
            );
        }
        AddTargetKind::Plugin => {
            crate::outln!(
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
        crate::outln!("\n{}", "✨ All dependencies are up to date!".green().bold());
        return;
    }

    crate::outln!("\n{}", "📦 Available Updates:".cyan().bold());
    crate::outln!(
        "{}",
        format!("Found {} update(s)", report.total_updates()).yellow()
    );

    if stable_only {
        crate::outln!("{}", "   (showing stable versions only)".dimmed());
    } else {
        crate::outln!(
            "{}",
            "   (showing all versions including pre-releases)".dimmed()
        );
    }

    if !report.version_updates.is_empty() {
        crate::outln!("\n{}:", "Version updates".cyan().bold());
        for (name, (old, new)) in &report.version_updates {
            crate::outln!(
                "  • {} {} → {}",
                name.white().bold(),
                old.red(),
                new.green().bold()
            );
        }
    }

    if !report.library_updates.is_empty() {
        crate::outln!("\n{}:", "Library updates".cyan().bold());
        for (name, (old, new)) in &report.library_updates {
            let stability = if Version::parse(new).is_stable() {
                "stable".green()
            } else {
                "pre-release".yellow()
            };
            crate::outln!(
                "  • {} {} → {} ({})",
                name.white().bold(),
                old.dimmed(),
                new.green().bold(),
                stability
            );
        }
    }

    if !report.plugin_updates.is_empty() {
        crate::outln!("\n{}:", "Plugin updates".cyan().bold());
        for (name, (old, new)) in &report.plugin_updates {
            crate::outln!(
                "  • {} {} → {}",
                name.white().bold(),
                old.red(),
                new.green().bold()
            );
        }
    }

    crate::outln!("\n{}", "To apply these updates, run:".dimmed());
    if stable_only {
        crate::outln!("  {}", "gvc update".cyan());
    } else {
        crate::outln!("  {}", "gvc update --no-stable-only".cyan());
    }
}

pub(super) fn print_outdated_report(report: &UpdateReport, stable_only: bool) {
    if report.is_empty() {
        crate::outln!("\n{}", "✨ All catalog entries are current!".green().bold());
        return;
    }

    crate::outln!("\n{}", "📦 Outdated Catalog Entries:".cyan().bold());
    let total = report.total_updates();
    let noun = if total == 1 { "entry" } else { "entries" };
    crate::outln!("{}", format!("Found {} outdated {}", total, noun).yellow());

    if stable_only {
        crate::outln!("{}", "   (latest stable versions)".dimmed());
    } else {
        crate::outln!("{}", "   (latest versions including pre-releases)".dimmed());
    }

    let rows = outdated_rows(report);
    let kind_width = rows
        .iter()
        .map(|row| row.kind.len())
        .chain(std::iter::once("Kind".len()))
        .max()
        .unwrap_or("Kind".len());
    let alias_width = rows
        .iter()
        .map(|row| row.alias.len())
        .chain(std::iter::once("Alias".len()))
        .max()
        .unwrap_or("Alias".len());
    let current_width = rows
        .iter()
        .map(|row| row.current.len())
        .chain(std::iter::once("Current".len()))
        .max()
        .unwrap_or("Current".len());

    crate::outln!(
        "\n  {:<kind_width$}  {:<alias_width$}  {:<current_width$}  Latest",
        "Kind".bold(),
        "Alias".bold(),
        "Current".bold()
    );

    for row in rows {
        crate::outln!(
            "  {:<kind_width$}  {:<alias_width$}  {:<current_width$}  {}",
            row.kind,
            row.alias.white().bold(),
            row.current.red(),
            row.latest.green().bold()
        );
    }

    crate::outln!("\n{}", "To apply these updates, run:".dimmed());
    if stable_only {
        crate::outln!("  {}", "gvc update".cyan());
    } else {
        crate::outln!("  {}", "gvc update --no-stable-only".cyan());
    }
}

pub(super) fn print_dependencies(doc: &DocumentMut) {
    let version_refs = collect_version_refs(doc);

    crate::outln!("\n{}", "📦 Dependencies:".cyan().bold());
    print_libraries(doc, &version_refs);
    print_plugins(doc, &version_refs);
    print_summary(doc);
}

#[derive(Clone, Debug)]
struct OutdatedRow {
    kind: &'static str,
    alias: String,
    current: String,
    latest: String,
}

fn outdated_rows(report: &UpdateReport) -> Vec<OutdatedRow> {
    let mut rows = Vec::new();

    rows.extend(
        report
            .version_updates
            .iter()
            .map(|(alias, (current, latest))| OutdatedRow {
                kind: "version",
                alias: alias.clone(),
                current: current.clone(),
                latest: latest.clone(),
            }),
    );

    rows.extend(
        report
            .library_updates
            .iter()
            .map(|(alias, (current, latest))| OutdatedRow {
                kind: "library",
                alias: alias.clone(),
                current: current.clone(),
                latest: latest.clone(),
            }),
    );

    rows.extend(
        report
            .plugin_updates
            .iter()
            .map(|(alias, (current, latest))| OutdatedRow {
                kind: "plugin",
                alias: alias.clone(),
                current: current.clone(),
                latest: latest.clone(),
            }),
    );

    rows.sort_by(|left, right| {
        left.kind
            .cmp(right.kind)
            .then_with(|| left.alias.cmp(&right.alias))
    });
    rows
}

pub(super) fn print_update_report(report: &UpdateReport) {
    if report.is_empty() {
        crate::outln!("\n{}", "No updates were found".yellow());
        return;
    }

    crate::outln!("\n{}", "Update Summary:".cyan().bold());
    crate::outln!(
        "{}",
        format!("Total updates: {}", report.total_updates()).green()
    );

    if !report.version_updates.is_empty() {
        crate::outln!("\n{}:", "Version updates".cyan());
        for (name, (old, new)) in &report.version_updates {
            crate::outln!(
                "  • {} {} → {}",
                name.white().bold(),
                old.red(),
                new.green()
            );
        }
    }

    if !report.library_updates.is_empty() {
        crate::outln!("\n{}:", "Library updates".cyan());
        for (name, (old, new)) in &report.library_updates {
            crate::outln!(
                "  • {} {} → {}",
                name.white().bold(),
                old.red(),
                new.green()
            );
        }
    }

    if !report.plugin_updates.is_empty() {
        crate::outln!("\n{}:", "Plugin updates".cyan());
        for (name, (old, new)) in &report.plugin_updates {
            crate::outln!(
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

    crate::outln!("\n{}", "Libraries:".yellow().bold());
    let mut lib_list: Vec<_> = libraries.iter().collect();
    lib_list.sort_by_key(|(key, _)| *key);

    for (name, value) in lib_list {
        let Some(details) = TomlUtils::extract_library_details(value) else {
            crate::outln!("  {} {}", name.yellow(), "(coordinate unknown)".dimmed());
            continue;
        };

        let coordinate = format!("{}:{}", details.group, details.artifact);
        match resolve_version(details.version, details.version_ref, version_refs) {
            Some(version) => crate::outln!("  {}", format!("{}:{}", coordinate, version).cyan()),
            None => crate::outln!("  {} {}", coordinate.cyan(), "(version unknown)".dimmed()),
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

    crate::outln!("\n{}", "Plugins:".yellow().bold());
    let mut plugin_list: Vec<_> = plugins.iter().collect();
    plugin_list.sort_by_key(|(key, _)| *key);

    for (name, value) in plugin_list {
        let Some(details) = TomlUtils::extract_plugin_details(name, value) else {
            crate::outln!("  {} {}", name.yellow(), "(plugin unknown)".dimmed());
            continue;
        };

        match resolve_version(details.version, details.version_ref, version_refs) {
            Some(version) => {
                crate::outln!("  {}", format!("{}:{}", details.id, version).magenta())
            }
            None => crate::outln!(
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

    crate::outln!("\n{}", "Summary:".cyan().bold());
    crate::outln!("  {} libraries", library_count.to_string().yellow());
    crate::outln!("  {} plugins", plugin_count.to_string().yellow());
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

fn update_map_json(updates: &HashMap<String, (String, String)>) -> Vec<Value> {
    let sorted: BTreeMap<_, _> = updates.iter().collect();
    sorted
        .into_iter()
        .map(|(alias, (current, latest))| {
            json!({
                "alias": alias,
                "current_version": current,
                "latest_version": latest,
            })
        })
        .collect()
}

fn collect_libraries_json(doc: &DocumentMut, version_refs: &HashMap<String, String>) -> Vec<Value> {
    let Some(libraries) = doc.get("libraries").and_then(|v| v.as_table()) else {
        return Vec::new();
    };

    let mut entries: Vec<_> = libraries.iter().collect();
    entries.sort_by_key(|(alias, _)| *alias);
    entries
        .into_iter()
        .map(|(alias, item)| {
            if let Some(details) = TomlUtils::extract_library_details(item) {
                let coordinate = format!("{}:{}", details.group, details.artifact);
                let version = resolve_version(
                    details.version.clone(),
                    details.version_ref.clone(),
                    version_refs,
                );
                json!({
                    "alias": alias,
                    "group": details.group,
                    "artifact": details.artifact,
                    "coordinate": coordinate,
                    "version": version,
                    "version_ref": details.version_ref,
                })
            } else {
                json!({
                    "alias": alias,
                    "coordinate": null,
                    "version": null,
                    "version_ref": null,
                })
            }
        })
        .collect()
}

fn collect_plugins_json(doc: &DocumentMut, version_refs: &HashMap<String, String>) -> Vec<Value> {
    let Some(plugins) = doc.get("plugins").and_then(|v| v.as_table()) else {
        return Vec::new();
    };

    let mut entries: Vec<_> = plugins.iter().collect();
    entries.sort_by_key(|(alias, _)| *alias);
    entries
        .into_iter()
        .map(|(alias, item)| {
            if let Some(details) = TomlUtils::extract_plugin_details(alias, item) {
                let coordinate = details.id.clone();
                let version = resolve_version(
                    details.version.clone(),
                    details.version_ref.clone(),
                    version_refs,
                );
                json!({
                    "alias": alias,
                    "id": details.id,
                    "coordinate": coordinate,
                    "version": version,
                    "version_ref": details.version_ref,
                })
            } else {
                json!({
                    "alias": alias,
                    "id": null,
                    "coordinate": null,
                    "version": null,
                    "version_ref": null,
                })
            }
        })
        .collect()
}

fn severity_label(severity: &DoctorSeverity) -> colored::ColoredString {
    match severity {
        DoctorSeverity::Info => "[info]".cyan(),
        DoctorSeverity::Warning => "[warning]".yellow(),
        DoctorSeverity::Error => "[error]".red().bold(),
    }
}
