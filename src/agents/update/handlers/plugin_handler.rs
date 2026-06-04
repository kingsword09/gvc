use crate::agents::update::context::UpdateReport;
use crate::agents::update::interaction::UpdateInteraction;
use crate::error::{GvcError, Result};
use crate::repository::{Coordinate, RepositoryClient, VersionStrategy};
use crate::utils::toml::{PluginDetails, TomlUtils};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::collections::HashMap;
use std::sync::Arc;
use toml_edit::DocumentMut;

/// Handles updates for the [plugins] section of the version catalog
///
/// This handler checks and updates Gradle plugins from the Plugin Portal.
pub struct PluginHandler<'a> {
    plugin_client: &'a (dyn RepositoryClient + Send + Sync),
    version_strategy: Arc<dyn VersionStrategy>,
    interaction: &'a mut UpdateInteraction,
}

impl<'a> PluginHandler<'a> {
    /// Create a new PluginHandler
    pub fn new(
        plugin_client: &'a (dyn RepositoryClient + Send + Sync),
        version_strategy: Arc<dyn VersionStrategy>,
        interaction: &'a mut UpdateInteraction,
    ) -> Self {
        Self {
            plugin_client,
            version_strategy,
            interaction,
        }
    }

    /// Update plugins section
    ///
    /// Checks each plugin for newer versions on the Gradle Plugin Portal
    /// and updates them if the user confirms (in interactive mode).
    pub fn update(&mut self, doc: &mut DocumentMut, stable_only: bool) -> Result<UpdateReport> {
        let mut report = UpdateReport::new();
        let candidates = Self::collect_candidates(doc);

        crate::outln!("\n{}", "Checking plugin updates...".cyan());

        let pb = ProgressBar::new(candidates.len() as u64);
        if self.interaction.is_enabled() || crate::utils::output::is_quiet() {
            pb.set_draw_target(ProgressDrawTarget::hidden());
        }
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  [{bar:40}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-"),
        );

        for candidate in candidates {
            pb.set_message(format!("Checking {}", candidate.alias));

            if let Some(updated) = self.check_candidate_update(&candidate, stable_only)? {
                self.apply_candidate_update(doc, &candidate, &updated.new_version)?;
                report.add_plugin_update(candidate.alias, updated.old_version, updated.new_version);
            }

            pb.inc(1);
        }
        pb.finish_and_clear();

        Ok(report)
    }

    /// Check plugins section (read-only)
    ///
    /// Checks for updates without modifying the catalog.
    pub fn check(&mut self, doc: &DocumentMut, stable_only: bool) -> Result<UpdateReport> {
        let mut report = UpdateReport::new();
        let candidates = Self::collect_candidates(doc);

        crate::outln!("\n{}", "Checking plugin updates...".cyan());

        let pb = ProgressBar::new(candidates.len() as u64);
        if crate::utils::output::is_quiet() {
            pb.set_draw_target(ProgressDrawTarget::hidden());
        }
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  [{bar:40}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-"),
        );

        for candidate in candidates {
            pb.set_message(format!("Checking {}", candidate.alias));

            if let Some(updated) = self.check_candidate_update(&candidate, stable_only)? {
                report.add_plugin_update(candidate.alias, updated.old_version, updated.new_version);
            }

            pb.inc(1);
        }
        pb.finish_and_clear();

        Ok(report)
    }

    fn check_candidate_update(
        &mut self,
        candidate: &PluginCandidate,
        stable_only: bool,
    ) -> Result<Option<DependencyUpdate>> {
        let coordinate = Coordinate::plugin(candidate.plugin_id.as_str());
        if let Some(latest) = self
            .plugin_client
            .fetch_latest_version(&coordinate, stable_only)?
        {
            if latest != candidate.current_version
                && self
                    .version_strategy
                    .is_upgrade(&candidate.current_version, &latest)
                && self.interaction.confirm_plugin(
                    &candidate.alias,
                    &candidate.current_version,
                    &latest,
                )?
            {
                return Ok(Some(DependencyUpdate {
                    old_version: candidate.current_version.clone(),
                    new_version: latest,
                }));
            }
        }
        Ok(None)
    }

    fn apply_candidate_update(
        &self,
        doc: &mut DocumentMut,
        candidate: &PluginCandidate,
        new_version: &str,
    ) -> Result<()> {
        if let Some(version_ref) = &candidate.version_ref {
            let versions = doc
                .get_mut("versions")
                .and_then(|v| v.as_table_mut())
                .ok_or_else(|| GvcError::TomlParsing("Missing [versions] section".to_string()))?;
            let entry = versions.get_mut(version_ref).ok_or_else(|| {
                GvcError::TomlParsing(format!("Version alias '{}' not found", version_ref))
            })?;

            if TomlUtils::update_version(entry, new_version) {
                return Ok(());
            }

            return Err(GvcError::TomlParsing(format!(
                "Unsupported version alias '{}' format",
                version_ref
            )));
        }

        let plugins = doc
            .get_mut("plugins")
            .and_then(|v| v.as_table_mut())
            .ok_or_else(|| GvcError::TomlParsing("Missing [plugins] section".to_string()))?;
        let item = plugins.get_mut(&candidate.alias).ok_or_else(|| {
            GvcError::TomlParsing(format!("Plugin '{}' not found in catalog", candidate.alias))
        })?;

        if TomlUtils::update_version(item, new_version) {
            return Ok(());
        }

        Err(GvcError::TomlParsing(
            "Unsupported plugin definition format for update".to_string(),
        ))
    }

    fn collect_candidates(doc: &DocumentMut) -> Vec<PluginCandidate> {
        let version_refs = Self::collect_version_refs(doc);
        let Some(plugins) = doc.get("plugins").and_then(|v| v.as_table()) else {
            return Vec::new();
        };

        plugins
            .iter()
            .filter_map(|(alias, item)| {
                let details = TomlUtils::extract_plugin_details(alias, item)?;
                Self::candidate_from_details(alias, details, &version_refs)
            })
            .collect()
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

    fn candidate_from_details(
        alias: &str,
        details: PluginDetails,
        version_refs: &HashMap<String, String>,
    ) -> Option<PluginCandidate> {
        if let Some(version) = details.version {
            return Some(PluginCandidate {
                alias: alias.to_string(),
                plugin_id: details.id,
                current_version: version,
                version_ref: None,
            });
        }

        let version_ref = details.version_ref?;
        let current_version = version_refs.get(&version_ref)?.clone();
        Some(PluginCandidate {
            alias: alias.to_string(),
            plugin_id: details.id,
            current_version,
            version_ref: Some(version_ref),
        })
    }
}

#[derive(Debug, Clone)]
struct DependencyUpdate {
    old_version: String,
    new_version: String,
}

#[derive(Debug, Clone)]
struct PluginCandidate {
    alias: String,
    plugin_id: String,
    current_version: String,
    version_ref: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::DefaultVersionStrategy;

    struct StaticPluginClient;

    impl RepositoryClient for StaticPluginClient {
        fn fetch_available_versions(&self, _coordinate: &Coordinate) -> Result<Vec<String>> {
            Ok(vec!["2.0.0".to_string(), "1.0.0".to_string()])
        }

        fn fetch_latest_version(
            &self,
            _coordinate: &Coordinate,
            _stable_only: bool,
        ) -> Result<Option<String>> {
            Ok(Some("2.0.0".to_string()))
        }
    }

    #[test]
    fn updates_inline_plugin_version() {
        let mut doc: DocumentMut = r#"
[plugins]
kotlin-jvm = { id = "org.jetbrains.kotlin.jvm", version = "1.0.0" }
"#
        .parse()
        .unwrap();
        let client = StaticPluginClient;
        let mut interaction = UpdateInteraction::new(false);
        let mut handler =
            PluginHandler::new(&client, DefaultVersionStrategy::shared(), &mut interaction);

        let report = handler.update(&mut doc, true).unwrap();

        assert_eq!(
            doc["plugins"]["kotlin-jvm"]
                .as_inline_table()
                .unwrap()
                .get("version")
                .and_then(|v| v.as_str()),
            Some("2.0.0")
        );
        assert_eq!(
            report.plugin_updates.get("kotlin-jvm"),
            Some(&("1.0.0".to_string(), "2.0.0".to_string()))
        );
    }

    #[test]
    fn updates_plugin_version_reference() {
        let mut doc: DocumentMut = r#"
[versions]
kotlin = "1.0.0"

[plugins]
kotlin-jvm = { id = "org.jetbrains.kotlin.jvm", version = { ref = "kotlin" } }
"#
        .parse()
        .unwrap();
        let client = StaticPluginClient;
        let mut interaction = UpdateInteraction::new(false);
        let mut handler =
            PluginHandler::new(&client, DefaultVersionStrategy::shared(), &mut interaction);

        let report = handler.update(&mut doc, true).unwrap();

        assert_eq!(doc["versions"]["kotlin"].as_str(), Some("2.0.0"));
        assert_eq!(
            report.plugin_updates.get("kotlin-jvm"),
            Some(&("1.0.0".to_string(), "2.0.0".to_string()))
        );
    }

    #[test]
    fn checks_plugin_version_reference() {
        let doc: DocumentMut = r#"
[versions]
kotlin = "1.0.0"

[plugins]
kotlin-jvm = { id = "org.jetbrains.kotlin.jvm", version = { ref = "kotlin" } }
"#
        .parse()
        .unwrap();
        let client = StaticPluginClient;
        let mut interaction = UpdateInteraction::new(false);
        let mut handler =
            PluginHandler::new(&client, DefaultVersionStrategy::shared(), &mut interaction);

        let report = handler.check(&doc, true).unwrap();

        assert_eq!(
            report.plugin_updates.get("kotlin-jvm"),
            Some(&("1.0.0".to_string(), "2.0.0".to_string()))
        );
    }
}
