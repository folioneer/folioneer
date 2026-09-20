use crate::context::currency::domain::rate_provider::{
    EurSnapshot, RateHistoryProvider, RateProvider,
};
use crate::context::currency::domain::CurrencyRateSource;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Duration;

const FRANKFURTER_URL: &str = "https://api.frankfurter.dev/v1/latest?base=EUR";
/// Frankfurter date-range endpoint (verified live 2026-07-12, FXR-070/SPF-036):
/// `/v1/{from}..{to}?base=EUR`.
const FRANKFURTER_RANGE_URL_TEMPLATE: &str = "https://api.frankfurter.dev/v1/{from}..{to}?base=EUR";
const REQUEST_TIMEOUT_SECS: u64 = 10;
/// A dated range response carries every published date in the span — a
/// decade of daily full-table rates is under 2 MiB, so 8 MiB bounds memory
/// against a hostile server while never truncating an honest payload.
const MAX_RANGE_BODY_BYTES: usize = 8 * 1024 * 1024;
/// Range bodies are hundreds of KiB; on a slow connection the snapshot
/// timeout would cut the download mid-body.
const RANGE_TIMEOUT_SECS: u64 = 30;
const MICROS_PER_UNIT: f64 = 1_000_000.0;

/// Production [`RateProvider`] backed by the Frankfurter JSON endpoint (ADR-009, FXR-070).
pub struct ReqwestFrankfurterClient {
    client: reqwest::Client,
}

impl ReqwestFrankfurterClient {
    /// Creates a new client with a 10-second per-request timeout. Fails only if
    /// the TLS backend cannot be initialised — an unrecoverable environment
    /// fault surfaced at startup rather than panicked on.
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .context("building the Frankfurter HTTP client")?;
        Ok(Self { client })
    }
}

#[async_trait]
impl RateProvider for ReqwestFrankfurterClient {
    async fn fetch_eur_snapshot(&self) -> Result<EurSnapshot> {
        let response = self
            .client
            .get(FRANKFURTER_URL)
            .send()
            .await
            .context("Frankfurter fetch request failed")?;
        if !response.status().is_success() {
            anyhow::bail!("Frankfurter returned status {}", response.status());
        }
        let body = crate::shared::infrastructure::http::read_capped_text(response)
            .await
            .context("Frankfurter response read failed")?;
        parse_frankfurter_snapshot(&body)
    }
}

#[async_trait]
impl RateHistoryProvider for ReqwestFrankfurterClient {
    async fn fetch_eur_range(&self, from: &str, to: &str) -> Result<Vec<EurSnapshot>> {
        // The raw substitution below must never receive anything but strict
        // ISO dates — guarded here so the outbound URL stays injection-proof
        // even if a future write path skips the domain's date validation.
        for date in [from, to] {
            chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .with_context(|| format!("fetch_eur_range: not an ISO date: {date}"))?;
        }
        let url = FRANKFURTER_RANGE_URL_TEMPLATE
            .replace("{from}", from)
            .replace("{to}", to);
        let response = self
            .client
            .get(url)
            .timeout(Duration::from_secs(RANGE_TIMEOUT_SECS))
            .send()
            .await
            .context("Frankfurter range fetch request failed")?;
        if !response.status().is_success() {
            anyhow::bail!("Frankfurter returned status {}", response.status());
        }
        let body = crate::shared::infrastructure::http::read_text_capped_at(
            response,
            MAX_RANGE_BODY_BYTES,
        )
        .await
        .context("Frankfurter range response read failed")?;
        parse_frankfurter_range(&body)
    }
}

/// Frankfurter JSON response shape (`{"date":"…","rates":{"USD":1.16,…}}`); the
/// `amount`/`base` fields are present but unused (we always request `base=EUR`).
#[derive(serde::Deserialize)]
struct FrankfurterResponse {
    date: String,
    rates: HashMap<String, f64>,
}

/// Parses the Frankfurter JSON response body into an `EurSnapshot` (FXR-070).
///
/// Expected shape: `{"amount":1.0,"base":"EUR","date":"YYYY-MM-DD","rates":{...}}`.
/// Rates are converted to i64 micros: `(value * 1_000_000.0).round() as i64`.
/// EUR itself is NOT present in the returned map (it is the implicit base).
pub(crate) fn parse_frankfurter_snapshot(body: &str) -> Result<EurSnapshot> {
    let parsed: FrankfurterResponse =
        serde_json::from_str(body).context("Frankfurter JSON parse failed")?;
    let rates = parsed
        .rates
        .into_iter()
        .map(|(code, value)| {
            // Reject non-finite or non-positive rates from an anomalous/compromised
            // feed before the micros cast (mirrors the Yahoo client guard).
            if !value.is_finite() || value <= 0.0 {
                anyhow::bail!("Frankfurter rate out of range for {code}: {value}");
            }
            Ok((code, (value * MICROS_PER_UNIT).round() as i64))
        })
        .collect::<Result<_>>()?;
    Ok(EurSnapshot {
        date: parsed.date,
        rates,
        source: CurrencyRateSource::Frankfurter,
    })
}

/// Frankfurter date-range JSON response shape:
/// `{"amount":1.0,"base":"EUR","start_date":"…","end_date":"…","rates":{"2026-07-01":{"USD":1.1383,…},…}}`.
/// Weekend/holiday days are simply absent keys in `rates` (SPF-037).
#[derive(serde::Deserialize)]
struct FrankfurterRangeResponse {
    rates: HashMap<String, HashMap<String, f64>>,
}

/// Parses the Frankfurter date-range JSON response body into one [`EurSnapshot`]
/// per published day (SPF-036), mirroring [`parse_frankfurter_snapshot`]'s
/// per-rate validation and micros conversion. Days absent from the response
/// (weekends, ECB holidays) simply produce no entry (SPF-037) — not an error.
pub(crate) fn parse_frankfurter_range(body: &str) -> Result<Vec<EurSnapshot>> {
    let parsed: FrankfurterRangeResponse =
        serde_json::from_str(body).context("Frankfurter range JSON parse failed")?;
    parsed
        .rates
        .into_iter()
        .map(|(date, day_rates)| {
            let rates = day_rates
                .into_iter()
                .map(|(code, value)| {
                    // Reject non-finite or non-positive rates from an anomalous/compromised
                    // feed before the micros cast (mirrors parse_frankfurter_snapshot).
                    if !value.is_finite() || value <= 0.0 {
                        anyhow::bail!(
                            "Frankfurter rate out of range for {code} on {date}: {value}"
                        );
                    }
                    Ok((code, (value * MICROS_PER_UNIT).round() as i64))
                })
                .collect::<Result<_>>()?;
            Ok(EurSnapshot {
                date,
                rates,
                source: CurrencyRateSource::Frankfurter,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::currency::domain::CurrencyRateSource;

    const FIXTURE: &str = r#"{"amount":1.0,"base":"EUR","date":"2026-06-01","rates":{"USD":1.1646,"GBP":0.86493,"JPY":185.74}}"#;

    // The constructor builds a reqwest client successfully in a normal environment.
    #[test]
    fn new_builds_a_client() {
        assert!(ReqwestFrankfurterClient::new().is_ok());
    }

    // FXR-070 — parses date and source from the Frankfurter JSON body
    #[test]
    fn parse_frankfurter_snapshot_parses_date_and_source() {
        let snapshot = parse_frankfurter_snapshot(FIXTURE).expect("should parse");
        assert_eq!(snapshot.date, "2026-06-01");
        assert_eq!(snapshot.source, CurrencyRateSource::Frankfurter);
    }

    // FXR-070 — USD rate is converted to 1_164_600 micros
    #[test]
    fn parse_frankfurter_snapshot_usd_rate_to_micros() {
        let snapshot = parse_frankfurter_snapshot(FIXTURE).expect("should parse");
        assert_eq!(snapshot.rates.get("USD").copied(), Some(1_164_600));
    }

    // FXR-070 — GBP rate is converted to 864_930 micros
    #[test]
    fn parse_frankfurter_snapshot_gbp_rate_to_micros() {
        let snapshot = parse_frankfurter_snapshot(FIXTURE).expect("should parse");
        assert_eq!(snapshot.rates.get("GBP").copied(), Some(864_930));
    }

    // FXR-070 — JPY rate is converted to 185_740_000 micros
    #[test]
    fn parse_frankfurter_snapshot_jpy_rate_to_micros() {
        let snapshot = parse_frankfurter_snapshot(FIXTURE).expect("should parse");
        assert_eq!(snapshot.rates.get("JPY").copied(), Some(185_740_000));
    }

    // FXR-070 — EUR is NOT present in the parsed rates map (it is the implicit base)
    #[test]
    fn parse_frankfurter_snapshot_eur_not_in_rates_map() {
        let snapshot = parse_frankfurter_snapshot(FIXTURE).expect("should parse");
        assert!(
            !snapshot.rates.contains_key("EUR"),
            "EUR must not appear as an explicit key in the rates map"
        );
    }

    // FXR-070 — malformed JSON body → Err
    #[test]
    fn parse_frankfurter_snapshot_malformed_json_returns_err() {
        let result = parse_frankfurter_snapshot("not valid json {{");
        assert!(result.is_err(), "malformed JSON must return Err");
    }

    // Hardening — a negative rate from an anomalous/compromised feed is rejected
    // before the micros cast rather than stored as a negative rate.
    #[test]
    fn parse_frankfurter_snapshot_rejects_negative_rate() {
        let body = r#"{"amount":1.0,"base":"EUR","date":"2026-06-01","rates":{"USD":-1.1646}}"#;
        let result = parse_frankfurter_snapshot(body);
        assert!(result.is_err(), "negative rate must return Err");
    }

    // Hardening — a zero rate is rejected (would later divide-by-zero in cross-rate).
    #[test]
    fn parse_frankfurter_snapshot_rejects_zero_rate() {
        let body = r#"{"amount":1.0,"base":"EUR","date":"2026-06-01","rates":{"USD":0.0}}"#;
        let result = parse_frankfurter_snapshot(body);
        assert!(result.is_err(), "zero rate must return Err");
    }

    // -------------------------------------------------------------------------
    // parse_frankfurter_range (SPF-036/037)
    // -------------------------------------------------------------------------

    const RANGE_FIXTURE: &str = r#"{"amount":1.0,"base":"EUR","start_date":"2026-06-29","end_date":"2026-07-01","rates":{"2026-06-29":{"USD":1.1400},"2026-07-01":{"USD":1.1383}}}"#;

    // SPF-036 — one EurSnapshot per published date in the range response.
    #[test]
    fn parse_frankfurter_range_returns_one_snapshot_per_date() {
        let snapshots = parse_frankfurter_range(RANGE_FIXTURE).expect("should parse");
        assert_eq!(snapshots.len(), 2, "one snapshot per published date");
        assert!(snapshots.iter().any(|s| s.date == "2026-06-29"));
        assert!(snapshots.iter().any(|s| s.date == "2026-07-01"));
    }

    // SPF-037 — a weekend day absent from the response produces no snapshot
    // for that date (not represented at all — not an error).
    #[test]
    fn parse_frankfurter_range_omits_weekend_days_entirely() {
        // 2026-06-30 (Tuesday) is present; the fixture already omits the
        // weekend of 2026-06-27/28 — asserting the count confirms no synthetic
        // gap-filling happens.
        let snapshots = parse_frankfurter_range(RANGE_FIXTURE).expect("should parse");
        assert!(
            !snapshots.iter().any(|s| s.date == "2026-06-27"),
            "a non-published date must never appear in the result (SPF-037)"
        );
    }

    // FXR-102 — every parsed snapshot is stamped source = Frankfurter.
    #[test]
    fn parse_frankfurter_range_stamps_frankfurter_source() {
        let snapshots = parse_frankfurter_range(RANGE_FIXTURE).expect("should parse");
        assert!(snapshots
            .iter()
            .all(|s| s.source == CurrencyRateSource::Frankfurter));
    }

    // The USD rate for 2026-07-01 is converted to micros, mirroring parse_frankfurter_snapshot.
    #[test]
    fn parse_frankfurter_range_converts_rate_to_micros() {
        let snapshots = parse_frankfurter_range(RANGE_FIXTURE).expect("should parse");
        let day = snapshots
            .iter()
            .find(|s| s.date == "2026-07-01")
            .expect("2026-07-01 snapshot must be present");
        assert_eq!(day.rates.get("USD").copied(), Some(1_138_300));
    }

    // Malformed JSON body → Err.
    #[test]
    fn parse_frankfurter_range_malformed_json_returns_err() {
        assert!(parse_frankfurter_range("not valid json {{").is_err());
    }
}
