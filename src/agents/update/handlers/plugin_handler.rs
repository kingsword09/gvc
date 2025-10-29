use crate::agents::update::context::UpdateReport;
use crate::agents::update::interaction::UpdateInteraction;
use crate::error::Result;
use crate::repository::{Coordinate, RepositoryClient, VersionStrategy};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::sync::Arc;
use toml_edit::{Item, Table, Value};

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
    pub fn update(&mut self, plugins: &mut Table, stable_only: bool) -> Result<UpdateReport> {
        let mut report = UpdateReport::new();
        let keys: Vec<String> = plugins.iter().map(|(k, _)| k.to_string()).collect();

        println!("\n{}", "Checking plugin updates...".cyan());

        let pb = ProgressBar::new(keys.len() as u64);
        if self.interaction.is_enabled() {
            pb.set_draw_target(ProgressDrawTarget::hidden());
        }
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  [{bar:40}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-"),
        );

        for key in keys {
            pb.set_message(format!("Checking {}", key));

            if let Some(plugin_value) = plugins.get_mut(&key) {
                if let Some(updated) = self.check_plugin_update(&key, plugin_value, stable_only)? {
                    report.add_plugin_update(key.clone(), updated.old_version, updated.new_version);
                }
            }

            pb.inc(1);
        }
        pb.finish_and_clear();

        Ok(report)
    }

    /// Check plugins section (read-only)
    ///
    /// Checks for updates without modifying the catalog.
    #[allow(dead_code)]
    pub fn check(&mut self, plugins: &Table, stable_only: bool) -> Result<UpdateReport> {
        let mut report = UpdateReport::new();
        let keys: Vec<String> = plugins.iter().map(|(k, _)| k.to_string()).collect();

        println!("\n{}", "Checking plugin updates...".cyan());

        let pb = ProgressBar::new(keys.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  [{bar:40}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-"),
        );

        for key in keys {
            pb.set_message(format!("Checking {}", key));

            if let Some(plugin_value) = plugins.get(&key) {
                if let Some(updated) = self.check_plugin_for_update(plugin_value, stable_only)? {
                    report.add_plugin_update(key.clone(), updated.old_version, updated.new_version);
                }
            }

            pb.inc(1);
        }
        pb.finish_and_clear();

        Ok(report)
    }

    /// Check a single plugin for updates (read-only)
    #[allow(dead_code)]
    fn check_plugin_for_update(
        &self,
        plugin_value: &Item,
        stable_only: bool,
    ) -> Result<Option<DependencyUpdate>> {
        // Plugins are queried from Gradle Plugin Portal
        // Format: { id = "org.jetbrains.kotlin.jvm", version = "1.9.0" }
        if let Some(table) = plugin_value.as_table() {
            let plugin_id = if let Some(id) = table.get("id").and_then(|v| v.as_str()) {
                id.to_string()
            } else {
                return Ok(None);
            };

            // Get current version
            let current_version = if let Some(version_item) = table.get("version") {
                if let Some(v) = version_item.as_str() {
                    v.to_string()
                } else if let Some(version_table) = version_item.as_table() {
                    // Handle version.ref case - skip for now as it's handled in versions section
                    if version_table.get("ref").is_some() {
                        return Ok(None);
                    }
                    return Ok(None);
                } else if let Some(version_inline) = version_item.as_inline_table() {
                    // Handle version.ref case - skip for now as it's handled in versions section
                    if version_inline.get("ref").is_some() {
                        return Ok(None);
                    }
                    return Ok(None);
                } else {
                    return Ok(None);
                }
            } else {
                return Ok(None);
            };

            // Fetch latest version from Plugin Portal
            let coordinate = Coordinate::plugin(plugin_id.as_str());
            if let Some(latest) = self
                .plugin_client
                .fetch_latest_version(&coordinate, stable_only)?
            {
                if latest != current_version
                    && self.version_strategy.is_upgrade(&current_version, &latest)
                {
                    return Ok(Some(DependencyUpdate {
                        old_version: current_version,
                        new_version: latest,
                    }));
                }
            }
        }
        Ok(None)
    }

    /// Check and update a single plugin
    fn check_plugin_update(
        &mut self,
        name: &str,
        plugin_value: &mut Item,
        stable_only: bool,
    ) -> Result<Option<DependencyUpdate>> {
        // Plugins are queried from Gradle Plugin Portal
        // Format: { id = "org.jetbrains.kotlin.jvm", version = "1.9.0" }
        if let Some(table) = plugin_value.as_table_mut() {
            let plugin_id = if let Some(id) = table.get("id").and_then(|v| v.as_str()) {
                id.to_string()
            } else {
                return Ok(None);
            };

            // Get current version
            let current_version = if let Some(version_item) = table.get("version") {
                if let Some(v) = version_item.as_str() {
                    v.to_string()
                } else if let Some(version_table) = version_item.as_table() {
                    // Handle version.ref case - skip for now as it's handled in versions section
                    if version_table.get("ref").is_some() {
                        return Ok(None);
                    }
                    return Ok(None);
                } else if let Some(version_inline) = version_item.as_inline_table() {
                    // Handle version.ref case - skip for now as it's handled in versions section
                    if version_inline.get("ref").is_some() {
                        return Ok(None);
                    }
                    return Ok(None);
                } else {
                    return Ok(None);
                }
            } else {
                return Ok(None);
            };

            // Fetch latest version from Plugin Portal
            let coordinate = Coordinate::plugin(plugin_id.as_str());
            if let Some(latest) = self
                .plugin_client
                .fetch_latest_version(&coordinate, stable_only)?
            {
                if latest != current_version
                    && self.version_strategy.is_upgrade(&current_version, &latest)
                    && self
                        .interaction
                        .confirm_plugin(name, &current_version, &latest)?
                {
                    *table.get_mut("version").unwrap() = Item::Value(Value::from(latest.as_str()));
                    return Ok(Some(DependencyUpdate {
                        old_version: current_version,
                        new_version: latest,
                    }));
                }
            }
        }
        Ok(None)
    }
}

#[derive(Debug, Clone)]
struct DependencyUpdate {
    old_version: String,
    new_version: String,
}
