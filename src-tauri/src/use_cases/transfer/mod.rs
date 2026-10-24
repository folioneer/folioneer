//! Transfer use case: a holding moved from one account to another at its cost (TRF).
//!
//! Checks the asset in the asset context and resolves the default rate in the currency
//! context, then lets the account context place both sides in one write.

/// Tauri command handlers for the transfer.
#[cfg(feature = "app")]
mod api;
/// Use-case-owned typed errors (composite + application leaf).
mod error;
/// Cross-BC orchestrator.
mod orchestrator;

#[cfg(feature = "app")]
pub use api::*;
pub use error::{TransferError, TransferTask};
pub use orchestrator::{AgentTransferError, TransferFigures, TransferUseCase};
