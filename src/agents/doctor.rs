use crate::utils::toml::TomlUtils;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use toml_edit::DocumentMut;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DoctorFinding {
    pub severity: DoctorSeverity,
    pub code: &'static str,
    pub message: String,
    pub recommendation: String,
    pub evidence: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct DoctorReport {
    pub findings: Vec<DoctorFinding>,
}

impl DoctorReport {
    pub fn add(
        &mut self,
        severity: DoctorSeverity,
        code: &'static str,
        message: impl Into<String>,
        recommendation: impl Into<String>,
        evidence: Vec<String>,
    ) {
        self.findings.push(DoctorFinding {
            severity,
            code,
            message: message.into(),
            recommendation: recommendation.into(),
            evidence,
        });
    }

    pub fn total(&self) -> usize {
        self.findings.len()
    }

    pub fn errors(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.severity == DoctorSeverity::Error)
            .count()
    }

    pub fn warnings(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.severity == DoctorSeverity::Warning)
            .count()
    }

    pub fn infos(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.severity == DoctorSeverity::Info)
            .count()
    }

    pub fn has_issues(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.severity != DoctorSeverity::Info)
    }
}

pub struct KotlinDoctor;

impl KotlinDoctor {
    pub fn analyze(doc: &DocumentMut) -> DoctorReport {
        let catalog = CatalogSnapshot::from_doc(doc);
        let mut report = DoctorReport::default();

        Self::check_kotlin_plugin_versions(&catalog, &mut report);
        Self::check_ksp_version(&catalog, &mut report);
        Self::check_compose_setup(&catalog, &mut report);
        Self::check_android_plugin_versions(&catalog, &mut report);

        report
    }

    fn check_kotlin_plugin_versions(catalog: &CatalogSnapshot, report: &mut DoctorReport) {
        let versions_by_value = group_versions(
            catalog
                .plugins
                .iter()
                .filter(|plugin| plugin.id.starts_with("org.jetbrains.kotlin"))
                .filter_map(|plugin| {
                    plugin
                        .version
                        .as_ref()
                        .map(|version| (version.clone(), plugin.describe()))
                }),
        );

        if versions_by_value.len() <= 1 {
            return;
        }

        report.add(
            DoctorSeverity::Warning,
            "kotlin_plugin_versions_mixed",
            "Multiple Kotlin Gradle plugin versions are declared in the catalog.",
            "Keep Kotlin plugin entries on the same version unless a Gradle plugin explicitly documents otherwise.",
            version_groups_evidence(&versions_by_value),
        );
    }

    fn check_ksp_version(catalog: &CatalogSnapshot, report: &mut DoctorReport) {
        let Some(ksp) = catalog
            .plugins
            .iter()
            .find(|plugin| plugin.id == "com.google.devtools.ksp")
        else {
            return;
        };

        let Some(ksp_version) = ksp.version.as_deref() else {
            return;
        };

        let Some(kotlin_version) = catalog.primary_kotlin_version() else {
            report.add(
                DoctorSeverity::Warning,
                "ksp_without_kotlin_version",
                "KSP is declared, but no Kotlin plugin version was found in the catalog.",
                "Declare the Kotlin Gradle plugin in [plugins] or use a shared Kotlin version alias so KSP compatibility can be validated.",
                vec![ksp.describe()],
            );
            return;
        };

        let Some((ksp_kotlin_prefix, _)) = ksp_version.split_once('-') else {
            report.add(
                DoctorSeverity::Warning,
                "ksp_version_unrecognized",
                "KSP version does not follow the expected '<kotlin-version>-<ksp-version>' format.",
                "Use a standard KSP release version so Kotlin compatibility is visible to tools and reviewers.",
                vec![ksp.describe()],
            );
            return;
        };

        if ksp_kotlin_prefix != kotlin_version.version {
            report.add(
                DoctorSeverity::Error,
                "ksp_kotlin_version_mismatch",
                "KSP is built for a different Kotlin version than the Kotlin Gradle plugin in the catalog.",
                "Use a KSP version whose prefix matches the Kotlin Gradle plugin version.",
                vec![
                    kotlin_version.describe(),
                    ksp.describe(),
                    format!("expected KSP prefix: {}", kotlin_version.version),
                ],
            );
        }
    }

    fn check_compose_setup(catalog: &CatalogSnapshot, report: &mut DoctorReport) {
        let has_compose_libraries = catalog.libraries.iter().any(|library| {
            library.group.starts_with("androidx.compose")
                || library.group.starts_with("org.jetbrains.compose")
        });

        let compose_compiler_library = catalog.libraries.iter().find(|library| {
            library.group == "androidx.compose.compiler" && library.name == "compiler"
        });

        let compose_plugin = catalog
            .plugins
            .iter()
            .find(|plugin| plugin.id == "org.jetbrains.kotlin.plugin.compose");

        let Some(kotlin_version) = catalog.primary_kotlin_version() else {
            return;
        };

        if parse_major(&kotlin_version.version).is_some_and(|major| major >= 2) {
            if has_compose_libraries && compose_plugin.is_none() {
                report.add(
                    DoctorSeverity::Warning,
                    "compose_plugin_missing_for_kotlin_2",
                    "Compose libraries are declared with Kotlin 2.x, but the Kotlin Compose compiler plugin is not in the catalog.",
                    "Add plugin 'org.jetbrains.kotlin.plugin.compose' and keep its version aligned with the Kotlin Gradle plugin.",
                    vec![kotlin_version.describe()],
                );
            }

            if let Some(library) = compose_compiler_library {
                report.add(
                    DoctorSeverity::Warning,
                    "legacy_compose_compiler_with_kotlin_2",
                    "The legacy androidx.compose.compiler:compiler dependency is declared with Kotlin 2.x.",
                    "Prefer the Kotlin Compose compiler Gradle plugin for Kotlin 2.x projects.",
                    vec![kotlin_version.describe(), library.describe()],
                );
            }
        }

        let Some(compose_plugin) = compose_plugin else {
            return;
        };

        let Some(compose_version) = compose_plugin.version.as_deref() else {
            return;
        };

        if compose_version != kotlin_version.version {
            report.add(
                DoctorSeverity::Error,
                "compose_plugin_kotlin_version_mismatch",
                "The Kotlin Compose compiler plugin version does not match the Kotlin Gradle plugin version.",
                "Use the same version for 'org.jetbrains.kotlin.plugin.compose' and the Kotlin Gradle plugin.",
                vec![kotlin_version.describe(), compose_plugin.describe()],
            );
        }
    }

    fn check_android_plugin_versions(catalog: &CatalogSnapshot, report: &mut DoctorReport) {
        let versions_by_value = group_versions(
            catalog
                .plugins
                .iter()
                .filter(|plugin| {
                    matches!(
                        plugin.id.as_str(),
                        "com.android.application"
                            | "com.android.library"
                            | "com.android.test"
                            | "com.android.dynamic-feature"
                    )
                })
                .filter_map(|plugin| {
                    plugin
                        .version
                        .as_ref()
                        .map(|version| (version.clone(), plugin.describe()))
                }),
        );

        if versions_by_value.len() <= 1 {
            return;
        }

        report.add(
            DoctorSeverity::Warning,
            "android_plugin_versions_mixed",
            "Multiple Android Gradle Plugin versions are declared in the catalog.",
            "Use one shared AGP version alias for com.android.* plugins unless a migration requires otherwise.",
            version_groups_evidence(&versions_by_value),
        );
    }
}

#[derive(Clone, Debug)]
struct CatalogSnapshot {
    libraries: Vec<LibraryEntry>,
    plugins: Vec<PluginEntry>,
}

impl CatalogSnapshot {
    fn from_doc(doc: &DocumentMut) -> Self {
        let version_refs = collect_version_refs(doc);
        Self {
            libraries: collect_libraries(doc, &version_refs),
            plugins: collect_plugins(doc, &version_refs),
        }
    }

    fn primary_kotlin_version(&self) -> Option<VersionSource> {
        self.plugins
            .iter()
            .find(|plugin| {
                matches!(
                    plugin.id.as_str(),
                    "org.jetbrains.kotlin.jvm"
                        | "org.jetbrains.kotlin.android"
                        | "org.jetbrains.kotlin.multiplatform"
                )
            })
            .and_then(|plugin| plugin.version_source())
            .or_else(|| {
                self.plugins
                    .iter()
                    .find(|plugin| plugin.id.starts_with("org.jetbrains.kotlin"))
                    .and_then(|plugin| plugin.version_source())
            })
            .or_else(|| {
                self.libraries
                    .iter()
                    .find(|library| {
                        library.group == "org.jetbrains.kotlin"
                            && library.name.starts_with("kotlin-stdlib")
                    })
                    .and_then(|library| library.version_source())
            })
    }
}

#[derive(Clone, Debug)]
struct LibraryEntry {
    alias: String,
    group: String,
    name: String,
    version: Option<String>,
    version_ref: Option<String>,
}

impl LibraryEntry {
    fn describe(&self) -> String {
        describe_entry(
            "library",
            &self.alias,
            &format!("{}:{}", self.group, self.name),
            self.version.as_deref(),
            self.version_ref.as_deref(),
        )
    }

    fn version_source(&self) -> Option<VersionSource> {
        self.version.as_ref().map(|version| VersionSource {
            source: self.describe(),
            version: version.clone(),
        })
    }
}

#[derive(Clone, Debug)]
struct PluginEntry {
    alias: String,
    id: String,
    version: Option<String>,
    version_ref: Option<String>,
}

impl PluginEntry {
    fn describe(&self) -> String {
        describe_entry(
            "plugin",
            &self.alias,
            &self.id,
            self.version.as_deref(),
            self.version_ref.as_deref(),
        )
    }

    fn version_source(&self) -> Option<VersionSource> {
        self.version.as_ref().map(|version| VersionSource {
            source: self.describe(),
            version: version.clone(),
        })
    }
}

#[derive(Clone, Debug)]
struct VersionSource {
    source: String,
    version: String,
}

impl VersionSource {
    fn describe(&self) -> String {
        self.source.clone()
    }
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

fn collect_libraries(
    doc: &DocumentMut,
    version_refs: &HashMap<String, String>,
) -> Vec<LibraryEntry> {
    let Some(libraries) = doc.get("libraries").and_then(|v| v.as_table()) else {
        return Vec::new();
    };

    libraries
        .iter()
        .filter_map(|(alias, item)| {
            let details = TomlUtils::extract_library_details(item)?;
            Some(LibraryEntry {
                alias: alias.to_string(),
                group: details.group,
                name: details.artifact,
                version: resolve_version(
                    details.version,
                    details.version_ref.clone(),
                    version_refs,
                ),
                version_ref: details.version_ref,
            })
        })
        .collect()
}

fn collect_plugins(doc: &DocumentMut, version_refs: &HashMap<String, String>) -> Vec<PluginEntry> {
    let Some(plugins) = doc.get("plugins").and_then(|v| v.as_table()) else {
        return Vec::new();
    };

    plugins
        .iter()
        .filter_map(|(alias, item)| {
            let details = TomlUtils::extract_plugin_details(alias, item)?;
            Some(PluginEntry {
                alias: alias.to_string(),
                id: details.id,
                version: resolve_version(
                    details.version,
                    details.version_ref.clone(),
                    version_refs,
                ),
                version_ref: details.version_ref,
            })
        })
        .collect()
}

fn resolve_version(
    version: Option<String>,
    version_ref: Option<String>,
    version_refs: &HashMap<String, String>,
) -> Option<String> {
    version.or_else(|| version_ref.and_then(|ref_name| version_refs.get(&ref_name).cloned()))
}

fn describe_entry(
    kind: &str,
    alias: &str,
    coordinate: &str,
    version: Option<&str>,
    version_ref: Option<&str>,
) -> String {
    match (version, version_ref) {
        (Some(version), Some(version_ref)) => {
            format!("{kind} '{alias}' ({coordinate}) = {version} via {version_ref}")
        }
        (Some(version), None) => format!("{kind} '{alias}' ({coordinate}) = {version}"),
        (None, Some(version_ref)) => {
            format!("{kind} '{alias}' ({coordinate}) uses unresolved version ref {version_ref}")
        }
        (None, None) => format!("{kind} '{alias}' ({coordinate}) has no version"),
    }
}

fn group_versions(
    entries: impl Iterator<Item = (String, String)>,
) -> BTreeMap<String, Vec<String>> {
    let mut grouped = BTreeMap::<String, Vec<String>>::new();
    for (version, source) in entries {
        grouped.entry(version).or_default().push(source);
    }
    grouped
}

fn version_groups_evidence(grouped: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    grouped
        .iter()
        .map(|(version, sources)| format!("{}: {}", version, sources.join(", ")))
        .collect()
}

fn parse_major(version: &str) -> Option<u32> {
    version
        .split(['.', '-'])
        .next()
        .and_then(|major| major.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analyze(input: &str) -> DoctorReport {
        let doc = input.parse::<DocumentMut>().unwrap();
        KotlinDoctor::analyze(&doc)
    }

    fn has_code(report: &DoctorReport, code: &str) -> bool {
        report.findings.iter().any(|finding| finding.code == code)
    }

    #[test]
    fn reports_ok_for_aligned_kotlin_android_catalog() {
        let report = analyze(
            r#"
[versions]
kotlin = "2.0.21"
ksp = "2.0.21-1.0.28"
agp = "8.7.2"

[plugins]
kotlin-android = { id = "org.jetbrains.kotlin.android", version = { ref = "kotlin" } }
kotlin-compose = { id = "org.jetbrains.kotlin.plugin.compose", version = { ref = "kotlin" } }
ksp = { id = "com.google.devtools.ksp", version = { ref = "ksp" } }
android-application = { id = "com.android.application", version = { ref = "agp" } }
android-library = { id = "com.android.library", version = { ref = "agp" } }

[libraries]
compose-ui = { module = "androidx.compose.ui:ui", version = "1.7.0" }
"#,
        );

        assert_eq!(report.total(), 0);
        assert!(!report.has_issues());
    }

    #[test]
    fn detects_ksp_kotlin_mismatch() {
        let report = analyze(
            r#"
[versions]
kotlin = "2.0.21"
ksp = "1.9.24-1.0.20"

[plugins]
kotlin-android = { id = "org.jetbrains.kotlin.android", version = { ref = "kotlin" } }
ksp = { id = "com.google.devtools.ksp", version = { ref = "ksp" } }
"#,
        );

        assert!(has_code(&report, "ksp_kotlin_version_mismatch"));
        assert_eq!(report.errors(), 1);
    }

    #[test]
    fn detects_compose_plugin_mismatch() {
        let report = analyze(
            r#"
[plugins]
kotlin-android = { id = "org.jetbrains.kotlin.android", version = "2.0.21" }
kotlin-compose = { id = "org.jetbrains.kotlin.plugin.compose", version = "2.0.20" }
"#,
        );

        assert!(has_code(&report, "compose_plugin_kotlin_version_mismatch"));
    }

    #[test]
    fn detects_missing_compose_plugin_for_kotlin_2_projects() {
        let report = analyze(
            r#"
[plugins]
kotlin-android = { id = "org.jetbrains.kotlin.android", version = "2.0.21" }

[libraries]
compose-ui = { module = "androidx.compose.ui:ui", version = "1.7.0" }
"#,
        );

        assert!(has_code(&report, "compose_plugin_missing_for_kotlin_2"));
    }

    #[test]
    fn detects_mixed_android_plugin_versions() {
        let report = analyze(
            r#"
[plugins]
android-application = { id = "com.android.application", version = "8.7.2" }
android-library = { id = "com.android.library", version = "8.6.1" }
"#,
        );

        assert!(has_code(&report, "android_plugin_versions_mixed"));
    }
}
