use crate::agents::update::context::UpdateReport;
use crate::agents::update::interaction::UpdateInteraction;
use crate::error::{GvcError, Result};
use crate::maven::version::Version;
use crate::repository::{Coordinate, RepositoryClient, VersionStrategy};
use crate::utils::toml::TomlUtils;
use colored::Colorize;
use regex::Regex;
use std::cmp::min;
use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;
use toml_edit::{DocumentMut, Item};

/// Handles targeted dependency updates.
///
/// The handler orchestrates matching, candidate selection, version selection,
/// and TOML mutation while keeping each concern in a focused helper below.
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
        let mut candidates = TargetCandidateCollector::new(doc, &matcher).collect();

        if candidates.is_empty() {
            println!(
                "{}",
                format!("No dependencies matched pattern '{}'.", pattern).yellow()
            );
            return Ok(UpdateReport::new());
        }

        let selected_index = {
            let mut selector = TargetCandidateSelector::new(self.interaction);
            selector.select(&candidates, pattern)?
        };
        let candidate = candidates.remove(selected_index);

        let chosen_version = {
            let mut selector = VersionSelector::new(
                self.library_client,
                self.plugin_client,
                Arc::clone(&self.version_strategy),
                self.interaction,
            );

            let Some(version) = selector.select(&candidate, stable_only)? else {
                return Ok(UpdateReport::new());
            };
            version
        };

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

    fn apply_update(
        &self,
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
                    .ok_or_else(|| GvcError::TomlParsing("Missing [libraries] section".into()))?;

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
            TargetKind::Plugin { version_ref, .. } => {
                if let Some(version_ref) = version_ref {
                    Self::apply_version_alias(doc, version_ref, new_version)?;
                } else {
                    let plugins = doc
                        .get_mut("plugins")
                        .and_then(|v| v.as_table_mut())
                        .ok_or_else(|| GvcError::TomlParsing("Missing [plugins] section".into()))?;

                    let item = plugins.get_mut(&candidate.name).ok_or_else(|| {
                        GvcError::TomlParsing(format!(
                            "Plugin '{}' not found in catalog",
                            candidate.name
                        ))
                    })?;

                    Self::apply_plugin_version(item, new_version)?;
                }
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

        Err(GvcError::TomlParsing(
            "Unsupported library format for targeted update".into(),
        ))
    }

    fn apply_version_alias(doc: &mut DocumentMut, name: &str, new_version: &str) -> Result<()> {
        let versions = doc
            .get_mut("versions")
            .and_then(|v| v.as_table_mut())
            .ok_or_else(|| GvcError::TomlParsing("Missing [versions] section".into()))?;

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
        if item.as_str().is_some() {
            *item = Item::Value(toml_edit::Value::from(new_version));
            return Ok(());
        }

        if let Some(table) = item.as_table_mut() {
            table.insert("version", Item::Value(toml_edit::Value::from(new_version)));
            return Ok(());
        }

        if let Some(inline_table) = item.as_inline_table_mut() {
            inline_table.insert("version", toml_edit::Value::from(new_version));
            return Ok(());
        }

        Err(GvcError::TomlParsing(
            "Unsupported plugin definition format for targeted update".into(),
        ))
    }
}

struct TargetCandidateCollector<'a> {
    doc: &'a DocumentMut,
    matcher: &'a PatternMatcher,
}

impl<'a> TargetCandidateCollector<'a> {
    fn new(doc: &'a DocumentMut, matcher: &'a PatternMatcher) -> Self {
        Self { doc, matcher }
    }

    fn collect(&self) -> Vec<TargetCandidate> {
        let mut candidates = Vec::new();
        self.collect_libraries(&mut candidates);
        self.collect_version_aliases(&mut candidates);
        self.collect_plugins(&mut candidates);
        candidates.sort_by_key(|candidate| candidate.display_name());
        candidates
    }

    fn collect_libraries(&self, candidates: &mut Vec<TargetCandidate>) {
        let Some(libraries) = self.doc.get("libraries").and_then(|v| v.as_table()) else {
            return;
        };

        for (name, item) in libraries.iter() {
            if self.matcher.matches(name) {
                if let Some(candidate) = Self::build_library_candidate(name, item) {
                    candidates.push(candidate);
                }
            }
        }
    }

    fn collect_version_aliases(&self, candidates: &mut Vec<TargetCandidate>) {
        let Some(versions) = self.doc.get("versions").and_then(|v| v.as_table()) else {
            return;
        };

        for (name, item) in versions.iter() {
            if !self.matcher.matches(name) {
                continue;
            }

            if let Some(current_version) = item.as_str() {
                if let Some((group, artifact)) = self.find_representative_coordinate(name) {
                    candidates.push(TargetCandidate {
                        name: name.to_string(),
                        current_version: current_version.to_string(),
                        kind: TargetKind::VersionAlias { group, artifact },
                    });
                }
            }
        }
    }

    fn collect_plugins(&self, candidates: &mut Vec<TargetCandidate>) {
        let Some(plugins) = self.doc.get("plugins").and_then(|v| v.as_table()) else {
            return;
        };

        let version_refs = self.collect_version_refs();
        for (name, item) in plugins.iter() {
            if self.matcher.matches(name) {
                if let Some(candidate) = Self::build_plugin_candidate(name, item, &version_refs) {
                    candidates.push(candidate);
                }
            }
        }
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

    fn build_plugin_candidate(
        name: &str,
        item: &Item,
        version_refs: &HashMap<String, String>,
    ) -> Option<TargetCandidate> {
        let details = TomlUtils::extract_plugin_details(name, item)?;

        if let Some(current_version) = details.version {
            return Some(TargetCandidate {
                name: name.to_string(),
                current_version,
                kind: TargetKind::Plugin {
                    plugin_id: details.id,
                    version_ref: None,
                },
            });
        }

        let version_ref = details.version_ref?;
        let current_version = version_refs.get(&version_ref)?.clone();
        Some(TargetCandidate {
            name: name.to_string(),
            current_version,
            kind: TargetKind::Plugin {
                plugin_id: details.id,
                version_ref: Some(version_ref),
            },
        })
    }

    fn collect_version_refs(&self) -> HashMap<String, String> {
        self.doc
            .get("versions")
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

    fn find_representative_coordinate(&self, version_key: &str) -> Option<(String, String)> {
        let libraries = self.doc.get("libraries").and_then(|v| v.as_table())?;
        for (_name, lib_value) in libraries.iter() {
            if TomlUtils::uses_version_ref(lib_value, version_key) {
                if let Some((group, artifact)) = TomlUtils::extract_group_artifact(lib_value) {
                    return Some((group, artifact));
                }
            }
        }
        None
    }
}

struct TargetCandidateSelector<'a> {
    interaction: &'a mut UpdateInteraction,
}

impl<'a> TargetCandidateSelector<'a> {
    fn new(interaction: &'a mut UpdateInteraction) -> Self {
        Self { interaction }
    }

    fn select(&mut self, candidates: &[TargetCandidate], pattern: &str) -> Result<usize> {
        if candidates.len() == 1 {
            println!(
                "{}",
                format!("Found one match: {}", candidates[0].describe_with_version()).cyan()
            );
            return Ok(0);
        }

        if !self.interaction.is_enabled() {
            return Err(GvcError::ProjectValidation(format!(
                "Filter pattern '{}' matched {} dependencies. Refine the pattern or use --interactive to choose one.",
                pattern,
                candidates.len()
            )));
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
                return Err(GvcError::UserCancelled);
            }

            if let Ok(choice) = trimmed.parse::<usize>() {
                if (1..=candidates.len()).contains(&choice) {
                    return Ok(choice - 1);
                }
            }

            println!("{}", "Invalid selection. Please try again.".red());
        }
    }
}

struct VersionSelector<'a> {
    library_client: &'a (dyn RepositoryClient + Send + Sync),
    plugin_client: &'a (dyn RepositoryClient + Send + Sync),
    version_strategy: Arc<dyn VersionStrategy>,
    interaction: &'a mut UpdateInteraction,
}

impl<'a> VersionSelector<'a> {
    fn new(
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

    fn select(&mut self, candidate: &TargetCandidate, stable_only: bool) -> Result<Option<String>> {
        let version_entries = self.fetch_versions_for_candidate(candidate, stable_only)?;
        if version_entries.is_empty() {
            println!(
                "{}",
                format!("No versions found for {}.", candidate.display_name()).yellow()
            );
            return Ok(None);
        }

        println!(
            "\n{}",
            format!("Available versions for {}:", candidate.display_name()).cyan()
        );

        if self.interaction.is_enabled() {
            self.prompt_for_version(candidate, &version_entries)
                .map(Some)
        } else {
            Ok(Some(
                self.select_latest_upgrade(candidate, &version_entries),
            ))
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
            TargetKind::Plugin { plugin_id, .. } => {
                let coordinate = Coordinate::plugin(plugin_id.as_str());
                self.plugin_client.fetch_available_versions(&coordinate)?
            }
        };

        let mut entries = Vec::with_capacity(versions.len());
        for raw in versions {
            let parsed = Version::parse(&raw);
            let is_stable = parsed.is_stable();

            if stable_only && !is_stable {
                continue;
            }

            entries.push(VersionEntry {
                is_current: candidate.current_version == raw,
                is_stable,
                value: raw,
            });
        }

        Ok(entries)
    }

    fn select_latest_upgrade(
        &self,
        candidate: &TargetCandidate,
        entries: &[VersionEntry],
    ) -> String {
        entries
            .iter()
            .find(|entry| {
                !entry.is_current
                    && self
                        .version_strategy
                        .is_upgrade(&candidate.current_version, &entry.value)
            })
            .map(|entry| entry.value.clone())
            .unwrap_or_else(|| candidate.current_version.clone())
    }

    fn prompt_for_version(
        &mut self,
        candidate: &TargetCandidate,
        entries: &[VersionEntry],
    ) -> Result<String> {
        let mut limit = min(entries.len(), 10);
        loop {
            Self::print_version_page(entries, limit);

            print!("Select version [1-{} | m/s/q]: ", limit);
            std::io::stdout().flush()?;

            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let trimmed = input.trim().to_lowercase();

            match trimmed.as_str() {
                "q" => return Err(GvcError::UserCancelled),
                "s" => return Ok(candidate.current_version.clone()),
                "m" => limit = Self::expand_limit(limit, entries.len()),
                _ => {
                    if let Some(version) =
                        self.try_select_prompt_choice(candidate, entries, limit, &trimmed)?
                    {
                        return Ok(version);
                    }
                }
            }
        }
    }

    fn print_version_page(entries: &[VersionEntry], limit: usize) {
        for (idx, entry) in entries.iter().take(limit).enumerate() {
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

        if limit < entries.len() {
            println!("  m ) Show more versions");
        }
        println!("  s ) Skip update");
        println!("  q ) Cancel");
    }

    fn expand_limit(limit: usize, total: usize) -> usize {
        if limit >= total {
            println!("{}", "All versions are already displayed.".yellow());
            limit
        } else {
            min(limit + 10, total)
        }
    }

    fn try_select_prompt_choice(
        &mut self,
        candidate: &TargetCandidate,
        entries: &[VersionEntry],
        limit: usize,
        input: &str,
    ) -> Result<Option<String>> {
        let Ok(choice) = input.parse::<usize>() else {
            println!("{}", "Invalid selection. Please try again.".red());
            return Ok(None);
        };

        if !(1..=limit).contains(&choice) {
            println!("{}", "Invalid selection. Please try again.".red());
            return Ok(None);
        }

        let entry = &entries[choice - 1];
        if entry.is_current {
            println!(
                "{}",
                "Selected version matches current version; choose another or skip.".yellow()
            );
            return Ok(None);
        }

        if !self
            .version_strategy
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
            return Ok(None);
        }

        if !self.confirm_update(candidate, &entry.value)? {
            return Ok(None);
        }

        Ok(Some(entry.value.clone()))
    }

    fn confirm_update(&mut self, candidate: &TargetCandidate, new_version: &str) -> Result<bool> {
        match &candidate.kind {
            TargetKind::Library { .. } | TargetKind::VersionAlias { .. } => {
                self.interaction.confirm_library(
                    &candidate.display_name(),
                    &candidate.current_version,
                    new_version,
                )
            }
            TargetKind::Plugin { .. } => self.interaction.confirm_plugin(
                &candidate.display_name(),
                &candidate.current_version,
                new_version,
            ),
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
            TargetKind::Plugin { plugin_id, .. } => {
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
    VersionAlias {
        group: String,
        artifact: String,
    },
    Library {
        group: String,
        artifact: String,
    },
    Plugin {
        plugin_id: String,
        version_ref: Option<String>,
    },
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
                "Filter pattern cannot be empty".into(),
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
