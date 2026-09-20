use crate::context::asset::domain::{DatedClose, PriceProvider, Quote};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::time::Duration;

/// Yahoo Finance chart endpoint (ADR-017). Keyless, no proof-of-work; returns
/// JSON with the latest price in `chart.result[0].meta`. The `/v8/chart/` path
/// does not require the cookie/"crumb" handshake of Yahoo's `/v7/quote` endpoint.
const YAHOO_CHART_URL: &str = "https://query1.finance.yahoo.com/v8/finance/chart/";
/// Yahoo rejects the default reqwest user agent; present a browser-like one.
const USER_AGENT: &str = "Mozilla/5.0";
const REQUEST_TIMEOUT_SECS: u64 = 10;
const SECONDS_PER_DAY: i64 = 86_400;
const MICROS_PER_UNIT: f64 = 1_000_000.0;
/// Minor-unit divisor for pence/cents/agorot quotes (MKT-125).
const MINOR_UNIT_DIVISOR: f64 = 100.0;

/// Production [`PriceProvider`] backed by the keyless Yahoo Finance chart endpoint
/// (ADR-017).
pub struct ReqwestYahooClient {
    client: reqwest::Client,
}

impl ReqwestYahooClient {
    /// Creates a new client with a 10-second per-request timeout. Fails only if
    /// the TLS backend cannot be initialised — an unrecoverable environment
    /// fault surfaced at startup rather than panicked on.
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .context("building the Yahoo Finance HTTP client")?;
        Ok(Self { client })
    }
}

#[async_trait]
impl PriceProvider for ReqwestYahooClient {
    async fn fetch_price(&self, symbol: &str) -> Result<Option<Quote>> {
        let url = format!("{YAHOO_CHART_URL}{symbol}?interval=1d&range=1d");
        let response = self
            .client
            .get(&url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await
            .with_context(|| format!("Yahoo fetch request failed for symbol: {symbol}"))?;
        // Yahoo returns 200 with a JSON `chart.error` for an unknown symbol; only a
        // genuine transport failure is a non-success status.
        if !response.status().is_success() {
            anyhow::bail!("Yahoo returned status {} for {symbol}", response.status());
        }
        let body = crate::shared::infrastructure::http::read_capped_text(response)
            .await
            .with_context(|| format!("Yahoo response read failed for symbol: {symbol}"))?;
        parse_quote(&body)
            .with_context(|| format!("Yahoo response parse failed for symbol: {symbol}"))
    }

    /// Fetches the daily-close series for the scheduled-fetch backfill window
    /// (SPF-030/031) via `/v8/chart/?period1=…&period2=…&interval=1d`.
    async fn fetch_daily_closes(
        &self,
        symbol: &str,
        from: &str,
        to: &str,
    ) -> anyhow::Result<Option<Vec<DatedClose>>> {
        let period1 = utc_day_start_epoch_seconds(from)?;
        // period2 is the end of `to` (23:59:59 UTC) so the range is inclusive.
        let period2 = utc_day_start_epoch_seconds(to)? + SECONDS_PER_DAY - 1;
        let url =
            format!("{YAHOO_CHART_URL}{symbol}?period1={period1}&period2={period2}&interval=1d");
        let response = self
            .client
            .get(&url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await
            .with_context(|| format!("Yahoo daily-close request failed for symbol: {symbol}"))?;
        // Yahoo returns 200 with a JSON `chart.error` for an unknown symbol; only a
        // genuine transport failure is a non-success status.
        if !response.status().is_success() {
            anyhow::bail!("Yahoo returned status {} for {symbol}", response.status());
        }
        let body = crate::shared::infrastructure::http::read_capped_text(response)
            .await
            .with_context(|| {
                format!("Yahoo daily-close response read failed for symbol: {symbol}")
            })?;
        parse_daily_closes(&body).with_context(|| {
            format!("Yahoo daily-close response parse failed for symbol: {symbol}")
        })
    }
}

/// Yahoo `/v8/chart/` response shape — only the fields we consume.
#[derive(serde::Deserialize)]
struct ChartEnvelope {
    chart: Chart,
}

#[derive(serde::Deserialize)]
struct Chart {
    result: Option<Vec<ChartResult>>,
    // Present (non-null) for an unknown / delisted symbol; its presence alone is
    // the skip signal (MKT-114), so the inner `{code, description}` is not modeled.
    error: Option<serde_json::Value>,
}

#[derive(serde::Deserialize)]
struct ChartResult {
    meta: Meta,
    /// UNIX timestamps, one per trading day in the requested range (SPF-030/031).
    /// Absent for the single-quote `parse_quote` request shape.
    timestamp: Option<Vec<i64>>,
    /// Parallel `close[]` series lives under `indicators.quote[0]`.
    indicators: Option<Indicators>,
}

#[derive(serde::Deserialize)]
struct Indicators {
    quote: Vec<QuoteSeries>,
}

#[derive(serde::Deserialize)]
struct QuoteSeries {
    /// Daily close, parallel to `ChartResult::timestamp`. `null` for a
    /// non-trading day within the requested range (SPF-032). Absent entirely
    /// (`"quote":[{}]`) for a thinly-traded listing with no bars in the
    /// window — the meta price is still usable, so the field must not make
    /// deserialization fail.
    #[serde(default)]
    close: Vec<Option<f64>>,
}

#[derive(serde::Deserialize)]
struct Meta {
    #[serde(rename = "regularMarketPrice")]
    regular_market_price: Option<f64>,
    currency: Option<String>,
    #[serde(rename = "regularMarketTime")]
    regular_market_time: Option<i64>,
    gmtoffset: Option<i64>,
}

/// Parses the latest quote from a Yahoo chart JSON body.
///
/// - `Ok(Some(quote))` — usable price (normalized to the major ISO unit per
///   MKT-125) with its observation date.
/// - `Ok(None)` — symbol not found (`chart.error` present) or no price in the
///   response; a quiet per-asset skip (MKT-114).
/// - `Err(_)` — malformed JSON or an anomalous (non-finite / non-positive) price.
fn parse_quote(body: &str) -> Result<Option<Quote>> {
    let envelope: ChartEnvelope =
        serde_json::from_str(body).context("Yahoo chart JSON parse failed")?;
    // Unknown / delisted symbol — Yahoo populates `error` and nulls `result`.
    if envelope.chart.error.is_some() {
        return Ok(None);
    }
    let Some(meta) = envelope
        .chart
        .result
        .and_then(|mut results| results.drain(..).next())
        .map(|r| r.meta)
    else {
        return Ok(None);
    };
    let Some(raw_price) = meta.regular_market_price else {
        return Ok(None);
    };
    // MKT-125 — pence/cents/agorot quotes are divided by 100 to the major ISO unit.
    let price = normalize_minor_unit(raw_price, meta.currency.as_deref());
    if !price.is_finite() || price <= 0.0 {
        return Err(anyhow!(
            "Yahoo price is non-finite or non-positive: {price}"
        ));
    }
    // Observation date (MKT-117): the regular-market timestamp shifted to the
    // exchange-local calendar date via gmtoffset. Validation/fallback is the
    // dispatcher's job (MKT-118); this forwards the raw date or `None`.
    let date = match meta.regular_market_time {
        Some(ts) => chrono::DateTime::from_timestamp(ts + meta.gmtoffset.unwrap_or(0), 0)
            .map(|dt| dt.naive_utc().date().format("%Y-%m-%d").to_string()),
        None => None,
    };
    Ok(Some(Quote {
        price: (price * MICROS_PER_UNIT).round() as i64,
        date,
    }))
}

/// MKT-125 — converts a price quoted in a currency's minor unit to its major ISO
/// unit. Yahoo reports London in `GBp` (pence), Johannesburg in `ZAc` (cents),
/// Tel Aviv in `ILA` (agorot). Any other currency is treated as already major.
fn normalize_minor_unit(price: f64, currency: Option<&str>) -> f64 {
    match currency {
        Some("GBp") | Some("ZAc") | Some("ILA") => price / MINOR_UNIT_DIVISOR,
        _ => price,
    }
}

/// Parses the daily-close series from a Yahoo chart JSON body covering a date
/// range (SPF-030/031).
///
/// - `Ok(Some(closes))` — the symbol is known: one [`DatedClose`] per completed
///   trading day in the response (MKT-125 sub-unit normalization applied);
///   non-trading days are simply absent from `timestamp[]`/`close[]` (SPF-032),
///   so a window without a trading day yields an empty series (MKT-196).
/// - `Ok(None)` — unknown/delisted symbol (`chart.error` present, MKT-114, MKT-196).
/// - `Err(_)` — malformed JSON.
fn parse_daily_closes(body: &str) -> Result<Option<Vec<DatedClose>>> {
    let envelope: ChartEnvelope =
        serde_json::from_str(body).context("Yahoo chart JSON parse failed")?;
    // Unknown / delisted symbol — Yahoo populates `error` and nulls `result` (MKT-114).
    if envelope.chart.error.is_some() {
        return Ok(None);
    }
    let Some(result) = envelope
        .chart
        .result
        .and_then(|mut results| results.drain(..).next())
    else {
        return Ok(Some(Vec::new()));
    };
    // Dates are the exchange-local calendar day via gmtoffset, UTC when absent
    // (same convention as `parse_quote`, MKT-117).
    let gmtoffset = result.meta.gmtoffset.unwrap_or(0);
    let currency = result.meta.currency;
    let timestamps = result.timestamp.unwrap_or_default();
    let close_series = result
        .indicators
        .and_then(|indicators| indicators.quote.into_iter().next())
        .map(|series| series.close)
        .unwrap_or_default();
    let mut daily_closes = Vec::new();
    for (timestamp, close) in timestamps.into_iter().zip(close_series) {
        // SPF-032 — a null close (non-trading day) produces no row.
        let Some(raw_close) = close else {
            continue;
        };
        // A non-finite or non-positive close is provider garbage, not data —
        // skipped like a null close (same guard as `parse_quote`, MKT-021).
        if !raw_close.is_finite() || raw_close <= 0.0 {
            continue;
        }
        // MKT-125 — pence/cents/agorot quotes are divided by 100 to the major ISO unit.
        let price = normalize_minor_unit(raw_close, currency.as_deref());
        let Some(date) = chrono::DateTime::from_timestamp(timestamp + gmtoffset, 0)
            .map(|datetime| datetime.naive_utc().date().format("%Y-%m-%d").to_string())
        else {
            continue;
        };
        daily_closes.push(DatedClose {
            date,
            price: (price * MICROS_PER_UNIT).round() as i64,
        });
    }
    Ok(Some(daily_closes))
}

/// Epoch seconds at 00:00:00 UTC on the given ISO `yyyy-mm-dd` date.
fn utc_day_start_epoch_seconds(date: &str) -> Result<i64> {
    let parsed = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .with_context(|| format!("invalid ISO date: {date}"))?;
    Ok(parsed
        .and_time(chrono::NaiveTime::MIN)
        .and_utc()
        .timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    // The constructor builds a reqwest client successfully in a normal environment.
    #[test]
    fn new_builds_a_client() {
        assert!(ReqwestYahooClient::new().is_ok());
    }

    // MKT-102/117 — a US quote: price to micros, date from the regular-market timestamp.
    #[test]
    fn parses_us_quote_price_and_date() {
        // regularMarketTime 1781294401 + gmtoffset -14400 → 2026-06-12 (EDT date).
        let body = r#"{"chart":{"result":[{"meta":{"currency":"USD","symbol":"AAPL","regularMarketPrice":291.13,"regularMarketTime":1781294401,"gmtoffset":-14400}}],"error":null}}"#;
        let quote = parse_quote(body).unwrap().expect("a usable quote");
        assert_eq!(quote.price, 291_130_000);
        assert_eq!(quote.date.as_deref(), Some("2026-06-12"));
    }

    // MKT-125 — a London (LSE) quote in GBp is divided by 100 to GBP.
    #[test]
    fn normalizes_gbp_pence_to_pounds() {
        let body = r#"{"chart":{"result":[{"meta":{"currency":"GBp","symbol":"VOD.L","regularMarketPrice":115.75,"regularMarketTime":1781287549,"gmtoffset":3600}}],"error":null}}"#;
        let quote = parse_quote(body).unwrap().expect("a usable quote");
        // 115.75 GBp = 1.1575 GBP → 1_157_500 micros.
        assert_eq!(quote.price, 1_157_500);
    }

    // MKT-125 — a major-ISO currency (USD) is stored unchanged (no division).
    #[test]
    fn major_currency_unchanged() {
        assert_eq!(normalize_minor_unit(291.13, Some("USD")), 291.13);
        assert_eq!(normalize_minor_unit(115.75, Some("GBp")), 1.1575);
        assert_eq!(normalize_minor_unit(50.0, Some("ZAc")), 0.5);
    }

    // MKT-114 — an unknown symbol (chart.error present) is a quiet skip (Ok(None)).
    #[test]
    fn unknown_symbol_returns_ok_none() {
        let body = r#"{"chart":{"result":null,"error":{"code":"Not Found","description":"No data found, symbol may be delisted"}}}"#;
        assert_eq!(parse_quote(body).unwrap(), None);
    }

    // No price in the response (null regularMarketPrice) is a quiet skip.
    #[test]
    fn missing_price_returns_ok_none() {
        let body =
            r#"{"chart":{"result":[{"meta":{"currency":"USD","symbol":"AAPL"}}],"error":null}}"#;
        assert_eq!(parse_quote(body).unwrap(), None);
    }

    // A thinly-traded listing with no bars in the window carries an EMPTY bar
    // series (`"quote":[{}]`) next to a usable meta price — the price must
    // parse (real-world shape: a Frankfurt-listed mutual fund).
    #[test]
    fn parses_meta_price_when_bar_series_is_empty() {
        let body = r#"{"chart":{"result":[{"meta":{"currency":"EUR","symbol":"0P0001RJOO.F","instrumentType":"MUTUALFUND","regularMarketPrice":136.29,"regularMarketTime":1784836800,"gmtoffset":7200},"indicators":{"quote":[{}],"adjclose":[{}]}}],"error":null}}"#;
        let quote = parse_quote(body).unwrap().expect("a usable quote");
        assert_eq!(quote.price, 136_290_000);
        assert_eq!(quote.date.as_deref(), Some("2026-07-23"));
    }

    // The same empty-bar-series shape in a range response yields zero daily
    // closes instead of a parse error (the scheduled fetch skips quietly).
    #[test]
    fn empty_bar_series_yields_no_daily_closes() {
        let body = r#"{"chart":{"result":[{"meta":{"currency":"EUR","symbol":"0P0001RJOO.F","gmtoffset":7200},"indicators":{"quote":[{}],"adjclose":[{}]}}],"error":null}}"#;
        assert!(parse_daily_closes(body)
            .unwrap()
            .expect("known symbol")
            .is_empty());
    }

    // A timestamp without gmtoffset still yields a date (offset defaults to 0/UTC).
    #[test]
    fn timestamp_without_gmtoffset_uses_utc() {
        let body = r#"{"chart":{"result":[{"meta":{"currency":"USD","symbol":"AAPL","regularMarketPrice":100.0,"regularMarketTime":1781294401}}],"error":null}}"#;
        let quote = parse_quote(body).unwrap().expect("a usable quote");
        assert!(quote.date.is_some());
    }

    // Malformed JSON is an Err.
    #[test]
    fn malformed_json_returns_err() {
        assert!(parse_quote("not json {{").is_err());
    }

    // A non-positive price from an anomalous feed is rejected.
    #[test]
    fn rejects_non_positive_price() {
        let body = r#"{"chart":{"result":[{"meta":{"currency":"USD","symbol":"AAPL","regularMarketPrice":0.0,"regularMarketTime":1781294401,"gmtoffset":0}}],"error":null}}"#;
        assert!(parse_quote(body).is_err());
    }

    // -------------------------------------------------------------------------
    // parse_daily_closes (SPF-030/031/032/125)
    // -------------------------------------------------------------------------

    // SPF-030/031 — a normal 3-day US range parses one DatedClose per trading day.
    #[test]
    fn parses_a_daily_close_series() {
        let body = r#"{"chart":{"result":[{
            "meta":{"currency":"USD","symbol":"AAPL"},
            "timestamp":[1781121600,1781208000,1781294400],
            "indicators":{"quote":[{"close":[290.0,291.0,292.0]}]}
        }],"error":null}}"#;
        let closes = parse_daily_closes(body).unwrap().expect("known symbol");
        assert_eq!(closes.len(), 3, "one DatedClose per trading day");
        assert_eq!(closes[0].price, 290_000_000);
        assert_eq!(closes[2].price, 292_000_000);
    }

    // SPF-032 — a `null` close (non-trading day inside the requested range)
    // produces no row for that day; the day is simply absent from the result.
    #[test]
    fn null_close_entries_produce_no_row() {
        let body = r#"{"chart":{"result":[{
            "meta":{"currency":"USD","symbol":"AAPL"},
            "timestamp":[1781121600,1781208000,1781294400],
            "indicators":{"quote":[{"close":[290.0,null,292.0]}]}
        }],"error":null}}"#;
        let closes = parse_daily_closes(body).unwrap().expect("known symbol");
        assert_eq!(
            closes.len(),
            2,
            "the null day must not produce a row (SPF-032)"
        );
    }

    // SPF-033 — a trading day still in progress at the run carries a null (or
    // absent) close in the chart series: only the completed days produce rows;
    // the in-progress day is written by the next run through the backfill.
    #[test]
    fn in_progress_trading_day_contributes_only_completed_closes() {
        let body = r#"{"chart":{"result":[{
            "meta":{"currency":"USD","symbol":"AAPL"},
            "timestamp":[1780948800,1781121600,1781208000],
            "indicators":{"quote":[{"close":[290.0,292.5,null]}]}
        }],"error":null}}"#;
        let closes = parse_daily_closes(body).unwrap().expect("known symbol");
        assert_eq!(
            closes.len(),
            2,
            "the in-progress (null-close) day must produce no row"
        );
        assert_eq!(closes[0].price, 290_000_000);
        assert_eq!(closes[1].price, 292_500_000);
    }

    // MKT-021 — a non-finite or non-positive close is provider garbage and is
    // skipped like a null close, never converted to a corrupted micro price.
    #[test]
    fn non_finite_or_non_positive_closes_produce_no_row() {
        let body = r#"{"chart":{"result":[{
            "meta":{"currency":"USD","symbol":"AAPL"},
            "timestamp":[1781121600,1781208000,1781294400,1781380800],
            "indicators":{"quote":[{"close":[290.0,0.0,-5.0,null]}]}
        }],"error":null}}"#;
        let closes = parse_daily_closes(body).unwrap().expect("known symbol");
        assert_eq!(closes.len(), 1, "zero and negative closes must be skipped");
        assert_eq!(closes[0].price, 290_000_000);
    }

    // MKT-125 — a London (LSE) daily-close series in GBp is divided by 100 to GBP.
    #[test]
    fn parses_daily_close_series_normalizes_gbp_pence() {
        let body = r#"{"chart":{"result":[{
            "meta":{"currency":"GBp","symbol":"VOD.L"},
            "timestamp":[1781121600],
            "indicators":{"quote":[{"close":[115.75]}]}
        }],"error":null}}"#;
        let closes = parse_daily_closes(body).unwrap().expect("known symbol");
        assert_eq!(closes.len(), 1);
        // 115.75 GBp = 1.1575 GBP → 1_157_500 micros.
        assert_eq!(closes[0].price, 1_157_500);
    }

    // MKT-114 / MKT-196 — an unknown symbol (chart.error present) is reported as
    // unknown, not as an empty series.
    #[test]
    fn daily_close_series_unknown_symbol_returns_none() {
        let body = r#"{"chart":{"result":null,"error":{"code":"Not Found","description":"No data found, symbol may be delisted"}}}"#;
        assert_eq!(parse_daily_closes(body).unwrap(), None);
    }

    // MKT-196 — a known symbol over a window without a trading day (a weekend)
    // carries no timestamp and an empty bar series: a known, empty series.
    #[test]
    fn daily_close_series_for_a_window_without_a_trading_day_is_known_and_empty() {
        let body = r#"{"chart":{"result":[{"meta":{"currency":"USD","symbol":"AAPL","gmtoffset":-14400},"indicators":{"quote":[{}],"adjclose":[{}]}}],"error":null}}"#;
        assert_eq!(parse_daily_closes(body).unwrap(), Some(Vec::new()));
    }

    // Malformed JSON is an Err.
    #[test]
    fn daily_close_series_malformed_json_returns_err() {
        assert!(parse_daily_closes("not json {{").is_err());
    }

    // SPF-030 — each close is dated to its trading day via the parallel timestamp.
    #[test]
    fn parses_daily_close_series_dates_each_close_to_its_trading_day() {
        // 1780948800 → 2026-06-08, 1781121600 → 2026-06-10 (UTC dates).
        let body = r#"{"chart":{"result":[{
            "meta":{"currency":"USD","symbol":"AAPL"},
            "timestamp":[1780948800,1781121600],
            "indicators":{"quote":[{"close":[290.0,292.0]}]}
        }],"error":null}}"#;
        let closes = parse_daily_closes(body).unwrap().expect("known symbol");
        assert_eq!(closes[0].date, "2026-06-08");
        assert_eq!(closes[1].date, "2026-06-10");
    }
}
