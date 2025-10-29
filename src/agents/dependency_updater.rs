use crate::error::{GvcError, Result};
use crate::maven::version::Version;
use crate::repository::{
    Coordinate, DefaultVersionStrategy, RepositoryClient, RepositoryFactory, VersionStrategy,
};
use crate::utils::toml::{LibraryDetails, TomlUtils};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use regex::Regex;
use std::cmp::min;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;
use toml_edit::{DocumentMut, Item, Value};

/// DependencyUpdater handles the actual dependency version updates
pub struct DependencyUpdater {
    library_client: Arc<dyn RepositoryClient>,
    plugin_client: Arc<dyn RepositoryClient>,
    version_strategy: Arc<dyn VersionStrategy>,
}

const VERSION_PAGE_SIZE: usize = 10;

impl DependencyUpdater {
    pub fn with_repositories(repositories: Vec<crate::gradle::Repository>) -> Result<Self> {
        Self::with_clients(
            RepositoryFactory::create_maven(repositories)?,
            RepositoryFactory::create_plugin_portal()?,
            DefaultVersionStrategy::shared(),
        )
    }

    fn with_clients(
        library_client: Arc<dyn RepositoryClient>,
        plugin_client: Arc<dyn RepositoryClient>,
        version_strategy: Arc<dyn VersionStrategy>,
    ) -> Result<Self> {
        Ok(Self {
            library_client,
            plugin_client,
            version_strategy,
        })
    }

    /// Check for updates without modifying the file
    pub fn check_for_updates<P: AsRef<Path>>(
        &self,
        catalog_path: P,
        stable_only: bool,
    ) -> Result<UpdateReport> {
        let catalog_path = catalog_path.as_ref();

        // Read TOML document (but don't write back)
        let content = fs::read_to_string(catalog_path)
            .map_err(|e| GvcError::TomlParsing(format!("Failed to read catalog: {}", e)))?;

        let doc = content
            .parse::<DocumentMut>()
            .map_err(|e| GvcError::TomlParsing(format!("Failed to parse TOML: {}", e)))?;

        let mut report = UpdateReport::new();

        // Check [versions] section first
        if let Some(versions) = doc.get("versions").and_then(|v| v.as_table()) {
            if let Some(libraries) = doc.get("libraries").and_then(|v| v.as_table()) {
                println!("\n{}", "Checking version variables...".cyan());
                self.check_versions_section(versions, libraries, stable_only, &mut report)?;
            }
        }

        // Check [libraries] section (direct versions only)
        if let Some(libraries) = doc.get("libraries").and_then(|v| v.as_table()) {
            println!("\n{}", "Checking library updates...".cyan());
            self.check_libraries_section(libraries, stable_only, &mut report)?;
        }

        Ok(report)
    }

    /// Update the version catalog file
    pub fn update_version_catalog<P: AsRef<Path>>(
        &self,
        catalog_path: P,
        stable_only: bool,
        interactive: bool,
    ) -> Result<UpdateReport> {
        let catalog_path = catalog_path.as_ref();

        // Read and parse TOML document
        let content = fs::read_to_string(catalog_path)
            .map_err(|e| GvcError::TomlParsing(format!("Failed to read catalog: {}", e)))?;

        let mut doc = content
            .parse::<DocumentMut>()
            .map_err(|e| GvcError::TomlParsing(format!("Failed to parse TOML: {}", e)))?;

        let mut report = UpdateReport::new();
        let mut interaction = Interaction::new(interactive);

        // Update [versions] section - need to check with libraries context
        println!("\n{}", "Checking version updates...".cyan());
        self.update_versions_with_context(&mut doc, stable_only, &mut report, &mut interaction)?;

        // Update [libraries] section
        if let Some(libraries) = doc.get_mut("libraries").and_then(|v| v.as_table_mut()) {
            println!("\n{}", "Checking library updates...".cyan());
            self.update_libraries_section(libraries, stable_only, &mut report, &mut interaction)?;
        }

        // Update [plugins] section
        if let Some(plugins) = doc.get_mut("plugins").and_then(|v| v.as_table_mut()) {
            println!("\n{}", "Checking plugin updates...".cyan());
            self.update_plugins_section(plugins, stable_only, &mut report, &mut interaction)?;
        }

        // Write back the updated document
        if !report.is_empty() {
            fs::write(catalog_path, doc.to_string())
                .map_err(|e| GvcError::TomlParsing(format!("Failed to write catalog: {}", e)))?;
        }

        Ok(report)
    }

    pub fn update_targeted_dependency<P: AsRef<Path>>(
        &self,
        catalog_path: P,
        stable_only: bool,
        interactive: bool,
        pattern: &str,
    ) -> Result<UpdateReport> {
        let catalog_path = catalog_path.as_ref();
        let content = fs::read_to_string(catalog_path)
            .map_err(|e| GvcError::TomlParsing(format!("Failed to read catalog: {}", e)))?;

        let mut doc = content
            .parse::<DocumentMut>()
            .map_err(|e| GvcError::TomlParsing(format!("Failed to parse TOML: {}", e)))?;

        let matcher = PatternMatcher::new(pattern)?;
        let mut candidates = self.collect_target_candidates(&doc, &matcher)?;

        if candidates.is_empty() {
            println!(
                "{}",
                format!("No dependencies matched pattern '{}'.", pattern).yellow()
            );
            return Ok(UpdateReport::new());
        }

        let selected_index = Self::prompt_candidate_selection(&candidates)?;
        let candidate = candidates.remove(selected_index);

        let version_entries = self.fetch_versions_for_candidate(&candidate)?;
        if version_entries.is_empty() {
            println!(
                "{}",
                format!("No versions found for {}.", candidate.display_name()).yellow()
            );
            return Ok(UpdateReport::new());
        }

        let chosen_version = if interactive {
            match Self::prompt_version_selection(&candidate, &version_entries)? {
                Some(version) => version,
                None => {
                    println!("{}", "No changes applied.".yellow());
                    return Ok(UpdateReport::new());
                }
            }
        } else {
            match Self::select_default_version(&version_entries, stable_only) {
                Some(version) => {
                    println!(
                        "{}",
                        format!(
                            "Automatically selecting version {} for {}.",
                            version.green().bold(),
                            candidate.display_name()
                        )
                        .cyan()
                    );
                    version
                }
                None => {
                    println!(
                        "{}",
                        format!(
                            "No newer version available for {}.",
                            candidate.display_name()
                        )
                        .yellow()
                    );
                    return Ok(UpdateReport::new());
                }
            }
        };

        if chosen_version == candidate.current_version {
            println!(
                "{}",
                "Selected version matches the current version; nothing to update.".yellow()
            );
            return Ok(UpdateReport::new());
        }

        let mut report = UpdateReport::new();
        Self::apply_target_update(&mut doc, &candidate, &chosen_version, &mut report)?;

        fs::write(catalog_path, doc.to_string())
            .map_err(|e| GvcError::TomlParsing(format!("Failed to write catalog: {}", e)))?;

        Ok(report)
    }

    fn update_versions_with_context(
        &self,
        doc: &mut DocumentMut,
        stable_only: bool,
        report: &mut UpdateReport,
        interaction: &mut Interaction,
    ) -> Result<()> {
        // Clone the data we need to read before mutating
        let versions_data: Vec<(String, String)> =
            if let Some(versions) = doc.get("versions").and_then(|v| v.as_table()) {
                versions
                    .iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.to_string(), s.to_string())))
                    .collect()
            } else {
                return Ok(());
            };

        let libraries_data: Vec<(String, toml_edit::Item)> =
            if let Some(libraries) = doc.get("libraries").and_then(|v| v.as_table()) {
                libraries
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.clone()))
                    .collect()
            } else {
                return Ok(());
            };

        if versions_data.is_empty() {
            return Ok(());
        }

        let pb = ProgressBar::new(versions_data.len() as u64);
        if interaction.is_enabled() {
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
                        && interaction.confirm(
                            UpdateCategory::Version,
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
        Ok(())
    }

    #[allow(dead_code)]
    fn update_versions_section(
        &self,
        versions: &mut toml_edit::Table,
        stable_only: bool,
        report: &mut UpdateReport,
    ) -> Result<()> {
        // This function is now deprecated - versions are updated via update_versions_with_context
        // Keep it for compatibility but make it a no-op
        let _ = (versions, stable_only, report);
        Ok(())
    }

    fn check_versions_section(
        &self,
        versions: &toml_edit::Table,
        libraries: &toml_edit::Table,
        stable_only: bool,
        report: &mut UpdateReport,
    ) -> Result<()> {
        let keys: Vec<String> = versions.iter().map(|(k, _)| k.to_string()).collect();
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
            let current_version = match versions.get(&version_key).and_then(|v| v.as_str()) {
                Some(v) => v,
                None => {
                    pb.inc(1);
                    continue;
                }
            };

            // Find the first library that references this version (as a representative)
            let representative_lib = libraries.iter().find_map(|(_name, lib_value)| {
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
                if latest != current_version
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
        Ok(())
    }

    fn check_libraries_section(
        &self,
        libraries: &toml_edit::Table,
        stable_only: bool,
        report: &mut UpdateReport,
    ) -> Result<()> {
        let keys: Vec<String> = libraries.iter().map(|(k, _)| k.to_string()).collect();
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
        Ok(())
    }

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

    fn update_libraries_section(
        &self,
        libraries: &mut toml_edit::Table,
        stable_only: bool,
        report: &mut UpdateReport,
        interaction: &mut Interaction,
    ) -> Result<()> {
        let keys: Vec<String> = libraries.iter().map(|(k, _)| k.to_string()).collect();
        let pb = ProgressBar::new(keys.len() as u64);
        if interaction.is_enabled() {
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
                if let Some(updated) =
                    self.check_library_update(&key, lib_value, stable_only, interaction)?
                {
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
        Ok(())
    }

    fn check_library_update(
        &self,
        name: &str,
        lib_value: &mut Item,
        stable_only: bool,
        interaction: &mut Interaction,
    ) -> Result<Option<DependencyUpdate>> {
        let details = match TomlUtils::extract_library_details(&*lib_value) {
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
            || !interaction.confirm(UpdateCategory::Library, name, &current, &latest)?
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

    fn update_plugins_section(
        &self,
        plugins: &mut toml_edit::Table,
        stable_only: bool,
        report: &mut UpdateReport,
        interaction: &mut Interaction,
    ) -> Result<()> {
        let keys: Vec<String> = plugins.iter().map(|(k, _)| k.to_string()).collect();
        let pb = ProgressBar::new(keys.len() as u64);
        if interaction.is_enabled() {
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
                if let Some(updated) =
                    self.check_plugin_update(&key, plugin_value, stable_only, interaction)?
                {
                    report.add_plugin_update(key.clone(), updated.old_version, updated.new_version);
                }
            }

            pb.inc(1);
        }
        pb.finish_and_clear();
        Ok(())
    }

    fn check_plugin_update(
        &self,
        name: &str,
        plugin_value: &mut Item,
        stable_only: bool,
        interaction: &mut Interaction,
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
                    && interaction.confirm(
                        UpdateCategory::Plugin,
                        name,
                        &current_version,
                        &latest,
                    )?
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

    fn collect_target_candidates(
        &self,
        doc: &DocumentMut,
        matcher: &PatternMatcher,
    ) -> Result<Vec<TargetCandidate>> {
        let mut candidates = Vec::new();

        if let Some(libraries) = doc.get("libraries").and_then(|v| v.as_table()) {
            for (name, item) in libraries.iter() {
                if matcher.matches(name) {
                    if let Some(candidate) = Self::build_library_candidate(name, item) {
                        candidates.push(candidate);
                    }
                }
            }
        }

        if let Some(versions) = doc.get("versions").and_then(|v| v.as_table()) {
            for (name, item) in versions.iter() {
                if !matcher.matches(name) {
                    continue;
                }

                if let Some(current_version) = item.as_str() {
                    if let Some((group, artifact)) = Self::find_representative_coordinate(doc, name)
                    {
                        candidates.push(TargetCandidate {
                            name: name.to_string(),
                            current_version: current_version.to_string(),
                            kind: TargetKind::VersionAlias { group, artifact },
                        });
                    }
                }
            }
        }

        if let Some(plugins) = doc.get("plugins").and_then(|v| v.as_table()) {
            for (name, item) in plugins.iter() {
                if matcher.matches(name) {
                    if let Some(candidate) = Self::build_plugin_candidate(name, item) {
                        candidates.push(candidate);
                    }
                }
            }
        }

        candidates.sort_by_key(|candidate| candidate.display_name());
        Ok(candidates)
    }

    fn build_library_candidate(name: &str, item: &Item) -> Option<TargetCandidate> {
        let details = TomlUtils::extract_library_details(item)?;
        let current_version = details.version?;

        Some(TargetCandidate {
            name: name.to_string(),
            current_version,
            kind: TargetKind::Library {
                group: details.group,
                artifact: details.artifact,
            },
        })
    }

    fn build_plugin_candidate(name: &str, item: &Item) -> Option<TargetCandidate> {
        if let Some(table) = item.as_table() {
            let plugin_id = table.get("id").and_then(|v| v.as_str())?;
            if let Some(current_version) = table.get("version").and_then(|v| v.as_str()) {
                return Some(TargetCandidate {
                    name: name.to_string(),
                    current_version: current_version.to_string(),
                    kind: TargetKind::Plugin {
                        plugin_id: plugin_id.to_string(),
                    },
                });
            }
        } else if let Some(inline_table) = item.as_inline_table() {
            let plugin_id = inline_table.get("id").and_then(|v| v.as_str())?;
            if let Some(current_version) = inline_table.get("version").and_then(|v| v.as_str()) {
                return Some(TargetCandidate {
                    name: name.to_string(),
                    current_version: current_version.to_string(),
                    kind: TargetKind::Plugin {
                        plugin_id: plugin_id.to_string(),
                    },
                });
            }
        }

        None
    }

    fn find_representative_coordinate(
        doc: &DocumentMut,
        version_key: &str,
    ) -> Option<(String, String)> {
        let libraries = doc.get("libraries").and_then(|v| v.as_table())?;
        for (_name, lib_value) in libraries.iter() {
            if TomlUtils::uses_version_ref(lib_value, version_key) {
                if let Some((group, artifact)) = TomlUtils::extract_group_artifact(lib_value) {
                    return Some((group, artifact));
                }
            }
        }
        None
    }

    fn prompt_candidate_selection(candidates: &[TargetCandidate]) -> Result<usize> {
        if candidates.len() == 1 {
            println!(
                "{}",
                format!("Found one match: {}", candidates[0].describe_with_version()).cyan()
            );
            return Ok(0);
        }

        println!(
            "{}",
            format!("Found {} matching dependencies:", candidates.len()).cyan()
        );
        for (idx, candidate) in candidates.iter().enumerate() {
            println!("  {:>2}) {}", idx + 1, candidate.describe_with_version());
        }

        loop {
            print!(
                "Select dependency to update [1-{}] (or 'q' to cancel): ",
                candidates.len()
            );
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let trimmed = input.trim();

            if trimmed.eq_ignore_ascii_case("q") {
                return Err(GvcError::UserCancelled);
            }

            if let Ok(choice) = trimmed.parse::<usize>() {
                if choice >= 1 && choice <= candidates.len() {
                    return Ok(choice - 1);
                }
            }

            println!("{}", "Invalid selection. Please try again.".red());
        }
    }

    fn fetch_versions_for_candidate(
        &self,
        candidate: &TargetCandidate,
    ) -> Result<Vec<VersionEntry>> {
        let versions = match &candidate.kind {
            TargetKind::Library { group, artifact }
            | TargetKind::VersionAlias { group, artifact } => {
                let coordinate = Coordinate::new(group, artifact);
                self.library_client.fetch_available_versions(&coordinate)?
            }
            TargetKind::Plugin { plugin_id } => {
                let coordinate = Coordinate::plugin(plugin_id.as_str());
                self.plugin_client.fetch_available_versions(&coordinate)?
            }
        };

        let mut entries = Vec::with_capacity(versions.len());
        for raw in versions {
            let parsed = Version::parse(&raw);
            let is_stable = parsed.is_stable();
            let is_current = candidate.current_version == raw;
            entries.push(VersionEntry {
                value: raw,
                is_stable,
                is_current,
            });
        }

        Ok(entries)
    }

    fn select_default_version(versions: &[VersionEntry], stable_only: bool) -> Option<String> {
        if stable_only {
            if let Some(entry) = versions
                .iter()
                .find(|entry| entry.is_stable && !entry.is_current)
            {
                return Some(entry.value.clone());
            }
        }

        versions
            .iter()
            .find(|entry| !entry.is_current)
            .map(|entry| entry.value.clone())
    }

    fn prompt_version_selection(
        candidate: &TargetCandidate,
        versions: &[VersionEntry],
    ) -> Result<Option<String>> {
        if versions.is_empty() {
            return Ok(None);
        }

        println!(
            "\n{}",
            format!("Available versions for {}:", candidate.display_name()).cyan()
        );

        let mut limit = versions.len().min(VERSION_PAGE_SIZE);
        loop {
            for (idx, entry) in versions.iter().take(limit).enumerate() {
                let mut labels = Vec::new();
                if entry.is_stable {
                    labels.push("stable");
                } else {
                    labels.push("pre-release");
                }
                if entry.is_current {
                    labels.push("current");
                }
                let label_str = if labels.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", labels.join(", "))
                };

                println!("  {:>2}) {}{}", idx + 1, entry.value.green(), label_str);
            }

            if limit < versions.len() {
                println!("  m ) Show more versions");
            }
            println!("  s ) Skip update");
            println!("  q ) Cancel");

            print!("Select version [1-{} | m/s/q]: ", limit);
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let trimmed = input.trim().to_lowercase();

            match trimmed.as_str() {
                "q" => return Err(GvcError::UserCancelled),
                "s" => return Ok(None),
                "m" => {
                    if limit >= versions.len() {
                        println!("{}", "All versions are already displayed.".yellow());
                    } else {
                        limit = min(limit + VERSION_PAGE_SIZE, versions.len());
                    }
                }
                _ => {
                    if let Ok(choice) = trimmed.parse::<usize>() {
                        if (1..=limit).contains(&choice) {
                            let entry = &versions[choice - 1];
                            if entry.is_current {
                                println!(
                                    "{}",
                                    "Selected version matches current version; choose another or skip."
                                        .yellow()
                                );
                                continue;
                            }
                            return Ok(Some(entry.value.clone()));
                        }
                    }
                    println!("{}", "Invalid selection. Please try again.".red());
                }
            }
        }
    }

    fn apply_target_update(
        doc: &mut DocumentMut,
        candidate: &TargetCandidate,
        new_version: &str,
        report: &mut UpdateReport,
    ) -> Result<()> {
        match &candidate.kind {
            TargetKind::VersionAlias { .. } => {
                Self::apply_version_alias(doc, &candidate.name, new_version)?;
                report.add_version_update(
                    candidate.name.clone(),
                    candidate.current_version.clone(),
                    new_version.to_string(),
                );
            }
            TargetKind::Library { group, artifact } => {
                let libraries = doc
                    .get_mut("libraries")
                    .and_then(|v| v.as_table_mut())
                    .ok_or_else(|| {
                        GvcError::TomlParsing("Missing [libraries] section".to_string())
                    })?;

                let item = libraries.get_mut(&candidate.name).ok_or_else(|| {
                    GvcError::TomlParsing(format!(
                        "Library '{}' not found in catalog",
                        candidate.name
                    ))
                })?;

                Self::apply_library_version(item, group, artifact, new_version)?;
                report.add_library_update(
                    candidate.name.clone(),
                    candidate.current_version.clone(),
                    new_version.to_string(),
                );
            }
            TargetKind::Plugin { .. } => {
                let plugins = doc
                    .get_mut("plugins")
                    .and_then(|v| v.as_table_mut())
                    .ok_or_else(|| {
                        GvcError::TomlParsing("Missing [plugins] section".to_string())
                    })?;

                let item = plugins.get_mut(&candidate.name).ok_or_else(|| {
                    GvcError::TomlParsing(format!(
                        "Plugin '{}' not found in catalog",
                        candidate.name
                    ))
                })?;

                Self::apply_plugin_version(item, new_version)?;
                report.add_plugin_update(
                    candidate.name.clone(),
                    candidate.current_version.clone(),
                    new_version.to_string(),
                );
            }
        }

        println!(
            "{}",
            format!(
                "Updated {}: {} → {}",
                candidate.display_name(),
                candidate.current_version.red(),
                new_version.green().bold()
            )
            .green()
        );

        Ok(())
    }

    fn apply_library_version(
        item: &mut Item,
        group: &str,
        artifact: &str,
        new_version: &str,
    ) -> Result<()> {
        if item.as_str().is_some() {
            let new_coord = format!("{}:{}:{}", group, artifact, new_version);
            *item = Item::Value(Value::from(new_coord.as_str()));
            return Ok(());
        }

        if let Some(inline_table) = item.as_inline_table_mut() {
            inline_table.insert("version", Value::from(new_version));
            return Ok(());
        }

        if let Some(table) = item.as_table_mut() {
            table.insert("version", Item::Value(Value::from(new_version)));
            return Ok(());
        }

        Err(GvcError::TomlParsing(
            "Unsupported library format for targeted update".to_string(),
        ))
    }

    fn apply_version_alias(doc: &mut DocumentMut, name: &str, new_version: &str) -> Result<()> {
        let versions = doc
            .get_mut("versions")
            .and_then(|v| v.as_table_mut())
            .ok_or_else(|| GvcError::TomlParsing("Missing [versions] section".to_string()))?;

        if versions.get(name).is_none() {
            return Err(GvcError::TomlParsing(format!(
                "Version alias '{}' not found",
                name
            )));
        }

        versions.insert(name, toml_edit::value(new_version));
        Ok(())
    }

    fn apply_plugin_version(item: &mut Item, new_version: &str) -> Result<()> {
        if let Some(table) = item.as_table_mut() {
            table.insert("version", Item::Value(Value::from(new_version)));
            return Ok(());
        }

        if let Some(inline_table) = item.as_inline_table_mut() {
            inline_table.insert("version", Value::from(new_version));
            return Ok(());
        }

        Err(GvcError::TomlParsing(
            "Unsupported plugin definition format for targeted update".to_string(),
        ))
    }
}

#[derive(Debug, Clone)]
struct DependencyUpdate {
    old_version: String,
    new_version: String,
}

#[derive(Copy, Clone)]
enum UpdateCategory {
    Version,
    Library,
    Plugin,
}

impl fmt::Display for UpdateCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            UpdateCategory::Version => "Version",
            UpdateCategory::Library => "Library",
            UpdateCategory::Plugin => "Plugin",
        };
        f.write_str(label)
    }
}

struct Interaction {
    enabled: bool,
    apply_all: bool,
}

impl Interaction {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            apply_all: false,
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn confirm(
        &mut self,
        category: UpdateCategory,
        name: &str,
        old: &str,
        new: &str,
    ) -> Result<bool> {
        if !self.enabled {
            return Ok(true);
        }

        let category_label = format!("[{}]", category);
        println!(
            "\n{} {} {} {} to {}",
            category_label.cyan().bold(),
            name.white().bold(),
            "from".dimmed(),
            old.red(),
            new.green().bold()
        );

        if self.apply_all {
            println!("{}", "Auto-applying (previously selected 'all').".dimmed());
            return Ok(true);
        }

        loop {
            print!("{}", "Apply this update? [Y/n/a/q]: ".bold());
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let decision = input.trim().to_lowercase();

            match decision.as_str() {
                "" | "y" | "yes" => {
                    return Ok(true);
                }
                "n" | "no" => {
                    println!("{}", "Skipping this update.".dimmed());
                    return Ok(false);
                }
                "a" | "all" => {
                    println!(
                        "{}",
                        "Applying this and all remaining updates.".green().bold()
                    );
                    self.apply_all = true;
                    return Ok(true);
                }
                "q" | "quit" => {
                    println!("{}", "Stopping update process at user request.".yellow());
                    return Err(GvcError::UserCancelled);
                }
                _ => {
                    println!(
                        "{}",
                        "Please answer with y(es), n(o), a(ll), or q(quit).".red()
                    );
                }
            }
        }
    }
}

#[derive(Clone)]
struct TargetCandidate {
    name: String,
    current_version: String,
    kind: TargetKind,
}

impl TargetCandidate {
    fn display_name(&self) -> String {
        match &self.kind {
            TargetKind::VersionAlias { group, artifact } => {
                format!("version alias '{}' ({}:{})", self.name, group, artifact)
            }
            TargetKind::Library { group, artifact } => {
                format!("library '{}' ({}:{})", self.name, group, artifact)
            }
            TargetKind::Plugin { plugin_id } => {
                format!("plugin '{}' ({})", self.name, plugin_id)
            }
        }
    }

    fn describe_with_version(&self) -> String {
        format!(
            "{} — current version {}",
            self.display_name(),
            self.current_version
        )
    }
}

#[derive(Clone)]
enum TargetKind {
    VersionAlias { group: String, artifact: String },
    Library { group: String, artifact: String },
    Plugin { plugin_id: String },
}

#[derive(Clone)]
struct VersionEntry {
    value: String,
    is_stable: bool,
    is_current: bool,
}

struct PatternMatcher {
    regex: Regex,
}

impl PatternMatcher {
    fn new(pattern: &str) -> Result<Self> {
        let trimmed = pattern.trim();
        if trimmed.is_empty() {
            return Err(GvcError::ProjectValidation(
                "Filter pattern cannot be empty".to_string(),
            ));
        }

        let adjusted = if trimmed.contains(['*', '?']) {
            trimmed.to_string()
        } else {
            format!("*{}*", trimmed)
        };

        let regex = Self::compile_glob(&adjusted)?;
        Ok(Self { regex })
    }

    fn matches(&self, value: &str) -> bool {
        self.regex.is_match(value)
    }

    fn compile_glob(pattern: &str) -> Result<Regex> {
        let mut regex = String::from("(?i)^");
        for ch in pattern.chars() {
            match ch {
                '*' => regex.push_str(".*"),
                '?' => regex.push('.'),
                '.' | '+' | '(' | ')' | '|' | '^' | '$' | '{' | '}' | '[' | ']' | '\\' => {
                    regex.push('\\');
                    regex.push(ch);
                }
                _ => regex.push(ch),
            }
        }
        regex.push('$');

        Regex::new(&regex).map_err(|e| {
            GvcError::ProjectValidation(format!("Invalid filter pattern '{}': {}", pattern, e))
        })
    }
}

#[derive(Debug, Clone)]
pub struct UpdateReport {
    pub version_updates: HashMap<String, (String, String)>,
    pub library_updates: HashMap<String, (String, String)>,
    pub plugin_updates: HashMap<String, (String, String)>,
}

impl UpdateReport {
    fn new() -> Self {
        Self {
            version_updates: HashMap::new(),
            library_updates: HashMap::new(),
            plugin_updates: HashMap::new(),
        }
    }

    pub fn add_version_update(&mut self, name: String, old_version: String, new_version: String) {
        self.version_updates
            .insert(name, (old_version, new_version));
    }

    fn add_library_update(&mut self, name: String, old: String, new: String) {
        self.library_updates.insert(name, (old, new));
    }

    fn add_plugin_update(&mut self, name: String, old: String, new: String) {
        self.plugin_updates.insert(name, (old, new));
    }

    pub fn is_empty(&self) -> bool {
        self.version_updates.is_empty()
            && self.library_updates.is_empty()
            && self.plugin_updates.is_empty()
    }

    pub fn total_updates(&self) -> usize {
        self.version_updates.len() + self.library_updates.len() + self.plugin_updates.len()
    }
}
