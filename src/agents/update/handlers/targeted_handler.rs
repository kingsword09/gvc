use crate::agents::update::context::UpdateReport;
use crate::agents::update::interaction::UpdateInteraction;
use crate::error::Result;
use crate::maven::version::Version;
use crate::repository::{Coordinate, RepositoryClient, VersionStrategy};
use crate::utils::toml::TomlUtils;
use colored::Colorize;
use regex::Regex;
use std::cmp::min;
use std::io::Write;
use std::sync::Arc;
use toml_edit::{DocumentMut, Item};

/// Handles targeted dependency updates
///
/// This handler is responsible for:
/// 1. Finding dependencies matching a pattern
/// 2. Allowing the user to select which one to update
/// 3. Showing available versions
/// 4. Updating the selected dependency
pub struct TargetedHandler<'a> {
    library_client: &'a (dyn RepositoryClient + Send + Sync),
    plugin_client: &'a (dyn RepositoryClient + Send + Sync),
    version_strategy: Arc<dyn VersionStrategy>,
    interaction: &'a mut UpdateInteraction,
}

impl<'a> TargetedHandler<'a> {
    /// Create a new TargetedHandler
    pub fn new(
        library_client: &'a (dyn RepositoryClient + Send + Sync),
        plugin_client: &'a (dyn RepositoryClient + Send + Sync),
        version_strategy: Arc<dyn VersionStrategy>,
        interaction: &'a mut UpdateInteraction,
    ) -> Self {
        Self {
            library_client,
            plugin_client,
            version_strategy,
            interaction,
        }
    }

    /// Update a specific dependency matching the pattern
    pub fn update(
        &mut self,
        doc: &mut DocumentMut,
        stable_only: bool,
        pattern: &str,
    ) -> Result<UpdateReport> {
        let matcher = PatternMatcher::new(pattern)?;
        let mut candidates = self.collect_candidates(doc, &matcher)?;

        if candidates.is_empty() {
            println!(
                "{}",
                format!("No dependencies matched pattern '{}'.", pattern).yellow()
            );
            return Ok(UpdateReport::new());
        }

        let selected_index = self.prompt_candidate_selection(&candidates)?;
        let candidate = candidates.remove(selected_index);

        let version_entries = self.fetch_versions_for_candidate(&candidate, stable_only)?;
        if version_entries.is_empty() {
            println!(
                "{}",
                format!("No versions found for {}.", candidate.display_name()).yellow()
            );
            return Ok(UpdateReport::new());
        }

        let context = VersionSelectionContext {
            entries: &version_entries,
            strategy: Arc::clone(&self.version_strategy),
            interaction: self.interaction,
        };
        let chosen_version = Self::select_version(&candidate, context)?;

        if chosen_version == candidate.current_version {
            println!(
                "{}",
                "Selected version matches the current version; nothing to update.".yellow()
            );
            return Ok(UpdateReport::new());
        }

        let mut report = UpdateReport::new();
        self.apply_update(doc, &candidate, &chosen_version, &mut report)?;

        Ok(report)
    }

    fn collect_candidates(
        &self,
        doc: &DocumentMut,
        matcher: &PatternMatcher,
    ) -> Result<Vec<TargetCandidate>> {
        let mut candidates = Vec::new();

        // Search in libraries
        if let Some(libraries) = doc.get("libraries").and_then(|v| v.as_table()) {
            for (name, item) in libraries.iter() {
                if matcher.matches(name) {
                    if let Some(candidate) = self.build_library_candidate(name, item) {
                        candidates.push(candidate);
                    }
                }
            }
        }

        // Search in versions
        if let Some(versions) = doc.get("versions").and_then(|v| v.as_table()) {
            for (name, item) in versions.iter() {
                if !matcher.matches(name) {
                    continue;
                }

                if let Some(current_version) = item.as_str() {
                    if let Some((group, artifact)) = self.find_representative_coordinate(doc, name)
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

        // Search in plugins
        if let Some(plugins) = doc.get("plugins").and_then(|v| v.as_table()) {
            for (name, item) in plugins.iter() {
                if matcher.matches(name) {
                    if let Some(candidate) = self.build_plugin_candidate(name, item) {
                        candidates.push(candidate);
                    }
                }
            }
        }

        candidates.sort_by_key(|candidate| candidate.display_name());
        Ok(candidates)
    }

    fn build_library_candidate(&self, name: &str, item: &Item) -> Option<TargetCandidate> {
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

    fn build_plugin_candidate(&self, name: &str, item: &Item) -> Option<TargetCandidate> {
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
        &self,
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

    fn prompt_candidate_selection(&self, candidates: &[TargetCandidate]) -> Result<usize> {
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
            std::io::stdout().flush()?;

            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let trimmed = input.trim();

            if trimmed.eq_ignore_ascii_case("q") {
                return Err(crate::error::GvcError::UserCancelled);
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
        stable_only: bool,
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

            // Filter by stable_only if requested
            if stable_only && !is_stable {
                continue;
            }

            let is_current = candidate.current_version == raw;
            entries.push(VersionEntry {
                value: raw,
                is_stable,
                is_current,
            });
        }

        Ok(entries)
    }

    fn select_version(
        candidate: &TargetCandidate,
        context: VersionSelectionContext,
    ) -> Result<String> {
        println!(
            "\n{}",
            format!("Available versions for {}:", candidate.display_name()).cyan()
        );

        let mut limit = min(context.entries.len(), 10);
        loop {
            for (idx, entry) in context.entries.iter().take(limit).enumerate() {
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

            if limit < context.entries.len() {
                println!("  m ) Show more versions");
            }
            println!("  s ) Skip update");
            println!("  q ) Cancel");

            print!("Select version [1-{} | m/s/q]: ", limit);
            std::io::stdout().flush()?;

            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let trimmed = input.trim().to_lowercase();

            match trimmed.as_str() {
                "q" => return Err(crate::error::GvcError::UserCancelled),
                "s" => return Ok(candidate.current_version.clone()),
                "m" => {
                    if limit >= context.entries.len() {
                        println!("{}", "All versions are already displayed.".yellow());
                    } else {
                        limit = min(limit + 10, context.entries.len());
                    }
                }
                _ => {
                    if let Ok(choice) = trimmed.parse::<usize>() {
                        if (1..=limit).contains(&choice) {
                            let entry = &context.entries[choice - 1];
                            if entry.is_current {
                                println!(
                                    "{}",
                                    "Selected version matches current version; choose another or skip."
                                        .yellow()
                                );
                                continue;
                            }

                            // Check if this is a valid upgrade using version strategy
                            if !context
                                .strategy
                                .is_upgrade(&candidate.current_version, &entry.value)
                            {
                                println!(
                                    "{}",
                                    format!(
                                        "Version {} is not a valid upgrade from {} according to version strategy.",
                                        entry.value, candidate.current_version
                                    )
                                    .yellow()
                                );
                                continue;
                            }

                            // Confirm with user if in interactive mode
                            let confirm = match &candidate.kind {
                                TargetKind::Library { .. } | TargetKind::VersionAlias { .. } => {
                                    context.interaction.confirm_library(
                                        &candidate.display_name(),
                                        &candidate.current_version,
                                        &entry.value,
                                    )?
                                }
                                TargetKind::Plugin { .. } => context.interaction.confirm_plugin(
                                    &candidate.display_name(),
                                    &candidate.current_version,
                                    &entry.value,
                                )?,
                            };

                            if !confirm {
                                continue;
                            }

                            return Ok(entry.value.clone());
                        }
                    }
                    println!("{}", "Invalid selection. Please try again.".red());
                }
            }
        }
    }

    fn apply_update(
        &self,
        doc: &mut DocumentMut,
        candidate: &TargetCandidate,
        new_version: &str,
        report: &mut UpdateReport,
    ) -> Result<()> {
        match &candidate.kind {
            TargetKind::VersionAlias { .. } => {
                self.apply_version_alias(doc, &candidate.name, new_version)?;
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
                        crate::error::GvcError::TomlParsing(
                            "Missing [libraries] section".to_string(),
                        )
                    })?;

                let item = libraries.get_mut(&candidate.name).ok_or_else(|| {
                    crate::error::GvcError::TomlParsing(format!(
                        "Library '{}' not found in catalog",
                        candidate.name
                    ))
                })?;

                self.apply_library_version(item, group, artifact, new_version)?;
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
                        crate::error::GvcError::TomlParsing("Missing [plugins] section".to_string())
                    })?;

                let item = plugins.get_mut(&candidate.name).ok_or_else(|| {
                    crate::error::GvcError::TomlParsing(format!(
                        "Plugin '{}' not found in catalog",
                        candidate.name
                    ))
                })?;

                self.apply_plugin_version(item, new_version)?;
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
        &self,
        item: &mut Item,
        group: &str,
        artifact: &str,
        new_version: &str,
    ) -> Result<()> {
        if item.as_str().is_some() {
            let new_coord = format!("{}:{}:{}", group, artifact, new_version);
            *item = Item::Value(toml_edit::Value::from(new_coord.as_str()));
            return Ok(());
        }

        if let Some(inline_table) = item.as_inline_table_mut() {
            inline_table.insert("version", toml_edit::Value::from(new_version));
            return Ok(());
        }

        if let Some(table) = item.as_table_mut() {
            table.insert("version", Item::Value(toml_edit::Value::from(new_version)));
            return Ok(());
        }

        Err(crate::error::GvcError::TomlParsing(
            "Unsupported library format for targeted update".to_string(),
        ))
    }

    fn apply_version_alias(
        &self,
        doc: &mut DocumentMut,
        name: &str,
        new_version: &str,
    ) -> Result<()> {
        let versions = doc
            .get_mut("versions")
            .and_then(|v| v.as_table_mut())
            .ok_or_else(|| {
                crate::error::GvcError::TomlParsing("Missing [versions] section".to_string())
            })?;

        if versions.get(name).is_none() {
            return Err(crate::error::GvcError::TomlParsing(format!(
                "Version alias '{}' not found",
                name
            )));
        }

        versions.insert(name, toml_edit::value(new_version));
        Ok(())
    }

    fn apply_plugin_version(&self, item: &mut Item, new_version: &str) -> Result<()> {
        if let Some(table) = item.as_table_mut() {
            table.insert("version", Item::Value(toml_edit::Value::from(new_version)));
            return Ok(());
        }

        if let Some(inline_table) = item.as_inline_table_mut() {
            inline_table.insert("version", toml_edit::Value::from(new_version));
            return Ok(());
        }

        Err(crate::error::GvcError::TomlParsing(
            "Unsupported plugin definition format for targeted update".to_string(),
        ))
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

struct VersionSelectionContext<'a> {
    entries: &'a [VersionEntry],
    strategy: Arc<dyn VersionStrategy>,
    interaction: &'a mut UpdateInteraction,
}

struct PatternMatcher {
    regex: Regex,
}

impl PatternMatcher {
    fn new(pattern: &str) -> Result<Self> {
        let trimmed = pattern.trim();
        if trimmed.is_empty() {
            return Err(crate::error::GvcError::ProjectValidation(
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
            crate::error::GvcError::ProjectValidation(format!(
                "Invalid filter pattern '{}': {}",
                pattern, e
            ))
        })
    }
}
