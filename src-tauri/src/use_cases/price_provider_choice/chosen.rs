use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;

use crate::context::asset::{DatedClose, DatedSplitFactor, PriceProvider, Quote};
use crate::core::logger::BACKEND;

/// The file of the data folder holding the choice: a provider's id, or [`OFF`].
const CHOICE_FILE: &str = "price-provider";
/// What the file holds once the user turned fetching off.
const OFF: &str = "off";
/// The position of "no provider"; an offered provider's is its index plus one.
const NO_PROVIDER: usize = 0;

/// An External provider a build offers.
pub struct OfferedPriceProvider {
    /// Its id, on the wire and in the choice's file.
    pub id: &'static str,
    /// The client.
    pub provider: Arc<dyn PriceProvider>,
}

/// What `extensions.rs` plugs in for prices (ADR-024).
pub struct PriceProviders {
    /// The External providers this build offers.
    pub offered: Vec<OfferedPriceProvider>,
    /// The one chosen on a computer that never chose; `None` is off.
    pub default: Option<&'static str>,
}

/// The file holding the choice in `data_dir`.
pub fn choice_file(data_dir: &Path) -> PathBuf {
    data_dir.join(CHOICE_FILE)
}

/// MKT-214 — keeps the choice on this computer, whole or not at all.
pub fn save_choice(file: &Path, chosen: Option<&str>) -> std::io::Result<()> {
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let scratch = file.with_extension("tmp");
    std::fs::write(&scratch, chosen.unwrap_or(OFF))?;
    std::fs::rename(&scratch, file)
}

/// The External provider this computer chose, or none: what every fetch goes through (MKT-210,
/// MKT-214). With none chosen it reaches nothing: no request leaves the computer.
pub struct ChosenPriceProvider {
    offered: Vec<OfferedPriceProvider>,
    chosen: AtomicUsize,
}

impl ChosenPriceProvider {
    /// The list of a build, on the choice kept in `data_dir`: the build's default when this
    /// computer never chose, off when what it holds is not on offer.
    pub fn of(providers: PriceProviders, data_dir: Option<&Path>) -> Self {
        let saved = data_dir.and_then(|dir| std::fs::read_to_string(choice_file(dir)).ok());
        let id = match saved.as_deref().map(str::trim) {
            None => providers.default,
            Some(OFF) => None,
            Some(id) => Some(id),
        };
        let chosen = position_in(&providers.offered, id);
        if chosen.is_none() {
            tracing::warn!(target: BACKEND, id = ?id, "price provider choice not on offer: off");
        }
        Self {
            offered: providers.offered,
            chosen: AtomicUsize::new(chosen.unwrap_or(NO_PROVIDER)),
        }
    }

    /// Whether a provider is chosen.
    pub fn is_on(&self) -> bool {
        self.chosen.load(Ordering::SeqCst) != NO_PROVIDER
    }

    /// The id of the provider chosen.
    pub fn chosen_id(&self) -> Option<&'static str> {
        self.active().map(|offered| offered.id)
    }

    /// The ids on offer.
    pub fn offered_ids(&self) -> Vec<&'static str> {
        self.offered.iter().map(|offered| offered.id).collect()
    }

    /// Whether `id` can be chosen; `None`, off, always can.
    pub fn offers(&self, id: Option<&str>) -> bool {
        position_in(&self.offered, id).is_some()
    }

    /// Follows a new choice; one not on offer changes nothing.
    pub fn choose(&self, id: Option<&str>) {
        if let Some(position) = position_in(&self.offered, id) {
            self.chosen.store(position, Ordering::SeqCst);
        }
    }

    fn active(&self) -> Option<&OfferedPriceProvider> {
        self.chosen
            .load(Ordering::SeqCst)
            .checked_sub(1)
            .and_then(|index| self.offered.get(index))
    }

    fn provider(&self) -> Result<&Arc<dyn PriceProvider>> {
        self.active()
            .map(|offered| &offered.provider)
            .ok_or_else(|| anyhow!("no External provider is chosen on this computer"))
    }
}

/// The position `id` stands for; `None` when it is not on offer.
fn position_in(offered: &[OfferedPriceProvider], id: Option<&str>) -> Option<usize> {
    match id {
        None => Some(NO_PROVIDER),
        Some(id) => offered
            .iter()
            .position(|candidate| candidate.id == id)
            .map(|index| index + 1),
    }
}

#[async_trait]
impl PriceProvider for ChosenPriceProvider {
    async fn fetch_price(&self, symbol: &str) -> Result<Option<Quote>> {
        self.provider()?.fetch_price(symbol).await
    }

    async fn fetch_daily_closes(
        &self,
        symbol: &str,
        from: &str,
        to: &str,
    ) -> Result<Option<Vec<DatedClose>>> {
        self.provider()?.fetch_daily_closes(symbol, from, to).await
    }

    async fn fetch_splits(&self, symbol: &str, from: &str) -> Result<Vec<DatedSplitFactor>> {
        self.provider()?.fetch_splits(symbol, from).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::asset::MockPriceProvider;

    /// A provider that answers `times` quotes of `price`, and panics on one more.
    fn quoting(price: i64, times: usize) -> Arc<dyn PriceProvider> {
        let mut provider = MockPriceProvider::new();
        provider
            .expect_fetch_price()
            .times(times)
            .returning(move |_| Ok(Some(Quote { price, date: None })));
        Arc::new(provider)
    }

    fn two(first: Arc<dyn PriceProvider>, second: Arc<dyn PriceProvider>) -> PriceProviders {
        PriceProviders {
            offered: vec![
                OfferedPriceProvider {
                    id: "first",
                    provider: first,
                },
                OfferedPriceProvider {
                    id: "second",
                    provider: second,
                },
            ],
            default: None,
        }
    }

    // MKT-214 — switching the provider: only the chosen one is called.
    #[tokio::test]
    async fn only_the_chosen_provider_is_called() {
        let chosen = ChosenPriceProvider::of(two(quoting(1, 1), quoting(2, 1)), None);
        chosen.choose(Some("first"));
        let quote = chosen
            .fetch_price("X")
            .await
            .expect("answer")
            .expect("quote");
        assert_eq!(quote.price, 1);
        chosen.choose(Some("second"));
        let quote = chosen
            .fetch_price("X")
            .await
            .expect("answer")
            .expect("quote");
        assert_eq!(quote.price, 2);
    }

    // MKT-210 — off, no provider is reached, whatever is asked (a mock with no expectation
    // would panic if called).
    #[tokio::test]
    async fn off_reaches_no_provider() {
        let chosen = ChosenPriceProvider::of(
            PriceProviders {
                offered: vec![OfferedPriceProvider {
                    id: "first",
                    provider: Arc::new(MockPriceProvider::new()),
                }],
                default: None,
            },
            None,
        );
        assert!(!chosen.is_on());
        assert!(chosen.fetch_price("X").await.is_err());
        assert!(chosen
            .fetch_daily_closes("X", "2026-01-01", "2026-01-31")
            .await
            .is_err());
        assert!(chosen.fetch_splits("X", "2026-01-01").await.is_err());
    }

    // MKT-214 — never chosen: the build's default. Chosen: what the file holds, off included.
    // What is not on offer is off, not the default.
    #[test]
    fn the_choice_comes_from_the_file_then_the_default() {
        let folder = tempfile::tempdir().expect("folder");
        let of = |default| {
            let mut providers = two(quoting(1, 0), quoting(2, 0));
            providers.default = default;
            ChosenPriceProvider::of(providers, Some(folder.path()))
        };
        assert_eq!(of(None).chosen_id(), None);
        assert_eq!(of(Some("second")).chosen_id(), Some("second"));

        let file = choice_file(folder.path());
        save_choice(&file, Some("first")).expect("saved");
        assert_eq!(of(None).chosen_id(), Some("first"));
        save_choice(&file, None).expect("saved");
        assert_eq!(of(Some("second")).chosen_id(), None);
        save_choice(&file, Some("gone")).expect("saved");
        assert_eq!(of(Some("second")).chosen_id(), None);
    }

    #[test]
    fn a_choice_not_on_offer_changes_nothing() {
        let chosen = ChosenPriceProvider::of(two(quoting(1, 0), quoting(2, 0)), None);
        chosen.choose(Some("first"));
        assert!(!chosen.offers(Some("gone")));
        chosen.choose(Some("gone"));
        assert_eq!(chosen.chosen_id(), Some("first"));
        assert!(chosen.offers(None));
        assert_eq!(chosen.offered_ids(), vec!["first", "second"]);
    }
}
