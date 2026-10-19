use serde::{Deserialize, Serialize};
use specta::Type;

/// A split the price provider reports for an asset (MKT-230): an observation kept and
/// synced like an asset price (SYN-021, CFR-050), identified by `(asset_id, date)`. It
/// changes no holding, price or transaction by itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ReportedSplit {
    /// ID of the asset the provider reports the split for.
    pub asset_id: String,
    /// The split's date as the provider reports it, ISO `yyyy-mm-dd`.
    pub date: String,
    /// New shares of the split factor, new : old (SPL-010).
    pub new: i64,
    /// Old shares of the split factor (SPL-010).
    pub old: i64,
}

impl ReportedSplit {
    /// Restores a reported split from storage without validation (B1 — restore factory).
    pub fn restore(asset_id: String, date: String, new: i64, old: i64) -> Self {
        Self {
            asset_id,
            date,
            new,
            old,
        }
    }

    /// The reported split of `asset_id` a provider's dated split factor stands for.
    pub fn of_asset(asset_id: &str, factor: &DatedSplitFactor) -> Self {
        Self {
            asset_id: asset_id.to_string(),
            date: factor.date.clone(),
            new: factor.new,
            old: factor.old,
        }
    }
}

/// A split factor on a date, as a [`super::PriceProvider`] reports it (MKT-230): the
/// provider names no asset, its caller does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatedSplitFactor {
    /// ISO `yyyy-mm-dd`, on the exchange's calendar.
    pub date: String,
    /// New shares of the split factor (SPL-010); positive.
    pub new: i64,
    /// Old shares of the split factor (SPL-010); positive.
    pub old: i64,
}
