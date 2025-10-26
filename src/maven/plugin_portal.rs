use crate::error::{GvcError, Result};
use crate::maven::version::VersionComparator;
use quick_xml::de::from_str;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::time::Duration;

const GRADLE_PLUGIN_PORTAL: &str = "https://plugins.gradle.org/m2";

/// Gradle Plugin Portal client
pub struct PluginPortalClient {
    client: Client,
}

impl PluginPortalClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("gvc/0.1.0")
            .build()
            .map_err(|e| GvcError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        Ok(Self { client })
    }

    /// Fetch the latest version of a Gradle plugin
    ///
    /// Plugin IDs like "org.jetbrains.kotlin.jvm" are converted to Maven coordinates:
    /// - Group: org.jetbrains.kotlin.jvm
    /// - Artifact: org.jetbrains.kotlin.jvm.gradle.plugin
    pub fn fetch_latest_plugin_version(
        &self,
        plugin_id: &str,
        stable_only: bool,
    ) -> Result<Option<String>> {
        // Gradle plugins are published with a special naming convention
        // Plugin ID: org.jetbrains.kotlin.jvm
        // Maven coordinate: org.jetbrains.kotlin.jvm:org.jetbrains.kotlin.jvm.gradle.plugin

        let group = plugin_id;
        let artifact = format!("{}.gradle.plugin", plugin_id);

        if std::env::var("GVC_VERBOSE").is_ok() {
            eprintln!(
                "[VERBOSE] Fetching plugin: {} ({}:{})",
                plugin_id, group, artifact
            );
        }

        let versions = self.fetch_all_plugin_versions(group, &artifact)?;

        if let Some(versions) = versions {
            if versions.is_empty() {
                return Ok(None);
            }

            if std::env::var("GVC_VERBOSE").is_ok() {
                eprintln!(
                    "[VERBOSE] Found {} versions for plugin {}",
                    versions.len(),
                    plugin_id
                );
            }

            Ok(VersionComparator::get_latest(&versions, stable_only))
        } else {
            Ok(None)
        }
    }

    fn fetch_all_plugin_versions(
        &self,
        group: &str,
        artifact: &str,
    ) -> Result<Option<Vec<String>>> {
        let group_path = group.replace('.', "/");
        let metadata_url = format!(
            "{}/{}/{}/maven-metadata.xml",
            GRADLE_PLUGIN_PORTAL, group_path, artifact
        );

        if std::env::var("GVC_VERBOSE").is_ok() {
            eprintln!("[VERBOSE] Fetching: {}", metadata_url);
        }

        let response = match self.client.get(&metadata_url).send() {
            Ok(resp) => resp,
            Err(e) => {
                if std::env::var("GVC_VERBOSE").is_ok() {
                    eprintln!("[VERBOSE] Request failed: {}", e);
                }
                return Ok(None);
            }
        };

        if !response.status().is_success() {
            if std::env::var("GVC_VERBOSE").is_ok() {
                eprintln!("[VERBOSE] HTTP {}: {}", response.status(), metadata_url);
            }
            return Ok(None);
        }

        let text = response
            .text()
            .map_err(|e| GvcError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        let metadata: MavenMetadata = from_str(&text).map_err(|e| {
            GvcError::TomlParsing(format!("Failed to parse plugin metadata: {}", e))
        })?;

        let versions: Vec<String> = metadata.versioning.versions.version.to_vec();

        Ok(Some(versions))
    }
}

#[derive(Debug, Deserialize)]
struct MavenMetadata {
    #[serde(rename = "groupId")]
    #[allow(dead_code)]
    group_id: String,
    #[serde(rename = "artifactId")]
    #[allow(dead_code)]
    artifact_id: String,
    versioning: Versioning,
}

#[derive(Debug, Deserialize)]
struct Versioning {
    #[allow(dead_code)]
    latest: Option<String>,
    #[allow(dead_code)]
    release: Option<String>,
    versions: Versions,
}

#[derive(Debug, Deserialize)]
struct Versions {
    version: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_id_to_maven_coordinate() {
        let plugin_id = "org.jetbrains.kotlin.jvm";
        let artifact = format!("{}.gradle.plugin", plugin_id);
        assert_eq!(artifact, "org.jetbrains.kotlin.jvm.gradle.plugin");
    }

    #[test]
    #[ignore] // Requires network access
    fn test_fetch_kotlin_plugin_version() {
        let client = PluginPortalClient::new().unwrap();
        let version = client.fetch_latest_plugin_version("org.jetbrains.kotlin.jvm", true);
        assert!(version.is_ok());
        if let Ok(Some(v)) = version {
            println!("Latest Kotlin plugin version: {}", v);
            assert!(!v.is_empty());
        }
    }
}
