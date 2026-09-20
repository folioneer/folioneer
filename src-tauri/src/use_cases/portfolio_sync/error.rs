//! Use-case composite for the seven cross-BC sync commands (D3): `PortfolioSyncError` wraps
//! every bounded context a first-device publish, a join rebuild, or a status read can touch,
//! plus a small tagged task sub-enum for the orchestrator's own guards.

use crate::context::account::AccountError;
use crate::context::asset::AssetError;
use crate::context::currency::CurrencyError;
use crate::context::sync::SyncError;

/// Orchestrator-level guards and the catch-all — the codes `PortfolioSyncError` itself never
/// re-declares. PR-B raises only `InstallationHoldsUserData` (the join branch, always
/// returned in PR-B per D3) and `PortfolioCreatedElsewhere` (delegated straight from
/// `SyncError`, wired here too since the orchestrator surfaces it verbatim); `HistoryIncomplete`
/// and `RebuildInterrupted` are PR-C join/rebuild codes declared now so the wire shape is
/// stable across PR-B/PR-C.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type, Clone)]
#[serde(tag = "code")]
pub enum PortfolioSyncTask {
    /// The joining installation holds user-entered records (SYN-014). PR-B always returns
    /// this for the join branch — the rebuild lands in PR-C.
    #[error("A fresh installation is required to join this portfolio")]
    InstallationHoldsUserData,

    /// A segment of the replay set could not be read while joining (SYN-036, PR-C).
    #[error("The published history is incomplete")]
    HistoryIncomplete,

    /// The rebuild transaction was interrupted; the device is left as before (SYN-080, PR-C).
    #[error("Joining was interrupted and has been rolled back")]
    RebuildInterrupted,

    /// An unexpected failure not attributable to a specific BC's database.
    #[error("An unexpected error occurred")]
    UnknownError,
}

/// The wire-facing error for every `use_cases::portfolio_sync` command.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type)]
#[serde(untagged)]
pub enum PortfolioSyncError {
    /// A sync-BC rejection (folder, passphrase, device state).
    #[error(transparent)]
    Sync(#[from] SyncError),
    /// An account-BC rejection surfaced while reading or rebuilding the portfolio.
    #[error(transparent)]
    Account(#[from] AccountError),
    /// An asset-BC rejection surfaced while reading or rebuilding the portfolio.
    #[error(transparent)]
    Asset(#[from] AssetError),
    /// A currency-BC rejection surfaced while reading or rebuilding the portfolio.
    #[error(transparent)]
    Currency(#[from] CurrencyError),
    /// An orchestrator-level guard or the catch-all.
    #[error(transparent)]
    Task(#[from] PortfolioSyncTask),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::to_value;
    use std::collections::HashMap;

    // error-model.md — every PortfolioSyncError variant serializes to a flat object carrying
    // a string `code` (guards the #[serde(untagged)] null-collapse regression) — one case per
    // wrapper.
    #[test]
    fn each_variant_emits_a_code() {
        let cases: Vec<PortfolioSyncError> = vec![
            SyncError::SyncDisabled.into(),
            AccountError::DatabaseError.into(),
            AssetError::DatabaseError.into(),
            CurrencyError::DatabaseError.into(),
            PortfolioSyncTask::InstallationHoldsUserData.into(),
        ];
        for err in cases {
            let value = to_value(&err).expect("serialize PortfolioSyncError");
            assert!(
                value.get("code").and_then(|c| c.as_str()).is_some(),
                "PortfolioSyncError variant did not emit a string `code`: {value}"
            );
        }
    }

    /// error-model.md § Anti-patterns — "two wrapper variants in a composite whose enums
    /// share a `code` discriminant" is a *silent* collision only when the colliding variants
    /// differ in shape: the frontend then cannot tell them apart. Every code two or more of
    /// the wrapped enums share is listed here with one value per enum, and each group must
    /// serialize to the same JSON — a shape-identical collision is harmless because the
    /// frontend reacts to `code` alone.
    #[test]
    fn every_colliding_code_is_shape_identical_across_the_wrapped_enums() {
        let collisions: Vec<(&str, Vec<PortfolioSyncError>)> = vec![
            (
                "NameEmpty",
                vec![AccountError::NameEmpty.into(), AssetError::NameEmpty.into()],
            ),
            (
                "DateInFuture",
                vec![
                    AccountError::DateInFuture.into(),
                    AssetError::DateInFuture.into(),
                    CurrencyError::DateInFuture.into(),
                ],
            ),
            (
                "InvalidCurrency",
                vec![
                    AccountError::InvalidCurrency {
                        currency: "XX".into(),
                    }
                    .into(),
                    AssetError::InvalidCurrency {
                        currency: "XX".into(),
                    }
                    .into(),
                    CurrencyError::InvalidCurrency {
                        currency: "XX".into(),
                    }
                    .into(),
                ],
            ),
            (
                "NotPositive",
                vec![
                    AssetError::NotPositive.into(),
                    CurrencyError::NotPositive.into(),
                ],
            ),
            (
                "InvalidDateFormat",
                vec![
                    AssetError::InvalidDateFormat { date: "bad".into() }.into(),
                    CurrencyError::InvalidDateFormat { date: "bad".into() }.into(),
                ],
            ),
            (
                "NonFinite",
                vec![
                    AssetError::NonFinite.into(),
                    CurrencyError::NonFinite.into(),
                ],
            ),
            (
                "DatabaseError",
                vec![
                    SyncError::DatabaseError.into(),
                    AccountError::DatabaseError.into(),
                    AssetError::DatabaseError.into(),
                    CurrencyError::DatabaseError.into(),
                ],
            ),
        ];
        for (code, values) in collisions {
            let shapes: Vec<serde_json::Value> = values
                .iter()
                .map(|value| to_value(value).expect("serialize PortfolioSyncError"))
                .collect();
            assert!(
                shapes
                    .iter()
                    .all(|shape| shape.get("code").and_then(|c| c.as_str()) == Some(code)),
                "{code}: every value must carry that code: {shapes:?}"
            );
            assert!(
                shapes.windows(2).all(|pair| pair[0] == pair[1]),
                "{code}: the colliding variants must serialize identically: {shapes:?}"
            );
        }
    }

    /// The `code` inventory behind the shape check above: every collision across the wrapped
    /// enums must appear in `every_colliding_code_is_shape_identical_across_the_wrapped_enums`,
    /// so a newly colliding variant is caught here and then proven shape-identical there.
    #[test]
    fn every_collision_across_the_wrapped_enums_is_listed() {
        let mut codes_by_enum: HashMap<&str, Vec<String>> = HashMap::new();
        codes_by_enum.insert(
            "SyncError",
            vec![
                "AlreadyEnabled",
                "SyncDisabled",
                "SyncPaused",
                "AlreadyPaused",
                "NotPaused",
                "PassphraseTooShort",
                "DeviceNameBlank",
                "FolderUnavailable",
                "UpdateRequired",
                "PassphraseMismatch",
                "PublishFailed",
                "PortfolioCreatedElsewhere",
                "NoticeNotFound",
                "FolderHoldsOtherPortfolio",
                "DatabaseError",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        );
        codes_by_enum.insert(
            "AccountError",
            vec![
                "NameEmpty",
                "AccountNotFound",
                "AmountNotPositive",
                "CascadingOversell",
                "ClosedPosition",
                "DatabaseError",
                "DateInFuture",
                "DateTooOld",
                "EndBeforeStart",
                "ExchangeRateNotPositive",
                "FeesNegative",
                "InsufficientCash",
                "InterestAmountInvalid",
                "InvalidCurrency",
                "InvalidDate",
                "InvalidTotalCost",
                "ManagementFeesDisabled",
                "NameAlreadyExists",
                "NegativeAveragePrice",
                "NegativeQuantity",
                "NoteOnCashAsset",
                "NoteOnUnheldAsset",
                "NoteTextEmpty",
                "NoteTextTooLong",
                "OpeningBalanceOnCashAsset",
                "Oversell",
                "PercentageAboveHundred",
                "PercentageNotPositive",
                "QuantityNotPositive",
                "RateAboveHundred",
                "RateNotPositive",
                "ScheduleAlreadyExists",
                "ScheduleNotFound",
                "SplitCollapsesPosition",
                "SplitFactorIsOne",
                "SplitFactorNotPositive",
                "SplitOnCashAsset",
                "ThresholdIncomplete",
                "ThresholdNotPositive",
                "TotalAmountBelowFees",
                "TotalAmountNotPositive",
                "TransactionNotFound",
                "UnitPriceNegative",
                "UnitPriceOutOfRange",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        );
        codes_by_enum.insert(
            "AssetError",
            vec![
                "Archived",
                "AssetNotFound",
                "CashAssetNotEditable",
                "CategoryNotFound",
                "DatabaseError",
                "DateInFuture",
                "DuplicateName",
                "InvalidCurrency",
                "InvalidDateFormat",
                "InvalidExchange",
                "InvalidIsinFormat",
                "InvalidRiskLevel",
                "LabelEmpty",
                "NameEmpty",
                "NonFinite",
                "NotPositive",
                "PriceNotFound",
                "ReferenceEmpty",
                "SystemProtected",
                "SystemReadonly",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        );
        codes_by_enum.insert(
            "CurrencyError",
            vec![
                "DatabaseError",
                "DateInFuture",
                "IdentityPair",
                "InvalidCurrency",
                "InvalidDateFormat",
                "NonFinite",
                "NotPositive",
                "ProviderUnreachable",
                "RateNotFound",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        );
        codes_by_enum.insert(
            "PortfolioSyncTask",
            vec![
                "InstallationHoldsUserData",
                "HistoryIncomplete",
                "RebuildInterrupted",
                "UnknownError",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        );

        let mut owners_by_code: HashMap<String, Vec<&str>> = HashMap::new();
        for (enum_name, codes) in &codes_by_enum {
            for code in codes {
                owners_by_code
                    .entry(code.clone())
                    .or_default()
                    .push(enum_name);
            }
        }

        let shape_checked = [
            "NameEmpty",
            "DateInFuture",
            "InvalidCurrency",
            "NotPositive",
            "InvalidDateFormat",
            "NonFinite",
            "DatabaseError",
        ];
        let unlisted_collisions: Vec<(String, Vec<&str>)> = owners_by_code
            .into_iter()
            .filter(|(code, owners)| owners.len() > 1 && !shape_checked.contains(&code.as_str()))
            .collect();

        assert!(
            unlisted_collisions.is_empty(),
            "error-model.md: a code shared by two wrapped enums must be proven shape-identical \
             in every_colliding_code_is_shape_identical_across_the_wrapped_enums — unlisted: \
             {unlisted_collisions:?}"
        );
    }
}
