use crate::context::currency::domain::rate_provider::{EurSnapshot, RateProvider};
use crate::core::logger::BACKEND;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// Composes an ordered list of `RateProvider`s, trying each in turn until one
/// returns `Ok` (ADR-009 chain: Frankfurter → ECB XML, FXR-070).
///
/// Returns the first successful snapshot. Returns `Err` only when every
/// provider has failed.
pub struct ChainedRateProvider {
    providers: Vec<Arc<dyn RateProvider>>,
}

impl ChainedRateProvider {
    /// Creates a new chain from an ordered list of providers.
    pub fn new(providers: Vec<Arc<dyn RateProvider>>) -> Self {
        Self { providers }
    }
}

#[async_trait]
impl RateProvider for ChainedRateProvider {
    async fn fetch_eur_snapshot(&self) -> Result<EurSnapshot> {
        let mut last_error: Option<anyhow::Error> = None;
        for provider in &self.providers {
            match provider.fetch_eur_snapshot().await {
                Ok(snapshot) => return Ok(snapshot),
                Err(error) => {
                    tracing::warn!(target: BACKEND, err = ?error, "rate provider failed; trying next tier");
                    last_error = Some(error);
                }
            }
        }
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("no rate providers configured")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::currency::domain::rate_provider::MockRateProvider;
    use crate::context::currency::domain::CurrencyRateSource;
    use std::collections::HashMap;

    fn make_snapshot(source: CurrencyRateSource) -> EurSnapshot {
        EurSnapshot {
            date: "2026-06-01".to_string(),
            rates: HashMap::from([("USD".to_string(), 1_164_600i64)]),
            source,
        }
    }

    // FXR-070 — first provider Ok → its snapshot returned, second provider never called
    #[tokio::test]
    async fn chained_provider_returns_first_ok_snapshot_without_calling_second() {
        let mut first = MockRateProvider::new();
        first
            .expect_fetch_eur_snapshot()
            .times(1)
            .returning(|| Ok(make_snapshot(CurrencyRateSource::Frankfurter)));

        let mut second = MockRateProvider::new();
        second.expect_fetch_eur_snapshot().times(0);

        let chain = ChainedRateProvider::new(vec![
            Arc::new(first) as Arc<dyn RateProvider>,
            Arc::new(second) as Arc<dyn RateProvider>,
        ]);

        let snapshot = chain
            .fetch_eur_snapshot()
            .await
            .expect("chain should succeed");
        assert_eq!(snapshot.source, CurrencyRateSource::Frankfurter);
    }

    // FXR-070 — first provider Err → second tried; second Ok → its snapshot returned
    #[tokio::test]
    async fn chained_provider_falls_back_to_second_when_first_fails() {
        let mut first = MockRateProvider::new();
        first
            .expect_fetch_eur_snapshot()
            .times(1)
            .returning(|| Err(anyhow::anyhow!("Frankfurter unreachable")));

        let mut second = MockRateProvider::new();
        second
            .expect_fetch_eur_snapshot()
            .times(1)
            .returning(|| Ok(make_snapshot(CurrencyRateSource::Ecb)));

        let chain = ChainedRateProvider::new(vec![
            Arc::new(first) as Arc<dyn RateProvider>,
            Arc::new(second) as Arc<dyn RateProvider>,
        ]);

        let snapshot = chain
            .fetch_eur_snapshot()
            .await
            .expect("chain should succeed via second provider");
        assert_eq!(snapshot.source, CurrencyRateSource::Ecb);
    }

    // FXR-070 — all providers Err → Err
    #[tokio::test]
    async fn chained_provider_returns_err_when_all_providers_fail() {
        let mut first = MockRateProvider::new();
        first
            .expect_fetch_eur_snapshot()
            .times(1)
            .returning(|| Err(anyhow::anyhow!("Frankfurter unreachable")));

        let mut second = MockRateProvider::new();
        second
            .expect_fetch_eur_snapshot()
            .times(1)
            .returning(|| Err(anyhow::anyhow!("ECB unreachable")));

        let chain = ChainedRateProvider::new(vec![
            Arc::new(first) as Arc<dyn RateProvider>,
            Arc::new(second) as Arc<dyn RateProvider>,
        ]);

        let result = chain.fetch_eur_snapshot().await;
        assert!(
            result.is_err(),
            "all providers failed → chain must return Err"
        );
    }
}
