use crate::error::{GvcError, Result};
use crate::maven::{
    MavenRepository, PluginPortalClient, VersionComparator, parse_maven_coordinate,
};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use toml_edit::{DocumentMut, Item, Value};

/// DependencyUpdater handles the actual dependency version updates
pub struct DependencyUpdater {
    maven_repo: MavenRepository,
    plugin_portal: PluginPortalClient,
}

impl DependencyUpdater {
    pub fn new() -> Result<Self> {
        Ok(Self {
            maven_repo: MavenRepository::new()?,
            plugin_portal: PluginPortalClient::new()?,
        })
    }

    pub fn with_repositories(repositories: Vec<crate::gradle::Repository>) -> Result<Self> {
        if repositories.is_empty() {
            // 如果没有提供仓库，使用默认的
            Self::new()
        } else {
            Ok(Self {
                maven_repo: MavenRepository::with_repositories(repositories)?,
                plugin_portal: PluginPortalClient::new()?,
            })
        }
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
    ) -> Result<UpdateReport> {
        let catalog_path = catalog_path.as_ref();

        // Read and parse TOML document
        let content = fs::read_to_string(catalog_path)
            .map_err(|e| GvcError::TomlParsing(format!("Failed to read catalog: {}", e)))?;

        let mut doc = content
            .parse::<DocumentMut>()
            .map_err(|e| GvcError::TomlParsing(format!("Failed to parse TOML: {}", e)))?;

        let mut report = UpdateReport::new();

        // Update [versions] section - need to check with libraries context
        println!("\n{}", "Checking version updates...".cyan());
        self.update_versions_with_context(&mut doc, stable_only, &mut report)?;

        // Update [libraries] section
        if let Some(libraries) = doc.get_mut("libraries").and_then(|v| v.as_table_mut()) {
            println!("\n{}", "Checking library updates...".cyan());
            self.update_libraries_section(libraries, stable_only, &mut report)?;
        }

        // Update [plugins] section
        if let Some(plugins) = doc.get_mut("plugins").and_then(|v| v.as_table_mut()) {
            println!("\n{}", "Checking plugin updates...".cyan());
            self.update_plugins_section(plugins, stable_only, &mut report)?;
        }

        // Write back the updated document
        if !report.is_empty() {
            fs::write(catalog_path, doc.to_string())
                .map_err(|e| GvcError::TomlParsing(format!("Failed to write catalog: {}", e)))?;
        }

        Ok(report)
    }

    fn update_versions_with_context(
        &self,
        doc: &mut DocumentMut,
        stable_only: bool,
        report: &mut UpdateReport,
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
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  [{bar:40}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-"),
        );

        for (version_key, current_version) in versions_data {
            pb.set_message(format!("Checking {}", version_key));

            // Find a library that uses this version reference
            let mut representative_lib: Option<(String, String)> = None;

            for (_lib_name, lib_value) in &libraries_data {
                let uses_this_version = if let Some(inline_table) = lib_value.as_inline_table() {
                    if let Some(version_item) = inline_table.get("version") {
                        if let Some(version_ref) = version_item.as_inline_table() {
                            version_ref.get("ref").and_then(|v| v.as_str())
                                == Some(version_key.as_str())
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else if let Some(table) = lib_value.as_table() {
                    if let Some(version_item) = table.get("version") {
                        if let Some(version_ref) = version_item.as_table() {
                            version_ref.get("ref").and_then(|v| v.as_str())
                                == Some(version_key.as_str())
                        } else if let Some(version_ref) = version_item.as_inline_table() {
                            version_ref.get("ref").and_then(|v| v.as_str())
                                == Some(version_key.as_str())
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };

                if uses_this_version {
                    let coordinate = if let Some(inline_table) = lib_value.as_inline_table() {
                        if let Some(module) = inline_table.get("module").and_then(|v| v.as_str()) {
                            parse_maven_coordinate(module)
                                .map(|(g, a, _)| (g.to_string(), a.to_string()))
                        } else if let Some(group) =
                            inline_table.get("group").and_then(|v| v.as_str())
                        {
                            inline_table
                                .get("name")
                                .and_then(|v| v.as_str())
                                .map(|name| (group.to_string(), name.to_string()))
                        } else {
                            None
                        }
                    } else if let Some(table) = lib_value.as_table() {
                        if let Some(module) = table.get("module").and_then(|v| v.as_str()) {
                            parse_maven_coordinate(module)
                                .map(|(g, a, _)| (g.to_string(), a.to_string()))
                        } else if let Some(group) = table.get("group").and_then(|v| v.as_str()) {
                            table
                                .get("name")
                                .and_then(|v| v.as_str())
                                .map(|name| (group.to_string(), name.to_string()))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if let Some((group, artifact)) = coordinate {
                        representative_lib = Some((group, artifact));
                        break;
                    }
                }
            }

            if let Some((group, artifact)) = representative_lib {
                if let Some(latest) =
                    self.maven_repo
                        .fetch_latest_version(&group, &artifact, stable_only)?
                {
                    if latest != current_version
                        && VersionComparator::is_newer(&latest, &current_version)
                    {
                        // Update the version in the document
                        if let Some(versions_mut) =
                            doc.get_mut("versions").and_then(|v| v.as_table_mut())
                        {
                            *versions_mut.get_mut(&version_key).unwrap() =
                                toml_edit::value(latest.as_str());
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
            let mut representative_lib: Option<(String, String)> = None;

            for (_lib_name, lib_value) in libraries.iter() {
                let uses_this_version = if let Some(inline_table) = lib_value.as_inline_table() {
                    // Check if version.ref matches
                    if let Some(version_item) = inline_table.get("version") {
                        if let Some(version_ref) = version_item.as_inline_table() {
                            version_ref.get("ref").and_then(|v| v.as_str()) == Some(&version_key)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else if let Some(table) = lib_value.as_table() {
                    if let Some(version_item) = table.get("version") {
                        if let Some(version_ref) = version_item.as_table() {
                            version_ref.get("ref").and_then(|v| v.as_str()) == Some(&version_key)
                        } else if let Some(version_ref) = version_item.as_inline_table() {
                            version_ref.get("ref").and_then(|v| v.as_str()) == Some(&version_key)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };

                if uses_this_version {
                    // Extract group:artifact from the first match
                    let coordinate = if let Some(inline_table) = lib_value.as_inline_table() {
                        if let Some(module) = inline_table.get("module").and_then(|v| v.as_str()) {
                            parse_maven_coordinate(module)
                                .map(|(g, a, _)| (g.to_string(), a.to_string()))
                        } else if let Some(group) =
                            inline_table.get("group").and_then(|v| v.as_str())
                        {
                            inline_table
                                .get("name")
                                .and_then(|v| v.as_str())
                                .map(|name| (group.to_string(), name.to_string()))
                        } else {
                            None
                        }
                    } else if let Some(table) = lib_value.as_table() {
                        if let Some(module) = table.get("module").and_then(|v| v.as_str()) {
                            parse_maven_coordinate(module)
                                .map(|(g, a, _)| (g.to_string(), a.to_string()))
                        } else if let Some(group) = table.get("group").and_then(|v| v.as_str()) {
                            table
                                .get("name")
                                .and_then(|v| v.as_str())
                                .map(|name| (group.to_string(), name.to_string()))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if let Some((group, artifact)) = coordinate {
                        representative_lib = Some((group, artifact));
                        break; // Only check the first library as representative
                    }
                }
            }

            // If no libraries reference this version, skip
            if representative_lib.is_none() {
                pb.inc(1);
                continue;
            }

            // Query latest version for the representative library only
            let (group, artifact) = representative_lib.unwrap();
            if let Some(latest) =
                self.maven_repo
                    .fetch_latest_version(&group, &artifact, stable_only)?
            {
                if latest != current_version {
                    // Verify this is actually an upgrade, not a downgrade
                    use crate::maven::version::VersionComparator;
                    if VersionComparator::is_newer(&latest, current_version) {
                        report.add_version_update(
                            version_key.clone(),
                            current_version.to_string(),
                            latest,
                        );
                    }
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
        // 只读取不修改
        if let Some(str_value) = lib_value.as_str() {
            if let Some((group, artifact, Some(current))) = parse_maven_coordinate(str_value) {
                if let Some(latest) =
                    self.maven_repo
                        .fetch_latest_version(&group, &artifact, stable_only)?
                {
                    if latest != current && VersionComparator::is_newer(&latest, &current) {
                        return Ok(Some(DependencyUpdate {
                            old_version: current.to_string(),
                            new_version: latest,
                        }));
                    }
                }
            }
        } else if let Some(inline_table) = lib_value.as_inline_table() {
            // Inline table format: { group = "...", name = "...", version = "..." } or { module = "...", version = "..." }
            let (group, artifact) =
                if let Some(module) = inline_table.get("module").and_then(|v| v.as_str()) {
                    if let Some((g, a, _)) = parse_maven_coordinate(module) {
                        (g.to_string(), a.to_string())
                    } else {
                        return Ok(None);
                    }
                } else if let Some(group) = inline_table.get("group").and_then(|v| v.as_str()) {
                    if let Some(name) = inline_table.get("name").and_then(|v| v.as_str()) {
                        (group.to_string(), name.to_string())
                    } else {
                        return Ok(None);
                    }
                } else {
                    return Ok(None);
                };

            if let Some(version_item) = inline_table.get("version") {
                if let Some(current_version_str) = version_item.as_str() {
                    if let Some(latest) =
                        self.maven_repo
                            .fetch_latest_version(&group, &artifact, stable_only)?
                    {
                        if latest != current_version_str
                            && VersionComparator::is_newer(&latest, current_version_str)
                        {
                            return Ok(Some(DependencyUpdate {
                                old_version: current_version_str.to_string(),
                                new_version: latest,
                            }));
                        }
                    }
                }
            }
        } else if let Some(table) = lib_value.as_table() {
            if let Some(module) = table.get("module").and_then(|v| v.as_str()) {
                if let Some((group, artifact, _)) = parse_maven_coordinate(module) {
                    if let Some(version_item) = table.get("version") {
                        if let Some(current_version_str) = version_item.as_str() {
                            if let Some(latest) = self.maven_repo.fetch_latest_version(
                                &group,
                                &artifact,
                                stable_only,
                            )? {
                                if latest != current_version_str
                                    && VersionComparator::is_newer(&latest, current_version_str)
                                {
                                    return Ok(Some(DependencyUpdate {
                                        old_version: current_version_str.to_string(),
                                        new_version: latest,
                                    }));
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    fn update_libraries_section(
        &self,
        libraries: &mut toml_edit::Table,
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

            if let Some(lib_value) = libraries.get_mut(&key) {
                if let Some(updated) = self.check_library_update(lib_value, stable_only)? {
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
        lib_value: &mut Item,
        stable_only: bool,
    ) -> Result<Option<DependencyUpdate>> {
        // Library can be a string like "group:artifact:version"
        // or a table like { module = "group:artifact", version = "1.0" }
        // or an inline table { group = "...", name = "...", version = "1.0" }
        // or { module = "group:artifact", version.ref = "some-version" }

        if let Some(str_value) = lib_value.as_str() {
            // Simple string format: "group:artifact:version"
            if let Some((group, artifact, Some(current))) = parse_maven_coordinate(str_value) {
                if let Some(latest) =
                    self.maven_repo
                        .fetch_latest_version(&group, &artifact, stable_only)?
                {
                    if latest != current {
                        let new_coord = format!("{}:{}:{}", group, artifact, latest);
                        *lib_value = Item::Value(Value::from(new_coord.as_str()));
                        return Ok(Some(DependencyUpdate {
                            old_version: current.to_string(),
                            new_version: latest,
                        }));
                    }
                }
            }
        } else if let Some(inline_table) = lib_value.as_inline_table_mut() {
            // Inline table format: { group = "...", name = "...", version = "..." }
            let (group, artifact) =
                if let Some(module) = inline_table.get("module").and_then(|v| v.as_str()) {
                    if let Some((g, a, _)) = parse_maven_coordinate(module) {
                        (g.to_string(), a.to_string())
                    } else {
                        return Ok(None);
                    }
                } else if let Some(group_str) = inline_table.get("group").and_then(|v| v.as_str()) {
                    if let Some(name_str) = inline_table.get("name").and_then(|v| v.as_str()) {
                        (group_str.to_string(), name_str.to_string())
                    } else {
                        return Ok(None);
                    }
                } else {
                    return Ok(None);
                };

            // Check if version is a direct value (not a reference)
            if let Some(version_item) = inline_table.get("version") {
                if let Some(current_version_str) = version_item.as_str() {
                    let current_version = current_version_str.to_string();
                    if let Some(latest) =
                        self.maven_repo
                            .fetch_latest_version(&group, &artifact, stable_only)?
                    {
                        if latest != current_version {
                            inline_table.insert("version", Value::from(latest.as_str()));
                            return Ok(Some(DependencyUpdate {
                                old_version: current_version,
                                new_version: latest,
                            }));
                        }
                    }
                }
                // If it's a reference (version.ref), we skip it here
            }
        } else if let Some(table) = lib_value.as_table_mut() {
            // Regular table format
            if let Some(module) = table.get("module").and_then(|v| v.as_str()) {
                if let Some((group, artifact, _)) = parse_maven_coordinate(module) {
                    // Check if version is a direct value or a reference
                    if let Some(version_item) = table.get_mut("version") {
                        if let Some(current_version_str) = version_item.as_str() {
                            let current_version = current_version_str.to_string();
                            // Direct version
                            if let Some(latest) = self.maven_repo.fetch_latest_version(
                                &group,
                                &artifact,
                                stable_only,
                            )? {
                                if latest != current_version {
                                    *version_item = Item::Value(Value::from(latest.as_str()));
                                    return Ok(Some(DependencyUpdate {
                                        old_version: current_version,
                                        new_version: latest,
                                    }));
                                }
                            }
                        }
                        // If it's a reference (version.ref), we skip it here
                    }
                }
            }
        }

        Ok(None)
    }

    fn update_plugins_section(
        &self,
        plugins: &mut toml_edit::Table,
        stable_only: bool,
        report: &mut UpdateReport,
    ) -> Result<()> {
        let keys: Vec<String> = plugins.iter().map(|(k, _)| k.to_string()).collect();
        let pb = ProgressBar::new(keys.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  [{bar:40}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-"),
        );

        for key in keys {
            pb.set_message(format!("Checking {}", key));

            if let Some(plugin_value) = plugins.get_mut(&key) {
                if let Some(updated) = self.check_plugin_update(plugin_value, stable_only)? {
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
            if let Some(latest) = self
                .plugin_portal
                .fetch_latest_plugin_version(&plugin_id, stable_only)?
            {
                if latest != current_version
                    && VersionComparator::is_newer(&latest, &current_version)
                {
                    // Update the version
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
