use crate::error::{GvcError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// TomlParserAgent handles TOML file operations
pub struct TomlParserAgent;

impl TomlParserAgent {
    pub fn new() -> Self {
        Self
    }

    /// Read and parse libs.versions.toml
    pub fn read_version_catalog<P: AsRef<Path>>(&self, path: P) -> Result<VersionCatalog> {
        let content = fs::read_to_string(path)
            .map_err(|e| GvcError::TomlParsing(format!("Failed to read file: {}", e)))?;

        toml::from_str(&content)
            .map_err(|e| GvcError::TomlParsing(format!("Failed to parse TOML: {}", e)))
    }

    /// Compare two version catalogs and generate a summary
    pub fn compare_catalogs(&self, before: &VersionCatalog, after: &VersionCatalog) -> UpdateSummary {
        let mut updated_versions = Vec::new();
        let updated_libraries = Vec::new();
        let updated_plugins = Vec::new();

        // Compare versions
        if let (Some(before_versions), Some(after_versions)) = (&before.versions, &after.versions) {
            for (key, after_value) in after_versions {
                if let Some(before_value) = before_versions.get(key) {
                    if before_value != after_value {
                        updated_versions.push(DependencyUpdate {
                            name: key.clone(),
                            old_version: before_value.clone(),
                            new_version: after_value.clone(),
                        });
                    }
                }
            }
        }

        UpdateSummary {
            updated_versions,
            updated_libraries,
            updated_plugins,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VersionCatalog {
    #[serde(default)]
    pub versions: Option<HashMap<String, String>>,
    #[serde(default)]
    pub libraries: Option<HashMap<String, LibrarySpec>>,
    #[serde(default)]
    pub plugins: Option<HashMap<String, PluginSpec>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum LibrarySpec {
    Simple(String),
    Detailed {
        module: Option<String>,
        version: Option<VersionRef>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum VersionRef {
    Direct(String),
    Reference { 
        #[serde(rename = "ref")]
        version_ref: String 
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PluginSpec {
    Simple(String),
    Detailed {
        id: String,
        version: Option<VersionRef>,
    },
}

#[derive(Debug, Clone)]
pub struct UpdateSummary {
    pub updated_versions: Vec<DependencyUpdate>,
    pub updated_libraries: Vec<DependencyUpdate>,
    pub updated_plugins: Vec<DependencyUpdate>,
}

#[derive(Debug, Clone)]
pub struct DependencyUpdate {
    pub name: String,
    pub old_version: String,
    pub new_version: String,
}

impl UpdateSummary {
    pub fn is_empty(&self) -> bool {
        self.updated_versions.is_empty()
            && self.updated_libraries.is_empty()
            && self.updated_plugins.is_empty()
    }

    pub fn total_updates(&self) -> usize {
        self.updated_versions.len() + self.updated_libraries.len() + self.updated_plugins.len()
    }
}
