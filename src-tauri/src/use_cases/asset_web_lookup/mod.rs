//! Asset Web Lookup — OpenFIGI search to pre-fill the Add Asset form (WEB).
//!
//! Exposes one Tauri command ([`lookup_asset`]) and the supporting types
//! ([`AssetLookupResult`], [`WebLookupError`], [`AssetWebLookupUseCase`]).
//! The concrete HTTP client ([`ReqwestOpenFigiClient`]) is also re-exported so
//! that `lib.rs` can wire it into the Tauri state at startup.

pub mod api;
/// Typed application-layer error for the web-lookup use case.
pub mod error;
pub mod orchestrator;
pub mod primary_listing_processor;

pub use api::*;
pub use error::WebLookupError;
pub use orchestrator::{AssetWebLookupUseCase, LookupMode, OpenFigiClient, ReqwestOpenFigiClient};
pub use primary_listing_processor::AssetLookupResult;
