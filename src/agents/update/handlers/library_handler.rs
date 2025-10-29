use crate::agents::update::context::UpdateReport;
use crate::agents::update::interaction::UpdateInteraction;
use crate::error::Result;
use crate::repository::{Coordinate, RepositoryClient, VersionStrategy};
use crate::utils::toml::{LibraryDetails, TomlUtils};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::sync::Arc;
use toml_edit::{Item, Table, Value};

/// Handles updates for the [libraries] section of the version catalog
///
/// This handler checks and updates library dependencies directly
/// (not using version references).
pub struct LibraryHandler<'a> {
    library_client: &'a (dyn RepositoryClient + Send + Sync),
    version_strategy: Arc<dyn VersionStrategy>,
    interaction: &'a mut UpdateInteraction,
}

impl<'a> LibraryHandler<'a> {
    /// Create a new LibraryHandler
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

    /// Update libraries section
    ///
    /// Checks each library for newer versions and updates them
    /// if the user confirms (in interactive mode).
    pub fn update(&mut self, libraries: &mut Table, stable_only: bool) -> Result<UpdateReport> {
        let mut report = UpdateReport::new();
        let keys: Vec<String> = libraries.iter().map(|(k, _)| k.to_string()).collect();

        println!("\n{}", "Checking library updates...".cyan());

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

            if let Some(lib_value) = libraries.get_mut(&key) {
                if let Some(updated) = self.check_library_update(&key, lib_value, stable_only)? {
                    report.add_library_update(
                        key.clone(),
                        updated.old_version,
                        updated.new_version,
                    );
                }
            }

            pb.inc(1);
        }
        pb.finish_and_clear();

        Ok(report)
    }

    /// Check libraries section (read-only)
    ///
    /// Checks for updates without modifying the catalog.
    pub fn check(&mut self, libraries: &Table, stable_only: bool) -> Result<UpdateReport> {
        let mut report = UpdateReport::new();
        let keys: Vec<String> = libraries.iter().map(|(k, _)| k.to_string()).collect();

        println!("\n{}", "Checking library updates...".cyan());

        let pb = ProgressBar::new(keys.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  [{bar:40}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-"),
        );

        for key in keys {
            pb.set_message(format!("Checking {}", key));

            if let Some(lib_value) = libraries.get(&key) {
                if let Some(updated) = self.check_library_for_update(lib_value, stable_only)? {
                    report.add_library_update(
                        key.clone(),
                        updated.old_version,
                        updated.new_version,
                    );
                }
            }

            pb.inc(1);
        }
        pb.finish_and_clear();

        Ok(report)
    }

    /// Check a single library for updates (read-only)
    fn check_library_for_update(
        &self,
        lib_value: &Item,
        stable_only: bool,
    ) -> Result<Option<DependencyUpdate>> {
        let details = match TomlUtils::extract_library_details(lib_value) {
            Some(details) => details,
            None => return Ok(None),
        };

        let current = match details.version {
            Some(ref version) => version,
            None => return Ok(None),
        };

        let coordinate = Coordinate::new(details.group.as_str(), details.artifact.as_str());
        if let Some(latest) = self
            .library_client
            .fetch_latest_version(&coordinate, stable_only)?
        {
            if latest != *current && self.version_strategy.is_upgrade(current, &latest) {
                return Ok(Some(DependencyUpdate {
                    old_version: current.to_string(),
                    new_version: latest,
                }));
            }
        }

        Ok(None)
    }

    /// Check and update a single library
    fn check_library_update(
        &mut self,
        name: &str,
        lib_value: &mut Item,
        stable_only: bool,
    ) -> Result<Option<DependencyUpdate>> {
        let details = match TomlUtils::extract_library_details(lib_value) {
            Some(details) => details,
            None => return Ok(None),
        };

        let LibraryDetails {
            group,
            artifact,
            version,
            ..
        } = details;

        let current = match version {
            Some(current) => current,
            None => return Ok(None), // skip when using version.ref
        };

        let coordinate = Coordinate::new(group.as_str(), artifact.as_str());
        let latest = match self
            .library_client
            .fetch_latest_version(&coordinate, stable_only)?
        {
            Some(latest) => latest,
            None => return Ok(None),
        };

        if latest == current
            || !self.version_strategy.is_upgrade(&current, &latest)
            || !self.interaction.confirm_library(name, &current, &latest)?
        {
            return Ok(None);
        }

        if lib_value.as_str().is_some() {
            let new_coord = format!("{}:{}:{}", group, artifact, latest);
            *lib_value = Item::Value(Value::from(new_coord));
        } else {
            TomlUtils::update_version(lib_value, latest.as_str());
        }

        Ok(Some(DependencyUpdate {
            old_version: current,
            new_version: latest,
        }))
    }
}

#[derive(Debug, Clone)]
struct DependencyUpdate {
    old_version: String,
    new_version: String,
}
