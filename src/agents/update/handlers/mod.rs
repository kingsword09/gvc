// Handler modules for different update operations
//
// Each handler is responsible for a specific type of update operation.
// They follow a consistent interface and can be composed together.

pub mod library_handler;
pub mod plugin_handler;
pub mod targeted_handler;
pub mod version_handler;

pub use library_handler::LibraryHandler;
pub use plugin_handler::PluginHandler;
pub use targeted_handler::TargetedHandler;
pub use version_handler::VersionHandler;
