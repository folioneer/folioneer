//! Update checker — detects, downloads, and installs application updates.
//!
//! Exposes three Tauri commands (`check_for_update`, `download_update`,
//! `install_update`) and the shared [`UpdateState`] that must be managed
//! via `app_handle.manage()` at startup.

pub mod api;
pub mod error;
pub mod service;

pub use api::*;
pub use error::UpdateError;
pub use service::{UpdateInfo, UpdateState};
