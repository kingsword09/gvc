pub mod catalog_auditor;
pub mod catalog_editor;
pub mod catalog_explainer;
pub mod dependency_updater;
pub mod doctor;
pub mod project_scanner;
pub mod version_control;

// New refactored update module
pub mod update;
pub use update::UpdateReport;

pub use catalog_auditor::CatalogAuditor;
pub use catalog_editor::{AddResult, AddTargetKind, CatalogEditor};
pub use catalog_explainer::{
    CatalogExplainer, WhyEntryKind, WhyMatchKind, WhyReport, WhyVersionSource,
};
pub use dependency_updater::DependencyUpdater;
pub use doctor::{DoctorReport, DoctorSeverity, KotlinDoctor};
pub use project_scanner::ProjectScannerAgent;
pub use version_control::VersionControlAgent;
