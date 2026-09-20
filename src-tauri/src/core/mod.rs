//! Core system utilities and shared components.

/// Application-wide event system.
pub mod event_bus;
pub use event_bus::{Event, SideEffectEventBus};

/// SQLite database connection and migrations.
mod db;
pub use db::Database;

/// Cross-context cash constants (CSH-014, CSH-017).
pub mod cash;

/// Logging infrastructure and frontend log bridge.
pub mod logger;
pub use logger::{BACKEND, FRONTEND};

/// Specta type serialization documentation.
pub mod specta_types;

/// Tauri-Specta builder configuration.
mod specta_builder;
pub use specta_builder::create_specta_builder;
