//! Asset Web Lookup orchestrator — issues HTTP calls to OpenFIGI and delegates
//! ranking, dedup, and exchange-name resolution to
//! [`primary_listing_processor`] (WEB-014, WEB-022, WEB-049, WEB-050).

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::result::Result as StdResult;
use std::sync::Arc;
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

use super::error::WebLookupError;
use super::primary_listing_processor::{self, AssetLookupResult, QueryContext, RawFigiHit};
use crate::context::asset::isin::validate_isin;
use crate::core::logger::BACKEND;

// ---------------------------------------------------------------------------
// LookupMode — explicit path selector (WEB-014, contract § LookupMode)
// ---------------------------------------------------------------------------

/// Explicit lookup path selector passed by the frontend (WEB-014).
///
/// `Isin` routes the query through `/v3/mapping` after ISO 6166 format
/// validation (WEB-016). `Keyword` routes through `/v3/search` with
/// diacritics normalization (WEB-015).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum LookupMode {
    /// ISIN path: validate format (WEB-016) then call `/v3/mapping`.
    Isin,
    /// Keyword path: normalize diacritics (WEB-015) then call `/v3/search`.
    Keyword,
}

/// Sentinel error raised by `ReqwestOpenFigiClient` when OpenFIGI returns
/// HTTP 429. Translation closures in `search` / `collect_keyword_hits`
/// downcast against this type to route to `WebLookupError::RateLimited`
/// (WEB-025) instead of the generic `NetworkError`.
#[derive(Debug, thiserror::Error)]
#[error("OpenFIGI rate-limit (HTTP 429)")]
struct RateLimitedError;

// ---------------------------------------------------------------------------
// OpenFigiClient trait (allows test mocking per B26)
// ---------------------------------------------------------------------------

/// Abstraction over the OpenFIGI HTTP API. Concrete production implementation
/// is [`ReqwestOpenFigiClient`]; tests use the `mockall`-generated mock.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait OpenFigiClient: Send + Sync {
    /// `/v3/mapping` with `idType=ID_ISIN` (WEB-014).
    async fn map_isin(&self, isin: &str) -> Result<Vec<RawFigiHit>>;

    /// `/v3/search` keyword endpoint. No request-time securityType filter —
    /// hits are classified post-response and ranked by the priority sort.
    async fn search_keyword(&self, query: &str) -> Result<Vec<RawFigiHit>>;

    /// Batched `/v3/mapping` call with `idType=ID_BB_GLOBAL_SHARE_CLASS_LEVEL`,
    /// returning all known listings for each share class (WEB-050 step 4).
    /// Result order matches the input id order.
    async fn map_share_classes(&self, ids: &[String]) -> Result<Vec<Vec<RawFigiHit>>>;
}

// ---------------------------------------------------------------------------
// AssetWebLookupUseCase
// ---------------------------------------------------------------------------

/// Orchestrates the OpenFIGI lookup: routes the query (WEB-014), fans out to
/// the share-class-mapping endpoint on the keyword path (WEB-050), then hands
/// the raw hits to [`primary_listing_processor::process_hits`] for dedup and
/// primary-listing pick. The final result is truncated to 30 entries (WEB-022).
pub struct AssetWebLookupUseCase {
    client: Arc<dyn OpenFigiClient>,
}

impl AssetWebLookupUseCase {
    /// Creates a new use case backed by the provided [`OpenFigiClient`].
    pub fn new(client: Arc<dyn OpenFigiClient>) -> Self {
        Self { client }
    }

    /// Searches OpenFIGI for instruments matching `query` along the explicit
    /// `mode` path (WEB-014). The ISIN path validates the query against ISO 6166
    /// (WEB-016) before any HTTP call; the keyword path normalizes diacritics
    /// (WEB-015) and issues a second HTTP call (WEB-050) to enrich each unique
    /// share class with its full set of listings, so primary venues missing
    /// from `/v3/search` (notably Euronext Paris for European stocks) are
    /// surfaced.
    ///
    /// Client errors are routed by [`translate_client_error`] (WEB-025): HTTP 429
    /// → `RateLimited`, everything else → `NetworkError`. ISIN format failures
    /// surface as `InvalidIsinFormat` with no HTTP call made. The full diagnostic
    /// chain is preserved server-side via `tracing::warn!`.
    pub async fn search(
        &self,
        query: String,
        mode: LookupMode,
    ) -> StdResult<Vec<AssetLookupResult>, WebLookupError> {
        match mode {
            LookupMode::Isin => {
                let normalized_isin =
                    validate_isin(&query).map_err(|_| WebLookupError::InvalidIsinFormat)?;
                let ctx = QueryContext {
                    isin: Some(normalized_isin.clone()),
                };
                let raw_hits = self.client.map_isin(&normalized_isin).await.map_err(|e| {
                    translate_client_error(e, &normalized_isin, "search: ISIN lookup")
                })?;
                let mut results = primary_listing_processor::process_hits(raw_hits, &ctx);
                results.truncate(30);
                Ok(results)
            }
            LookupMode::Keyword => {
                let normalized = normalize_query(query.trim());
                let ctx = QueryContext { isin: None };
                let raw_hits = self.collect_keyword_hits(&normalized).await?;
                let mut results = primary_listing_processor::process_hits(raw_hits, &ctx);
                results.truncate(30);
                Ok(results)
            }
        }
    }

    /// Keyword path (WEB-050): does the initial `/v3/search` call, collects the
    /// unique non-null `share_class_figi` values, then batches them into a
    /// single `/v3/mapping` call. Listings returned by mapping replace the
    /// initial keyword hits for each share class (mapping is the authoritative
    /// list of all venues for a share class). Hits with a null share class
    /// pass through to the processor, which drops them.
    async fn collect_keyword_hits(
        &self,
        query: &str,
    ) -> StdResult<Vec<RawFigiHit>, WebLookupError> {
        let initial = self.client.search_keyword(query).await.map_err(|e| {
            translate_client_error(e, query, "collect_keyword_hits: search_keyword")
        })?;
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut share_class_ids: Vec<String> = Vec::new();
        for hit in &initial {
            if let Some(id) = hit.share_class_figi.as_deref() {
                if seen.insert(id) {
                    share_class_ids.push(id.to_string());
                }
            }
        }
        if share_class_ids.is_empty() {
            return Ok(initial);
        }
        let enriched = self
            .client
            .map_share_classes(&share_class_ids)
            .await
            .map_err(|e| {
                translate_client_error(e, query, "collect_keyword_hits: map_share_classes")
            })?;
        Ok(enriched.into_iter().flatten().collect())
    }
}

// ---------------------------------------------------------------------------
// Query normalization (WEB-015)
// ---------------------------------------------------------------------------

/// Strips Latin diacritics from a query before sending it to OpenFIGI (WEB-015).
///
/// OpenFIGI's name index is unaccented — `"Société Générale"` returns 0 hits
/// while `"Societe Generale"` returns 100. Normalizing the query upfront makes
/// the search behave the way users expect when typing names with accents.
///
/// Implementation: Unicode NFD decomposition then drop combining marks. Handles
/// the full Latin diacritic range (é/è/ê/ç/à/ô/ñ/ü/…) without a hardcoded
/// table. ISIN inputs are unaffected (no combining marks in ASCII-alphanumeric).
fn normalize_query(s: &str) -> String {
    s.nfd().filter(|c| !is_combining_mark(*c)).collect()
}

// ---------------------------------------------------------------------------
// OpenFIGI HTTP response types (private deserialization structs)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct OpenFigiHit {
    name: String,
    ticker: Option<String>,
    #[serde(rename = "securityType")]
    security_type: Option<String>,
    currency: Option<String>,
    #[serde(rename = "exchCode")]
    exchange_code: Option<String>,
    #[serde(rename = "micCode")]
    mic_code: Option<String>,
    #[serde(rename = "shareClassFIGI")]
    share_class_figi: Option<String>,
    #[serde(rename = "compositeFIGI")]
    composite_figi: Option<String>,
}

/// One item in the `/v3/mapping` response array.
#[derive(Deserialize)]
struct MappingResultItem {
    data: Option<Vec<OpenFigiHit>>,
}

/// The `/v3/search` response envelope.
#[derive(Deserialize)]
struct SearchResponse {
    data: Vec<OpenFigiHit>,
}

// ---------------------------------------------------------------------------
// ReqwestOpenFigiClient — production HTTP implementation
// ---------------------------------------------------------------------------

const MAP_URL: &str = "https://api.openfigi.com/v3/mapping";
const SEARCH_URL: &str = "https://api.openfigi.com/v3/search";

/// Production [`OpenFigiClient`] backed by `reqwest` with rustls (WEB-021 — no
/// API key).
pub struct ReqwestOpenFigiClient {
    client: reqwest::Client,
}

impl Default for ReqwestOpenFigiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ReqwestOpenFigiClient {
    /// Creates a new client using the system's default TLS configuration.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl OpenFigiClient for ReqwestOpenFigiClient {
    async fn map_isin(&self, isin: &str) -> Result<Vec<RawFigiHit>> {
        let body = serde_json::json!([{"idType": "ID_ISIN", "idValue": isin}]);
        let resp = self
            .client
            .post(MAP_URL)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("OpenFIGI ISIN mapping request failed for ISIN: {isin}"))?;

        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(anyhow::Error::from(RateLimitedError));
        }
        if !resp.status().is_success() {
            anyhow::bail!("OpenFIGI mapping returned {}", resp.status());
        }

        let items: Vec<MappingResultItem> = resp
            .json()
            .await
            .context("failed to deserialize OpenFIGI mapping response")?;
        Ok(items
            .into_iter()
            .flat_map(|item| item.data.unwrap_or_default())
            .map(hit_to_raw)
            .collect())
    }

    async fn search_keyword(&self, query: &str) -> Result<Vec<RawFigiHit>> {
        // OpenFIGI's search accepts only a single securityType string (not an
        // array), so post-classification via map_security_type plus the
        // priority sort handles the broader asset-class coverage instead of
        // a request-time filter.
        let body = serde_json::json!({
            "query": query,
        });
        let resp = self
            .client
            .post(SEARCH_URL)
            .json(&body)
            .send()
            .await
            .context("OpenFIGI keyword search request failed")?;

        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(anyhow::Error::from(RateLimitedError));
        }
        if !resp.status().is_success() {
            anyhow::bail!("OpenFIGI search returned {}", resp.status());
        }

        let search_resp: SearchResponse = resp
            .json()
            .await
            .context("failed to deserialize OpenFIGI search response")?;
        Ok(search_resp.data.into_iter().map(hit_to_raw).collect())
    }

    async fn map_share_classes(&self, ids: &[String]) -> Result<Vec<Vec<RawFigiHit>>> {
        let body: Vec<_> = ids
            .iter()
            .map(|id| {
                serde_json::json!({
                    "idType": "ID_BB_GLOBAL_SHARE_CLASS_LEVEL",
                    "idValue": id,
                })
            })
            .collect();
        let resp = self
            .client
            .post(MAP_URL)
            .json(&body)
            .send()
            .await
            .context("OpenFIGI share-class mapping request failed")?;

        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(anyhow::Error::from(RateLimitedError));
        }
        if !resp.status().is_success() {
            anyhow::bail!("OpenFIGI share-class mapping returned {}", resp.status());
        }

        let items: Vec<MappingResultItem> = resp
            .json()
            .await
            .context("failed to deserialize OpenFIGI share-class mapping response")?;
        Ok(items
            .into_iter()
            .map(|item| {
                item.data
                    .unwrap_or_default()
                    .into_iter()
                    .map(hit_to_raw)
                    .collect()
            })
            .collect())
    }
}

fn hit_to_raw(h: OpenFigiHit) -> RawFigiHit {
    RawFigiHit {
        name: h.name,
        ticker: h.ticker,
        security_type: h.security_type,
        currency: h.currency,
        exchange_code: h.exchange_code,
        mic_code: h.mic_code,
        share_class_figi: h.share_class_figi,
        composite_figi: h.composite_figi,
    }
}

// ---------------------------------------------------------------------------
// Application-layer translation
// ---------------------------------------------------------------------------

/// Routes a client error to the right [`WebLookupError`] variant
/// (WEB-025): `RateLimitedError` → `RateLimited` (transient, user-recoverable);
/// every other error → `NetworkError`. The full diagnostic chain is preserved
/// server-side via `tracing::warn!`.
fn translate_client_error(err: anyhow::Error, query: &str, site: &str) -> WebLookupError {
    if err.downcast_ref::<RateLimitedError>().is_some() {
        tracing::warn!(
            target: BACKEND,
            query = %query,
            "{site} rate-limited (WEB-025)",
        );
        WebLookupError::RateLimited
    } else {
        tracing::warn!(
            target: BACKEND,
            query = %query,
            err = ?err,
            "{site} failed (WEB-025)",
        );
        WebLookupError::NetworkError
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::eq;

    fn raw_hit(
        name: &str,
        exchange: Option<&str>,
        share_class: Option<&str>,
        ticker: Option<&str>,
        currency: Option<&str>,
        security_type: Option<&str>,
    ) -> RawFigiHit {
        RawFigiHit {
            name: name.to_string(),
            ticker: ticker.map(str::to_string),
            security_type: security_type.map(str::to_string),
            currency: currency.map(str::to_string),
            exchange_code: exchange.map(str::to_string),
            mic_code: None,
            share_class_figi: share_class.map(str::to_string),
            composite_figi: None,
        }
    }

    // ------------------------------------------------------------------
    // WEB-015 query normalization (diacritics)
    // ------------------------------------------------------------------

    // OpenFIGI's name index is unaccented — these are the diacritics most
    // likely to surface in European stock names.
    #[test]
    fn normalize_query_strips_french_diacritics() {
        assert_eq!(normalize_query("Société Générale"), "Societe Generale");
        assert_eq!(normalize_query("Crédit Agricole"), "Credit Agricole");
        assert_eq!(normalize_query("Pernod Ricard"), "Pernod Ricard");
    }

    #[test]
    fn normalize_query_strips_german_umlauts_via_nfd() {
        // NFD decomposes ü to "u" + combining diaeresis; the combining mark is
        // dropped, leaving "u". OpenFIGI stores Münchener as "MUNCHENER".
        assert_eq!(normalize_query("Münchener Rück"), "Munchener Ruck");
    }

    #[test]
    fn normalize_query_strips_spanish_tilde() {
        assert_eq!(normalize_query("España"), "Espana");
    }

    #[test]
    fn normalize_query_is_noop_on_ascii() {
        assert_eq!(normalize_query("ASML"), "ASML");
        assert_eq!(normalize_query("FR0000130809"), "FR0000130809");
    }

    #[tokio::test]
    async fn accented_keyword_query_is_normalized_before_search() {
        // The user types accented input; the gateway must receive the stripped form.
        let mut mock = MockOpenFigiClient::new();
        mock.expect_search_keyword()
            .with(eq("Societe Generale"))
            .times(1)
            .returning(|_| Ok(vec![]));
        mock.expect_map_isin().times(0);
        mock.expect_map_share_classes().times(0);

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        assert!(uc
            .search("Société Générale".to_string(), LookupMode::Keyword)
            .await
            .is_ok());
    }

    // ------------------------------------------------------------------
    // WEB-050 — keyword path triggers share-class enrichment
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn keyword_path_calls_map_share_classes_with_unique_ids() {
        let initial_hits = vec![
            raw_hit(
                "X",
                Some("UV"),
                Some("SC1"),
                None,
                None,
                Some("Common Stock"),
            ),
            raw_hit(
                "X",
                Some("XT"),
                Some("SC1"),
                None,
                None,
                Some("Common Stock"),
            ),
            raw_hit(
                "Y",
                Some("UN"),
                Some("SC2"),
                None,
                None,
                Some("Common Stock"),
            ),
            raw_hit("Z", Some("X1"), None, None, None, Some("Common Stock")), // null SC dropped
        ];

        let mut mock = MockOpenFigiClient::new();
        mock.expect_search_keyword()
            .times(1)
            .returning(move |_| Ok(initial_hits.clone()));
        mock.expect_map_share_classes()
            .withf(|ids| ids == ["SC1".to_string(), "SC2".to_string()])
            .times(1)
            .returning(|_| Ok(vec![vec![], vec![]]));
        mock.expect_map_isin().times(0);

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        assert!(uc
            .search("anything".to_string(), LookupMode::Keyword)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn keyword_path_skips_enrichment_when_no_share_class_present() {
        let initial_hits = vec![raw_hit(
            "X",
            Some("X1"),
            None,
            None,
            None,
            Some("Common Stock"),
        )];

        let mut mock = MockOpenFigiClient::new();
        mock.expect_search_keyword()
            .times(1)
            .returning(move |_| Ok(initial_hits.clone()));
        mock.expect_map_share_classes().times(0);

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        assert!(uc
            .search("anything".to_string(), LookupMode::Keyword)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn isin_path_does_not_call_map_share_classes() {
        let mut mock = MockOpenFigiClient::new();
        mock.expect_map_isin().times(1).returning(|_| {
            Ok(vec![raw_hit(
                "X",
                Some("FP"),
                Some("SC1"),
                Some("AI"),
                Some("EUR"),
                Some("Common Stock"),
            )])
        });
        mock.expect_search_keyword().times(0);
        mock.expect_map_share_classes().times(0);

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        assert!(uc
            .search("FR0000120073".to_string(), LookupMode::Isin)
            .await
            .is_ok());
    }

    // ------------------------------------------------------------------
    // WEB-022 — final 30-row truncation
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn truncates_results_to_thirty() {
        // Initial keyword response carries 35 distinct share classes; mapping
        // returns one populated hit per share class so the dedup pipeline
        // produces 35 candidate result rows. The final truncation must cap
        // them to 30.
        let initial: Vec<RawFigiHit> = (0..35)
            .map(|i| {
                raw_hit(
                    &format!("Fund {i}"),
                    Some("FP"),
                    Some(&format!("SC{i}")),
                    None,
                    None,
                    Some("Common Stock"),
                )
            })
            .collect();
        let enriched: Vec<Vec<RawFigiHit>> = initial
            .iter()
            .map(|h| {
                vec![raw_hit(
                    &h.name,
                    Some("FP"),
                    h.share_class_figi.as_deref(),
                    Some("TICK"),
                    Some("EUR"),
                    Some("Common Stock"),
                )]
            })
            .collect();

        let mut mock = MockOpenFigiClient::new();
        mock.expect_search_keyword()
            .times(1)
            .returning(move |_| Ok(initial.clone()));
        mock.expect_map_share_classes()
            .times(1)
            .returning(move |_| Ok(enriched.clone()));

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        let results = uc
            .search("fund".to_string(), LookupMode::Keyword)
            .await
            .unwrap();
        assert_eq!(results.len(), 30);
    }

    #[tokio::test]
    async fn empty_share_class_mapping_yields_empty_result() {
        let initial = vec![raw_hit(
            "X",
            Some("UV"),
            Some("SC1"),
            None,
            None,
            Some("Common Stock"),
        )];
        let mut mock = MockOpenFigiClient::new();
        mock.expect_search_keyword()
            .times(1)
            .returning(move |_| Ok(initial.clone()));
        mock.expect_map_share_classes()
            .times(1)
            .returning(|_| Ok(vec![vec![]]));

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        let results = uc
            .search("anything".to_string(), LookupMode::Keyword)
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    // ------------------------------------------------------------------
    // WEB-049 — exchange code resolution end-to-end
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn resolves_known_exchange_code_to_readable_name() {
        let isin = "FR0000120073";
        let hit = raw_hit(
            "AIR LIQUIDE SA",
            Some("FP"),
            Some("SC1"),
            Some("AI"),
            Some("EUR"),
            Some("Common Stock"),
        );
        let mut mock = MockOpenFigiClient::new();
        mock.expect_map_isin()
            .with(eq(isin))
            .times(1)
            .returning(move |_| Ok(vec![hit.clone()]));

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        let results = uc.search(isin.to_string(), LookupMode::Isin).await.unwrap();
        // exchange is now Option<Exchange>; verify the canonical code is resolved
        let exchange = results[0]
            .exchange
            .as_ref()
            .expect("exchange should be resolved for FP → XPAR");
        assert_eq!(exchange.code, "XPAR");
    }

    // ------------------------------------------------------------------
    // WEB-046 — reference field source
    // ------------------------------------------------------------------

    // WEB-046 — on the ISIN path, `reference` holds the ticker from the hit
    // (so the FE always pre-fills `Asset.reference` with a value Yahoo can
    // resolve) and `isin` holds the normalized ISIN query.
    #[tokio::test]
    async fn isin_path_fills_ticker_in_reference_and_isin_in_isin() {
        let isin = "US0378331005";
        let hit = raw_hit(
            "Apple Inc.",
            Some("UN"),
            Some("SC1"),
            Some("AAPL"),
            Some("USD"),
            Some("Common Stock"),
        );
        let mut mock = MockOpenFigiClient::new();
        mock.expect_map_isin()
            .with(eq(isin))
            .times(1)
            .returning(move |_| Ok(vec![hit.clone()]));

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        let results = uc.search(isin.to_string(), LookupMode::Isin).await.unwrap();
        assert_eq!(results[0].reference.as_deref(), Some("AAPL"));
        assert_eq!(results[0].isin.as_deref(), Some(isin));
    }

    // WEB-046 — on the keyword path, `isin` stays None because OpenFIGI's
    // `/v3/search` response does not expose ISIN.
    #[tokio::test]
    async fn isin_is_none_on_keyword_path() {
        let initial = vec![raw_hit(
            "Apple Inc.",
            Some("UN"),
            Some("SC1"),
            Some("AAPL"),
            Some("USD"),
            Some("Common Stock"),
        )];
        let enriched = vec![vec![raw_hit(
            "Apple Inc.",
            Some("UN"),
            Some("SC1"),
            Some("AAPL"),
            Some("USD"),
            Some("Common Stock"),
        )]];
        let mut mock = MockOpenFigiClient::new();
        mock.expect_search_keyword()
            .times(1)
            .returning(move |_| Ok(initial.clone()));
        mock.expect_map_share_classes()
            .times(1)
            .returning(move |_| Ok(enriched.clone()));

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        let results = uc
            .search("apple".to_string(), LookupMode::Keyword)
            .await
            .unwrap();
        assert!(results[0].isin.is_none());
    }

    #[tokio::test]
    async fn reference_is_ticker_on_keyword_path_when_present() {
        let initial = vec![raw_hit(
            "Apple Inc.",
            Some("UN"),
            Some("SC1"),
            Some("AAPL"),
            Some("USD"),
            Some("Common Stock"),
        )];
        let enriched = vec![vec![raw_hit(
            "Apple Inc.",
            Some("UN"),
            Some("SC1"),
            Some("AAPL"),
            Some("USD"),
            Some("Common Stock"),
        )]];
        let mut mock = MockOpenFigiClient::new();
        mock.expect_search_keyword()
            .times(1)
            .returning(move |_| Ok(initial.clone()));
        mock.expect_map_share_classes()
            .times(1)
            .returning(move |_| Ok(enriched.clone()));

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        let results = uc
            .search("apple".to_string(), LookupMode::Keyword)
            .await
            .unwrap();
        assert_eq!(results[0].reference.as_deref(), Some("AAPL"));
    }

    // ------------------------------------------------------------------
    // WEB-025 — error propagation
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn search_keyword_failure_translates_to_network_error() {
        let mut mock = MockOpenFigiClient::new();
        mock.expect_search_keyword()
            .times(1)
            .returning(|_| Err(anyhow::anyhow!("connection refused")));

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        let err = uc
            .search("AAPL".to_string(), LookupMode::Keyword)
            .await
            .unwrap_err();
        assert!(matches!(err, WebLookupError::NetworkError), "got: {err:?}");
    }

    #[tokio::test]
    async fn map_share_classes_failure_translates_to_network_error() {
        let initial = vec![raw_hit(
            "X",
            Some("UV"),
            Some("SC1"),
            None,
            None,
            Some("Common Stock"),
        )];
        let mut mock = MockOpenFigiClient::new();
        mock.expect_search_keyword()
            .times(1)
            .returning(move |_| Ok(initial.clone()));
        mock.expect_map_share_classes()
            .times(1)
            .returning(|_| Err(anyhow::anyhow!("rate limited")));

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        let err = uc
            .search("anything".to_string(), LookupMode::Keyword)
            .await
            .unwrap_err();
        assert!(matches!(err, WebLookupError::NetworkError), "got: {err:?}");
    }

    #[tokio::test]
    async fn map_isin_failure_translates_to_network_error() {
        let mut mock = MockOpenFigiClient::new();
        mock.expect_map_isin()
            .times(1)
            .returning(|_| Err(anyhow::anyhow!("HTTP 500")));

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        let err = uc
            .search("FR0000120073".to_string(), LookupMode::Isin)
            .await
            .unwrap_err();
        assert!(matches!(err, WebLookupError::NetworkError), "got: {err:?}");
    }

    // ------------------------------------------------------------------
    // WEB-025 — 429 routes to RateLimited (distinct from NetworkError)
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn map_isin_rate_limit_translates_to_rate_limited() {
        let mut mock = MockOpenFigiClient::new();
        mock.expect_map_isin()
            .times(1)
            .returning(|_| Err(anyhow::Error::from(RateLimitedError)));

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        let err = uc
            .search("FR0000120073".to_string(), LookupMode::Isin)
            .await
            .unwrap_err();
        assert!(matches!(err, WebLookupError::RateLimited), "got: {err:?}");
    }

    #[tokio::test]
    async fn search_keyword_rate_limit_translates_to_rate_limited() {
        let mut mock = MockOpenFigiClient::new();
        mock.expect_search_keyword()
            .times(1)
            .returning(|_| Err(anyhow::Error::from(RateLimitedError)));

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        let err = uc
            .search("AAPL".to_string(), LookupMode::Keyword)
            .await
            .unwrap_err();
        assert!(matches!(err, WebLookupError::RateLimited), "got: {err:?}");
    }

    #[tokio::test]
    async fn map_share_classes_rate_limit_translates_to_rate_limited() {
        let initial = vec![raw_hit(
            "X",
            Some("UV"),
            Some("SC1"),
            None,
            None,
            Some("Common Stock"),
        )];
        let mut mock = MockOpenFigiClient::new();
        mock.expect_search_keyword()
            .times(1)
            .returning(move |_| Ok(initial.clone()));
        mock.expect_map_share_classes()
            .times(1)
            .returning(|_| Err(anyhow::Error::from(RateLimitedError)));

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        let err = uc
            .search("anything".to_string(), LookupMode::Keyword)
            .await
            .unwrap_err();
        assert!(matches!(err, WebLookupError::RateLimited), "got: {err:?}");
    }

    // ------------------------------------------------------------------
    // WEB-014 (amended) — explicit LookupMode routing
    // ------------------------------------------------------------------

    /// `LookupMode::Isin` with a valid ISIN must call `map_isin` with the
    /// normalized form and never call `search_keyword` (WEB-014).
    #[tokio::test]
    async fn lookup_with_isin_mode_validates_and_calls_map_isin() {
        let isin = "IE00B53L3W79";
        let mut mock = MockOpenFigiClient::new();
        mock.expect_map_isin()
            .with(eq(isin))
            .times(1)
            .returning(|_| Ok(vec![]));
        mock.expect_search_keyword().times(0);
        mock.expect_map_share_classes().times(0);

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        assert!(uc.search(isin.to_string(), LookupMode::Isin).await.is_ok());
    }

    /// `LookupMode::Keyword` must call `search_keyword` and never call
    /// `map_isin`, even when the query happens to look like an ISIN (WEB-014).
    #[tokio::test]
    async fn lookup_with_keyword_mode_calls_search_keyword() {
        let query = "IE00B53L3W79"; // looks like an ISIN, but mode forces keyword
        let mut mock = MockOpenFigiClient::new();
        mock.expect_search_keyword()
            .with(eq(query))
            .times(1)
            .returning(|_| Ok(vec![]));
        mock.expect_map_isin().times(0);
        mock.expect_map_share_classes().times(0);

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        assert!(uc
            .search(query.to_string(), LookupMode::Keyword)
            .await
            .is_ok());
    }

    /// `LookupMode::Isin` with an invalid ISIN format must return
    /// `Err(InvalidIsinFormat)` without making any HTTP calls (WEB-016, WEB-025).
    #[tokio::test]
    async fn lookup_with_isin_mode_rejects_invalid_format() {
        let mut mock = MockOpenFigiClient::new();
        mock.expect_map_isin().times(0);
        mock.expect_search_keyword().times(0);
        mock.expect_map_share_classes().times(0);

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        let err = uc
            .search("NOTANISIN".to_string(), LookupMode::Isin)
            .await
            .unwrap_err();
        assert!(
            matches!(err, WebLookupError::InvalidIsinFormat),
            "got: {err:?}"
        );
    }

    /// `LookupMode::Isin` with a lowercase/whitespace ISIN must normalize the
    /// input; the `AssetLookupResult.isin` field on every returned result must
    /// contain the uppercased+trimmed ISIN (WEB-046).
    #[tokio::test]
    async fn lookup_with_isin_mode_forwards_normalized_isin_in_isin_field() {
        let normalized = "IE00B53L3W79";
        let hit = raw_hit(
            "iShares Core S&P 500 UCITS ETF",
            Some("ID"),
            Some("SC1"),
            Some("CSPX"),
            Some("USD"),
            Some("Common Stock"),
        );
        let mut mock = MockOpenFigiClient::new();
        mock.expect_map_isin()
            .with(eq(normalized))
            .times(1)
            .returning(move |_| Ok(vec![hit.clone()]));

        let uc = AssetWebLookupUseCase::new(Arc::new(mock));
        let results = uc
            .search("  ie00b53l3w79  ".to_string(), LookupMode::Isin)
            .await
            .unwrap();
        assert_eq!(results[0].isin.as_deref(), Some(normalized));
    }
}
