use crate::error::{GvcError, Result};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Default)]
pub(super) struct MetadataCache {
    entries: Mutex<HashMap<String, Vec<String>>>,
}

impl MetadataCache {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn get(&self, key: &str) -> Result<Option<Vec<String>>> {
        let entries = self.entries.lock().map_err(|_| Self::lock_error())?;
        Ok(entries.get(key).cloned())
    }

    pub(super) fn insert(&self, key: String, versions: Vec<String>) -> Result<()> {
        let mut entries = self.entries.lock().map_err(|_| Self::lock_error())?;
        entries.insert(key, versions);
        Ok(())
    }

    fn lock_error() -> GvcError {
        GvcError::Io(std::io::Error::other("metadata cache lock poisoned"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_returns_cloned_versions() {
        let cache = MetadataCache::new();
        cache
            .insert(
                "https://example.test/maven-metadata.xml".to_string(),
                vec!["1.0.0".to_string()],
            )
            .unwrap();

        let mut cached = cache
            .get("https://example.test/maven-metadata.xml")
            .unwrap()
            .unwrap();
        cached.push("2.0.0".to_string());

        assert_eq!(
            cache
                .get("https://example.test/maven-metadata.xml")
                .unwrap()
                .unwrap(),
            vec!["1.0.0".to_string()]
        );
    }

    #[test]
    fn missing_cache_key_returns_none() {
        let cache = MetadataCache::new();
        assert!(cache.get("missing").unwrap().is_none());
    }
}
