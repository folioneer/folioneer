use crate::context::account::AccountError;

/// Application-layer rejections specific to the `record_dividend` use case —
/// cross-BC asset and holding checks performed by the orchestrator before
/// delegating to `AccountService::record_dividend`.
///
/// Tagged with `#[serde(tag = "code")]` so it serializes verbatim across the
/// Tauri boundary into a flat `{ code: "..." }` shape.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type, Clone)]
#[serde(tag = "code")]
pub enum DividendTask {
    /// No asset exists with the requested ID (DIV-011).
    #[error("Asset not found")]
    AssetNotFound,
    /// The asset is not currently held (quantity = 0 or no holding) (DIV-011).
    #[error("Asset is not currently held in this account")]
    AssetNotHeld,
    /// Target asset is a system Cash Asset — dividends must be on non-cash
    /// holdings (DIV-011).
    #[error("Dividends cannot be recorded against a cash asset")]
    DividendOnCashAsset,
}

/// Use-case composite for the **record dividend** failure surface.
///
/// - `AccountError` — every account-BC rejection (lookup, infrastructure, and
///   the transaction-factory date / amount / rate validation).
/// - `DividendTask` — use-case-owned (this file), the cross-BC
///   asset/holding checks.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type)]
#[serde(untagged)]
pub enum DividendError {
    /// Account-BC rejection (lookup, infra, transaction validation).
    #[error(transparent)]
    Account(#[from] AccountError),
    /// Use-case-layer rejection (cross-BC asset checks).
    #[error(transparent)]
    UseCase(#[from] DividendTask),
}

#[cfg(test)]
mod dividend_error_wire_tests {
    use super::*;

    /// error-model.md — every `DividendError` variant must serialize to a flat
    /// object carrying a string `code` (guards the `#[serde(untagged)]`
    /// null-collapse regression across all three leaves).
    #[test]
    fn each_variant_emits_a_code() {
        let cases: Vec<DividendError> = vec![
            AccountError::AccountNotFound {
                account_id: "acc-1".to_string(),
            }
            .into(),
            DividendTask::AssetNotFound.into(),
            DividendTask::AssetNotHeld.into(),
            DividendTask::DividendOnCashAsset.into(),
            AccountError::AmountNotPositive.into(),
            AccountError::ExchangeRateNotPositive.into(),
        ];
        for err in cases {
            let value = serde_json::to_value(&err).expect("serialize DividendError");
            assert!(
                value.get("code").and_then(|c| c.as_str()).is_some(),
                "DividendError variant did not emit a string `code`: {value}"
            );
        }
    }
}

/// Application-layer rejections specific to the `record_free_shares` use case —
/// cross-BC asset and holding checks performed by the orchestrator before
/// delegating to `AccountService::record_free_shares`.
///
/// Tagged with `#[serde(tag = "code")]` so it serializes verbatim across the
/// Tauri boundary into a flat `{ code: "..." }` shape.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type, Clone)]
#[serde(tag = "code")]
pub enum FreeSharesTask {
    /// No asset exists with the requested ID (FSD-011).
    #[error("Asset not found")]
    AssetNotFound,
    /// The asset is not currently held (quantity = 0 or no holding) (FSD-011).
    #[error("Asset is not currently held in this account")]
    AssetNotHeld,
    /// Target asset is a system Cash Asset — free shares must be on non-cash
    /// holdings (FSD-011).
    #[error("Free shares cannot be recorded against a cash asset")]
    FreeSharesOnCashAsset,
}

/// Application-layer rejections specific to the `record_split` use case —
/// cross-BC asset and holding checks performed by the orchestrator before
/// delegating to `AccountService::record_split`.
///
/// Tagged with `#[serde(tag = "code")]` so it serializes verbatim across the
/// Tauri boundary into a flat `{ code: "..." }` shape.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type, Clone)]
#[serde(tag = "code")]
pub enum SplitTask {
    /// No asset exists with the requested ID (SPL-012).
    #[error("Asset not found")]
    AssetNotFound,
    /// The asset is not currently held (quantity = 0 or no holding) (SPL-012).
    #[error("Asset is not currently held in this account")]
    AssetNotHeld,
}

/// Use-case composite for the **record split** failure surface.
///
/// - `AccountError` — every account-BC rejection (lookup, infrastructure, the
///   split factory validation SPL-011, and the replay guards SPL-012/021).
/// - `SplitTask` — use-case-owned (this file), the cross-BC asset checks.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type)]
#[serde(untagged)]
pub enum SplitError {
    /// Account-BC rejection (lookup, infra, factory + replay validation).
    #[error(transparent)]
    Account(#[from] AccountError),
    /// Use-case-layer rejection (cross-BC asset checks).
    #[error(transparent)]
    UseCase(#[from] SplitTask),
}

#[cfg(test)]
mod split_error_wire_tests {
    use super::*;

    /// error-model.md — every `SplitError` variant must serialize to a flat
    /// object carrying a string `code` (guards the `#[serde(untagged)]`
    /// null-collapse regression across both leaves).
    #[test]
    fn each_variant_emits_a_code() {
        let cases: Vec<SplitError> = vec![
            AccountError::AccountNotFound {
                account_id: "acc-1".to_string(),
            }
            .into(),
            SplitTask::AssetNotFound.into(),
            SplitTask::AssetNotHeld.into(),
            AccountError::SplitFactorNotPositive.into(),
            AccountError::SplitFactorIsOne.into(),
            AccountError::SplitOnCashAsset.into(),
            AccountError::SplitCollapsesPosition.into(),
            AccountError::ClosedPosition.into(),
        ];
        for err in cases {
            let value = serde_json::to_value(&err).expect("serialize SplitError");
            assert!(
                value.get("code").and_then(|c| c.as_str()).is_some(),
                "SplitError variant did not emit a string `code`: {value}"
            );
        }
    }
}

/// Use-case composite for the **record free shares** failure surface.
///
/// - `AccountError` — every account-BC rejection (lookup, infrastructure, and
///   the transaction-factory date / quantity validation).
/// - `FreeSharesTask` — use-case-owned (this file), the cross-BC
///   asset/holding checks.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type)]
#[serde(untagged)]
pub enum FreeSharesError {
    /// Account-BC rejection (lookup, infra, transaction validation).
    #[error(transparent)]
    Account(#[from] AccountError),
    /// Use-case-layer rejection (cross-BC asset checks).
    #[error(transparent)]
    UseCase(#[from] FreeSharesTask),
}

#[cfg(test)]
mod free_shares_error_wire_tests {
    use super::*;

    /// error-model.md — every `FreeSharesError` variant must serialize to a flat
    /// object carrying a string `code` (guards the `#[serde(untagged)]`
    /// null-collapse regression across all three leaves).
    #[test]
    fn each_variant_emits_a_code() {
        // FSD-011 — all eligibility error variants must serialize correctly
        let cases: Vec<FreeSharesError> = vec![
            AccountError::AccountNotFound {
                account_id: "acc-1".to_string(),
            }
            .into(),
            FreeSharesTask::AssetNotFound.into(),
            FreeSharesTask::AssetNotHeld.into(),
            FreeSharesTask::FreeSharesOnCashAsset.into(),
            AccountError::QuantityNotPositive.into(),
            AccountError::DateInFuture.into(),
        ];
        for err in cases {
            let value = serde_json::to_value(&err).expect("serialize FreeSharesError");
            assert!(
                value.get("code").and_then(|c| c.as_str()).is_some(),
                "FreeSharesError variant did not emit a string `code`: {value}"
            );
        }
    }
}

/// Application-layer rejections specific to the `open_holding` use case —
/// cross-BC asset checks performed by the orchestrator before delegating to
/// `AccountService::open_holding`.
///
/// Tagged with `#[serde(tag = "code")]` so it serializes verbatim across the
/// Tauri boundary into a flat `{ code: "..." }` shape.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type, Clone)]
#[serde(tag = "code")]
pub enum OpenHoldingTask {
    /// No asset exists with the requested ID (TRX-056).
    #[error("Asset not found")]
    AssetNotFound,
    /// Target asset is archived — cannot open a holding (TRX-050).
    /// The orchestrator does not auto-unarchive; the caller must unarchive
    /// explicitly through the asset BC first.
    #[error("Cannot open a holding for an archived asset")]
    ArchivedAsset,
    /// Target asset is a system Cash Asset (CSH-061). Initial cash should be
    /// recorded via `record_deposit`, which goes through the cash-recording
    /// path and lazy-creates the Cash Holding.
    #[error("Opening balance cannot be recorded against a cash asset; use record_deposit instead")]
    OpeningBalanceOnCashAsset,
}

/// Use-case composite for the **open holding** failure surface — the single
/// command `open_holding` (TRX-042) and its full chain of rejections.
///
/// - `AccountError` — every account-BC rejection: `AccountNotFound`,
///   `DatabaseError` (incl. asset-side `get_asset_by_id` infra failures
///   tunnelled here so the wire carries a single `{ code: "DatabaseError" }`),
///   `InvalidTotalCost`, and the transaction-factory date / quantity invariants.
/// - `OpenHoldingTask` — use-case-owned (this file), the 3 cross-BC
///   rejections (`AssetNotFound`, `ArchivedAsset`, `OpeningBalanceOnCashAsset`).
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type)]
#[serde(untagged)]
pub enum OpenHoldingError {
    /// Account-BC rejection (lookup, infra, opening-balance + transaction validation).
    #[error(transparent)]
    Account(#[from] AccountError),
    /// Use-case-layer rejection (cross-BC asset checks).
    #[error(transparent)]
    UseCase(#[from] OpenHoldingTask),
}

/// Application-layer rejections specific to the `record_management_fee` use case —
/// cross-BC asset and holding checks performed by the orchestrator before
/// delegating to `AccountService::record_management_fee`.
///
/// Tagged with `#[serde(tag = "code")]` so it serializes verbatim across the
/// Tauri boundary into a flat `{ code: "..." }` shape.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type, Clone)]
#[serde(tag = "code")]
pub enum ManagementFeeTask {
    /// No asset exists with the requested ID (FEE-011).
    #[error("Asset not found")]
    AssetNotFound,
    /// The asset is not currently held (quantity = 0 or no holding) (FEE-011).
    #[error("Asset is not currently held in this account")]
    AssetNotHeld,
    /// Target asset is a system Cash Asset — management fees must be on non-cash
    /// holdings (FEE-011).
    #[error("Management fees cannot be recorded against a cash asset")]
    ManagementFeeOnCashAsset,
}

/// Use-case composite for the **record management fee** failure surface.
///
/// - `AccountError` — every account-BC rejection (lookup, infrastructure, and
///   the transaction-factory date / percent validation).
/// - `ManagementFeeTask` — use-case-owned (this file), the cross-BC
///   asset/holding checks.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type)]
#[serde(untagged)]
pub enum ManagementFeeError {
    /// Account-BC rejection (lookup, infra, transaction validation).
    #[error(transparent)]
    Account(#[from] AccountError),
    /// Use-case-layer rejection (cross-BC asset checks).
    #[error(transparent)]
    UseCase(#[from] ManagementFeeTask),
}

/// Application-layer rejections specific to the `record_interest` use case —
/// cross-BC asset and holding checks performed by the orchestrator before
/// delegating to `AccountService::record_interest`. The account's Cash Asset is
/// always a valid target (INT-023), so there is no cash-asset rejection here.
///
/// Tagged with `#[serde(tag = "code")]` so it serializes verbatim across the
/// Tauri boundary into a flat `{ code: "..." }` shape.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type, Clone)]
#[serde(tag = "code")]
pub enum InterestTask {
    /// No asset exists with the requested ID (INT-011).
    #[error("Asset not found")]
    AssetNotFound,
    /// The non-cash asset is not `interest_bearing` (INT-012).
    #[error("Asset is not an eligible interest target")]
    InterestNotEligible,
    /// The non-cash asset is not currently held (quantity = 0 or no holding) (INT-011).
    #[error("Asset is not currently held in this account")]
    AssetNotHeld,
}

/// Use-case composite for the **record interest** failure surface.
///
/// - `AccountError` — every account-BC rejection (lookup, infrastructure, and
///   the INT-021 amount / date validation).
/// - `InterestTask` — use-case-owned (this file), the cross-BC
///   asset/holding checks.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type)]
#[serde(untagged)]
pub enum InterestError {
    /// Account-BC rejection (lookup, infra, transaction validation).
    #[error(transparent)]
    Account(#[from] AccountError),
    /// Use-case-layer rejection (cross-BC asset checks).
    #[error(transparent)]
    UseCase(#[from] InterestTask),
}

#[cfg(test)]
mod interest_error_wire_tests {
    use super::*;

    /// error-model.md — every `InterestError` variant must serialize to a flat
    /// object carrying a string `code` (guards the `#[serde(untagged)]`
    /// null-collapse regression across all three leaves).
    #[test]
    fn each_variant_emits_a_code() {
        let cases: Vec<InterestError> = vec![
            AccountError::AccountNotFound {
                account_id: "acc-1".to_string(),
            }
            .into(),
            InterestTask::AssetNotFound.into(),
            InterestTask::InterestNotEligible.into(),
            InterestTask::AssetNotHeld.into(),
            AccountError::InterestAmountInvalid.into(),
            AccountError::PercentageNotPositive.into(),
            AccountError::PercentageAboveHundred.into(),
            AccountError::QuantityNotPositive.into(),
            AccountError::DateInFuture.into(),
        ];
        for err in cases {
            let value = serde_json::to_value(&err).expect("serialize InterestError");
            assert!(
                value.get("code").and_then(|c| c.as_str()).is_some(),
                "InterestError variant did not emit a string `code`: {value}"
            );
        }
    }
}

#[cfg(test)]
mod management_fee_error_wire_tests {
    use super::*;

    /// error-model.md — every `ManagementFeeError` variant must serialize to a flat
    /// object carrying a string `code` (guards the `#[serde(untagged)]`
    /// null-collapse regression across all three leaves).
    #[test]
    fn each_variant_emits_a_code() {
        let cases: Vec<ManagementFeeError> = vec![
            AccountError::AccountNotFound {
                account_id: "acc-1".to_string(),
            }
            .into(),
            ManagementFeeTask::AssetNotFound.into(),
            ManagementFeeTask::AssetNotHeld.into(),
            ManagementFeeTask::ManagementFeeOnCashAsset.into(),
            AccountError::PercentageNotPositive.into(),
            AccountError::PercentageAboveHundred.into(),
            AccountError::DateInFuture.into(),
        ];
        for err in cases {
            let value = serde_json::to_value(&err).expect("serialize ManagementFeeError");
            assert!(
                value.get("code").and_then(|c| c.as_str()).is_some(),
                "ManagementFeeError variant did not emit a string `code`: {value}"
            );
        }
    }
}

#[cfg(test)]
mod open_holding_error_wire_tests {
    use super::*;
    use crate::context::account::AccountError;

    /// The cash-line rejection exists at both the command level
    /// (`OpenHoldingTask`, CSH-061) and the domain level
    /// (`AccountError`, defense in depth). CSH-061 defines a SINGLE wire code,
    /// so both wrappers intentionally serialize to the identical
    /// `{ "code": "OpeningBalanceOnCashAsset" }` — the FE mapping and i18n key
    /// are shared. This test pins the aliasing so a future rename of either
    /// side surfaces as a failure instead of a silent wire fork.
    #[test]
    fn cash_line_rejection_shares_one_wire_code_across_both_layers() {
        let command_level: OpenHoldingError = OpenHoldingTask::OpeningBalanceOnCashAsset.into();
        let domain_level: OpenHoldingError = AccountError::OpeningBalanceOnCashAsset.into();
        let expected = serde_json::json!({ "code": "OpeningBalanceOnCashAsset" });
        assert_eq!(serde_json::to_value(&command_level).unwrap(), expected);
        assert_eq!(serde_json::to_value(&domain_level).unwrap(), expected);
    }
}
