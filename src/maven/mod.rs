pub mod repository;
pub mod version;

pub use repository::{MavenRepository, DependencyMetadata, parse_maven_coordinate};
pub use version::{Version, VersionComparator};
