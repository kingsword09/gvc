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

        // 1. 尝试从 settings.gradle.kts 读取
        if let Ok(repos) = self.parse_settings_gradle_kts() {
            repositories.extend(repos);
        }

        // 2. 尝试从 settings.gradle 读取
        if let Ok(repos) = self.parse_settings_gradle() {
            repositories.extend(repos);
        }

        // 3. 尝试从 build.gradle.kts 读取
        if let Ok(repos) = self.parse_build_gradle_kts() {
            repositories.extend(repos);
        }

        // 4. 尝试从 build.gradle 读取
        if let Ok(repos) = self.parse_build_gradle() {
            repositories.extend(repos);
        }

        if repositories.is_empty() {
            // 如果没找到任何仓库配置，使用默认的
            println!("⚠️  No repositories found in Gradle config, using defaults");
            repositories = self.get_default_repositories();
        }

        // 去重
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

        // 匹配 mavenCentral()
        if content.contains("mavenCentral()") {
            repositories.push(Repository {
                name: "Maven Central".to_string(),
                url: "https://repo1.maven.org/maven2".to_string(),
                group_filters: Vec::new(),
            });
        }

        // 匹配 google()
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

        // 匹配 gradlePluginPortal()
        if content.contains("gradlePluginPortal()") {
            repositories.push(Repository {
                name: "Gradle Plugin Portal".to_string(),
                url: "https://plugins.gradle.org/m2".to_string(),
                group_filters: Vec::new(),
            });
        }

        // 匹配自定义 maven { url = uri("...") }
        let maven_url_regex =
            Regex::new(r#"maven\s*\{\s*url\s*=\s*uri\s*\(\s*["']([^"']+)["']\s*\)\s*\}"#)
                .map_err(|e| GvcError::TomlParsing(format!("Regex error: {}", e)))?;

        for cap in maven_url_regex.captures_iter(content) {
            if let Some(url) = cap.get(1) {
                repositories.push(Repository {
                    name: format!("Custom ({})", Self::shorten_url(url.as_str())),
                    url: url.as_str().to_string(),
                    group_filters: Vec::new(),
                });
            }
        }

        // 匹配 maven("...")
        let maven_simple_regex = Regex::new(r#"maven\s*\(\s*["']([^"']+)["']\s*\)"#)
            .map_err(|e| GvcError::TomlParsing(format!("Regex error: {}", e)))?;

        for cap in maven_simple_regex.captures_iter(content) {
            if let Some(url) = cap.get(1) {
                repositories.push(Repository {
                    name: format!("Custom ({})", Self::shorten_url(url.as_str())),
                    url: url.as_str().to_string(),
                    group_filters: Vec::new(),
                });
            }
        }

        Ok(repositories)
    }

    /// Extract repositories from Groovy DSL content
    fn extract_repositories_groovy(&self, content: &str) -> Result<Vec<Repository>> {
        let mut repositories = Vec::new();

        // 匹配 mavenCentral()
        if content.contains("mavenCentral()") {
            repositories.push(Repository {
                name: "Maven Central".to_string(),
                url: "https://repo1.maven.org/maven2".to_string(),
                group_filters: Vec::new(),
            });
        }

        // 匹配 google()
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

        // 匹配 jcenter() (已废弃但仍可能存在)
        if content.contains("jcenter()") {
            repositories.push(Repository {
                name: "JCenter (Deprecated)".to_string(),
                url: "https://jcenter.bintray.com".to_string(),
                group_filters: Vec::new(),
            });
        }

        // 匹配 maven { url 'https://...' }
        let maven_url_regex = Regex::new(r#"maven\s*\{\s*url\s+['"]([^'"]+)['"]"#)
            .map_err(|e| GvcError::TomlParsing(format!("Regex error: {}", e)))?;

        for cap in maven_url_regex.captures_iter(content) {
            if let Some(url) = cap.get(1) {
                repositories.push(Repository {
                    name: format!("Custom ({})", Self::shorten_url(url.as_str())),
                    url: url.as_str().to_string(),
                    group_filters: Vec::new(),
                });
            }
        }

        // 匹配 maven { url = 'https://...' }
        let maven_url_equals_regex = Regex::new(r#"maven\s*\{\s*url\s*=\s*['"]([^'"]+)['"]"#)
            .map_err(|e| GvcError::TomlParsing(format!("Regex error: {}", e)))?;

        for cap in maven_url_equals_regex.captures_iter(content) {
            if let Some(url) = cap.get(1) {
                repositories.push(Repository {
                    name: format!("Custom ({})", Self::shorten_url(url.as_str())),
                    url: url.as_str().to_string(),
                    group_filters: Vec::new(),
                });
            }
        }

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
            // 标准化URL（移除末尾的斜杠）
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
        assert!(repos
            .iter()
            .any(|r| r.url == "https://repo1.maven.org/maven2"));
        assert!(repos.iter().any(|r| r.url == "https://jitpack.io"));
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
    fn test_deduplicate() {
        let repos = vec![
            Repository {
                name: "Maven Central".to_string(),
                url: "https://repo1.maven.org/maven2".to_string(),
                group_filters: Vec::new(),
            },
            Repository {
                name: "Maven Central".to_string(),
                url: "https://repo1.maven.org/maven2/".to_string(), // 末尾斜杠
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
