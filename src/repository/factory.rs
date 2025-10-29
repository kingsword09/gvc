use crate::error::Result;
use crate::gradle::Repository as GradleRepository;
use crate::maven::{MavenRepository, PluginPortalClient};
use crate::repository::RepositoryClient;
use std::sync::Arc;

pub struct RepositoryFactory;

impl RepositoryFactory {
    pub fn create_maven(repositories: Vec<GradleRepository>) -> Result<Arc<dyn RepositoryClient>> {
        let client = if repositories.is_empty() {
            MavenRepository::new()?
        } else {
            MavenRepository::with_repositories(repositories)?
        };
        Ok(Arc::new(client))
    }

    pub fn create_plugin_portal() -> Result<Arc<dyn RepositoryClient>> {
        let client = PluginPortalClient::new()?;
        Ok(Arc::new(client))
    }
}
