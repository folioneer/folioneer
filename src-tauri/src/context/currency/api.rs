// The `#[tauri::command]` expansion generates an `unreachable!` arm that the
// crate-level `deny(clippy::unreachable)` would reject; allow it at this boundary
// module, consistent with the other BCs' `api.rs`.
#![allow(clippy::unreachable)]

use crate::context::currency::application::CurrencyService;
use crate::context::currency::domain::{CurrencyPair, CurrencyPairSummary, CurrencyRate};
use crate::context::currency::error::CurrencyError;

use std::result::Result as StdResult;
use std::sync::Arc;

/// Converts a human-readable decimal rate (`f64`) to i64 micros at the IPC
/// boundary (FXR-024, ADR-001). Returns `CurrencyError::NonFinite` when the
/// value is not finite, or `CurrencyError::NotPositive` when it would produce
/// zero or negative micros after conversion. Rounds to the nearest micro so a
/// human decimal like `0.92` maps to `920_000` rather than a truncated unit.
fn rate_f64_to_micros(rate: f64) -> StdResult<i64, CurrencyError> {
    if !rate.is_finite() {
        return Err(CurrencyError::NonFinite);
    }
    let micros = (rate * 1_000_000.0).round() as i64;
    if micros <= 0 {
        return Err(CurrencyError::NotPositive);
    }
    Ok(micros)
}

/// Declares a currency pair (FXR-054). Idempotent: returns the existing pair
/// rather than duplicating it.
#[tauri::command]
#[specta::specta]
pub async fn declare_currency_pair(
    svc: tauri::State<'_, Arc<CurrencyService>>,
    from_currency: String,
    to_currency: String,
) -> StdResult<CurrencyPair, CurrencyError> {
    svc.declare_currency_pair(from_currency, to_currency).await
}

/// Records a rate for a pair (FXR-025). Converts `rate: f64` to i64 micros
/// at the IPC boundary (FXR-024). Sets `source = Manual` (FXR-101).
/// Ensures the pair exists as a side-effect (FXR-013).
#[tauri::command]
#[specta::specta]
pub async fn record_currency_rate(
    svc: tauri::State<'_, Arc<CurrencyService>>,
    from_currency: String,
    to_currency: String,
    date: String,
    rate: f64,
) -> StdResult<CurrencyRate, CurrencyError> {
    let rate_micros = rate_f64_to_micros(rate)?;
    svc.record_currency_rate(from_currency, to_currency, date, rate_micros)
        .await
}

/// Updates an existing rate (FXR-052). Same-date = in-place overwrite;
/// changed-date = delete-old + upsert-new. Returns `RateNotFound` when the
/// original record does not exist.
#[tauri::command]
#[specta::specta]
pub async fn update_currency_rate(
    svc: tauri::State<'_, Arc<CurrencyService>>,
    from_currency: String,
    to_currency: String,
    original_date: String,
    new_date: String,
    new_rate: f64,
) -> StdResult<(), CurrencyError> {
    let new_rate_micros = rate_f64_to_micros(new_rate)?;
    svc.update_currency_rate(
        from_currency,
        to_currency,
        original_date,
        new_date,
        new_rate_micros,
    )
    .await
}

/// Deletes the rate at `(from_currency, to_currency, date)` (FXR-053).
/// Returns `RateNotFound` when absent; never removes the pair (FXR-014).
#[tauri::command]
#[specta::specta]
pub async fn delete_currency_rate(
    svc: tauri::State<'_, Arc<CurrencyService>>,
    from_currency: String,
    to_currency: String,
    date: String,
) -> StdResult<(), CurrencyError> {
    svc.delete_currency_rate(from_currency, to_currency, date)
        .await
}

/// Returns all pairs enriched with their most-recent rate (FXR-051).
#[tauri::command]
#[specta::specta]
pub async fn get_currency_pairs(
    svc: tauri::State<'_, Arc<CurrencyService>>,
) -> StdResult<Vec<CurrencyPairSummary>, CurrencyError> {
    svc.list_currency_pairs().await
}

/// Returns all rates for the given pair ordered by date descending (FXR-050).
/// Returns an empty list for an unknown pair — never `RateNotFound`.
#[tauri::command]
#[specta::specta]
pub async fn get_currency_rates(
    svc: tauri::State<'_, Arc<CurrencyService>>,
    from_currency: String,
    to_currency: String,
) -> StdResult<Vec<CurrencyRate>, CurrencyError> {
    svc.list_currency_rates(from_currency, to_currency).await
}

#[cfg(test)]
mod tests {
    use super::*;

    // FXR-024 — rate_f64_to_micros converts a valid decimal to i64 micros
    #[test]
    fn rate_f64_to_micros_converts_valid_decimal() {
        assert_eq!(rate_f64_to_micros(0.92).unwrap(), 920_000);
        assert_eq!(rate_f64_to_micros(1.0).unwrap(), 1_000_000);
        assert_eq!(rate_f64_to_micros(1.234567).unwrap(), 1_234_567);
    }

    // FXR-021 (NonFinite path) — rate_f64_to_micros rejects NaN
    #[test]
    fn rate_f64_to_micros_rejects_nan() {
        let err = rate_f64_to_micros(f64::NAN).unwrap_err();
        assert!(matches!(err, CurrencyError::NonFinite), "got: {err:?}");
    }

    // FXR-021 (NonFinite path) — rate_f64_to_micros rejects positive infinity
    #[test]
    fn rate_f64_to_micros_rejects_pos_infinity() {
        let err = rate_f64_to_micros(f64::INFINITY).unwrap_err();
        assert!(matches!(err, CurrencyError::NonFinite), "got: {err:?}");
    }

    // FXR-021 (NonFinite path) — rate_f64_to_micros rejects negative infinity
    #[test]
    fn rate_f64_to_micros_rejects_neg_infinity() {
        let err = rate_f64_to_micros(f64::NEG_INFINITY).unwrap_err();
        assert!(matches!(err, CurrencyError::NonFinite), "got: {err:?}");
    }

    // FXR-021 (NotPositive path) — rate_f64_to_micros rejects zero
    #[test]
    fn rate_f64_to_micros_rejects_zero() {
        let err = rate_f64_to_micros(0.0).unwrap_err();
        assert!(matches!(err, CurrencyError::NotPositive), "got: {err:?}");
    }

    // FXR-021 (NotPositive path) — rate_f64_to_micros rejects negative values
    #[test]
    fn rate_f64_to_micros_rejects_negative() {
        let err = rate_f64_to_micros(-0.5).unwrap_err();
        assert!(matches!(err, CurrencyError::NotPositive), "got: {err:?}");
    }
}
