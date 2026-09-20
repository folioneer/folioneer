//! Application layer of the sync bounded context (B0/B38): the first-device enrolment, the
//! join rebuild, the sync run with its apply executor (the resolution engine's only
//! consumer), the settling-interval batcher, and the device-lifecycle service.

/// The apply executor — carries out the resolution engine's decisions (D4).
pub mod apply;
/// Enrolling as the first device of a new shared portfolio (SYN-013/026/081).
pub mod first_publish;
/// The intake of one run: other devices' manifests, segments and cursors (SYN-033/034/037).
mod intake;
/// Joining a portfolio another device created — the full-history rebuild (SYN-014/036/080).
mod join;
/// Settling-interval batching, 5s / 30s cap (SYN-067).
pub mod publisher;
/// One sync run: publish, read, apply (SYN-060/061/065/067/069).
pub mod run;
/// `SyncService` — device lifecycle, status assembly, notice dismissal.
pub mod service;

pub use first_publish::FirstPublish;
pub use join::JoinError;
pub use publisher::Publisher;
pub use run::{SyncGate, SyncRun};
pub use service::SyncService;
