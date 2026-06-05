use crate::agents::{DoctorReport, DoctorSeverity};
use crate::utils::toml::TomlUtils;
use std::collections::{BTreeMap, BTreeSet};
use toml_edit::{DocumentMut, Item};

pub struct CatalogAuditor;

impl CatalogAuditor {
    pub fn analyze(doc: &DocumentMut) -> DoctorReport {
        let snapshot = CatalogAuditSnapshot::from_doc(doc);
        let mut report = DoctorReport::default();

        Self::check_missing_version_refs(&snapshot, &mut report);
        Self::check_duplicate_coordinates(&snapshot, &mut report);
        Self::check_inline_versions(&snapshot, &mut report);
        Self::check_unused_versions(&snapshot, &mut report);
        Self::check_duplicate_version_values(&snapshot, &mut report);

        report
    }

    fn check_missing_version_refs(snapshot: &CatalogAuditSnapshot, report: &mut DoctorReport) {
        let missing: Vec<String> = snapshot
            .entries
            .iter()
            .filter_map(|entry| {
                let version_ref = entry.version_ref.as_ref()?;
                (!snapshot.version_aliases.contains_key(version_ref)).then(|| {
                    format!(
                        "{} '{}' references missing version alias '{}'",
                        entry.kind.as_str(),
                        entry.alias,
                        version_ref
                    )
                })
            })
            .collect();

        if missing.is_empty() {
            return;
        }

        report.add(
            DoctorSeverity::Error,
            "version_ref_missing",
            "Some catalog entries reference version aliases that are not declared in [versions].",
            "Add the missing [versions] keys or update the entries to reference existing aliases.",
            missing,
        );
    }

    fn check_duplicate_coordinates(snapshot: &CatalogAuditSnapshot, report: &mut DoctorReport) {
        let mut by_coordinate: BTreeMap<(CatalogEntryKind, String), Vec<&CatalogEntry>> =
            BTreeMap::new();

        for entry in &snapshot.entries {
            by_coordinate
                .entry((entry.kind, entry.coordinate.clone()))
                .or_default()
                .push(entry);
        }

        let duplicates: Vec<String> = by_coordinate
            .into_iter()
            .filter_map(|((kind, coordinate), entries)| {
                if entries.len() <= 1 {
                    return None;
                }

                let aliases = entries
                    .iter()
                    .map(|entry| entry.alias.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                Some(format!(
                    "{} '{}' is declared by aliases: {}",
                    kind.as_str(),
                    coordinate,
                    aliases
                ))
            })
            .collect();

        if duplicates.is_empty() {
            return;
        }

        report.add(
            DoctorSeverity::Warning,
            "duplicate_coordinates",
            "Multiple catalog aliases point to the same dependency or plugin coordinate.",
            "Keep one alias per coordinate unless different aliases are intentionally used for migration.",
            duplicates,
        );
    }

    fn check_inline_versions(snapshot: &CatalogAuditSnapshot, report: &mut DoctorReport) {
        let inline_versions: Vec<String> = snapshot
            .entries
            .iter()
            .filter(|entry| entry.version.is_some() && entry.version_ref.is_none())
            .map(|entry| {
                format!(
                    "{} '{}' uses inline version '{}'",
                    entry.kind.as_str(),
                    entry.alias,
                    entry.version.as_deref().unwrap_or_default()
                )
            })
            .collect();

        if inline_versions.is_empty() {
            return;
        }

        report.add(
            DoctorSeverity::Warning,
            "inline_versions",
            "Some catalog entries declare versions inline instead of using version.ref.",
            "Move repeated or shared versions into [versions] and reference them with version.ref.",
            inline_versions,
        );
    }

    fn check_unused_versions(snapshot: &CatalogAuditSnapshot, report: &mut DoctorReport) {
        let unused: Vec<String> = snapshot
            .version_aliases
            .iter()
            .filter(|(alias, _)| !snapshot.used_version_refs.contains(*alias))
            .map(|(alias, value)| format!("version '{}' = {}", alias, value))
            .collect();

        if unused.is_empty() {
            return;
        }

        report.add(
            DoctorSeverity::Info,
            "unused_version_aliases",
            "Some [versions] aliases are not referenced by catalog libraries or plugins.",
            "Remove unused aliases, or keep them if build scripts intentionally read libs.versions directly.",
            unused,
        );
    }

    fn check_duplicate_version_values(snapshot: &CatalogAuditSnapshot, report: &mut DoctorReport) {
        let mut aliases_by_value: BTreeMap<&str, Vec<&str>> = BTreeMap::new();

        for (alias, value) in &snapshot.version_aliases {
            aliases_by_value
                .entry(value.as_str())
                .or_default()
                .push(alias.as_str());
        }

        let duplicates: Vec<String> = aliases_by_value
            .into_iter()
            .filter_map(|(value, aliases)| {
                (aliases.len() > 1).then(|| {
                    format!(
                        "version value {} is shared by aliases: {}",
                        value,
                        aliases.join(", ")
                    )
                })
            })
            .collect();

        if duplicates.is_empty() {
            return;
        }

        report.add(
            DoctorSeverity::Info,
            "duplicate_version_values",
            "Multiple [versions] aliases use the same version value.",
            "Consolidate aliases when they represent the same upgrade cadence; keep them separate when versions should move independently.",
            duplicates,
        );
    }
}

#[derive(Clone, Debug)]
struct CatalogAuditSnapshot {
    version_aliases: BTreeMap<String, String>,
    used_version_refs: BTreeSet<String>,
    entries: Vec<CatalogEntry>,
}

impl CatalogAuditSnapshot {
    fn from_doc(doc: &DocumentMut) -> Self {
        let version_aliases = collect_version_aliases(doc);
        let entries = collect_entries(doc);
        let used_version_refs = entries
            .iter()
            .filter_map(|entry| entry.version_ref.clone())
            .collect();

        Self {
            version_aliases,
            used_version_refs,
            entries,
        }
    }
}

#[derive(Clone, Debug)]
struct CatalogEntry {
    kind: CatalogEntryKind,
    alias: String,
    coordinate: String,
    version: Option<String>,
    version_ref: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CatalogEntryKind {
    Library,
    Plugin,
}

impl CatalogEntryKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Library => "library",
            Self::Plugin => "plugin",
        }
    }
}

fn collect_version_aliases(doc: &DocumentMut) -> BTreeMap<String, String> {
    let Some(versions) = doc.get("versions").and_then(|item| item.as_table()) else {
        return BTreeMap::new();
    };

    versions
        .iter()
        .filter_map(|(alias, item)| {
            version_item_label(item).map(|value| (alias.to_string(), value))
        })
        .collect()
}

fn collect_entries(doc: &DocumentMut) -> Vec<CatalogEntry> {
    let mut entries = Vec::new();

    if let Some(libraries) = doc.get("libraries").and_then(|item| item.as_table()) {
        for (alias, item) in libraries {
            let Some(details) = TomlUtils::extract_library_details(item) else {
                continue;
            };

            entries.push(CatalogEntry {
                kind: CatalogEntryKind::Library,
                alias: alias.to_string(),
                coordinate: format!("{}:{}", details.group, details.artifact),
                version: details.version,
                version_ref: details.version_ref,
            });
        }
    }

    if let Some(plugins) = doc.get("plugins").and_then(|item| item.as_table()) {
        for (alias, item) in plugins {
            let Some(details) = TomlUtils::extract_plugin_details(alias, item) else {
                continue;
            };

            entries.push(CatalogEntry {
                kind: CatalogEntryKind::Plugin,
                alias: alias.to_string(),
                coordinate: details.id,
                version: details.version,
                version_ref: details.version_ref,
            });
        }
    }

    entries
}

fn version_item_label(item: &Item) -> Option<String> {
    if let Some(version) = item.as_str() {
        return Some(format!("'{}'", version));
    }

    if item.as_inline_table().is_some() || item.as_table().is_some() {
        let label = item.to_string().trim().replace('\n', " ");
        if !label.is_empty() {
            return Some(label);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analyze(input: &str) -> DoctorReport {
        let doc: DocumentMut = input.parse().unwrap();
        CatalogAuditor::analyze(&doc)
    }

    #[test]
    fn reports_missing_version_refs_as_errors() {
        let report = analyze(
            r#"
[libraries]
core = { module = "androidx.core:core-ktx", version = { ref = "androidxCore" } }
"#,
        );

        assert_eq!(report.errors(), 1);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "version_ref_missing")
        );
    }

    #[test]
    fn reports_duplicate_coordinates_and_inline_versions() {
        let report = analyze(
            r#"
[libraries]
core = { module = "androidx.core:core-ktx", version = "1.12.0" }
coreAgain = { group = "androidx.core", name = "core-ktx", version = "1.12.0" }
"#,
        );

        assert_eq!(report.warnings(), 2);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "duplicate_coordinates")
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "inline_versions")
        );
    }

    #[test]
    fn reports_unused_and_duplicate_version_values_as_info() {
        let report = analyze(
            r#"
[versions]
kotlin = "2.0.21"
compose = "2.0.21"
okhttp = "4.12.0"

[plugins]
kotlinAndroid = { id = "org.jetbrains.kotlin.android", version = { ref = "kotlin" } }
"#,
        );

        assert_eq!(report.infos(), 2);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "unused_version_aliases")
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "duplicate_version_values")
        );
    }
}
