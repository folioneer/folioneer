//! The External provider of an E2E run (ADR-020): the suite exercises the fetch flows
//! without any network call.

use crate::context::asset::domain::{DatedClose, PriceProvider, Quote};

/// Every symbol is unknown to it, as a provider answers for a symbol it has no data for
/// (MKT-114, MKT-196).
pub struct NoDataProvider;

#[async_trait::async_trait]
impl PriceProvider for NoDataProvider {
    async fn fetch_price(&self, _symbol: &str) -> anyhow::Result<Option<Quote>> {
        Ok(None)
    }

    async fn fetch_daily_closes(
        &self,
        _symbol: &str,
        _from: &str,
        _to: &str,
    ) -> anyhow::Result<Option<Vec<DatedClose>>> {
        Ok(None)
    }
}
