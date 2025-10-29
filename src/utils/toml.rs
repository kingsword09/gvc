use crate::maven::parse_maven_coordinate;
use toml_edit::{Item, Value};

/// Canonical representation of a library entry inside the version catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryDetails {
    pub group: String,
    pub artifact: String,
    pub version: Option<String>,
    pub version_ref: Option<String>,
}

/// Helpers for inspecting and updating Gradle version catalog TOML structures.
pub struct TomlUtils;

impl TomlUtils {
    /// Extracts `(group, artifact)` from strings, inline tables, or standard tables.
    pub fn extract_group_artifact(item: &Item) -> Option<(String, String)> {
        if let Some(str_value) = item.as_str() {
            return parse_maven_coordinate(str_value).map(|(g, a, _)| (g, a));
        }

        if let Some(inline_table) = item.as_inline_table() {
            if let Some(module) = inline_table.get("module").and_then(|v| v.as_str()) {
                return parse_maven_coordinate(module).map(|(g, a, _)| (g, a));
            }

            if let (Some(group), Some(name)) = (
                inline_table.get("group").and_then(|v| v.as_str()),
                inline_table.get("name").and_then(|v| v.as_str()),
            ) {
                return Some((group.to_string(), name.to_string()));
            }
        }

        if let Some(table) = item.as_table() {
            if let Some(module) = table.get("module").and_then(|v| v.as_str()) {
                return parse_maven_coordinate(module).map(|(g, a, _)| (g, a));
            }

            if let (Some(group), Some(name)) = (
                table.get("group").and_then(|v| v.as_str()),
                table.get("name").and_then(|v| v.as_str()),
            ) {
                return Some((group.to_string(), name.to_string()));
            }
        }

        None
    }

    /// Extract a concrete version string defined on the item.
    pub fn extract_version(item: &Item) -> Option<String> {
        if let Some(inline_table) = item.as_inline_table() {
            return inline_table
                .get("version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }

        if let Some(table) = item.as_table() {
            return table
                .get("version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }

        None
    }

    /// Extract a version reference key `{ version = { ref = "foo" } }`.
    pub fn extract_version_ref(item: &Item) -> Option<String> {
        if let Some(inline_table) = item.as_inline_table() {
            return inline_table
                .get("version")
                .and_then(|item| item.as_inline_table())
                .and_then(|table| table.get("ref"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }

        if let Some(table) = item.as_table() {
            if let Some(version_item) = table.get("version") {
                if let Some(as_table) = version_item.as_table() {
                    return as_table
                        .get("ref")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
                if let Some(as_inline) = version_item.as_inline_table() {
                    return as_inline
                        .get("ref")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
            }
        }

        None
    }

    /// Updates an item's version to a concrete value, returning true if a change was applied.
    pub fn update_version(item: &mut Item, new_version: &str) -> bool {
        if item.as_str().is_some() {
            *item = Item::Value(Value::from(new_version));
            return true;
        }

        if let Some(inline_table) = item.as_inline_table_mut() {
            inline_table.insert("version", Value::from(new_version));
            return true;
        }

        if let Some(table) = item.as_table_mut() {
            table.insert("version", Item::Value(Value::from(new_version)));
            return true;
        }

        false
    }

    /// Returns true when the item uses the supplied version reference key.
    pub fn uses_version_ref(item: &Item, version_key: &str) -> bool {
        Self::extract_version_ref(item)
            .map(|key| key == version_key)
            .unwrap_or(false)
    }

    /// Extracts a normalized `LibraryDetails` from a library item.
    pub fn extract_library_details(item: &Item) -> Option<LibraryDetails> {
        if let Some(raw) = item.as_str() {
            let (group, artifact, version) = parse_maven_coordinate(raw)?;
            return Some(LibraryDetails {
                group,
                artifact,
                version,
                version_ref: None,
            });
        }

        let (group, artifact) = Self::extract_group_artifact(item)?;
        let version = Self::extract_version(item);
        let version_ref = Self::extract_version_ref(item);

        Some(LibraryDetails {
            group,
            artifact,
            version,
            version_ref,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toml_edit::DocumentMut;

    #[test]
    fn extracts_group_artifact_from_string() {
        let doc: DocumentMut = r#"lib = "com.test:artifact:1.0.0""#.parse().unwrap();
        let item = doc.get("lib").unwrap();
        assert_eq!(
            TomlUtils::extract_group_artifact(item),
            Some(("com.test".to_string(), "artifact".to_string()))
        );
    }

    #[test]
    fn extracts_version_ref() {
        let doc: DocumentMut =
            r#"lib = { group = "com.test", name = "artifact", version = { ref = "core" } }"#
                .parse()
                .unwrap();
        let item = doc.get("lib").unwrap();
        assert_eq!(
            TomlUtils::extract_version_ref(item),
            Some("core".to_string())
        );
    }

    #[test]
    fn updates_inline_table_version() {
        let mut doc: DocumentMut =
            r#"lib = { group = "com.test", name = "artifact", version = "1.0.0" }"#
                .parse()
                .unwrap();
        let item = doc.get_mut("lib").unwrap();
        assert!(TomlUtils::update_version(item, "2.0.0"));
        assert_eq!(
            TomlUtils::extract_version(doc.get("lib").unwrap()).as_deref(),
            Some("2.0.0")
        );
    }

    #[test]
    fn extracts_library_details_from_inline_definition() {
        let doc: DocumentMut =
            r#"lib = { group = "com.test", name = "artifact", version = "1.0.0" }"#
                .parse()
                .unwrap();
        let item = doc.get("lib").unwrap();
        let details = TomlUtils::extract_library_details(item).unwrap();
        assert_eq!(details.group, "com.test");
        assert_eq!(details.artifact, "artifact");
        assert_eq!(details.version.as_deref(), Some("1.0.0"));
        assert!(details.version_ref.is_none());
    }

    #[test]
    fn extracts_library_details_with_version_reference() {
        let doc: DocumentMut =
            r#"lib = { module = "com.test:artifact", version = { ref = "core" } }"#
                .parse()
                .unwrap();
        let item = doc.get("lib").unwrap();
        let details = TomlUtils::extract_library_details(item).unwrap();
        assert_eq!(details.group, "com.test");
        assert_eq!(details.artifact, "artifact");
        assert!(details.version.is_none());
        assert_eq!(details.version_ref.as_deref(), Some("core"));
    }
}
