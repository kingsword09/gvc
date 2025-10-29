pub mod catalog_editor;
pub mod dependency_updater;
pub mod project_scanner;
pub mod version_control;

// New refactored update module
pub mod update;
pub use update::UpdateReport;

pub use catalog_editor::{AddResult, AddTargetKind, CatalogEditor};
pub use dependency_updater::DependencyUpdater;
pub use project_scanner::ProjectScannerAgent;
pub use version_control::VersionControlAgent;
