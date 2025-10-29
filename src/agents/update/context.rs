use crate::error::Result;
use std::collections::HashMap;
use std::path::Path;

/// Represents different types of catalog update operations
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum UpdateType {
    /// Update [versions] section using library context
    VersionsWithContext,
    /// Update [libraries] section
    Libraries,
    /// Update [plugins] section
    Plugins,
    /// Update targeted dependency
    Targeted,
    /// Check for updates (no modifications)
    Check,
}

/// Context for an update operation
#[derive(Debug, Clone)]
pub struct UpdateContext<'a> {
    /// Path to the version catalog file
    pub catalog_path: &'a Path,
    /// Type of update operation
    pub _update_type: UpdateType,
    /// Whether to consider only stable versions
    pub _stable_only: bool,
    /// Whether to run in interactive mode
    pub _interactive: bool,
}

impl<'a> UpdateContext<'a> {
    /// Create a new update context
    pub fn new(
        catalog_path: &'a Path,
        update_type: UpdateType,
        stable_only: bool,
        interactive: bool,
    ) -> Self {
        Self {
            catalog_path,
            _update_type: update_type,
            _stable_only: stable_only,
            _interactive: interactive,
        }
    }

    /// Load the TOML document from the catalog path
    pub fn load_document(&self) -> Result<toml_edit::DocumentMut> {
        let content = std::fs::read_to_string(self.catalog_path).map_err(|e| {
            crate::error::GvcError::TomlParsing(format!("Failed to read catalog: {}", e))
        })?;

        content.parse::<toml_edit::DocumentMut>().map_err(|e| {
            crate::error::GvcError::TomlParsing(format!("Failed to parse TOML: {}", e))
        })
    }

    /// Save the updated document back to the catalog path
    pub fn save_document(&self, doc: &toml_edit::DocumentMut) -> Result<()> {
        std::fs::write(self.catalog_path, doc.to_string()).map_err(|e| {
            crate::error::GvcError::TomlParsing(format!("Failed to write catalog: {}", e))
        })
    }
}

/// Tracks the changes made during an update operation
#[derive(Debug, Clone, Default)]
pub struct UpdateReport {
    /// Version updates from [versions] section
    pub version_updates: HashMap<String, (String, String)>,
    /// Library updates from [libraries] section
    pub library_updates: HashMap<String, (String, String)>,
    /// Plugin updates from [plugins] section
    pub plugin_updates: HashMap<String, (String, String)>,
}

impl UpdateReport {
    /// Create a new empty update report
    pub fn new() -> Self {
        Self {
            version_updates: HashMap::new(),
            library_updates: HashMap::new(),
            plugin_updates: HashMap::new(),
        }
    }

    /// Add a version update to the report
    pub fn add_version_update(&mut self, name: String, old_version: String, new_version: String) {
        self.version_updates
            .insert(name, (old_version, new_version));
    }

    /// Add a library update to the report
    pub fn add_library_update(&mut self, name: String, old: String, new: String) {
        self.library_updates.insert(name, (old, new));
    }

    /// Add a plugin update to the report
    pub fn add_plugin_update(&mut self, name: String, old: String, new: String) {
        self.plugin_updates.insert(name, (old, new));
    }

    /// Check if the report is empty (no updates)
    pub fn is_empty(&self) -> bool {
        self.version_updates.is_empty()
            && self.library_updates.is_empty()
            && self.plugin_updates.is_empty()
    }

    /// Get the total number of updates
    pub fn total_updates(&self) -> usize {
        self.version_updates.len() + self.library_updates.len() + self.plugin_updates.len()
    }
}
