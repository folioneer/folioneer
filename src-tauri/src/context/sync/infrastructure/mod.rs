//! Infrastructure layer of the sync bounded context (B0/B38, flat-first per B41).

/// `SqliteChangeRecorder` — the SQLite-backed `ChangeRecorder` (PR-A, D1) — and
/// `SqliteChangeLogRepository`, the publish run's and the enrolment's access to the change log.
pub mod change_log;
/// Header / manifest / segment serialization + the data format version gate (SYN-035, D8).
pub mod codec;
/// Argon2id key derivation + XChaCha20-Poly1305 seal/open + passphrase check (SYN-050/051/055).
pub mod crypto;
/// `SqliteSyncStateRepository` — device singleton, cursors, held-back changes, notices.
pub mod device;
/// `FsFolderStore` — filesystem layout, temp-then-rename publishing (SYN-030/031/032).
pub mod folder_store;

pub use change_log::{RecordedChangeHook, SqliteChangeLogRepository, SqliteChangeRecorder};
pub use device::SqliteSyncStateRepository;
pub use folder_store::FsFolderStore;

/// Parses a stored enum column; an unknown value is logged under `table` and surfaces as
/// `DatabaseError`.
fn parse_stored<T: std::str::FromStr>(
    table: &'static str,
    column: &'static str,
    value: &str,
) -> Result<T, crate::context::sync::error::SyncError> {
    T::from_str(value).map_err(|_| {
        tracing::error!(target: crate::core::logger::BACKEND, table, column, value, "unknown stored value");
        crate::context::sync::error::SyncError::DatabaseError
    })
}
