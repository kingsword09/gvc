use crate::agents::update::context::UpdateReport;
use crate::agents::update::interaction::UpdateInteraction;
use crate::error::Result;
use crate::repository::{Coordinate, RepositoryClient, VersionStrategy};
use crate::utils::toml::TomlUtils;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::sync::Arc;
use toml_edit::DocumentMut;

/// Handles updates for the [versions] section of the version catalog
///
/// This handler is responsible for checking and updating version aliases
/// in the [versions] section. It works with library context to determine
/// which version to check against.
pub struct VersionHandler<'a> {
    library_client: &'a (dyn RepositoryClient + Send + Sync),
    version_strategy: Arc<dyn VersionStrategy>,
    interaction: &'a mut UpdateInteraction,
}

impl<'a> VersionHandler<'a> {
    /// Create a new VersionHandler
    pub fn new(
        library_client: &'a (dyn RepositoryClient + Send + Sync),
        version_strategy: Arc<dyn VersionStrategy>,
        interaction: &'a mut UpdateInteraction,
    ) -> Self {
        Self {
            library_client,
            version_strategy,
            interaction,
        }
    }

    /// Update versions section with library context
    ///
    /// This method:
    /// 1. Extracts version aliases from [versions] section
    /// 2. For each version, finds a representative library
    /// 3. Checks for updates to that library
    /// 4. Prompts user for confirmation (if interactive)
    /// 5. Updates the version alias if confirmed
    pub fn update(&mut self, doc: &mut DocumentMut, stable_only: bool) -> Result<UpdateReport> {
        let mut report = UpdateReport::new();

        // Clone the data we need to read before mutating
        let versions_data: Vec<(String, String)> =
            if let Some(versions) = doc.get("versions").and_then(|v| v.as_table()) {
                versions
                    .iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.to_string(), s.to_string())))
                    .collect()
            } else {
                return Ok(report);
            };

        let libraries_data: Vec<(String, toml_edit::Item)> =
            if let Some(libraries) = doc.get("libraries").and_then(|v| v.as_table()) {
                libraries
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.clone()))
                    .collect()
            } else {
                return Ok(report);
            };

        if versions_data.is_empty() {
            return Ok(report);
        }

        println!("\n{}", "Checking version updates...".cyan());

        let pb = ProgressBar::new(versions_data.len() as u64);
        if self.interaction.is_enabled() {
            pb.set_draw_target(ProgressDrawTarget::hidden());
        }
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  [{bar:40}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-"),
        );

        for (version_key, current_version) in versions_data {
            pb.set_message(format!("Checking {}", version_key));

            // Find a library that uses this version reference
            let representative_lib = libraries_data.iter().find_map(|(_name, lib_value)| {
                if !TomlUtils::uses_version_ref(lib_value, &version_key) {
                    return None;
                }
                TomlUtils::extract_library_details(lib_value)
                    .map(|details| (details.group, details.artifact))
            });

            if let Some((group, artifact)) = representative_lib {
                let coordinate = Coordinate::new(group.as_str(), artifact.as_str());
                if let Some(latest) = self
                    .library_client
                    .fetch_latest_version(&coordinate, stable_only)?
                {
                    if latest != current_version
                        && self.version_strategy.is_upgrade(&current_version, &latest)
                        && self.interaction.confirm_version(
                            &version_key,
                            &current_version,
                            &latest,
                        )?
                    {
                        if let Some(entry) = doc
                            .get_mut("versions")
                            .and_then(|v| v.as_table_mut())
                            .and_then(|table| table.get_mut(&version_key))
                        {
                            TomlUtils::update_version(entry, latest.as_str());
                            report.add_version_update(version_key.clone(), current_version, latest);
                        }
                    }
                }
            }

            pb.inc(1);
        }
        pb.finish_and_clear();

        Ok(report)
    }

    /// Check versions section (read-only, no modifications)
    ///
    /// This method performs the same logic as update() but without
    /// modifying the document or prompting the user.
    pub fn check(&mut self, doc: &DocumentMut, stable_only: bool) -> Result<UpdateReport> {
        let mut report = UpdateReport::new();

        let versions_data: Vec<(String, String)> =
            if let Some(versions) = doc.get("versions").and_then(|v| v.as_table()) {
                versions
                    .iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.to_string(), s.to_string())))
                    .collect()
            } else {
                return Ok(report);
            };

        let libraries_data: Vec<(String, toml_edit::Item)> =
            if let Some(libraries) = doc.get("libraries").and_then(|v| v.as_table()) {
                libraries
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.clone()))
                    .collect()
            } else {
                return Ok(report);
            };

        if versions_data.is_empty() {
            return Ok(report);
        }

        println!("\n{}", "Checking version variables...".cyan());

        let keys: Vec<String> = versions_data.iter().map(|(k, _)| k.clone()).collect();
        let pb = ProgressBar::new(keys.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  [{bar:40}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-"),
        );

        for version_key in keys {
            pb.set_message(format!("Checking {}", version_key));

            // Get current version
            let current_version = match versions_data
                .iter()
                .find(|(k, _)| k == &version_key)
                .map(|(_, v)| v)
            {
                Some(v) => v,
                None => {
                    pb.inc(1);
                    continue;
                }
            };

            // Find the first library that references this version (as a representative)
            let representative_lib = libraries_data.iter().find_map(|(_name, lib_value)| {
                if !TomlUtils::uses_version_ref(lib_value, &version_key) {
                    return None;
                }
                TomlUtils::extract_library_details(lib_value)
                    .map(|details| (details.group, details.artifact))
            });

            // If no libraries reference this version, skip
            if representative_lib.is_none() {
                pb.inc(1);
                continue;
            }

            // Query latest version for the representative library only
            let (group, artifact) = representative_lib.unwrap();
            let coordinate = Coordinate::new(group.as_str(), artifact.as_str());
            if let Some(latest) = self
                .library_client
                .fetch_latest_version(&coordinate, stable_only)?
            {
                if latest != *current_version
                    && self.version_strategy.is_upgrade(current_version, &latest)
                {
                    report.add_version_update(
                        version_key.clone(),
                        current_version.to_string(),
                        latest,
                    );
                }
            }

            pb.inc(1);
        }
        pb.finish_and_clear();

        Ok(report)
    }
}
