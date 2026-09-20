/// Application layer: the `CurrencyService` that orchestrates the aggregates.
pub mod service;

pub use service::{CurrencyService, ResolvedRate};
