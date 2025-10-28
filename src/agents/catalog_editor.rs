use crate::error::{GvcError, Result};
use crate::maven::parse_maven_coordinate;
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, InlineTable, Item, Table, Value, value};

#[derive(Debug)]
pub struct AddResult {
    pub alias: String,
    pub version_alias: String,
    pub target: AddTargetKind,
}

#[derive(Debug, Clone, Copy)]
pub enum AddTargetKind {
    Library,
    Plugin,
}

pub struct CatalogEditor {
    catalog_path: PathBuf,
}

impl CatalogEditor {
    pub fn new<P: AsRef<Path>>(catalog_path: P) -> Self {
        Self {
            catalog_path: catalog_path.as_ref().to_path_buf(),
        }
    }

    pub fn add_library(
        &self,
        coordinate: &str,
        alias_override: Option<&str>,
        version_alias_override: Option<&str>,
    ) -> Result<AddResult> {
        let (group, artifact, version) = parse_library_coordinate(coordinate)?;
        let mut doc = self.load_document()?;

        let alias = alias_override
            .map(sanitize_alias)
            .unwrap_or_else(|| generate_library_alias(&group, &artifact));
        let version_alias = version_alias_override
            .map(sanitize_alias)
            .unwrap_or_else(|| generate_version_alias(&group));

        ensure_section(&mut doc, "versions");
        ensure_section(&mut doc, "libraries");

        {
            let libraries = doc["libraries"].as_table().ok_or_else(|| {
                GvcError::TomlParsing("Failed to access [libraries] table".into())
            })?;

            if libraries.contains_key(&alias) {
                return Err(GvcError::ProjectValidation(format!(
                    "Library alias '{}' already exists in [libraries]",
                    alias
                )));
            }

            if library_exists(libraries, &group, &artifact) {
                return Err(GvcError::ProjectValidation(format!(
                    "Library '{}:{}' already exists in [libraries]",
                    group, artifact
                )));
            }
        }

        let updated_alias = {
            let versions = doc["versions"]
                .as_table_mut()
                .ok_or_else(|| GvcError::TomlParsing("Failed to access [versions] table".into()))?;

            upsert_version_alias(versions, &version_alias, version.clone())?
        };

        let libraries = doc["libraries"]
            .as_table_mut()
            .ok_or_else(|| GvcError::TomlParsing("Failed to access [libraries] table".into()))?;

        let mut entry = InlineTable::new();
        entry.insert("module", Value::from(format!("{}:{}", group, artifact)));
        entry.insert(
            "version",
            Value::InlineTable(version_ref_inline(&version_alias)),
        );
        entry.fmt();

        libraries.insert(&alias, Item::Value(Value::InlineTable(entry)));

        if updated_alias {
            println!(
                "   Updated version alias '{}' with value '{}'",
                version_alias, version
            );
        }

        self.write_document(&doc)?;

        Ok(AddResult {
            alias,
            version_alias,
            target: AddTargetKind::Library,
        })
    }

    pub fn add_plugin(
        &self,
        coordinate: &str,
        alias_override: Option<&str>,
        version_alias_override: Option<&str>,
    ) -> Result<AddResult> {
        let (plugin_id, version) = parse_plugin_coordinate(coordinate)?;
        let mut doc = self.load_document()?;

        let alias = alias_override
            .map(sanitize_alias)
            .unwrap_or_else(|| generate_plugin_alias(&plugin_id));
        let version_alias = version_alias_override
            .map(sanitize_alias)
            .unwrap_or_else(|| generate_plugin_version_alias(&plugin_id));

        ensure_section(&mut doc, "versions");
        ensure_section(&mut doc, "plugins");

        {
            let plugins = doc["plugins"]
                .as_table()
                .ok_or_else(|| GvcError::TomlParsing("Failed to access [plugins] table".into()))?;

            if plugins.contains_key(&alias) {
                return Err(GvcError::ProjectValidation(format!(
                    "Plugin alias '{}' already exists in [plugins]",
                    alias
                )));
            }

            if plugin_exists(plugins, &plugin_id) {
                return Err(GvcError::ProjectValidation(format!(
                    "Plugin '{}' already exists in [plugins]",
                    plugin_id
                )));
            }
        }

        let updated_alias = {
            let versions = doc["versions"]
                .as_table_mut()
                .ok_or_else(|| GvcError::TomlParsing("Failed to access [versions] table".into()))?;

            upsert_version_alias(versions, &version_alias, version.clone())?
        };

        let plugins = doc["plugins"]
            .as_table_mut()
            .ok_or_else(|| GvcError::TomlParsing("Failed to access [plugins] table".into()))?;

        let mut entry = InlineTable::new();
        entry.insert("id", Value::from(plugin_id));
        entry.insert(
            "version",
            Value::InlineTable(version_ref_inline(&version_alias)),
        );
        entry.fmt();

        plugins.insert(&alias, Item::Value(Value::InlineTable(entry)));

        if updated_alias {
            println!(
                "   Updated version alias '{}' with value '{}'",
                version_alias, version
            );
        }

        self.write_document(&doc)?;

        Ok(AddResult {
            alias,
            version_alias,
            target: AddTargetKind::Plugin,
        })
    }

    fn load_document(&self) -> Result<DocumentMut> {
        let content = fs::read_to_string(&self.catalog_path).map_err(|e| {
            GvcError::TomlParsing(format!(
                "Failed to read catalog '{}': {}",
                self.catalog_path.display(),
                e
            ))
        })?;

        content.parse::<DocumentMut>().map_err(|e| {
            GvcError::TomlParsing(format!(
                "Failed to parse catalog '{}': {}",
                self.catalog_path.display(),
                e
            ))
        })
    }

    fn write_document(&self, doc: &DocumentMut) -> Result<()> {
        fs::write(&self.catalog_path, doc.to_string()).map_err(|e| {
            GvcError::TomlParsing(format!(
                "Failed to write catalog '{}': {}",
                self.catalog_path.display(),
                e
            ))
        })
    }
}

fn ensure_section(doc: &mut DocumentMut, name: &str) {
    if !doc.contains_key(name) {
        let mut table = Table::new();
        table.set_implicit(false);
        doc[name] = Item::Table(table);
    }
}

fn upsert_version_alias(table: &mut Table, key: &str, version: String) -> Result<bool> {
    if let Some(existing) = table.get_mut(key) {
        if let Some(existing_version) = existing.as_str() {
            if existing_version == version {
                return Ok(false);
            }

            *existing = Item::Value(Value::from(version));
            return Ok(true);
        }

        return Err(GvcError::ProjectValidation(format!(
            "Version alias '{}' already exists but is not a string",
            key
        )));
    }

    table.insert(key, value(version));
    Ok(true)
}

fn library_exists(table: &Table, group: &str, artifact: &str) -> bool {
    table
        .iter()
        .any(|(_, item)| library_item_matches(item, group, artifact))
}

fn library_item_matches(item: &Item, group: &str, artifact: &str) -> bool {
    if let Some(s) = item.as_str() {
        if let Some((g, a, _)) = parse_maven_coordinate(s) {
            if g == group && a == artifact {
                return true;
            }
        }
    }

    if let Some(inline) = item.as_inline_table() {
        if let Some(module) = inline.get("module").and_then(|v| v.as_str()) {
            if let Some((g, a, _)) = parse_maven_coordinate(module) {
                if g == group && a == artifact {
                    return true;
                }
            }
        }

        let grouped = inline.get("group").and_then(|v| v.as_str());
        let named = inline.get("name").and_then(|v| v.as_str());
        if let (Some(g), Some(a)) = (grouped, named) {
            if g == group && a == artifact {
                return true;
            }
        }
    }

    if let Some(table) = item.as_table() {
        if let Some(module) = table.get("module").and_then(|v| v.as_str()) {
            if let Some((g, a, _)) = parse_maven_coordinate(module) {
                if g == group && a == artifact {
                    return true;
                }
            }
        }

        let grouped = table.get("group").and_then(|v| v.as_str());
        let named = table.get("name").and_then(|v| v.as_str());
        if let (Some(g), Some(a)) = (grouped, named) {
            if g == group && a == artifact {
                return true;
            }
        }
    }

    false
}

fn plugin_exists(table: &Table, plugin_id: &str) -> bool {
    table
        .iter()
        .any(|(_, item)| plugin_item_matches(item, plugin_id))
}

fn plugin_item_matches(item: &Item, plugin_id: &str) -> bool {
    if let Some(inline) = item.as_inline_table() {
        if let Some(id) = inline.get("id").and_then(|v| v.as_str()) {
            return id == plugin_id;
        }
    }

    if let Some(table) = item.as_table() {
        if let Some(id) = table.get("id").and_then(|v| v.as_str()) {
            return id == plugin_id;
        }
    }

    false
}

pub(crate) fn parse_library_coordinate(input: &str) -> Result<(String, String, String)> {
    let parts: Vec<&str> = input.split(':').collect();
    if parts.len() != 3 {
        return Err(GvcError::ProjectValidation(format!(
            "Invalid library coordinate '{}'. Expected format group:artifact:version",
            input
        )));
    }

    let group = parts[0].trim();
    let artifact = parts[1].trim();
    let version = parts[2].trim();

    if group.is_empty() || artifact.is_empty() || version.is_empty() {
        return Err(GvcError::ProjectValidation(format!(
            "Invalid library coordinate '{}'. None of group, artifact, version may be empty",
            input
        )));
    }

    Ok((group.to_string(), artifact.to_string(), version.to_string()))
}

pub(crate) fn parse_plugin_coordinate(input: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = input.split(':').collect();
    if parts.len() != 2 {
        return Err(GvcError::ProjectValidation(format!(
            "Invalid plugin coordinate '{}'. Expected format plugin.id:version",
            input
        )));
    }

    let plugin_id = parts[0].trim();
    let version = parts[1].trim();

    if plugin_id.is_empty() || version.is_empty() {
        return Err(GvcError::ProjectValidation(format!(
            "Invalid plugin coordinate '{}'. Plugin id and version may not be empty",
            input
        )));
    }

    Ok((plugin_id.to_string(), version.to_string()))
}

fn sanitize_alias(raw: &str) -> String {
    normalize_tokens(&[raw])
}

fn generate_library_alias(group: &str, artifact: &str) -> String {
    let mut tokens = group_tokens(group);
    tokens.extend(artifact.split(['-', '.']).map(str::to_string));
    normalize_tokens(&tokens)
}

fn generate_plugin_alias(plugin_id: &str) -> String {
    let tokens = plugin_tokens(plugin_id);
    normalize_tokens(&tokens)
}

fn generate_version_alias(group: &str) -> String {
    let tokens = group_tokens(group);
    if tokens.is_empty() {
        return "version".to_string();
    }

    if tokens.len() >= 3 {
        let base = normalize_tokens(&tokens[..2]);
        format!("{}-version", base)
    } else {
        normalize_tokens(&tokens)
    }
}

fn generate_plugin_version_alias(plugin_id: &str) -> String {
    let tokens = plugin_tokens(plugin_id);
    if tokens.is_empty() {
        return "plugin-version".to_string();
    }

    if tokens.len() >= 3 {
        let base = normalize_tokens(&tokens[..2]);
        format!("{}-version", base)
    } else {
        normalize_tokens(&tokens)
    }
}

fn group_tokens(group: &str) -> Vec<String> {
    group
        .split('.')
        .filter(|part| !part.is_empty())
        .filter(|part| !matches!(*part, "org" | "com" | "net" | "io" | "dev"))
        .map(|part| part.replace(|c: char| !c.is_ascii_alphanumeric(), ""))
        .filter(|part| !part.is_empty())
        .collect()
}

fn plugin_tokens(plugin_id: &str) -> Vec<String> {
    plugin_id
        .split('.')
        .filter(|part| !part.is_empty())
        .filter(|part| !matches!(*part, "org" | "com" | "net" | "io" | "dev"))
        .map(|part| part.replace(|c: char| !c.is_ascii_alphanumeric(), ""))
        .filter(|part| !part.is_empty())
        .collect()
}

fn normalize_tokens<T>(tokens: T) -> String
where
    T: IntoIterator,
    T::Item: AsRef<str>,
{
    let mut normalized = Vec::new();
    for token in tokens {
        let lowered = token.as_ref().to_lowercase();
        if lowered.is_empty() {
            continue;
        }
        if normalized
            .last()
            .map(|last: &String| last == &lowered)
            .unwrap_or(false)
        {
            continue;
        }
        normalized.push(lowered);
    }

    normalized.join("-")
}

fn version_ref_inline(version_alias: &str) -> InlineTable {
    let mut inline = InlineTable::new();
    inline.insert("ref", Value::from(version_alias));
    inline.fmt();
    inline
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_alias_generation_removes_common_prefix() {
        let alias =
            generate_library_alias("org.jetbrains.compose.components", "components-resources");
        assert_eq!(alias, "jetbrains-compose-components-resources");
    }

    #[test]
    fn library_version_alias_prefers_first_tokens() {
        let version_alias = generate_version_alias("org.jetbrains.compose.components");
        assert_eq!(version_alias, "jetbrains-compose-version");
    }

    #[test]
    fn plugin_alias_generation_skips_common_prefix() {
        let alias = generate_plugin_alias("com.android.application");
        assert_eq!(alias, "android-application");
    }

    #[test]
    fn plugin_version_alias_truncates_tokens() {
        let version_alias = generate_plugin_version_alias("com.android.application");
        assert_eq!(version_alias, "android-application");
    }

    #[test]
    fn parse_library_coordinate_valid() {
        let (group, artifact, version) =
            parse_library_coordinate("androidx.lifecycle:lifecycle-runtime-ktx:2.6.2").unwrap();
        assert_eq!(group, "androidx.lifecycle");
        assert_eq!(artifact, "lifecycle-runtime-ktx");
        assert_eq!(version, "2.6.2");
    }

    #[test]
    fn parse_plugin_coordinate_valid() {
        let (id, version) = parse_plugin_coordinate("org.jetbrains.kotlin.jvm:1.9.0").unwrap();
        assert_eq!(id, "org.jetbrains.kotlin.jvm");
        assert_eq!(version, "1.9.0");
    }
}
