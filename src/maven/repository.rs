use crate::error::{GvcError, Result};
use crate::gradle::Repository as GradleRepository;
use crate::maven::version::VersionComparator;
use quick_xml::de::from_str;
use regex::Regex;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::time::Duration;

const DEFAULT_MAVEN_CENTRAL: &str = "https://repo1.maven.org/maven2";
const GOOGLE_MAVEN: &str = "https://dl.google.com/dl/android/maven2";

/// Maven repository client
pub struct MavenRepository {
    client: reqwest::blocking::Client,
    repositories: Vec<GradleRepository>,
}

impl MavenRepository {
    pub fn new() -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| GvcError::Io(std::io::Error::other(e)))?;

        Ok(Self {
            client,
            repositories: vec![
                GradleRepository {
                    name: "Maven Central".to_string(),
                    url: DEFAULT_MAVEN_CENTRAL.to_string(),
                    group_filters: Vec::new(),
                },
                GradleRepository {
                    name: "Google Maven".to_string(),
                    url: GOOGLE_MAVEN.to_string(),
                    group_filters: vec![
                        ".*google.*".to_string(),
                        ".*android.*".to_string(),
                        ".*androidx.*".to_string(),
                    ],
                },
            ],
        })
    }

    pub fn with_repositories(repositories: Vec<GradleRepository>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| GvcError::Io(std::io::Error::other(e)))?;

        let repos = if repositories.is_empty() {
            vec![
                GradleRepository {
                    name: "Maven Central".to_string(),
                    url: DEFAULT_MAVEN_CENTRAL.to_string(),
                    group_filters: Vec::new(),
                },
                GradleRepository {
                    name: "Google Maven".to_string(),
                    url: GOOGLE_MAVEN.to_string(),
                    group_filters: vec![
                        ".*google.*".to_string(),
                        ".*android.*".to_string(),
                        ".*androidx.*".to_string(),
                    ],
                },
            ]
        } else {
            repositories
        };

        Ok(Self {
            client,
            repositories: repos,
        })
    }

    /// Fetch the latest version of a dependency from repositories
    /// Stops at the first repository that has the artifact to avoid excessive requests
    /// Uses group_filters to skip repositories that don't match the group
    pub fn fetch_latest_version(
        &self,
        group: &str,
        artifact: &str,
        stable_only: bool,
    ) -> Result<Option<String>> {
        // Try repositories in order, return first successful result
        for repo in &self.repositories {
            // Skip repository if it has filters and group doesn't match any of them
            if !repo.group_filters.is_empty() && !Self::matches_filters(group, &repo.group_filters)
            {
                continue;
            }

            if let Ok(Some(versions)) =
                self.fetch_all_versions_from_repository(&repo.url, group, artifact)
            {
                if !versions.is_empty() {
                    // Found versions in this repository, use them
                    return Ok(VersionComparator::get_latest(&versions, stable_only));
                }
            }
        }

        // No repository had this artifact
        Ok(None)
    }

    /// Check if a group matches any of the regex filters
    fn matches_filters(group: &str, filters: &[String]) -> bool {
        for filter_pattern in filters {
            if let Ok(re) = Regex::new(filter_pattern) {
                if re.is_match(group) {
                    return true;
                }
            }
        }
        false
    }

    fn fetch_all_versions_from_repository(
        &self,
        repo_url: &str,
        group: &str,
        artifact: &str,
    ) -> Result<Option<Vec<String>>> {
        let group_path = group.replace('.', "/");
        let metadata_url = format!(
            "{}/{}/{}/maven-metadata.xml",
            repo_url, group_path, artifact
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
            .map_err(|e| GvcError::Io(std::io::Error::other(e)))?;

        let metadata: MavenMetadata = from_str(&text)
            .map_err(|e| GvcError::TomlParsing(format!("Failed to parse Maven metadata: {}", e)))?;

        let versions: Vec<String> = metadata.versioning.versions.version.to_vec();

        Ok(Some(versions))
    }

    /// Fetch metadata for a dependency
    #[allow(dead_code)]
    pub fn fetch_metadata(
        &self,
        group: &str,
        artifact: &str,
    ) -> Result<Option<DependencyMetadata>> {
        for repo in &self.repositories {
            if let Ok(Some(metadata)) =
                self.fetch_metadata_from_repository(&repo.url, group, artifact)
            {
                return Ok(Some(metadata));
            }
        }
        Ok(None)
    }

    fn fetch_metadata_from_repository(
        &self,
        repo_url: &str,
        group: &str,
        artifact: &str,
    ) -> Result<Option<DependencyMetadata>> {
        let group_path = group.replace('.', "/");
        let metadata_url = format!(
            "{}/{}/{}/maven-metadata.xml",
            repo_url, group_path, artifact
        );

        let response = match self.client.get(&metadata_url).send() {
            Ok(resp) => resp,
            Err(_) => return Ok(None),
        };

        if !response.status().is_success() {
            return Ok(None);
        }

        let text = response
            .text()
            .map_err(|e| GvcError::Io(std::io::Error::other(e)))?;

        let maven_metadata: MavenMetadata = from_str(&text)
            .map_err(|e| GvcError::TomlParsing(format!("Failed to parse Maven metadata: {}", e)))?;

        Ok(Some(DependencyMetadata {
            group: maven_metadata.group_id,
            artifact: maven_metadata.artifact_id,
            versions: maven_metadata.versioning.versions.version,
            latest: maven_metadata.versioning.latest,
            release: maven_metadata.versioning.release,
        }))
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DependencyMetadata {
    pub group: String,
    pub artifact: String,
    pub versions: Vec<String>,
    pub latest: Option<String>,
    pub release: Option<String>,
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

/// Parse a Maven coordinate (e.g., "com.example:artifact:1.0.0")
pub fn parse_maven_coordinate(coordinate: &str) -> Option<(String, String, Option<String>)> {
    let parts: Vec<&str> = coordinate.split(':').collect();
    match parts.len() {
        2 => Some((parts[0].to_string(), parts[1].to_string(), None)),
        3 => Some((
            parts[0].to_string(),
            parts[1].to_string(),
            Some(parts[2].to_string()),
        )),
        _ => None,
    }
}
