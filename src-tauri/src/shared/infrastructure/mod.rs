//! Shared infrastructure adapters reused by multiple bounded contexts.

/// `ChangeRecorder` port — every synced repository write appends a change through it
/// (SYN-020, ADR-019).
pub mod change_recorder;
/// Composition root wiring repositories into application services.
pub mod container;
/// The isolated data folder an E2E run injects (debug builds only).
pub mod e2e_run;
/// Outbound HTTP response helpers.
pub mod http;
/// Daily fetch scheduler abstraction + platform adapters (SPF-012, SPF-017).
pub mod scheduler;
