use crate::error::{GvcError, Result};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

/// Gradle repository configuration
#[derive(Debug, Clone)]
pub struct Repository {
    pub name: String,
    pub url: String,
    /// Regex patterns for group filtering (from mavenContent.includeGroupByRegex)
    pub group_filters: Vec<String>,
}

/// Gradle project configuration
#[derive(Debug, Clone)]
pub struct GradleConfig {
    pub repositories: Vec<Repository>,
}

/// Parser for Gradle configuration files
pub struct GradleConfigParser {
    project_path: PathBuf,
}

impl GradleConfigParser {
    pub fn new<P: AsRef<Path>>(project_path: P) -> Self {
        Self {
            project_path: project_path.as_ref().to_path_buf(),
        }
    }

    /// Parse Gradle configuration and extract repositories
    pub fn parse(&self) -> Result<GradleConfig> {
        let mut repositories = Vec::new();

        // 1. Attempt to read settings.gradle.kts
        if let Ok(repos) = self.parse_settings_gradle_kts() {
            repositories.extend(repos);
        }

        // 2. Attempt to read settings.gradle
        if let Ok(repos) = self.parse_settings_gradle() {
            repositories.extend(repos);
        }

        // 3. Attempt to read build.gradle.kts
        if let Ok(repos) = self.parse_build_gradle_kts() {
            repositories.extend(repos);
        }

        // 4. Attempt to read build.gradle
        if let Ok(repos) = self.parse_build_gradle() {
            repositories.extend(repos);
        }

        if repositories.is_empty() {
            // Fall back to defaults when no repositories are configured explicitly
            crate::outln!("⚠️  No repositories found in Gradle config, using defaults");
            repositories = self.get_default_repositories();
        }

        // Remove duplicated repositories
        repositories = self.deduplicate_repositories(repositories);

        Ok(GradleConfig { repositories })
    }

    /// Parse settings.gradle.kts (Kotlin DSL)
    fn parse_settings_gradle_kts(&self) -> Result<Vec<Repository>> {
        let path = self.project_path.join("settings.gradle.kts");
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&path)?;
        self.extract_repositories_kotlin(&content)
    }

    /// Parse settings.gradle (Groovy)
    fn parse_settings_gradle(&self) -> Result<Vec<Repository>> {
        let path = self.project_path.join("settings.gradle");
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&path)?;
        self.extract_repositories_groovy(&content)
    }

    /// Parse build.gradle.kts (Kotlin DSL)
    fn parse_build_gradle_kts(&self) -> Result<Vec<Repository>> {
        let path = self.project_path.join("build.gradle.kts");
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&path)?;
        self.extract_repositories_kotlin(&content)
    }

    /// Parse build.gradle (Groovy)
    fn parse_build_gradle(&self) -> Result<Vec<Repository>> {
        let path = self.project_path.join("build.gradle");
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&path)?;
        self.extract_repositories_groovy(&content)
    }

    /// Extract repositories from Kotlin DSL content
    fn extract_repositories_kotlin(&self, content: &str) -> Result<Vec<Repository>> {
        let mut repositories = Vec::new();

        // Match mavenCentral()
        if content.contains("mavenCentral()") {
            repositories.push(Repository {
                name: "Maven Central".to_string(),
                url: "https://repo1.maven.org/maven2".to_string(),
                group_filters: Vec::new(),
            });
        }

        // Match google()
        if content.contains("google()") {
            repositories.push(Repository {
                name: "Google Maven".to_string(),
                url: "https://dl.google.com/dl/android/maven2".to_string(),
                group_filters: vec![
                    ".*google.*".to_string(),
                    ".*android.*".to_string(),
                    ".*androidx.*".to_string(),
                ],
            });
        }

        // Match gradlePluginPortal()
        if content.contains("gradlePluginPortal()") {
            repositories.push(Repository {
                name: "Gradle Plugin Portal".to_string(),
                url: "https://plugins.gradle.org/m2".to_string(),
                group_filters: Vec::new(),
            });
        }

        repositories.extend(Self::extract_custom_maven_repositories(content)?);

        // Match maven("...")
        let maven_simple_regex = Regex::new(r#"maven\s*\(\s*["']([^"']+)["']\s*\)"#)
            .map_err(|e| GvcError::TomlParsing(format!("Regex error: {}", e)))?;

        for cap in maven_simple_regex.captures_iter(content) {
            if let Some(url) = cap.get(1) {
                let url = Self::unescape_gradle_string(url.as_str());
                repositories.push(Repository {
                    name: format!("Custom ({})", Self::shorten_url(&url)),
                    url,
                    group_filters: Vec::new(),
                });
            }
        }

        Ok(repositories)
    }

    /// Extract repositories from Groovy DSL content
    fn extract_repositories_groovy(&self, content: &str) -> Result<Vec<Repository>> {
        let mut repositories = Vec::new();

        // Match mavenCentral()
        if content.contains("mavenCentral()") {
            repositories.push(Repository {
                name: "Maven Central".to_string(),
                url: "https://repo1.maven.org/maven2".to_string(),
                group_filters: Vec::new(),
            });
        }

        // Match google()
        if content.contains("google()") {
            repositories.push(Repository {
                name: "Google Maven".to_string(),
                url: "https://dl.google.com/dl/android/maven2".to_string(),
                group_filters: vec![
                    ".*google.*".to_string(),
                    ".*android.*".to_string(),
                    ".*androidx.*".to_string(),
                ],
            });
        }

        // Match jcenter() (deprecated but still seen in legacy projects)
        if content.contains("jcenter()") {
            repositories.push(Repository {
                name: "JCenter (Deprecated)".to_string(),
                url: "https://jcenter.bintray.com".to_string(),
                group_filters: Vec::new(),
            });
        }

        repositories.extend(Self::extract_custom_maven_repositories(content)?);

        Ok(repositories)
    }

    /// Get default repositories as fallback
    fn get_default_repositories(&self) -> Vec<Repository> {
        vec![
            Repository {
                name: "Maven Central".to_string(),
                url: "https://repo1.maven.org/maven2".to_string(),
                group_filters: Vec::new(),
            },
            Repository {
                name: "Google Maven".to_string(),
                url: "https://dl.google.com/dl/android/maven2".to_string(),
                group_filters: vec![
                    ".*google.*".to_string(),
                    ".*android.*".to_string(),
                    ".*androidx.*".to_string(),
                ],
            },
        ]
    }

    /// Remove duplicate repositories
    fn deduplicate_repositories(&self, repos: Vec<Repository>) -> Vec<Repository> {
        let mut seen_urls = std::collections::HashSet::new();
        let mut unique_repos = Vec::new();

        for repo in repos {
            // Normalise the URL by stripping any trailing slash
            let normalized_url = repo.url.trim_end_matches('/').to_string();

            if seen_urls.insert(normalized_url.clone()) {
                unique_repos.push(Repository {
                    name: repo.name,
                    url: normalized_url,
                    group_filters: repo.group_filters,
                });
            }
        }

        unique_repos
    }

    fn extract_custom_maven_repositories(content: &str) -> Result<Vec<Repository>> {
        let mut repositories = Vec::new();

        for block in Self::extract_named_blocks(content, "maven") {
            let Some(url) = Self::extract_maven_url(&block)? else {
                continue;
            };

            repositories.push(Repository {
                name: format!("Custom ({})", Self::shorten_url(&url)),
                url,
                group_filters: Self::extract_group_filters(&block)?,
            });
        }

        Ok(repositories)
    }

    fn extract_maven_url(block: &str) -> Result<Option<String>> {
        let patterns = [
            r#"url\s*=\s*uri\s*\(\s*["']([^"']+)["']\s*\)"#,
            r#"url\s*=\s*["']([^"']+)["']"#,
            r#"url\s+["']([^"']+)["']"#,
        ];

        for pattern in patterns {
            let regex = Regex::new(pattern)
                .map_err(|e| GvcError::TomlParsing(format!("Regex error: {}", e)))?;
            if let Some(url) = regex
                .captures(block)
                .and_then(|cap| cap.get(1).map(|m| Self::unescape_gradle_string(m.as_str())))
            {
                return Ok(Some(url));
            }
        }

        Ok(None)
    }

    fn extract_group_filters(block: &str) -> Result<Vec<String>> {
        let regex = Regex::new(r#"includeGroupByRegex\s*(?:\(\s*)?["']([^"']+)["']\s*\)?"#)
            .map_err(|e| GvcError::TomlParsing(format!("Regex error: {}", e)))?;

        Ok(regex
            .captures_iter(block)
            .filter_map(|cap| cap.get(1).map(|m| Self::unescape_gradle_string(m.as_str())))
            .collect())
    }

    fn unescape_gradle_string(value: &str) -> String {
        let mut unescaped = String::with_capacity(value.len());
        let mut chars = value.chars();

        while let Some(ch) = chars.next() {
            if ch != '\\' {
                unescaped.push(ch);
                continue;
            }

            let Some(next) = chars.next() else {
                unescaped.push(ch);
                break;
            };

            match next {
                '\\' => unescaped.push('\\'),
                '"' => unescaped.push('"'),
                '\'' => unescaped.push('\''),
                'n' => unescaped.push('\n'),
                'r' => unescaped.push('\r'),
                't' => unescaped.push('\t'),
                _ => {
                    unescaped.push('\\');
                    unescaped.push(next);
                }
            }
        }

        unescaped
    }

    fn extract_named_blocks(content: &str, name: &str) -> Vec<String> {
        let bytes = content.as_bytes();
        let name_bytes = name.as_bytes();
        let mut blocks = Vec::new();
        let mut cursor = 0;

        while cursor < bytes.len() {
            let Some(relative) = content[cursor..].find(name) else {
                break;
            };
            let start = cursor + relative;
            let after_name = start + name_bytes.len();

            if start > 0 && Self::is_identifier_byte(bytes[start - 1]) {
                cursor = after_name;
                continue;
            }

            if after_name < bytes.len() && Self::is_identifier_byte(bytes[after_name]) {
                cursor = after_name;
                continue;
            }

            let mut brace = after_name;
            while brace < bytes.len() && bytes[brace].is_ascii_whitespace() {
                brace += 1;
            }

            if brace >= bytes.len() || bytes[brace] != b'{' {
                cursor = after_name;
                continue;
            }

            if let Some(end) = Self::find_matching_brace(content, brace) {
                blocks.push(content[brace + 1..end].to_string());
                cursor = end + 1;
            } else {
                break;
            }
        }

        blocks
    }

    fn find_matching_brace(content: &str, open: usize) -> Option<usize> {
        let bytes = content.as_bytes();
        let mut depth = 0usize;
        let mut string_quote = None;
        let mut escaped = false;

        for (index, byte) in bytes.iter().enumerate().skip(open) {
            if let Some(quote) = string_quote {
                if escaped {
                    escaped = false;
                } else if *byte == b'\\' {
                    escaped = true;
                } else if *byte == quote {
                    string_quote = None;
                }
                continue;
            }

            match *byte {
                b'\'' | b'"' => string_quote = Some(*byte),
                b'{' => depth += 1,
                b'}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some(index);
                    }
                }
                _ => {}
            }
        }

        None
    }

    fn is_identifier_byte(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'_'
    }

    /// Shorten URL for display
    fn shorten_url(url: &str) -> String {
        if let Some(domain_start) = url.find("://").map(|i| i + 3) {
            let domain_part = &url[domain_start..];
            if let Some(slash_pos) = domain_part.find('/') {
                return domain_part[..slash_pos].to_string();
            }
            return domain_part.to_string();
        }
        url.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_kotlin_dsl() {
        let content = r#"
repositories {
    mavenCentral()
    google()
    maven { url = uri("https://jitpack.io") }
}
        "#;

        let parser = GradleConfigParser::new(".");
        let repos = parser.extract_repositories_kotlin(content).unwrap();

        assert_eq!(repos.len(), 3);
        assert!(
            repos
                .iter()
                .any(|r| r.url == "https://repo1.maven.org/maven2")
        );
        assert!(repos.iter().any(|r| r.url == "https://jitpack.io"));
    }

    #[test]
    fn test_extract_kotlin_maven_content_filter() {
        let content = r#"
repositories {
    maven {
        url = uri("https://repo.example.com/releases")
        mavenContent {
            includeGroupByRegex("com\\.example\\..*")
            includeGroupByRegex("org\\.sample")
        }
    }
}
        "#;

        let parser = GradleConfigParser::new(".");
        let repos = parser.extract_repositories_kotlin(content).unwrap();

        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].url, "https://repo.example.com/releases");
        assert_eq!(
            repos[0].group_filters,
            vec!["com\\.example\\..*".to_string(), "org\\.sample".to_string()]
        );
    }

    #[test]
    fn test_extract_groovy_dsl() {
        let content = r#"
repositories {
    mavenCentral()
    google()
    maven { url 'https://jitpack.io' }
}
        "#;

        let parser = GradleConfigParser::new(".");
        let repos = parser.extract_repositories_groovy(content).unwrap();

        assert_eq!(repos.len(), 3);
        assert!(repos.iter().any(|r| r.url == "https://jitpack.io"));
    }

    #[test]
    fn test_extract_groovy_maven_content_filter() {
        let content = r#"
repositories {
    maven {
        url 'https://repo.example.com/releases'
        mavenContent {
            includeGroupByRegex 'com\\.example\\..*'
        }
    }
}
        "#;

        let parser = GradleConfigParser::new(".");
        let repos = parser.extract_repositories_groovy(content).unwrap();

        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].url, "https://repo.example.com/releases");
        assert_eq!(
            repos[0].group_filters,
            vec!["com\\.example\\..*".to_string()]
        );
    }

    #[test]
    fn test_deduplicate() {
        let repos = vec![
            Repository {
                name: "Maven Central".to_string(),
                url: "https://repo1.maven.org/maven2".to_string(),
                group_filters: Vec::new(),
            },
            Repository {
                name: "Maven Central".to_string(),
                url: "https://repo1.maven.org/maven2/".to_string(), // trailing slash
                group_filters: Vec::new(),
            },
            Repository {
                name: "Google".to_string(),
                url: "https://dl.google.com/dl/android/maven2".to_string(),
                group_filters: Vec::new(),
            },
        ];

        let parser = GradleConfigParser::new(".");
        let unique = parser.deduplicate_repositories(repos);

        assert_eq!(unique.len(), 2);
    }
}
