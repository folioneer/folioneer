//! The one file a build differs by: which external data sources are plugged into the
//! application, and which channel its updates come from. Both entry points (`run` and
//! `run_scheduled_fetch_headless`) take them from here and nowhere else (ADR-020).

use std::sync::Arc;

use crate::context::asset::{PriceProvider, ReqwestYahooClient};
use crate::context::currency::{
    ChainedRateProvider, RateHistoryProvider, RateProvider, ReqwestEcbClient,
    ReqwestFrankfurterClient,
};
use crate::use_cases::asset_web_lookup::{OpenFigiClient, ReqwestOpenFigiClient};
use crate::use_cases::update_checker::UpdateChannel;

/// The external data sources of a build.
pub struct Providers {
    /// The External provider — latest quotes and daily closes (ADR-017). `None` in a
    /// build composed without one: no fetch task, no scheduled fetch and no price
    /// history backfill exists (MKT-210).
    pub price: Option<Arc<dyn PriceProvider>>,
    /// Latest exchange rates, tried in order (ADR-009).
    pub rate: Arc<dyn RateProvider>,
    /// Exchange-rate history, for the rate backfills.
    pub rate_history: Arc<dyn RateHistoryProvider>,
    /// Asset lookup by ISIN or name.
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
        price: Some(Arc::new(ReqwestYahooClient::new()?)),
        rate,
        rate_history: frankfurter,
        asset_lookup: Arc::new(ReqwestOpenFigiClient::new()),
    })
}

/// The channel this build's updates come from: the endpoint of `tauri.conf.json`,
/// with no request header.
pub fn update_channel() -> UpdateChannel {
    UpdateChannel::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    // #037 — the public build plugs in a source for prices, rates, rate history and
    // asset lookup; a missing one would leave a feature silently dead.
    #[test]
    fn the_public_build_plugs_in_every_external_data_source() {
        let providers = providers().expect("providers");

        assert_eq!(
            Arc::strong_count(providers.price.as_ref().expect("price")),
            1
        );
        assert_eq!(Arc::strong_count(&providers.rate), 1);
        assert_eq!(Arc::strong_count(&providers.asset_lookup), 1);
        // The rate provider chain holds the same client as the rate history.
        assert_eq!(Arc::strong_count(&providers.rate_history), 2);
    }

    // #037 — the public build sends no header of its own and names no endpoint: its
    // updates come from the address in `tauri.conf.json`, anonymously.
    #[test]
    fn the_public_build_updates_from_the_configured_endpoint_without_headers() {
        let channel = update_channel();

        assert!(channel.endpoints.is_empty());
        assert!(channel.headers.is_empty());
    }
}
