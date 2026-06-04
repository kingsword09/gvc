use crate::error::{GvcError, Result};
use crate::maven::metadata_cache::MetadataCache;
use crate::maven::version::{Version, VersionComparator};
use crate::repository::{Coordinate, RepositoryClient};
use crate::utils::verbose;
use quick_xml::de::from_str;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::time::Duration;

const GRADLE_PLUGIN_PORTAL: &str = "https://plugins.gradle.org/m2";
const MAX_METADATA_BYTES: usize = 10 * 1024 * 1024;

/// Gradle Plugin Portal client
pub struct PluginPortalClient {
    client: Client,
    metadata_cache: MetadataCache,
}

impl PluginPortalClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("gvc")
            .danger_accept_invalid_certs(false)
            .build()
            .map_err(|e| GvcError::Io(std::io::Error::other(e)))?;

        Ok(Self {
            client,
            metadata_cache: MetadataCache::new(),
        })
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

        verbose::log(format!(
            "Fetching plugin: {} ({}:{})",
            plugin_id, group, artifact
        ));

        let versions = self.fetch_all_plugin_versions(group, &artifact)?;

        if let Some(versions) = versions {
            if versions.is_empty() {
                return Ok(None);
            }

            verbose::log(format!(
                "Found {} versions for plugin {}",
                versions.len(),
                plugin_id
            ));

            Ok(VersionComparator::get_latest(&versions, stable_only))
        } else {
            Ok(None)
        }
    }

    /// Fetch available versions for a plugin, sorted from newest to oldest.
    pub fn fetch_available_plugin_versions(&self, plugin_id: &str) -> Result<Vec<String>> {
        let group = plugin_id;
        let artifact = format!("{}.gradle.plugin", plugin_id);

        if let Some(versions) = self.fetch_all_plugin_versions(group, &artifact)? {
            if versions.is_empty() {
                return Ok(Vec::new());
            }

            let mut parsed: Vec<Version> =
                versions.into_iter().map(|v| Version::parse(&v)).collect();
            parsed.sort();
            parsed.dedup_by(|a, b| a.original == b.original);
            Ok(parsed.into_iter().rev().map(|v| v.original).collect())
        } else {
            Ok(Vec::new())
        }
    }

    fn fetch_all_plugin_versions(
        &self,
        group: &str,
        artifact: &str,
    ) -> Result<Option<Vec<String>>> {
        let metadata_url = Self::metadata_url(group, artifact);

        if let Some(versions) = self.metadata_cache.get(&metadata_url)? {
            verbose::log(format!("Cache hit: {}", metadata_url));
            return Ok(Some(versions));
        }

        verbose::log(format!("Fetching: {}", metadata_url));

        let response = match self.client.get(&metadata_url).send() {
            Ok(resp) => resp,
            Err(e) => {
                verbose::log(format!("Request failed: {}", e));
                return Ok(None);
            }
        };

        if !response.status().is_success() {
            verbose::log(format!("HTTP {}: {}", response.status(), metadata_url));
            return Ok(None);
        }

        let text = response
            .text()
            .map_err(|e| GvcError::Io(std::io::Error::other(e)))?;

        if text.len() > MAX_METADATA_BYTES {
            return Err(GvcError::Io(std::io::Error::other(
                "Plugin metadata response exceeded 10MB limit",
            )));
        }

        let metadata: MavenMetadata = from_str(&text).map_err(|e| {
            GvcError::TomlParsing(format!("Failed to parse plugin metadata: {}", e))
        })?;

        let versions: Vec<String> = metadata.versioning.versions.version.to_vec();
        self.metadata_cache.insert(metadata_url, versions.clone())?;

        Ok(Some(versions))
    }

    fn metadata_url(group: &str, artifact: &str) -> String {
        let group_path = group.replace('.', "/");
        format!(
            "{}/{}/{}/maven-metadata.xml",
            GRADLE_PLUGIN_PORTAL, group_path, artifact
        )
    }
}

impl RepositoryClient for PluginPortalClient {
    fn fetch_available_versions(&self, coordinate: &Coordinate) -> Result<Vec<String>> {
        self.fetch_available_plugin_versions(&coordinate.group)
    }

    fn fetch_latest_version(
        &self,
        coordinate: &Coordinate,
        stable_only: bool,
    ) -> Result<Option<String>> {
        self.fetch_latest_plugin_version(&coordinate.group, stable_only)
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
    fn builds_plugin_metadata_url() {
        assert_eq!(
            PluginPortalClient::metadata_url(
                "org.jetbrains.kotlin.jvm",
                "org.jetbrains.kotlin.jvm.gradle.plugin"
            ),
            "https://plugins.gradle.org/m2/org/jetbrains/kotlin/jvm/org.jetbrains.kotlin.jvm.gradle.plugin/maven-metadata.xml"
        );
    }

    #[test]
    #[ignore = "hits the live Gradle Plugin Portal; run manually when validating network behavior"]
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
