use crate::error::Result;
use crate::maven::version::VersionComparator;
use std::sync::Arc;

pub mod factory;
pub use factory::RepositoryFactory;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coordinate {
    pub group: String,
    pub artifact: String,
}

impl Coordinate {
    pub fn new(group: impl Into<String>, artifact: impl Into<String>) -> Self {
        Self {
            group: group.into(),
            artifact: artifact.into(),
        }
    }

    pub fn plugin(plugin_id: impl Into<String>) -> Self {
        let id = plugin_id.into();
        Self {
            group: id.clone(),
            artifact: id,
        }
    }
}

pub trait RepositoryClient: Send + Sync {
    fn fetch_available_versions(&self, coordinate: &Coordinate) -> Result<Vec<String>>;

    fn fetch_latest_version(
        &self,
        coordinate: &Coordinate,
        stable_only: bool,
    ) -> Result<Option<String>>;
}

pub trait VersionStrategy: Send + Sync {
    fn select_latest(&self, versions: &[String], stable_only: bool) -> Option<String>;
    fn is_upgrade(&self, current: &str, candidate: &str) -> bool;
}

#[derive(Debug, Default)]
pub struct DefaultVersionStrategy;

impl VersionStrategy for DefaultVersionStrategy {
    fn select_latest(&self, versions: &[String], stable_only: bool) -> Option<String> {
        VersionComparator::get_latest(versions, stable_only)
    }

    fn is_upgrade(&self, current: &str, candidate: &str) -> bool {
        VersionComparator::is_newer(candidate, current)
    }
}

impl DefaultVersionStrategy {
    pub fn shared() -> Arc<dyn VersionStrategy> {
        Arc::new(Self)
    }
}
