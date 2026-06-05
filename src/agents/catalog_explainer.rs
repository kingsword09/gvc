use crate::error::{GvcError, Result};
use crate::utils::toml::TomlUtils;
use serde::Serialize;
use std::collections::BTreeMap;
use toml_edit::DocumentMut;

pub struct CatalogExplainer;

impl CatalogExplainer {
    pub fn explain(doc: &DocumentMut, query: &str) -> Result<WhyReport> {
        let query = query.trim();
        if query.is_empty() {
            return Err(GvcError::ProjectValidation(
                "Query is required. Use an alias, library coordinate, or plugin id.".into(),
            ));
        }

        let snapshot = CatalogExplainSnapshot::from_doc(doc);
        let (matched_by, matches) = snapshot.find(query);
        if matches.is_empty() {
            return Err(GvcError::ProjectValidation(format!(
                "No catalog entry matched '{}'. Use an alias, library coordinate, or plugin id.",
                query
            )));
        }

        let entries = matches
            .into_iter()
            .map(|entry| snapshot.explain_entry(entry))
            .collect();

        Ok(WhyReport {
            query: query.to_string(),
            matched_by,
            entries,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WhyReport {
    pub query: String,
    pub matched_by: WhyMatchKind,
    pub entries: Vec<WhyEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WhyMatchKind {
    Alias,
    AliasCaseInsensitive,
    Coordinate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WhyEntry {
    pub kind: WhyEntryKind,
    pub alias: String,
    pub coordinate: String,
    pub version: WhyVersion,
    pub duplicate_aliases: Vec<String>,
    pub recommendations: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WhyEntryKind {
    Library,
    Plugin,
}

impl WhyEntryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Library => "library",
            Self::Plugin => "plugin",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WhyVersion {
    pub declared: Option<String>,
    pub version_ref: Option<String>,
    pub resolved: Option<String>,
    pub source: WhyVersionSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WhyVersionSource {
    Inline,
    VersionRef,
    MissingVersionRef,
    Unspecified,
}

#[derive(Clone, Debug)]
struct CatalogExplainSnapshot {
    version_aliases: BTreeMap<String, String>,
    entries: Vec<CatalogExplainEntry>,
}

impl CatalogExplainSnapshot {
    fn from_doc(doc: &DocumentMut) -> Self {
        Self {
            version_aliases: collect_version_aliases(doc),
            entries: collect_entries(doc),
        }
    }

    fn find(&self, query: &str) -> (WhyMatchKind, Vec<&CatalogExplainEntry>) {
        let alias_matches: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| entry.alias == query)
            .collect();
        if !alias_matches.is_empty() {
            return (WhyMatchKind::Alias, alias_matches);
        }

        let coordinate = normalize_coordinate_query(query);
        let coordinate_matches: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| entry.coordinate == coordinate)
            .collect();
        if !coordinate_matches.is_empty() {
            return (WhyMatchKind::Coordinate, coordinate_matches);
        }

        let query_lower = query.to_lowercase();
        let alias_ci_matches: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| entry.alias.to_lowercase() == query_lower)
            .collect();
        (WhyMatchKind::AliasCaseInsensitive, alias_ci_matches)
    }

    fn explain_entry(&self, entry: &CatalogExplainEntry) -> WhyEntry {
        let duplicate_aliases = self.duplicate_aliases(entry);
        let version = self.explain_version(entry);
        let recommendations = recommendations_for(entry, &version, &duplicate_aliases);

        WhyEntry {
            kind: entry.kind,
            alias: entry.alias.clone(),
            coordinate: entry.coordinate.clone(),
            version,
            duplicate_aliases,
            recommendations,
        }
    }

    fn duplicate_aliases(&self, entry: &CatalogExplainEntry) -> Vec<String> {
        let mut aliases: Vec<_> = self
            .entries
            .iter()
            .filter(|candidate| candidate.kind == entry.kind)
            .filter(|candidate| candidate.coordinate == entry.coordinate)
            .filter(|candidate| candidate.alias != entry.alias)
            .map(|candidate| candidate.alias.clone())
            .collect();
        aliases.sort();
        aliases
    }

    fn explain_version(&self, entry: &CatalogExplainEntry) -> WhyVersion {
        if let Some(version) = entry.version.as_ref() {
            return WhyVersion {
                declared: Some(version.clone()),
                version_ref: None,
                resolved: Some(version.clone()),
                source: WhyVersionSource::Inline,
            };
        }

        if let Some(version_ref) = entry.version_ref.as_ref() {
            return match self.version_aliases.get(version_ref) {
                Some(resolved) => WhyVersion {
                    declared: None,
                    version_ref: Some(version_ref.clone()),
                    resolved: Some(resolved.clone()),
                    source: WhyVersionSource::VersionRef,
                },
                None => WhyVersion {
                    declared: None,
                    version_ref: Some(version_ref.clone()),
                    resolved: None,
                    source: WhyVersionSource::MissingVersionRef,
                },
            };
        }

        WhyVersion {
            declared: None,
            version_ref: None,
            resolved: None,
            source: WhyVersionSource::Unspecified,
        }
    }
}

#[derive(Clone, Debug)]
struct CatalogExplainEntry {
    kind: WhyEntryKind,
    alias: String,
    coordinate: String,
    version: Option<String>,
    version_ref: Option<String>,
}

fn collect_version_aliases(doc: &DocumentMut) -> BTreeMap<String, String> {
    doc.get("versions")
        .and_then(|item| item.as_table())
        .map(|versions| {
            versions
                .iter()
                .filter_map(|(alias, item)| {
                    item.as_str()
                        .map(|version| (alias.to_string(), version.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn collect_entries(doc: &DocumentMut) -> Vec<CatalogExplainEntry> {
    let mut entries = Vec::new();

    if let Some(libraries) = doc.get("libraries").and_then(|item| item.as_table()) {
        for (alias, item) in libraries {
            let Some(details) = TomlUtils::extract_library_details(item) else {
                continue;
            };

            entries.push(CatalogExplainEntry {
                kind: WhyEntryKind::Library,
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

            entries.push(CatalogExplainEntry {
                kind: WhyEntryKind::Plugin,
                alias: alias.to_string(),
                coordinate: details.id,
                version: details.version,
                version_ref: details.version_ref,
            });
        }
    }

    entries
}

fn normalize_coordinate_query(query: &str) -> String {
    let parts: Vec<_> = query.split(':').collect();
    if parts.len() >= 3 {
        return format!("{}:{}", parts[0], parts[1]);
    }

    query.to_string()
}

fn recommendations_for(
    entry: &CatalogExplainEntry,
    version: &WhyVersion,
    duplicate_aliases: &[String],
) -> Vec<String> {
    let mut recommendations = Vec::new();

    match version.source {
        WhyVersionSource::Inline => {
            recommendations.push("Consider moving this inline version into [versions] when it is shared or expected to move with related entries.".to_string());
        }
        WhyVersionSource::MissingVersionRef => {
            if let Some(version_ref) = version.version_ref.as_deref() {
                recommendations.push(format!(
                    "Define [versions].{} or update the entry to reference an existing version alias.",
                    version_ref
                ));
            }
        }
        WhyVersionSource::Unspecified => {
            recommendations.push(format!(
                "Declare a version for {} '{}' or ensure it is intentionally managed elsewhere.",
                entry.kind.as_str(),
                entry.alias
            ));
        }
        WhyVersionSource::VersionRef => {}
    }

    if !duplicate_aliases.is_empty() {
        recommendations.push(format!(
            "Review duplicate aliases for the same coordinate: {}.",
            duplicate_aliases.join(", ")
        ));
    }

    recommendations
}

#[cfg(test)]
mod tests {
    use super::*;

    fn explain(input: &str, query: &str) -> WhyReport {
        let doc: DocumentMut = input.parse().unwrap();
        CatalogExplainer::explain(&doc, query).unwrap()
    }

    #[test]
    fn explains_library_by_alias_with_version_ref() {
        let report = explain(
            r#"
[versions]
core = "1.12.0"

[libraries]
androidxCore = { module = "androidx.core:core-ktx", version = { ref = "core" } }
"#,
            "androidxCore",
        );

        assert_eq!(report.matched_by, WhyMatchKind::Alias);
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].kind, WhyEntryKind::Library);
        assert_eq!(report.entries[0].coordinate, "androidx.core:core-ktx");
        assert_eq!(
            report.entries[0].version.resolved.as_deref(),
            Some("1.12.0")
        );
        assert_eq!(
            report.entries[0].version.source,
            WhyVersionSource::VersionRef
        );
    }

    #[test]
    fn explains_coordinate_matches_and_duplicates() {
        let report = explain(
            r#"
[libraries]
core = { module = "androidx.core:core-ktx", version = "1.12.0" }
coreAgain = { group = "androidx.core", name = "core-ktx", version = "1.12.0" }
"#,
            "androidx.core:core-ktx:1.12.0",
        );

        assert_eq!(report.matched_by, WhyMatchKind::Coordinate);
        assert_eq!(report.entries.len(), 2);
        assert_eq!(
            report.entries[0].duplicate_aliases,
            vec!["coreAgain".to_string()]
        );
    }

    #[test]
    fn reports_missing_version_ref() {
        let report = explain(
            r#"
[plugins]
kotlin = { id = "org.jetbrains.kotlin.jvm", version = { ref = "kotlinVersion" } }
"#,
            "kotlin",
        );

        assert_eq!(
            report.entries[0].version.source,
            WhyVersionSource::MissingVersionRef
        );
        assert_eq!(
            report.entries[0].version.version_ref.as_deref(),
            Some("kotlinVersion")
        );
        assert!(!report.entries[0].recommendations.is_empty());
    }

    #[test]
    fn rejects_empty_query() {
        let doc: DocumentMut = "".parse().unwrap();
        let err = CatalogExplainer::explain(&doc, "  ").unwrap_err();
        assert!(matches!(err, GvcError::ProjectValidation(_)));
    }
}
