use std::cmp::Ordering;

/// Version representation supporting various formats
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub original: String,
    pub parsed: VersionType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionType {
    Semantic(semver::Version),
    Numeric(Vec<u32>),
    Snapshot(String),
    Unknown(String),
}

impl Version {
    pub fn parse(version: &str) -> Self {
        let parsed = if let Ok(v) = semver::Version::parse(version) {
            VersionType::Semantic(v)
        } else if version.ends_with("-SNAPSHOT") {
            VersionType::Snapshot(version.to_string())
        } else if let Some(numeric) = Self::parse_numeric(version) {
            VersionType::Numeric(numeric)
        } else {
            VersionType::Unknown(version.to_string())
        };

        Version {
            original: version.to_string(),
            parsed,
        }
    }

    fn parse_numeric(version: &str) -> Option<Vec<u32>> {
        let parts: Vec<&str> = version.split('.').collect();
        let mut numbers = Vec::new();

        for part in parts {
            if let Ok(num) = part.parse::<u32>() {
                numbers.push(num);
            } else {
                return None;
            }
        }

        if numbers.is_empty() {
            None
        } else {
            Some(numbers)
        }
    }

    pub fn is_stable(&self) -> bool {
        let lower = self.original.to_lowercase();

        // Check for common unstable markers
        let unstable_markers = [
            "alpha", "beta", "rc", "snapshot", "dev", "-dev", "+dev",
            ".dev", // Various dev version formats
            "m1", "m2", "m3", // Milestone versions
            "eap", "preview", "canary",
        ];

        for marker in &unstable_markers {
            if lower.contains(marker) {
                return false;
            }
        }

        // For semantic versions, also check pre-release
        match &self.parsed {
            VersionType::Semantic(v) => v.pre.is_empty(),
            VersionType::Snapshot(_) => false,
            _ => true,
        }
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        match (&self.parsed, &other.parsed) {
            (VersionType::Semantic(a), VersionType::Semantic(b)) => a.cmp(b),
            (VersionType::Numeric(a), VersionType::Numeric(b)) => {
                for (av, bv) in a.iter().zip(b.iter()) {
                    match av.cmp(bv) {
                        Ordering::Equal => continue,
                        other => return other,
                    }
                }
                a.len().cmp(&b.len())
            }
            (VersionType::Snapshot(_), _) => Ordering::Less,
            (_, VersionType::Snapshot(_)) => Ordering::Greater,
            _ => self.original.cmp(&other.original),
        }
    }
}

pub struct VersionComparator;

impl VersionComparator {
    /// Get the latest version from a list
    pub fn get_latest(versions: &[String], stable_only: bool) -> Option<String> {
        let mut parsed_versions: Vec<Version> =
            versions.iter().map(|v| Version::parse(v)).collect();

        if stable_only {
            parsed_versions.retain(|v| v.is_stable());
        }

        parsed_versions.sort();
        parsed_versions.last().map(|v| v.original.clone())
    }

    /// Check if version `a` is newer than version `b`
    pub fn is_newer(a: &str, b: &str) -> bool {
        let va = Version::parse(a);
        let vb = Version::parse(b);
        va > vb
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing() {
        let v1 = Version::parse("1.0.0");
        let v2 = Version::parse("1.0.1");
        assert!(v2 > v1);
    }

    #[test]
    fn test_stable_detection() {
        assert!(Version::parse("1.0.0").is_stable());
        assert!(!Version::parse("1.0.0-alpha").is_stable());
        assert!(!Version::parse("1.0.0-SNAPSHOT").is_stable());
    }

    #[test]
    fn test_get_latest() {
        let versions = vec![
            "1.0.0".to_string(),
            "1.1.0-alpha".to_string(),
            "1.0.1".to_string(),
        ];

        let latest = VersionComparator::get_latest(&versions, false);
        assert_eq!(latest, Some("1.1.0-alpha".to_string()));

        let latest_stable = VersionComparator::get_latest(&versions, true);
        assert_eq!(latest_stable, Some("1.0.1".to_string()));
    }
}
