// Update module - handles dependency version updates with a clean, modular architecture
//
// This module is part of the refactoring effort to break down the large
// DependencyUpdater into focused, single-responsibility components.
//
// Architecture:
// - UpdateContext: Provides context for update operations
// - UpdateReport: Tracks changes made during updates
// - Handlers: Specific handlers for different update types (versions, libraries, plugins)
// - UpdateInteraction: Manages user interaction for interactive updates
pub mod context;
pub mod handlers;
pub mod interaction;

pub use context::UpdateReport;
// Note: UpdateContext and UpdateType are intentionally not exported
// as they're internal implementation details
