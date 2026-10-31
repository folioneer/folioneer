use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use super::error::PriceProviderChoiceError;
use crate::core::logger::BACKEND;
use crate::use_cases::shared::chosen_price_provider::{save_choice, ChosenPriceProvider};

/// MKT-214 — the External provider chosen on this computer, and what the build offers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PriceProviderChoice {
    /// The id chosen; `None` is off.
    pub chosen: Option<String>,
    /// The ids on offer.
    pub offered: Vec<String>,
}

/// SPF-070 — the scheduled fetch follows the choice.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait FetchScheduleFollower: Send + Sync {
    /// Registers again what is configured when `on`, removes the schedule otherwise.
    async fn follow(&self, on: bool);
}

/// Reads and changes the choice (MKT-214).
pub struct PriceProviderChoiceUseCase {
    chosen: Arc<ChosenPriceProvider>,
    file: PathBuf,
    schedule: Arc<dyn FetchScheduleFollower>,
}

impl PriceProviderChoiceUseCase {
    /// Creates the use case over the provider every fetch goes through, keeping the choice
    /// in `file`.
    pub fn new(
        chosen: Arc<ChosenPriceProvider>,
        file: PathBuf,
        schedule: Arc<dyn FetchScheduleFollower>,
    ) -> Self {
        Self {
            chosen,
            file,
            schedule,
        }
    }

    /// The choice, as it stands.
    pub fn read(&self) -> PriceProviderChoice {
        PriceProviderChoice {
            chosen: self.chosen.chosen_id().map(str::to_string),
            offered: self
                .chosen
                .offered_ids()
                .into_iter()
                .map(str::to_string)
                .collect(),
        }
    }

    /// Keeps a new choice; it holds at once, without a restart.
    pub async fn set(
        &self,
        chosen: Option<String>,
    ) -> Result<PriceProviderChoice, PriceProviderChoiceError> {
        let id = chosen.as_deref();
        if !self.chosen.offers(id) {
            return Err(PriceProviderChoiceError::NotOffered);
        }
        save_choice(&self.file, id).map_err(|error| {
            tracing::error!(target: BACKEND, err = %error, "price provider choice not saved");
            PriceProviderChoiceError::NotSaved
        })?;
        self.chosen.choose(id);
        self.schedule.follow(self.chosen.is_on()).await;
        tracing::info!(target: BACKEND, chosen = ?id, "price provider chosen");
        Ok(self.read())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::asset::MockPriceProvider;
    use crate::use_cases::shared::chosen_price_provider::{
        choice_file, OfferedPriceProvider, PriceProviders,
    };

    fn offered() -> PriceProviders {
        PriceProviders {
            offered: vec![OfferedPriceProvider {
                id: "first",
                provider: Arc::new(MockPriceProvider::new()),
            }],
            default: None,
        }
    }

    fn following(on: bool) -> Arc<dyn FetchScheduleFollower> {
        let mut schedule = MockFetchScheduleFollower::new();
        schedule
            .expect_follow()
            .withf(move |asked| *asked == on)
            .times(1)
            .return_const(());
        Arc::new(schedule)
    }

    // MKT-214 — chosen and kept: a new run of the application finds it.
    #[tokio::test]
    async fn a_choice_holds_at_once_and_is_kept() {
        let folder = tempfile::tempdir().expect("folder");
        let chosen = Arc::new(ChosenPriceProvider::of(offered(), Some(folder.path())));
        let use_case = PriceProviderChoiceUseCase::new(
            Arc::clone(&chosen),
            choice_file(folder.path()),
            following(true),
        );
        assert_eq!(use_case.read().chosen, None, "off by default");

        let answer = use_case.set(Some("first".to_string())).await.expect("set");
        assert_eq!(answer.chosen.as_deref(), Some("first"));
        assert_eq!(answer.offered, vec!["first".to_string()]);
        assert!(chosen.is_on());
        let next_run = ChosenPriceProvider::of(offered(), Some(folder.path()));
        assert_eq!(next_run.chosen_id(), Some("first"));
    }

    // SPF-070 — turned off, the schedule goes with it, and it stays off at the next run.
    #[tokio::test]
    async fn off_takes_the_schedule_away() {
        let folder = tempfile::tempdir().expect("folder");
        let mut providers = offered();
        providers.default = Some("first");
        let chosen = Arc::new(ChosenPriceProvider::of(providers, Some(folder.path())));
        let use_case = PriceProviderChoiceUseCase::new(
            Arc::clone(&chosen),
            choice_file(folder.path()),
            following(false),
        );
        assert_eq!(use_case.set(None).await.expect("set").chosen, None);
        assert!(!chosen.is_on());
        let mut providers = offered();
        providers.default = Some("first");
        let next_run = ChosenPriceProvider::of(providers, Some(folder.path()));
        assert_eq!(next_run.chosen_id(), None);
    }

    #[tokio::test]
    async fn a_refused_choice_changes_nothing() {
        let folder = tempfile::tempdir().expect("folder");
        let chosen = Arc::new(ChosenPriceProvider::of(offered(), Some(folder.path())));
        let use_case = PriceProviderChoiceUseCase::new(
            Arc::clone(&chosen),
            choice_file(folder.path()),
            Arc::new(MockFetchScheduleFollower::new()),
        );
        assert_eq!(
            use_case.set(Some("gone".to_string())).await,
            Err(PriceProviderChoiceError::NotOffered)
        );
        assert!(!choice_file(folder.path()).exists(), "nothing was written");

        // A file whose folder cannot exist: under a file.
        let blocker = folder.path().join("blocker");
        std::fs::write(&blocker, "").expect("blocker");
        let unsaved = PriceProviderChoiceUseCase::new(
            Arc::clone(&chosen),
            blocker.join("price-provider"),
            Arc::new(MockFetchScheduleFollower::new()),
        );
        assert_eq!(
            unsaved.set(Some("first".to_string())).await,
            Err(PriceProviderChoiceError::NotSaved)
        );
        assert!(!chosen.is_on());
    }
}
