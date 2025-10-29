use crate::error::{GvcError, Result};
use crate::gradle::Repository as GradleRepository;
use crate::maven::version::{Version, VersionComparator};
use crate::repository::{Coordinate, RepositoryClient};
use quick_xml::de::from_str;
use regex::Regex;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::net::IpAddr;
use std::time::Duration;
use url::Url;

const DEFAULT_MAVEN_CENTRAL: &str = "https://repo1.maven.org/maven2";
const GOOGLE_MAVEN: &str = "https://dl.google.com/dl/android/maven2";
const MAX_METADATA_BYTES: usize = 10 * 1024 * 1024;

/// Maven repository client
pub struct MavenRepository {
    client: reqwest::blocking::Client,
    repositories: Vec<GradleRepository>,
}

impl MavenRepository {
    pub fn new() -> Result<Self> {
        let client = Self::build_client()?;
        let repositories = Self::ensure_valid_repositories(Self::default_repositories())?;

        Ok(Self {
            client,
            repositories,
        })
    }

    pub fn with_repositories(repositories: Vec<GradleRepository>) -> Result<Self> {
        let client = Self::build_client()?;
        let repositories = if repositories.is_empty() {
            Self::default_repositories()
        } else {
            repositories
        };

        let repositories = Self::ensure_valid_repositories(repositories)?;

        Ok(Self {
            client,
            repositories,
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

    /// Fetch all available versions for a dependency, sorted from newest to oldest.
    pub fn fetch_available_versions(&self, group: &str, artifact: &str) -> Result<Vec<String>> {
        for repo in &self.repositories {
            if !repo.group_filters.is_empty() && !Self::matches_filters(group, &repo.group_filters)
            {
                continue;
            }

            if let Ok(Some(versions)) =
                self.fetch_all_versions_from_repository(&repo.url, group, artifact)
            {
                if versions.is_empty() {
                    continue;
                }

                let mut parsed: Vec<Version> =
                    versions.into_iter().map(|v| Version::parse(&v)).collect();
                parsed.sort();
                parsed.dedup_by(|a, b| a.original == b.original);
                let ordered = parsed.into_iter().rev().map(|v| v.original).collect();
                return Ok(ordered);
            }
        }

        Ok(Vec::new())
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

        if text.len() > MAX_METADATA_BYTES {
            return Err(GvcError::Io(std::io::Error::other(
                "Maven metadata response exceeded 10MB limit",
            )));
        }

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

impl RepositoryClient for MavenRepository {
    fn fetch_available_versions(&self, coordinate: &Coordinate) -> Result<Vec<String>> {
        MavenRepository::fetch_available_versions(self, &coordinate.group, &coordinate.artifact)
    }

    fn fetch_latest_version(
        &self,
        coordinate: &Coordinate,
        stable_only: bool,
    ) -> Result<Option<String>> {
        MavenRepository::fetch_latest_version(
            self,
            &coordinate.group,
            &coordinate.artifact,
            stable_only,
        )
    }
}

impl MavenRepository {
    fn build_client() -> Result<Client> {
        Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("gvc")
            .danger_accept_invalid_certs(false)
            .build()
            .map_err(|e| GvcError::Io(std::io::Error::other(e)))
    }

    fn default_repositories() -> Vec<GradleRepository> {
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
    }

    fn ensure_valid_repositories(
        repositories: Vec<GradleRepository>,
    ) -> Result<Vec<GradleRepository>> {
        for repo in &repositories {
            Self::validate_repository_url(&repo.url)?;
        }
        Ok(repositories)
    }

    fn validate_repository_url(url: &str) -> Result<()> {
        let parsed = Url::parse(url)
            .map_err(|_| GvcError::ProjectValidation(format!("Invalid repository URL: {url}")))?;

        match parsed.scheme() {
            "https" | "http" => {}
            scheme => {
                return Err(GvcError::ProjectValidation(format!(
                    "Unsupported repository scheme: {scheme}"
                )));
            }
        }

        if let Some(host) = parsed.host_str() {
            if Self::is_private_host(host) {
                return Err(GvcError::ProjectValidation(format!(
                    "Repository host '{host}' is not allowed"
                )));
            }
        }

        Ok(())
    }

    fn is_private_host(host: &str) -> bool {
        if host.eq_ignore_ascii_case("localhost") {
            return true;
        }

        if let Ok(ip) = host.parse::<IpAddr>() {
            match ip {
                IpAddr::V4(v4) => v4.is_private() || v4.is_loopback(),
                IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local(),
            }
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_https_repository() {
        assert!(
            MavenRepository::validate_repository_url("https://repo.maven.apache.org/maven2")
                .is_ok()
        );
    }

    #[test]
    fn rejects_invalid_scheme() {
        let err = MavenRepository::validate_repository_url("ftp://example.com").unwrap_err();
        assert!(matches!(err, GvcError::ProjectValidation(_)));
    }

    #[test]
    fn rejects_private_host() {
        let err = MavenRepository::validate_repository_url("https://127.0.0.1/repo").unwrap_err();
        assert!(matches!(err, GvcError::ProjectValidation(_)));
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
