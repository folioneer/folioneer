/// Cross-rate computation from EUR-base snapshots (FXR-080/082/083).
pub mod cross_rate;
/// CurrencyPair aggregate, repository trait, and CurrencyPairSummary.
pub mod currency_pair;
/// CurrencyRate aggregate, repository trait, and CurrencyRateSource.
pub mod currency_rate;
/// RateProvider trait and EurSnapshot (ADR-009, FXR-070).
pub mod rate_provider;

pub use currency_pair::{CurrencyPair, CurrencyPairRepository, CurrencyPairSummary};
pub use currency_rate::{CurrencyRate, CurrencyRateRepository, CurrencyRateSource};
pub use rate_provider::{EurSnapshot, RateHistoryProvider, RateProvider};

#[cfg(test)]
pub use currency_pair::MockCurrencyPairRepository;
#[cfg(test)]
pub use currency_rate::MockCurrencyRateRepository;
