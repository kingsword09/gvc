use crate::agents::update::{
    context::{UpdateContext, UpdateReport},
    handlers::{LibraryHandler, PluginHandler, TargetedHandler, VersionHandler},
    interaction::UpdateInteraction,
};
use crate::error::Result;
use crate::repository::{
    DefaultVersionStrategy, RepositoryClient, RepositoryFactory, VersionStrategy,
};
use std::path::Path;
use std::sync::Arc;

/// DependencyUpdater handles the actual dependency version updates
///
/// This struct has been refactored to delegate to focused handler components
/// instead of handling all update logic internally. This improves maintainability
/// and testability.
pub struct DependencyUpdater {
    library_client: Arc<dyn RepositoryClient>,
    plugin_client: Arc<dyn RepositoryClient>,
    version_strategy: Arc<dyn VersionStrategy>,
}

impl DependencyUpdater {
    /// Create a new DependencyUpdater with the given repositories
    pub fn with_repositories(repositories: Vec<crate::gradle::Repository>) -> Result<Self> {
        Self::with_clients(
            RepositoryFactory::create_maven(repositories)?,
            RepositoryFactory::create_plugin_portal()?,
            DefaultVersionStrategy::shared(),
        )
    }

    fn with_clients(
        library_client: Arc<dyn RepositoryClient>,
        plugin_client: Arc<dyn RepositoryClient>,
        version_strategy: Arc<dyn VersionStrategy>,
    ) -> Result<Self> {
        Ok(Self {
            library_client,
            plugin_client,
            version_strategy,
        })
    }

    /// Check for updates without modifying the file
    ///
    /// This is a read-only operation that reports available updates.
    pub fn check_for_updates<P: AsRef<Path>>(
        &self,
        catalog_path: P,
        stable_only: bool,
    ) -> Result<UpdateReport> {
        let catalog_path = catalog_path.as_ref();
        let context = UpdateContext::new(
            catalog_path,
            crate::agents::update::context::UpdateType::Check,
            stable_only,
            false, // No interaction in check mode
        );

        let doc = context.load_document()?;
        let mut report = UpdateReport::new();
        let mut interaction = UpdateInteraction::new(false); // No interaction in check mode

        // Check [versions] section first
        if let Some(_versions) = doc.get("versions").and_then(|v| v.as_table()) {
            if let Some(_libraries) = doc.get("libraries").and_then(|v| v.as_table()) {
                let mut handler = VersionHandler::new(
                    self.library_client.as_ref(),
                    Arc::clone(&self.version_strategy),
                    &mut interaction,
                );
                let version_report = handler.check(&doc, stable_only)?;
                // Merge reports
                for (k, v) in version_report.version_updates {
                    report.add_version_update(k, v.0, v.1);
                }
            }
        }

        // Check [libraries] section
        if let Some(libraries) = doc.get("libraries").and_then(|v| v.as_table()) {
            let mut handler = LibraryHandler::new(
                self.library_client.as_ref(),
                Arc::clone(&self.version_strategy),
                &mut interaction,
            );
            let library_report = handler.check(libraries, stable_only)?;
            // Merge reports
            for (k, v) in library_report.library_updates {
                report.add_library_update(k, v.0, v.1);
            }
        }

        Ok(report)
    }

    /// Update the version catalog file
    ///
    /// This method updates all sections of the catalog based on the
    /// provided configuration. It will prompt the user for confirmation
    /// if interactive mode is enabled.
    pub fn update_version_catalog<P: AsRef<Path>>(
        &self,
        catalog_path: P,
        stable_only: bool,
        interactive: bool,
    ) -> Result<UpdateReport> {
        let catalog_path = catalog_path.as_ref();
        let context = UpdateContext::new(
            catalog_path,
            crate::agents::update::context::UpdateType::Libraries,
            stable_only,
            interactive,
        );

        let mut doc = context.load_document()?;
        let mut report = UpdateReport::new();
        let mut interaction = UpdateInteraction::new(interactive);

        // Update [versions] section
        if let Some(_versions) = doc.get("versions").and_then(|v| v.as_table()) {
            if let Some(_libraries) = doc.get("libraries").and_then(|v| v.as_table()) {
                let mut handler = VersionHandler::new(
                    self.library_client.as_ref(),
                    Arc::clone(&self.version_strategy),
                    &mut interaction,
                );
                let version_report = handler.update(&mut doc, stable_only)?;
                // Merge reports
                for (k, v) in version_report.version_updates {
                    report.add_version_update(k, v.0, v.1);
                }
            }
        }

        // Update [libraries] section
        if let Some(libraries) = doc.get_mut("libraries").and_then(|v| v.as_table_mut()) {
            let mut handler = LibraryHandler::new(
                self.library_client.as_ref(),
                Arc::clone(&self.version_strategy),
                &mut interaction,
            );
            let library_report = handler.update(libraries, stable_only)?;
            // Merge reports
            for (k, v) in library_report.library_updates {
                report.add_library_update(k, v.0, v.1);
            }
        }

        // Update [plugins] section
        if let Some(plugins) = doc.get_mut("plugins").and_then(|v| v.as_table_mut()) {
            let mut handler = PluginHandler::new(
                self.plugin_client.as_ref(),
                Arc::clone(&self.version_strategy),
                &mut interaction,
            );
            let plugin_report = handler.update(plugins, stable_only)?;
            // Merge reports
            for (k, v) in plugin_report.plugin_updates {
                report.add_plugin_update(k, v.0, v.1);
            }
        }

        // Write back the updated document
        if !report.is_empty() {
            context.save_document(&doc)?;
        }

        Ok(report)
    }

    /// Update a specific dependency by pattern
    ///
    /// This method finds dependencies matching the given pattern and
    /// allows the user to select which one to update.
    pub fn update_targeted_dependency<P: AsRef<Path>>(
        &self,
        catalog_path: P,
        stable_only: bool,
        interactive: bool,
        pattern: &str,
    ) -> Result<UpdateReport> {
        let catalog_path = catalog_path.as_ref();
        let context = UpdateContext::new(
            catalog_path,
            crate::agents::update::context::UpdateType::Targeted,
            stable_only,
            interactive,
        );

        let mut doc = context.load_document()?;
        let mut interaction = UpdateInteraction::new(interactive);

        let mut handler = TargetedHandler::new(
            self.library_client.as_ref(),
            self.plugin_client.as_ref(),
            Arc::clone(&self.version_strategy),
            &mut interaction,
        );

        let report = handler.update(&mut doc, stable_only, pattern)?;

        // Write back the updated document
        if !report.is_empty() {
            context.save_document(&doc)?;
        }

        Ok(report)
    }
}
