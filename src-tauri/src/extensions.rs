//! The one file a build differs by: which external data sources are plugged into the
//! application, and which channel its updates come from. Both entry points (`run` and
//! `run_scheduled_fetch_headless`) take them from here and nowhere else (ADR-020).

use std::sync::Arc;

#[cfg(debug_assertions)]
use crate::context::asset::NoDataProvider;
use crate::context::currency::{
    ChainedRateProvider, RateHistoryProvider, RateProvider, ReqwestEcbClient,
    ReqwestFrankfurterClient,
};
use crate::shared::infrastructure::e2e_run;
use crate::use_cases::asset_web_lookup::{OpenFigiClient, ReqwestOpenFigiClient};
use crate::use_cases::shared::chosen_price_provider::{OfferedPriceProvider, PriceProviders};
#[cfg(feature = "app")]
use crate::use_cases::update_checker::UpdateChannel;

mod yahoo_client;

use yahoo_client::ReqwestYahooClient;

/// The id of Yahoo Finance among the External providers (ADR-024).
const YAHOO_FINANCE: &str = "yahoo_finance";

/// The external data sources of a build.
pub struct Providers {
    /// The External providers offered — latest quotes and daily closes (ADR-024). Each
    /// computer chooses one or none; with none, no fetch task, no scheduled fetch and no
    /// price history backfill runs (MKT-210, MKT-214).
    pub price: PriceProviders,
    /// Latest exchange rates, tried in order (ADR-009).
    // Read only by the Tauri shell; the headless core leaves it unread.
    #[cfg_attr(not(feature = "app"), allow(dead_code))]
    pub rate: Arc<dyn RateProvider>,
    /// Exchange-rate history, for the rate backfills.
    pub rate_history: Arc<dyn RateHistoryProvider>,
    /// Asset lookup by ISIN or name.
    // Read only by the Tauri shell; the headless core leaves it unread.
    #[cfg_attr(not(feature = "app"), allow(dead_code))]
    pub asset_lookup: Arc<dyn OpenFigiClient>,
}

/// Builds this build's external data sources. Fails when an HTTP client cannot be
/// initialised, so the caller reports it instead of panicking.
pub fn providers() -> anyhow::Result<Providers> {
    let frankfurter = Arc::new(ReqwestFrankfurterClient::new()?);
    let rate: Arc<dyn RateProvider> = Arc::new(ChainedRateProvider::new(vec![
        Arc::clone(&frankfurter) as Arc<dyn RateProvider>,
        Arc::new(ReqwestEcbClient::new()?) as Arc<dyn RateProvider>,
    ]));
    Ok(Providers {
        price: price_providers(e2e_run::e2e_data_dir().is_some())?,
        rate,
        rate_history: frankfurter,
        asset_lookup: Arc::new(ReqwestOpenFigiClient::new()),
    })
}

/// The public build's External providers: Yahoo Finance, off until a computer chooses it
/// (ADR-024). An E2E run — a debug build started by the suite — gets one that answers "no
/// data" in its place, already on, so the fetch flows stay reachable without any network
/// call. A release build contains no such provider, whatever it is asked.
fn price_providers(is_e2e_run: bool) -> anyhow::Result<PriceProviders> {
    #[cfg(debug_assertions)]
    if is_e2e_run {
        return Ok(PriceProviders {
            offered: vec![OfferedPriceProvider {
                id: YAHOO_FINANCE,
                provider: Arc::new(NoDataProvider),
            }],
            default: Some(YAHOO_FINANCE),
        });
    }
    let _ = is_e2e_run;
    Ok(PriceProviders {
        offered: vec![OfferedPriceProvider {
            id: YAHOO_FINANCE,
            provider: Arc::new(ReqwestYahooClient::new()?),
        }],
        default: None,
    })
}

#[cfg(feature = "app")]
/// The channel this build's updates come from: the endpoint of `tauri.conf.json`,
/// with no request header.
pub fn update_channel() -> UpdateChannel {
    UpdateChannel::default()
}

/// The name of the distribution channel this build belongs to, shown beside its version
/// (UPD-030) — which build this is, unlike the update channel above, which says where its
/// updates come from. The public build belongs to none.
// Read only by the Tauri shell; the headless core leaves it unread.
#[cfg_attr(not(feature = "app"), allow(dead_code))]
pub fn distribution_channel() -> Option<&'static str> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ADR-024, MKT-214 — the public build plugs in every external data source, and offers
    // Yahoo Finance off: no computer fetches a price until it chooses to.
    #[test]
    fn the_public_build_offers_a_price_provider_off_by_default() {
        let providers = providers().expect("providers");

        assert_eq!(providers.price.default, None);
        let ids: Vec<&str> = providers
            .price
            .offered
            .iter()
            .map(|offered| offered.id)
            .collect();
        assert_eq!(ids, vec![YAHOO_FINANCE]);
        assert_eq!(Arc::strong_count(&providers.rate), 1);
        assert_eq!(Arc::strong_count(&providers.asset_lookup), 1);
        // The rate provider chain holds the same client as the rate history.
        assert_eq!(Arc::strong_count(&providers.rate_history), 2);
    }

    // TODO-038 — an E2E run gets an External provider, on, that answers "no data" for every
    // symbol, latest quote and daily closes alike, without calling anything.
    #[tokio::test]
    async fn an_e2e_run_gets_a_provider_that_answers_no_data() {
        assert_eq!(price_providers(false).expect("providers").default, None);

        let providers = price_providers(true).expect("providers");
        assert_eq!(providers.default, Some(YAHOO_FINANCE));
        let provider = &providers.offered.first().expect("offered").provider;

        assert!(provider
            .fetch_price("AAPL")
            .await
            .expect("answer")
            .is_none());
        assert!(provider
            .fetch_daily_closes("AAPL", "2026-01-01", "2026-01-31")
            .await
            .expect("answer")
            .is_none());
    }

    // TODO-037 — the public build sends no header of its own and names no endpoint: its
    // updates come from the address in `tauri.conf.json`, anonymously.
    #[cfg(feature = "app")]
    #[test]
    fn the_public_build_updates_from_the_configured_endpoint_without_headers() {
        let channel = update_channel();

        assert!(channel.endpoints.is_empty());
        assert!(channel.headers.is_empty());
    }

    // UPD-030 — the public build names no channel: its version shows alone.
    #[test]
    fn the_public_build_names_no_distribution_channel() {
        assert_eq!(distribution_channel(), None);
    }
}
