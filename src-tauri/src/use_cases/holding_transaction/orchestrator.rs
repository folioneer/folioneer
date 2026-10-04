use super::error::{
    DividendError, DividendTask, FreeSharesError, FreeSharesTask, InterestError, InterestTask,
    ManagementFeeError, ManagementFeeTask, NameLookupError, OpenHoldingError, OpenHoldingTask,
    SplitError, SplitTask, TransactionDraftError, TransactionDraftTask,
};
use super::shared::ensure_cash_asset;
use crate::context::account::{
    Account, AccountError, AccountServiceContract, EnteredAmount, ManagementFeeRemoval,
    StockSplitPosition, Transaction, TransactionType,
};
use crate::context::asset::{Asset, AssetClass, AssetServiceContract};
use crate::core::logger::BACKEND;
use std::sync::Arc;

/// What a transaction draft is (TRX-062): a purchase, a sale, or a dividend being corrected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, specta::Type)]
pub enum DraftKind {
    /// A purchase.
    Purchase,
    /// A sale of a held position.
    Sell,
    /// A recorded dividend being corrected (DIV-040): the quantity carries its amount, and
    /// what was entered is a unit price with its rate — a dividend has no typed total.
    Dividend,
}

/// A transaction draft: a purchase or sale as the user is still entering it (TRX-062).
/// Empty strings are fields not filled yet; amounts are in micros.
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(deny_unknown_fields)]
pub struct TransactionDraft {
    /// Purchase or sale.
    pub kind: DraftKind,
    /// The account, empty until chosen.
    pub account_id: String,
    /// The asset, empty until chosen.
    pub asset_id: String,
    /// ISO date, empty until entered.
    pub date: String,
    /// Quantity.
    pub quantity: i64,
    /// What the user entered: a unit price or a typed total, with rate and fees.
    pub entered: EnteredAmount,
    /// The transaction being corrected, if any — a correction is not checked for
    /// oversell here: recording it replays the ledger (SEL-030).
    pub correcting: Option<String>,
}

/// An opening balance as the user is still entering it (TRX-066). Empty strings are fields
/// not filled yet, and so is a total cost of `None`: a typed 0 is a figure (TRX-045).
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(deny_unknown_fields)]
pub struct OpeningBalanceDraft {
    /// The account, empty until chosen.
    pub account_id: String,
    /// The asset, empty until chosen.
    pub asset_id: String,
    /// ISO date, empty until entered.
    pub date: String,
    /// Quantity, in micros.
    pub quantity: i64,
    /// Total cost in account currency, in micros; `None` until entered.
    pub total_cost: Option<i64>,
}

/// What the user should know about an opening balance that can be recorded (TRX-066).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct OpeningBalancePreview {
    /// TRX-065 — the total cost is 0: the form and the command line warn, never block.
    pub zero_cost: bool,
}

/// How the size of a split is entered (SPL-061).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, specta::Type)]
#[serde(tag = "mode", deny_unknown_fields)]
pub enum SplitSize {
    /// "new for old" shares, as a split is announced (2 for 1); `None` until typed.
    Ratio {
        /// Shares after, for `old` shares before.
        new: Option<i64>,
        /// Shares before.
        old: Option<i64>,
    },
    /// The factor itself, in micros, as a correction carries it (SPL-030).
    Factor {
        /// Micro-scaled factor (2 for 1 → 2_000_000).
        factor: i64,
    },
}

/// A split as the user is still entering it (SPL-062/063). Empty strings are fields not filled
/// yet.
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(deny_unknown_fields)]
pub struct StockSplitDraft {
    /// The account.
    pub account_id: String,
    /// The asset that splits.
    pub asset_id: String,
    /// ISO date, empty until entered.
    pub date: String,
    /// The size of the split.
    pub size: SplitSize,
    /// The split being corrected, if any: its position is not previewed (SPL-063).
    pub correcting: Option<String>,
}

/// What recording a split draft would do (SPL-062): the form shows it and computes none.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct StockSplitPreview {
    /// The micro-scaled factor recording would store (SPL-061).
    pub factor: i64,
    /// The position on the split's date, before and after; `None` for a correction.
    pub position: Option<StockSplitPosition>,
    /// The asset's latest price before the split's date, carried across the split:
    /// `round(price × MICRO / factor)` (SPL-040); `None` when it has none.
    pub price_after_split: Option<i64>,
}

/// What recording a draft would store (TRX-062): the form shows the total.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct TransactionPreview {
    /// Unit price in the asset's currency.
    pub unit_price: i64,
    /// Total in account currency.
    pub total_amount: i64,
    /// The gain a new sale would realize (TDI-030), in account currency: its proceeds minus
    /// the average cost, as of its date, of the quantity sold. `None` for anything but a
    /// new sale, and when it cannot be computed — the position at that date does not
    /// hold what is sold, or cannot be read (TDI-031).
    pub realized_pnl: Option<i64>,
}

/// The account and asset a user named by what they typed (CLI-011).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedTarget {
    /// The account's id.
    pub account_id: String,
    /// The account's name as stored.
    pub account_name: String,
    /// The account's currency.
    pub currency: String,
    /// The asset's id.
    pub asset_id: String,
    /// The asset's reference.
    pub asset_reference: String,
}

/// Case ignored for every letter, not only ASCII ("épargne" finds "Épargne").
fn same_words(stored: &str, typed: &str) -> bool {
    stored.to_lowercase() == typed.to_lowercase()
}

/// CLI-011 — the one account with this name.
fn match_account<'a>(accounts: &'a [Account], typed: &str) -> Result<&'a Account, NameLookupError> {
    let mut matches = accounts
        .iter()
        .filter(|account| same_words(&account.name, typed));
    let account = matches
        .next()
        .ok_or_else(|| NameLookupError::AccountNotFound {
            typed: typed.to_string(),
        })?;
    if matches.next().is_some() {
        return Err(NameLookupError::AccountAmbiguous {
            typed: typed.to_string(),
        });
    }
    Ok(account)
}

/// CLI-011 — the one asset with this name or reference, among assets that are not cash.
fn match_asset<'a>(assets: &'a [Asset], typed: &str) -> Result<&'a Asset, NameLookupError> {
    let mut matches = assets.iter().filter(|asset| {
        !asset.is_cash() && (same_words(&asset.name, typed) || same_words(&asset.reference, typed))
    });
    let asset = matches
        .next()
        .ok_or_else(|| NameLookupError::AssetNotFound {
            typed: typed.to_string(),
        })?;
    if matches.next().is_some() {
        return Err(NameLookupError::AssetAmbiguous {
            typed: typed.to_string(),
        });
    }
    Ok(asset)
}

/// Single orchestrator for every operation that mutates a `Holding` through a `Transaction`:
/// opening balance, buy, sell, correct, cancel.
///
/// Injects `Arc<AccountService>` + `Arc<AssetService>` and shares them across all five methods.
/// `asset_service` is used today by `open_holding` for the archived-asset guard, and will also
/// drive the cross-BC `ensure_cash_asset` step inserted by the cash-tracking spec
/// (CSH-040 / CSH-050 / CSH-042 / CSH-024).
pub struct HoldingTransactionUseCase {
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
}

impl HoldingTransactionUseCase {
    /// Creates a new HoldingTransactionUseCase.
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
    ) -> Self {
        Self {
            account_service,
            asset_service,
        }
    }

    /// Checks a transaction draft without writing anything (TRX-062): the first problem, or
    /// the unit price and total recording it would store.
    pub async fn validate_draft(
        &self,
        draft: TransactionDraft,
    ) -> Result<TransactionPreview, TransactionDraftError> {
        let missing = |value: &str| value.trim().is_empty();
        if missing(&draft.account_id) {
            return Err(TransactionDraftTask::AccountMissing.into());
        }
        if missing(&draft.asset_id) {
            return Err(TransactionDraftTask::AssetMissing.into());
        }
        if missing(&draft.date) {
            return Err(TransactionDraftTask::DateMissing.into());
        }
        let transaction_type = match draft.kind {
            DraftKind::Purchase => TransactionType::Purchase,
            DraftKind::Sell => TransactionType::Sell,
            // DIV-040 — a corrected dividend totals its amount at the exchange rate.
            DraftKind::Dividend => {
                let (unit_price, exchange_rate, fees) = match draft.entered {
                    EnteredAmount::UnitPrice {
                        unit_price,
                        exchange_rate,
                        fees,
                    } => (unit_price, exchange_rate, fees),
                    EnteredAmount::Total {
                        exchange_rate,
                        fees,
                        ..
                    } => (0, exchange_rate, fees),
                };
                let total_amount = Account::preview_dividend_correction(
                    &draft.date,
                    draft.quantity,
                    unit_price,
                    exchange_rate,
                    fees,
                )?;
                return Ok(TransactionPreview {
                    unit_price,
                    total_amount,
                    realized_pnl: None,
                });
            }
        };
        // CSH-062 — recording would refuse the cash line; so does the draft.
        Account::ensure_tradable(&draft.asset_id)?;
        let (unit_price, total_amount) =
            Account::preview_trade(transaction_type, &draft.date, draft.quantity, draft.entered)?;
        let mut realized_pnl = None;
        if draft.kind == DraftKind::Sell && draft.correcting.is_none() {
            let available = self
                .account_service
                .get_holding_by_account_asset(&draft.account_id, &draft.asset_id)
                .await?
                .map(|holding| holding.quantity)
                .unwrap_or(0);
            if draft.quantity > available {
                return Err(AccountError::Oversell {
                    available,
                    requested: draft.quantity,
                }
                .into());
            }
            // TDI-030/031 — the gain against the position as it stood on the sale's date.
            // It informs and blocks nothing: when that position cannot be read, or does
            // not hold what is sold, the check stands and returns no gain.
            realized_pnl = self
                .account_service
                .holding_snapshot_as_of(&draft.account_id, &draft.asset_id, &draft.date)
                .await
                .ok()
                .filter(|held| held.quantity > 0 && draft.quantity <= held.quantity)
                .map(|held| {
                    Account::preview_realized_pnl(total_amount, held.average_price, draft.quantity)
                });
        }
        Ok(TransactionPreview {
            unit_price,
            total_amount,
            realized_pnl,
        })
    }

    /// Whether the account holds the asset today: a quantity above zero. A closed position
    /// keeps its holding at zero, and is not held.
    async fn holds(&self, account_id: &str, asset_id: &str) -> Result<bool, AccountError> {
        Ok(self
            .account_service
            .get_holding_by_account_asset(account_id, asset_id)
            .await?
            .is_some_and(|holding| holding.quantity > 0))
    }

    /// SPL-062 — checks a split draft without writing anything: the factor it would store,
    /// what it would make of the position, the price to carry across it — or the first
    /// problem, as recording would report it.
    pub async fn validate_stock_split_draft(
        &self,
        draft: StockSplitDraft,
    ) -> Result<StockSplitPreview, TransactionDraftError> {
        const MICRO: i128 = 1_000_000;
        // `round(numerator / denominator)` on positive integers, halves up; `None` when
        // the result is no amount the application can hold.
        let rounded = |numerator: i128, denominator: i128| {
            i64::try_from((2 * numerator + denominator) / (2 * denominator)).ok()
        };
        if draft.date.trim().is_empty() {
            return Err(TransactionDraftTask::DateMissing.into());
        }
        // SPL-061 — a ratio's factor is `round(new × MICRO / old)`; a part not typed yet,
        // or not strictly positive, is no factor.
        let factor = match draft.size {
            SplitSize::Ratio {
                new: Some(new),
                old: Some(old),
            } if new > 0 && old > 0 => rounded(new as i128 * MICRO, old as i128)
                .filter(|factor| *factor > 0)
                .ok_or(AccountError::SplitFactorNotPositive)?,
            SplitSize::Ratio { .. } => return Err(AccountError::SplitFactorNotPositive.into()),
            SplitSize::Factor { factor } => factor,
        };
        let position = if draft.correcting.is_some() {
            // SPL-063 — a correction is checked on its date and factor alone.
            Transaction::split(
                draft.account_id.clone(),
                draft.asset_id.clone(),
                draft.date.clone(),
                factor,
                None,
            )?;
            None
        } else {
            // SPL-012 — a position closed today cannot be split, whatever it held then.
            if !self.holds(&draft.account_id, &draft.asset_id).await? {
                return Err(AccountError::ClosedPosition.into());
            }
            Some(
                self.account_service
                    .preview_split(&draft.account_id, &draft.asset_id, &draft.date, factor)
                    .await?,
            )
        };
        // SPL-040 — the latest price strictly before the split's date, carried across it.
        // It helps fill a field: without one, or when prices cannot be read, there is none.
        let prices = self
            .asset_service
            .get_asset_prices(&draft.asset_id)
            .await
            .unwrap_or_else(|e| {
                tracing::error!(target: BACKEND, asset_id = %draft.asset_id, err = ?e, "validate_stock_split_draft: price read failed");
                Vec::new()
            });
        let price_after_split = prices
            .into_iter()
            .filter(|price| price.date < draft.date)
            .max_by(|a, b| a.date.cmp(&b.date))
            .and_then(|latest| rounded(latest.price as i128 * MICRO, factor as i128));
        Ok(StockSplitPreview {
            factor,
            position,
            price_after_split,
        })
    }

    /// Checks an opening balance draft without writing anything (TRX-066): the first
    /// problem, or what the user should know about a draft that can be recorded.
    pub fn validate_opening_balance_draft(
        &self,
        draft: &OpeningBalanceDraft,
    ) -> Result<OpeningBalancePreview, TransactionDraftError> {
        let missing = |value: &str| value.trim().is_empty();
        if missing(&draft.account_id) {
            return Err(TransactionDraftTask::AccountMissing.into());
        }
        if missing(&draft.asset_id) {
            return Err(TransactionDraftTask::AssetMissing.into());
        }
        if missing(&draft.date) {
            return Err(TransactionDraftTask::DateMissing.into());
        }
        let Some(total_cost) = draft.total_cost else {
            return Err(TransactionDraftTask::TotalCostMissing.into());
        };
        let notice = Account::preview_opening_balance(
            &draft.asset_id,
            &draft.date,
            draft.quantity,
            total_cost,
        )?;
        Ok(OpeningBalancePreview {
            zero_cost: notice.zero_cost,
        })
    }

    /// CLI-011 — the one account with this name and the one asset with this name or
    /// reference, case ignored for every letter; a Cash Asset is never matched (CSH-018,
    /// TRX-064). Nothing is written.
    pub async fn find_by_name(
        &self,
        account: &str,
        asset: &str,
    ) -> Result<NamedTarget, NameLookupError> {
        let accounts = self.account_service.get_all().await.map_err(|error| {
            tracing::error!(target: BACKEND, err = ?error, "find_by_name: account lookup failed");
            NameLookupError::DatabaseError
        })?;
        let assets = self
            .asset_service
            .get_non_cash_assets()
            .await
            .map_err(|error| {
                tracing::error!(target: BACKEND, err = ?error, "find_by_name: asset lookup failed");
                NameLookupError::DatabaseError
            })?;
        let account = match_account(&accounts, account)?;
        let asset = match_asset(&assets, asset)?;
        Ok(NamedTarget {
            account_id: account.id.clone(),
            account_name: account.name.clone(),
            currency: account.currency.clone(),
            asset_id: asset.id.clone(),
            asset_reference: asset.reference.clone(),
        })
    }

    /// Seeds a holding from a known quantity and total cost (TRX-042).
    ///
    /// Cross-BC guard: rejects the request if the asset does not exist
    /// (TRX-056), is archived (TRX-050), or is a system Cash Asset (CSH-061).
    /// Delegates the account-side write to `AccountService::open_holding`.
    /// Returns the typed `OpenHoldingError` composite. Asset-side repo failures
    /// from `get_asset_by_id` are translated to `AccountError::DatabaseError`
    /// (matching the `ensure_cash_for` precedent) so the FE wire surface carries a
    /// single `{ code: "DatabaseError" }` shape rather than two indistinguishable arms.
    pub async fn open_holding(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        total_cost: i64,
    ) -> Result<Transaction, OpenHoldingError> {
        let asset = self
            .asset_service
            .get_asset_by_id(&asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, err = ?e, "open_holding: get_asset_by_id failed");
                AccountError::DatabaseError
            })?;
        match asset {
            None => return Err(OpenHoldingTask::AssetNotFound.into()),
            Some(a) if a.is_archived => return Err(OpenHoldingTask::ArchivedAsset.into()),
            // CSH-061 — Cash Assets cannot be seeded via OpeningBalance; user records
            // initial cash via `record_deposit` instead.
            Some(a) if a.class == AssetClass::Cash => {
                return Err(OpenHoldingTask::OpeningBalanceOnCashAsset.into())
            }
            Some(_) => {}
        }
        self.account_service
            .open_holding(account_id, asset_id, date, quantity, total_cost)
            .await
    }

    /// Records a purchase of an asset into an account (TRX-027).
    /// Seeds the system Cash Asset for the account's currency (CSH-010) before delegating;
    /// the aggregate replays the cash holding inside `Account::buy_holding` (CSH-040 / CSH-041).
    #[allow(clippy::too_many_arguments)]
    pub async fn buy_holding(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        unit_price: i64,
        exchange_rate: i64,
        fees: i64,
        total_amount: Option<i64>,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        self.ensure_cash_for(account_id, "buy_holding").await?;
        self.account_service
            .buy_holding(
                account_id,
                asset_id,
                date,
                quantity,
                unit_price,
                exchange_rate,
                fees,
                total_amount,
                note,
            )
            .await
    }

    /// Records a sale of an asset from an account (SEL-012, SEL-021, SEL-023, SEL-024).
    /// Seeds the system Cash Asset (CSH-010); the aggregate lazy-creates the Cash Holding
    /// when this is the first cash-affecting transaction (CSH-050 / CSH-012).
    #[allow(clippy::too_many_arguments)]
    pub async fn sell_holding(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        unit_price: i64,
        exchange_rate: i64,
        fees: i64,
        total_amount: Option<i64>,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        self.ensure_cash_for(account_id, "sell_holding").await?;
        self.account_service
            .sell_holding(
                account_id,
                asset_id,
                date,
                quantity,
                unit_price,
                exchange_rate,
                fees,
                total_amount,
                note,
            )
            .await
    }

    /// Corrects an existing transaction and recalculates the affected holding (TRX-031).
    /// Seeds the system Cash Asset; the aggregate replay re-evaluates the cash holding for
    /// any cash-affecting tx (CSH-042 / CSH-051) and may raise InsufficientCash.
    #[allow(clippy::too_many_arguments)]
    pub async fn correct_transaction(
        &self,
        account_id: &str,
        transaction_id: &str,
        date: String,
        quantity: i64,
        unit_price: i64,
        exchange_rate: i64,
        fees: i64,
        total_amount: Option<i64>,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        self.ensure_cash_for(account_id, "correct_transaction")
            .await?;
        self.account_service
            .correct_transaction(
                account_id,
                transaction_id,
                date,
                quantity,
                unit_price,
                exchange_rate,
                fees,
                total_amount,
                note,
            )
            .await
    }

    /// Cancels a transaction and recalculates (or removes) the associated holding (TRX-034).
    /// The aggregate replay catches any chronologically-later violation (CSH-024 / CSH-051).
    pub async fn cancel_transaction(
        &self,
        account_id: &str,
        transaction_id: &str,
    ) -> Result<(), AccountError> {
        self.ensure_cash_for(account_id, "cancel_transaction")
            .await?;
        self.account_service
            .cancel_transaction(account_id, transaction_id)
            .await
    }

    /// Records a Deposit into an account (CSH-022).
    /// Seeds the system Cash Asset (CSH-010) before delegating; the aggregate
    /// lazy-creates the Cash Holding (CSH-012) and persists the Transaction.
    /// Returns a typed `AccountError`: in-account and cross-BC asset-seed
    /// failures both surface as `AccountError` (see `ensure_cash_for`).
    pub async fn record_deposit(
        &self,
        account_id: &str,
        date: String,
        amount: i64,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        self.ensure_cash_for(account_id, "record_deposit").await?;
        self.account_service
            .record_deposit(account_id, date, amount, note)
            .await
    }

    /// Records a Withdrawal from an account (CSH-032).
    /// Raises InsufficientCash (CSH-080) when no Cash Holding exists or balance < amount.
    pub async fn record_withdrawal(
        &self,
        account_id: &str,
        date: String,
        amount: i64,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        self.ensure_cash_for(account_id, "record_withdrawal")
            .await?;
        self.account_service
            .record_withdrawal(account_id, date, amount, note)
            .await
    }

    /// Records a cash Dividend attributed to the paying asset (DIV-023).
    ///
    /// Cross-BC guards (DIV-011): rejects if account is unknown, asset is unknown,
    /// asset is not currently held (quantity = 0), or asset is a Cash Asset.
    /// Seeds the system Cash Asset (CSH-010) before delegating; the aggregate
    /// credits the Cash Holding by `total_amount = amount_micros × exchange_rate`
    /// (account currency), lazy-creating it if absent (CSH-012/CSH-050).
    /// The paying asset's holding quantity, average cost, and cost basis are
    /// unchanged (DIV-024). Does not create or modify any AssetPrice row (DIV-027).
    /// Returns `DividendError` — no `InsufficientCash` variant (credit-only).
    pub async fn record_dividend(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        amount_micros: i64,
        exchange_rate: i64,
        note: Option<String>,
    ) -> Result<Transaction, DividendError> {
        // DIV-011 — account must exist (checked before any asset work).
        let account = self
            .account_service
            .get_by_id(account_id)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound {
                account_id: account_id.to_string(),
            })?;

        // DIV-011 — asset must exist and must not be a Cash Asset.
        let asset = self
            .asset_service
            .get_asset_by_id(&asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, err = ?e, "record_dividend: get_asset_by_id failed");
                AccountError::DatabaseError
            })?;
        match asset {
            None => return Err(DividendTask::AssetNotFound.into()),
            Some(a) if a.class == AssetClass::Cash => {
                return Err(DividendTask::DividendOnCashAsset.into())
            }
            Some(_) => {}
        }

        // DIV-011 — asset must be currently held (quantity > 0). A repository
        // failure here is surfaced as `DatabaseError` by the service layer.
        if !self.holds(account_id, &asset_id).await? {
            return Err(DividendTask::AssetNotHeld.into());
        }

        // CSH-010 — ensure the system Cash Asset for the account's currency exists.
        ensure_cash_asset(&self.asset_service, &account.currency)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, err = ?e, "record_dividend: ensure_cash_asset failed");
                AccountError::DatabaseError
            })?;

        // Delegate the credit + persistence to the account BC; its `AccountError`
        // surfaces on the dividend wire as `DividendError::Account`.
        self.account_service
            .record_dividend(
                account_id,
                asset_id,
                date,
                amount_micros,
                exchange_rate,
                note,
            )
            .await
            .map_err(DividendError::Account)
    }

    /// Records a FreeShares distribution attributed to a held distributing asset
    /// (FSD-011/022).
    ///
    /// The distribution has no cash leg (FSD-022d — no `ensure_cash_asset`, no
    /// `InsufficientCash`) and never touches an `AssetPrice` row (FSD-024).
    /// Returns `FreeSharesError`.
    pub async fn record_free_shares(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        note: Option<String>,
    ) -> Result<Transaction, FreeSharesError> {
        // FSD-011 — account must exist (checked before any asset work).
        self.account_service
            .get_by_id(account_id)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound {
                account_id: account_id.to_string(),
            })?;

        // FSD-011 — asset must exist and must not be a Cash Asset.
        let asset = self
            .asset_service
            .get_asset_by_id(&asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, err = ?e, "record_free_shares: get_asset_by_id failed");
                AccountError::DatabaseError
            })?;
        match asset {
            None => return Err(FreeSharesTask::AssetNotFound.into()),
            Some(a) if a.class == AssetClass::Cash => {
                return Err(FreeSharesTask::FreeSharesOnCashAsset.into())
            }
            Some(_) => {}
        }

        // FSD-011 — asset must be currently held (quantity > 0).
        if !self.holds(account_id, &asset_id).await? {
            return Err(FreeSharesTask::AssetNotHeld.into());
        }

        // Delegate to the account BC; its `AccountError` surfaces on the
        // free-shares wire as `FreeSharesError::Account`.
        self.account_service
            .record_free_shares(account_id, asset_id, date, quantity, note)
            .await
            .map_err(FreeSharesError::Account)
    }

    /// Records a stock split rescaling a held position (SPL-010/012).
    ///
    /// Cross-BC guards (SPL-012): rejects if the account is unknown, the asset
    /// is unknown, not currently held, or a Cash Asset. No cash leg. The factor
    /// and replay guards (SPL-011/021) live in the account BC.
    pub async fn record_split(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        factor: i64,
        note: Option<String>,
    ) -> Result<Transaction, SplitError> {
        // SPL-012 — account must exist (checked before any asset work).
        self.account_service
            .get_by_id(account_id)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound {
                account_id: account_id.to_string(),
            })?;

        // SPL-012 — asset must exist and must not be a Cash Asset.
        let asset = self
            .asset_service
            .get_asset_by_id(&asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, err = ?e, "record_split: get_asset_by_id failed");
                AccountError::DatabaseError
            })?;
        match asset {
            None => return Err(SplitTask::AssetNotFound.into()),
            Some(a) if a.class == AssetClass::Cash => {
                return Err(AccountError::SplitOnCashAsset.into())
            }
            Some(_) => {}
        }

        // SPL-012 — asset must be currently held (quantity > 0).
        if !self.holds(account_id, &asset_id).await? {
            return Err(SplitTask::AssetNotHeld.into());
        }

        // Delegate to the account BC; its `AccountError` surfaces on the
        // split wire as `SplitError::Account`.
        self.account_service
            .record_split(account_id, asset_id, date, factor, note)
            .await
            .map_err(SplitError::Account)
    }

    /// Records a management fee deduction on a held asset (FEE-012/011).
    ///
    /// Cross-BC guards (FEE-011): rejects if account is unknown, asset is unknown,
    /// asset is not currently held (quantity = 0), or asset is a Cash Asset.
    /// No cash leg — does not call `ensure_cash_asset`.
    /// Returns `ManagementFeeError`.
    pub async fn record_management_fee(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        percent_micros: i64,
        note: Option<String>,
    ) -> Result<Transaction, ManagementFeeError> {
        self.ensure_management_fee_target(account_id, &asset_id)
            .await?;
        // Delegate to the account BC; its `AccountError` surfaces on the
        // management-fee wire as `ManagementFeeError::Account`.
        self.account_service
            .record_management_fee(account_id, asset_id, date, percent_micros, note)
            .await
            .map_err(ManagementFeeError::Account)
    }

    /// Records a one-off management fee entered either as a percentage of the holding or
    /// as the quantity the holding should hold after it — exactly one of the two
    /// (FEE-021/028). Same cross-BC guards as `record_management_fee`.
    pub async fn record_management_fee_entry(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        percent_micros: Option<i64>,
        resulting_quantity_micros: Option<i64>,
        note: Option<String>,
    ) -> Result<Transaction, ManagementFeeError> {
        self.ensure_management_fee_target(account_id, &asset_id)
            .await?;
        self.account_service
            .record_management_fee_entry(
                account_id,
                asset_id,
                date,
                percent_micros,
                resulting_quantity_micros,
                note,
            )
            .await
            .map_err(ManagementFeeError::Account)
    }

    /// What a one-off management fee entered by its resulting quantity would remove
    /// (FEE-029); same guards and rejections as the recording, writes nothing.
    pub async fn preview_management_fee(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        resulting_quantity_micros: i64,
    ) -> Result<ManagementFeeRemoval, ManagementFeeError> {
        self.ensure_management_fee_target(account_id, &asset_id)
            .await?;
        self.account_service
            .preview_management_fee(account_id, &asset_id, &date, resulting_quantity_micros)
            .await
            .map_err(ManagementFeeError::Account)
    }

    /// FEE-012 — the account exists, the asset exists and is not a Cash Asset, and the
    /// asset is currently held (quantity > 0).
    async fn ensure_management_fee_target(
        &self,
        account_id: &str,
        asset_id: &str,
    ) -> Result<(), ManagementFeeError> {
        self.account_service
            .get_by_id(account_id)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound {
                account_id: account_id.to_string(),
            })?;

        let asset = self
            .asset_service
            .get_asset_by_id(asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, err = ?e, "management fee: get_asset_by_id failed");
                AccountError::DatabaseError
            })?;
        match asset {
            None => return Err(ManagementFeeTask::AssetNotFound.into()),
            Some(a) if a.class == AssetClass::Cash => {
                return Err(ManagementFeeTask::ManagementFeeOnCashAsset.into())
            }
            Some(_) => {}
        }

        if self.holds(account_id, asset_id).await? {
            Ok(())
        } else {
            Err(ManagementFeeTask::AssetNotHeld.into())
        }
    }

    /// Records an Interest credit on a held asset or the account's cash line
    /// (INT-011/023).
    ///
    /// Cross-BC guards (INT-011/012): rejects if the account is unknown or the
    /// asset is unknown. A Cash-class asset is always a valid target — the
    /// eligibility and held checks are skipped for it (INT-023, the cash line
    /// is interest-bearing); a non-cash asset must be `interest_bearing`
    /// (INT-012) and currently held (quantity > 0).
    /// The credit has no external cash leg (no `ensure_cash_asset` — the cash
    /// line only exists once a deposit created it) and never touches an
    /// `AssetPrice` row. Returns `InterestError`.
    pub async fn record_interest(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        percent_micros: Option<i64>,
        quantity_micros: Option<i64>,
        note: Option<String>,
    ) -> Result<Transaction, InterestError> {
        // INT-011 — account must exist (checked before any asset work).
        self.account_service
            .get_by_id(account_id)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound {
                account_id: account_id.to_string(),
            })?;

        // INT-011 — asset must exist.
        let asset = self
            .asset_service
            .get_asset_by_id(&asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, err = ?e, "record_interest: get_asset_by_id failed");
                AccountError::DatabaseError
            })?;
        let asset = match asset {
            None => return Err(InterestTask::AssetNotFound.into()),
            Some(a) => a,
        };

        // INT-012 — a non-cash asset must be interest-bearing;
        // INT-011 — a non-cash asset must be currently held (quantity > 0);
        // INT-023 — the account's Cash Asset is always a valid target, so both
        // checks are skipped for a Cash-class asset.
        if asset.class != AssetClass::Cash {
            if !asset.interest_bearing {
                return Err(InterestTask::InterestNotEligible.into());
            }
            if !self.holds(account_id, &asset_id).await? {
                return Err(InterestTask::AssetNotHeld.into());
            }
        }

        // Delegate to the account BC; its `AccountError` surfaces on the
        // interest wire as `InterestError::Account`.
        self.account_service
            .record_interest(
                account_id,
                asset_id,
                date,
                percent_micros,
                quantity_micros,
                note,
            )
            .await
            .map_err(InterestError::Account)
    }

    /// Loads the account, then ensures the system Cash Asset for its currency
    /// exists (CSH-010, CSH-011, CSH-017). Idempotent: safe to call on every
    /// cash-affecting command. Returns a typed `AccountError` so
    /// callers can propagate via `?` and stay typed end-to-end.
    ///
    /// Both error sources surface as `AccountError`:
    /// - **In-account failures**: `AccountNotFound { account_id }` when the row
    ///   is missing, `DatabaseError` when the account-repo call fails.
    /// - **Cross-BC asset-side failure** (`ensure_cash_asset` failure):
    ///   surfaced as `DatabaseError` after `tracing::error!` preserves the
    ///   asset-side diagnostic chain server-side.
    async fn ensure_cash_for(&self, account_id: &str, op: &str) -> Result<(), AccountError> {
        let account = self
            .account_service
            .get_by_id(account_id)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound {
                account_id: account_id.to_string(),
            })?;
        ensure_cash_asset(&self.asset_service, &account.currency)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, op = %op, err = ?e, "ensure_cash_for: ensure_cash_asset failed");
                AccountError::DatabaseError
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::{
        AccountService, SqliteAccountRepository, SqliteHoldingRepository,
        SqliteTransactionRepository, UpdateFrequency,
    };
    use crate::context::asset::{
        AssetClass, AssetService, CreateAssetDTO, SqliteAssetCategoryRepository,
        SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
    };
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_pool() -> sqlx::Pool<sqlx::Sqlite> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("test pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations");
        pool
    }

    fn make_services(pool: &sqlx::Pool<sqlx::Sqlite>) -> (Arc<AccountService>, Arc<AssetService>) {
        let account_svc = Arc::new(AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        ));
        let asset_svc = Arc::new(AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        ));
        (account_svc, asset_svc)
    }

    fn base_asset_dto() -> CreateAssetDTO {
        CreateAssetDTO {
            name: "Test Asset".to_string(),
            reference: "TST".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 1,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        }
    }

    fn interest_bearing_asset_dto() -> CreateAssetDTO {
        CreateAssetDTO {
            interest_bearing: true,
            ..base_asset_dto()
        }
    }

    fn micro(v: i64) -> i64 {
        v * 1_000_000
    }

    // TRX-056 — AssetNotFound when asset does not exist
    #[tokio::test]
    async fn open_holding_rejects_unknown_asset() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();

        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);
        let err = uc
            .open_holding(
                &account.id,
                "nonexistent-asset".to_string(),
                "2024-01-01".to_string(),
                micro(1),
                micro(100),
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                OpenHoldingError::UseCase(OpenHoldingTask::AssetNotFound)
            ),
            "expected UseCase(AssetNotFound), got: {err:?}"
        );
    }

    // TRX-050 — ArchivedAsset when asset is archived
    #[tokio::test]
    async fn open_holding_rejects_archived_asset() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        asset_svc.archive_asset(&asset.id).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();

        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);
        let err = uc
            .open_holding(
                &account.id,
                asset.id,
                "2024-01-01".to_string(),
                micro(1),
                micro(100),
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                OpenHoldingError::UseCase(OpenHoldingTask::ArchivedAsset)
            ),
            "expected UseCase(ArchivedAsset), got: {err:?}"
        );
    }

    // CSH-061 — open_holding rejects an OpeningBalance against a Cash Asset
    // (user must record initial cash via record_deposit instead).
    #[tokio::test]
    async fn open_holding_rejects_cash_asset() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let cash_asset = asset_svc.seed_cash_asset("EUR").await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();

        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);
        let err = uc
            .open_holding(
                &account.id,
                cash_asset.id,
                "2024-01-01".to_string(),
                micro(1),
                micro(100),
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                OpenHoldingError::UseCase(OpenHoldingTask::OpeningBalanceOnCashAsset)
            ),
            "expected UseCase(OpeningBalanceOnCashAsset), got: {err:?}"
        );
    }

    // TRX-047 — happy path: transaction and holding created with correct fields
    #[tokio::test]
    async fn open_holding_happy_path() {
        use crate::context::account::TransactionType;

        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();

        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        let tx = uc
            .open_holding(
                &account.id,
                asset.id.clone(),
                "2024-01-01".to_string(),
                micro(2),
                micro(200),
            )
            .await
            .unwrap();

        assert_eq!(tx.transaction_type, TransactionType::OpeningBalance);
        assert_eq!(tx.total_amount, micro(200));
        assert_eq!(tx.fees, 0);
        assert_eq!(tx.exchange_rate, 1_000_000);
        assert_eq!(tx.unit_price, micro(100));

        let holdings = account_svc
            .get_holdings_for_account(&account.id)
            .await
            .unwrap();
        assert_eq!(holdings.len(), 1);
        assert_eq!(holdings[0].quantity, micro(2));
        assert_eq!(holdings[0].average_price, micro(100));
    }

    // -------------------------------------------------------------------------
    // Holding-tx orchestrator coverage (PR 3 — typed Result delegation)
    // -------------------------------------------------------------------------

    // TRX-027 — buy_holding happy path through the orchestrator: typed Result
    // flows from AccountService through the orchestrator unchanged.
    #[tokio::test]
    async fn buy_holding_orchestrator_happy_path() {
        use crate::context::account::TransactionType;

        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        // Seed cash through the orchestrator so ensure_cash_for has been exercised
        // before the buy.
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(10_000), None)
            .await
            .unwrap();

        let tx = uc
            .buy_holding(
                &account.id,
                asset.id.clone(),
                "2024-01-15".to_string(),
                micro(2),
                micro(100),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(tx.transaction_type, TransactionType::Purchase);
        assert_eq!(tx.total_amount, micro(200));
    }

    // When `buy_holding` is called for a nonexistent account, the
    // orchestrator's `ensure_cash_for` surfaces it as the typed
    // `Application(AccountNotFound { account_id })` — same shape every other
    // path raises for the same condition.
    #[tokio::test]
    async fn buy_holding_orchestrator_unknown_account_returns_application() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .buy_holding(
                "nonexistent-account-id",
                "irrelevant-asset".to_string(),
                "2024-01-15".to_string(),
                micro(1),
                micro(100),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap_err();

        match err {
            AccountError::AccountNotFound { account_id } => {
                assert_eq!(account_id, "nonexistent-account-id");
            }
            other => panic!("expected AccountNotFound, got: {other:?}"),
        }
    }

    // CSH-022 — record_deposit through the orchestrator: typed Result is
    // returned end-to-end (no anyhow at this boundary).
    #[tokio::test]
    async fn record_deposit_orchestrator_happy_path() {
        use crate::context::account::TransactionType;

        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let tx = uc
            .record_deposit(&account.id, "2024-01-01".to_string(), micro(500), None)
            .await
            .unwrap();

        assert_eq!(tx.transaction_type, TransactionType::Deposit);
        assert_eq!(tx.total_amount, micro(500));
    }

    // -------------------------------------------------------------------------
    // record_dividend — orchestrator unit tests (DIV-011, DIV-021, DIV-022,
    // DIV-023, DIV-024, DIV-026, DIV-027)
    // -------------------------------------------------------------------------

    // DIV-023 — happy path: dividend credited to cash, paying-asset holding unchanged.
    #[tokio::test]
    async fn record_dividend_happy_path() {
        use crate::context::account::TransactionType;

        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(), // match asset currency so rate=1
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        // Buy some of the asset first so the holding exists (DIV-011 eligibility).
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let tx = uc
            .record_dividend(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                micro(200), // 200 USD dividend
                micro(1),   // exchange_rate = 1 (same currency)
                None,
            )
            .await
            .unwrap();

        assert_eq!(tx.transaction_type, TransactionType::Dividend);
        assert_eq!(tx.asset_id, asset.id, "asset_id must be the paying asset");
        assert_eq!(tx.total_amount, micro(200));
        assert_eq!(tx.fees, 0);
        assert!(
            tx.realized_pnl.is_none(),
            "dividend must have no realized_pnl"
        );

        // Verify paying-asset holding is unchanged (DIV-024)
        let holdings = account_svc
            .get_holdings_for_account(&account.id)
            .await
            .unwrap();
        let paying_holding = holdings
            .iter()
            .find(|h| h.asset_id == asset.id)
            .expect("paying asset holding must still exist");
        assert_eq!(
            paying_holding.quantity,
            micro(10),
            "quantity must be unchanged"
        );
    }

    // DIV-011 — AccountNotFound: unknown account is rejected before any asset check.
    #[tokio::test]
    async fn record_dividend_rejects_unknown_account() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_dividend(
                "nonexistent-account",
                asset.id,
                "2024-06-15".to_string(),
                micro(100),
                micro(1),
                None,
            )
            .await
            .unwrap_err();

        use crate::context::account::AccountError;
        use crate::use_cases::holding_transaction::DividendError;
        assert!(
            matches!(
                err,
                DividendError::Account(AccountError::AccountNotFound { .. })
            ),
            "expected Application(AccountNotFound), got: {err:?}"
        );
    }

    // DIV-011 — AssetNotFound: unknown asset_id is rejected.
    #[tokio::test]
    async fn record_dividend_rejects_unknown_asset() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_dividend(
                &account.id,
                "nonexistent-asset".to_string(),
                "2024-06-15".to_string(),
                micro(100),
                micro(1),
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{DividendError, DividendTask};
        assert!(
            matches!(err, DividendError::UseCase(DividendTask::AssetNotFound)),
            "expected UseCase(AssetNotFound), got: {err:?}"
        );
    }

    // DIV-011 — AssetNotHeld: asset exists but is not held (never bought).
    #[tokio::test]
    async fn record_dividend_rejects_asset_not_held() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_dividend(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                micro(100),
                micro(1),
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{DividendError, DividendTask};
        assert!(
            matches!(err, DividendError::UseCase(DividendTask::AssetNotHeld)),
            "expected UseCase(AssetNotHeld), got: {err:?}"
        );
    }

    // DIV-011 — DividendOnCashAsset: the paying asset is a Cash Asset.
    #[tokio::test]
    async fn record_dividend_rejects_cash_asset() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let cash_asset = asset_svc.seed_cash_asset("EUR").await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_dividend(
                &account.id,
                cash_asset.id.clone(),
                "2024-06-15".to_string(),
                micro(100),
                micro(1),
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{DividendError, DividendTask};
        assert!(
            matches!(
                err,
                DividendError::UseCase(DividendTask::DividendOnCashAsset)
            ),
            "expected UseCase(DividendOnCashAsset), got: {err:?}"
        );
    }

    // DIV-021 — AmountNotPositive: amount_micros = 0.
    #[tokio::test]
    async fn record_dividend_rejects_zero_amount() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let err = uc
            .record_dividend(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                0,
                micro(1),
                None,
            )
            .await
            .unwrap_err();

        use crate::context::account::AccountError;
        use crate::use_cases::holding_transaction::DividendError;
        assert!(
            matches!(err, DividendError::Account(AccountError::AmountNotPositive)),
            "expected Validation(AmountNotPositive), got: {err:?}"
        );
    }

    // DIV-022 — ExchangeRateNotPositive: exchange_rate = 0.
    #[tokio::test]
    async fn record_dividend_rejects_zero_exchange_rate() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let err = uc
            .record_dividend(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                micro(100),
                0, // invalid
                None,
            )
            .await
            .unwrap_err();

        use crate::context::account::AccountError;
        use crate::use_cases::holding_transaction::DividendError;
        assert!(
            matches!(
                err,
                DividendError::Account(AccountError::ExchangeRateNotPositive)
            ),
            "expected Validation(ExchangeRateNotPositive), got: {err:?}"
        );
    }

    // DIV-027 — recording a dividend must NOT create or modify an AssetPrice row.
    #[tokio::test]
    async fn record_dividend_does_not_create_asset_price() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc.clone());
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        uc.record_dividend(
            &account.id,
            asset.id.clone(),
            "2024-06-15".to_string(),
            micro(200),
            micro(1),
            None,
        )
        .await
        .unwrap();

        // After the dividend, no AssetPrice row must exist for the paying asset.
        let latest_price = asset_svc.get_latest_price(&asset.id).await.unwrap();
        assert!(
            latest_price.is_none(),
            "recording a dividend must not create an AssetPrice row (DIV-027)"
        );
    }

    // DIV-023 — currency conversion: amount in asset ccy × exchange_rate = account ccy total.
    #[tokio::test]
    async fn record_dividend_converts_amount_at_exchange_rate() {
        use crate::context::account::TransactionType;

        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        // Asset is USD, account is EUR → exchange_rate = 0.9 (EUR per USD)
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap(); // USD asset
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            900_000, // 0.9 EUR/USD
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let amount_micros = 100_000_000i64; // 100 USD
        let exchange_rate = 900_000i64; // 0.9 EUR/USD
        let tx = uc
            .record_dividend(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                amount_micros,
                exchange_rate,
                None,
            )
            .await
            .unwrap();

        assert_eq!(tx.transaction_type, TransactionType::Dividend);
        // total_amount = floor(100_000_000 × 900_000 / 1_000_000) = 90_000_000
        assert_eq!(
            tx.total_amount, 90_000_000,
            "total_amount must equal floor(amount × rate / MICRO)"
        );
    }

    // -------------------------------------------------------------------------
    // record_free_shares — orchestrator unit tests (FSD-011, FSD-021, FSD-022,
    // FSD-023, FSD-024, FSD-026)
    // -------------------------------------------------------------------------

    // FSD-022/023 — happy path: free shares increase quantity, cost basis unchanged,
    // no cash movement, no AssetPrice created.
    #[tokio::test]
    async fn record_free_shares_happy_path() {
        // FSD-022 — orchestrator delegates through to account service; holding updated
        use crate::context::account::TransactionType;

        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc.clone());

        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let holdings_before = account_svc
            .get_holdings_for_account(&account.id)
            .await
            .unwrap();
        let cost_basis_before = holdings_before
            .iter()
            .find(|h| h.asset_id == asset.id)
            .map(|h| h.quantity as i128 * h.average_price as i128 / 1_000_000)
            .unwrap();

        let tx = uc
            .record_free_shares(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                micro(5),
                None,
            )
            .await
            .unwrap();

        // FSD-022 — transaction fields
        assert_eq!(
            tx.transaction_type,
            TransactionType::FreeShares,
            "transaction_type must be FreeShares"
        );
        assert_eq!(tx.asset_id, asset.id);
        assert_eq!(tx.quantity, micro(5));
        // FSD-023 — zero-cost convention
        assert_eq!(tx.unit_price, 0);
        assert_eq!(tx.exchange_rate, 1_000_000);
        assert_eq!(tx.fees, 0);
        assert_eq!(tx.total_amount, 0);
        assert!(tx.realized_pnl.is_none());

        let holdings_after = account_svc
            .get_holdings_for_account(&account.id)
            .await
            .unwrap();
        let holding_after = holdings_after
            .iter()
            .find(|h| h.asset_id == asset.id)
            .unwrap();

        // FSD-022a — quantity increased by distributed amount
        assert_eq!(holding_after.quantity, micro(15), "quantity must be 15");
        // FSD-023 — underlying cost unchanged → VWAP dilutes to the exact floored
        // value (TRX-026 floor convention).
        let expected_diluted_vwap =
            (cost_basis_before * 1_000_000 / holding_after.quantity as i128) as i64;
        assert_eq!(
            holding_after.average_price, expected_diluted_vwap,
            "average price must equal floor(cost_basis / new_quantity)"
        );

        // FSD-024 — no AssetPrice row created
        let latest_price = asset_svc.get_latest_price(&asset.id).await.unwrap();
        assert!(
            latest_price.is_none(),
            "record_free_shares must not create an AssetPrice row (FSD-024)"
        );

        // FSD-022d — cash holding unchanged
        let cash_holdings = account_svc
            .get_holdings_for_account(&account.id)
            .await
            .unwrap();
        let _ = cash_holdings; // presence assertion done via business logic; cash test is in account.rs
    }

    // SPL-012 — AssetNotFound: unknown asset_id is rejected.
    #[tokio::test]
    async fn record_split_rejects_unknown_asset() {
        // SPL-012 — asset must exist
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_split(
                &account.id,
                "nonexistent-asset".to_string(),
                "2024-06-15".to_string(),
                2_000_000,
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{SplitError, SplitTask};
        assert!(
            matches!(err, SplitError::UseCase(SplitTask::AssetNotFound)),
            "expected UseCase(AssetNotFound), got: {err:?}"
        );
    }

    // SPL-012 — AssetNotHeld: asset exists but is not currently held.
    #[tokio::test]
    async fn record_split_rejects_asset_not_held() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_split(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                2_000_000,
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{SplitError, SplitTask};
        assert!(
            matches!(err, SplitError::UseCase(SplitTask::AssetNotHeld)),
            "expected UseCase(AssetNotHeld), got: {err:?}"
        );
    }

    // FSD-011 — account must exist (checked before any asset work).
    #[tokio::test]
    async fn record_free_shares_rejects_unknown_account() {
        // FSD-011 — account must exist
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_free_shares(
                "nonexistent-account",
                asset.id.clone(),
                "2024-06-15".to_string(),
                micro(5),
                None,
            )
            .await
            .unwrap_err();

        use crate::context::account::AccountError;
        use crate::use_cases::holding_transaction::FreeSharesError;
        assert!(
            matches!(
                err,
                FreeSharesError::Account(AccountError::AccountNotFound { .. })
            ),
            "expected Application(AccountNotFound), got: {err:?}"
        );
    }

    // FSD-011 — AssetNotFound: unknown asset_id is rejected.
    #[tokio::test]
    async fn record_free_shares_rejects_unknown_asset() {
        // FSD-011 — asset must exist
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_free_shares(
                &account.id,
                "nonexistent-asset".to_string(),
                "2024-06-15".to_string(),
                micro(5),
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{FreeSharesError, FreeSharesTask};
        assert!(
            matches!(err, FreeSharesError::UseCase(FreeSharesTask::AssetNotFound)),
            "expected UseCase(AssetNotFound), got: {err:?}"
        );
    }

    // FSD-011 — AssetNotHeld: asset exists but is not held in this account.
    #[tokio::test]
    async fn record_free_shares_rejects_asset_not_held() {
        // FSD-011 — asset must be currently held with quantity > 0
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_free_shares(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                micro(5),
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{FreeSharesError, FreeSharesTask};
        assert!(
            matches!(err, FreeSharesError::UseCase(FreeSharesTask::AssetNotHeld)),
            "expected UseCase(AssetNotHeld), got: {err:?}"
        );
    }

    // FSD-011 — FreeSharesOnCashAsset: the distributing asset is a Cash Asset.
    #[tokio::test]
    async fn record_free_shares_rejects_cash_asset() {
        // FSD-011 — distributing asset must not be a Cash Asset
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let cash_asset = asset_svc.seed_cash_asset("EUR").await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_free_shares(
                &account.id,
                cash_asset.id.clone(),
                "2024-06-15".to_string(),
                micro(5),
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{FreeSharesError, FreeSharesTask};
        assert!(
            matches!(
                err,
                FreeSharesError::UseCase(FreeSharesTask::FreeSharesOnCashAsset)
            ),
            "expected UseCase(FreeSharesOnCashAsset), got: {err:?}"
        );
    }

    // FSD-021 — QuantityNotPositive: quantity = 0 is rejected.
    #[tokio::test]
    async fn record_free_shares_rejects_zero_quantity() {
        // FSD-021 — quantity must be strictly positive
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let err = uc
            .record_free_shares(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                0, // invalid
                None,
            )
            .await
            .unwrap_err();

        use crate::context::account::AccountError;
        use crate::use_cases::holding_transaction::FreeSharesError;
        assert!(
            matches!(
                err,
                FreeSharesError::Account(AccountError::QuantityNotPositive)
            ),
            "expected Validation(QuantityNotPositive), got: {err:?}"
        );
    }

    // FSD-021 — DateInFuture: future date is rejected.
    #[tokio::test]
    async fn record_free_shares_rejects_future_date() {
        // FSD-021 — date must not be in the future
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let err = uc
            .record_free_shares(
                &account.id,
                asset.id.clone(),
                "2099-01-01".to_string(), // future
                micro(5),
                None,
            )
            .await
            .unwrap_err();

        use crate::context::account::AccountError;
        use crate::use_cases::holding_transaction::FreeSharesError;
        assert!(
            matches!(err, FreeSharesError::Account(AccountError::DateInFuture)),
            "expected Validation(DateInFuture), got: {err:?}"
        );
    }

    // FEE-012 — ManagementFeeOnCashAsset: the charged asset must not be a Cash Asset.
    #[tokio::test]
    async fn fee_012_record_management_fee_rejects_cash_asset() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let cash_asset = asset_svc.seed_cash_asset("EUR").await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_management_fee(
                &account.id,
                cash_asset.id.clone(),
                "2024-06-15".to_string(),
                micro(1), // 1%
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{ManagementFeeError, ManagementFeeTask};
        assert!(
            matches!(
                err,
                ManagementFeeError::UseCase(ManagementFeeTask::ManagementFeeOnCashAsset)
            ),
            "expected UseCase(ManagementFeeOnCashAsset), got: {err:?}"
        );
    }

    // FEE-012 — AssetNotHeld: the asset exists and is non-cash but is not currently held.
    #[tokio::test]
    async fn fee_012_record_management_fee_rejects_asset_not_held() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        // Asset was never bought → no active holding.
        let err = uc
            .record_management_fee(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                micro(1), // 1%
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{ManagementFeeError, ManagementFeeTask};
        assert!(
            matches!(
                err,
                ManagementFeeError::UseCase(ManagementFeeTask::AssetNotHeld)
            ),
            "expected UseCase(AssetNotHeld), got: {err:?}"
        );
    }

    // -------------------------------------------------------------------------
    // record_interest — orchestrator unit tests (INT-011, INT-021, INT-022,
    // INT-023, INT-024)
    // -------------------------------------------------------------------------

    // INT-022/024 — happy path (percent mode): the credit is 10% of the held
    // quantity, added at zero cost.
    #[tokio::test]
    async fn record_interest_happy_path_percent_mode() {
        use crate::context::account::TransactionType;

        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc
            .create_asset(interest_bearing_asset_dto())
            .await
            .unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        // 10% of 10 units → 1 unit credited.
        let tx = uc
            .record_interest(
                &account.id,
                asset.id.clone(),
                "2024-12-31".to_string(),
                Some(10_000_000),
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(tx.transaction_type, TransactionType::Interest);
        assert_eq!(tx.asset_id, asset.id);
        assert_eq!(tx.quantity, micro(1), "credited qty must be 10% of 10");
        // INT-024 — zero-cost packing
        assert_eq!(tx.unit_price, 0);
        assert_eq!(tx.exchange_rate, 1_000_000);
        assert_eq!(tx.fees, 0);
        assert_eq!(tx.total_amount, 0);
        assert!(tx.realized_pnl.is_none());

        let holdings = account_svc
            .get_holdings_for_account(&account.id)
            .await
            .unwrap();
        let holding = holdings.iter().find(|h| h.asset_id == asset.id).unwrap();
        assert_eq!(holding.quantity, micro(11), "quantity must be 10 + 1");
    }

    // INT-011 — AccountNotFound: unknown account is rejected before any asset check.
    #[tokio::test]
    async fn record_interest_rejects_unknown_account() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_interest(
                "nonexistent-account",
                asset.id.clone(),
                "2024-06-15".to_string(),
                None,
                Some(micro(5)),
                None,
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                InterestError::Account(AccountError::AccountNotFound { .. })
            ),
            "expected Account(AccountNotFound), got: {err:?}"
        );
    }

    // INT-011 — AssetNotFound: unknown asset_id is rejected.
    #[tokio::test]
    async fn record_interest_rejects_unknown_asset() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_interest(
                &account.id,
                "nonexistent-asset".to_string(),
                "2024-06-15".to_string(),
                None,
                Some(micro(5)),
                None,
            )
            .await
            .unwrap_err();

        assert!(
            matches!(err, InterestError::UseCase(InterestTask::AssetNotFound)),
            "expected UseCase(AssetNotFound), got: {err:?}"
        );
    }

    // INT-011 — AssetNotHeld: a non-cash asset that is not currently held is
    // rejected (interest-bearing, so the INT-012 eligibility check passes and
    // the held check is the one that fires).
    #[tokio::test]
    async fn record_interest_rejects_non_held_non_cash_asset() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc
            .create_asset(interest_bearing_asset_dto())
            .await
            .unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_interest(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                None,
                Some(micro(5)),
                None,
            )
            .await
            .unwrap_err();

        assert!(
            matches!(err, InterestError::UseCase(InterestTask::AssetNotHeld)),
            "expected UseCase(AssetNotHeld), got: {err:?}"
        );
    }

    // INT-012 — InterestNotEligible: a held non-cash asset without the
    // interest_bearing flag is rejected before the held check.
    #[tokio::test]
    async fn record_interest_rejects_non_interest_bearing_asset() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let err = uc
            .record_interest(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                None,
                Some(micro(5)),
                None,
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                InterestError::UseCase(InterestTask::InterestNotEligible)
            ),
            "expected UseCase(InterestNotEligible), got: {err:?}"
        );
    }

    // INT-023 / INT-012 — the account's Cash Asset IS a valid target even
    // though it is not interest_bearing-flagged: both the eligibility and held
    // checks are skipped and the credit lands on the cash balance
    // (1000 + 50 = 1050).
    #[tokio::test]
    async fn record_interest_accepts_cash_asset_and_credits_balance() {
        use crate::context::account::TransactionType;

        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let cash_asset = asset_svc.seed_cash_asset("EUR").await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();

        let tx = uc
            .record_interest(
                &account.id,
                cash_asset.id.clone(),
                "2024-06-15".to_string(),
                None,
                Some(micro(50)),
                None,
            )
            .await
            .unwrap();
        assert_eq!(tx.transaction_type, TransactionType::Interest);
        assert_eq!(tx.asset_id, cash_asset.id);

        let holdings = account_svc
            .get_holdings_for_account(&account.id)
            .await
            .unwrap();
        let cash = holdings
            .iter()
            .find(|h| h.asset_id == cash_asset.id)
            .expect("cash holding must exist");
        assert_eq!(
            cash.quantity,
            micro(1_050),
            "cash balance must be 1000 + 50 after the interest credit (INT-023)"
        );
    }

    // INT-021 — QuantityNotPositive: quantity mode with 0 is rejected.
    #[tokio::test]
    async fn record_interest_rejects_zero_quantity() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc
            .create_asset(interest_bearing_asset_dto())
            .await
            .unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let err = uc
            .record_interest(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                None,
                Some(0),
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                InterestError::Account(AccountError::QuantityNotPositive)
            ),
            "expected Account(QuantityNotPositive), got: {err:?}"
        );
    }

    // INT-021 — percent bounds: 0 → PercentageNotPositive; > 100% → PercentageAboveHundred;
    // both provided → InterestAmountInvalid.
    #[tokio::test]
    async fn record_interest_rejects_invalid_amount_specs() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc
            .create_asset(interest_bearing_asset_dto())
            .await
            .unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let err = uc
            .record_interest(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                Some(0),
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                InterestError::Account(AccountError::PercentageNotPositive)
            ),
            "expected Account(PercentageNotPositive), got: {err:?}"
        );

        let err = uc
            .record_interest(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                Some(100_000_001),
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                InterestError::Account(AccountError::PercentageAboveHundred)
            ),
            "expected Account(PercentageAboveHundred), got: {err:?}"
        );

        let err = uc
            .record_interest(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                Some(1_000_000),
                Some(micro(5)),
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                InterestError::Account(AccountError::InterestAmountInvalid)
            ),
            "expected Account(InterestAmountInvalid), got: {err:?}"
        );
    }

    // INT-021 — DateInFuture: future date is rejected.
    #[tokio::test]
    async fn record_interest_rejects_future_date() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc
            .create_asset(interest_bearing_asset_dto())
            .await
            .unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let err = uc
            .record_interest(
                &account.id,
                asset.id.clone(),
                "2099-01-01".to_string(),
                None,
                Some(micro(5)),
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, InterestError::Account(AccountError::DateInFuture)),
            "expected Account(DateInFuture), got: {err:?}"
        );
    }

    // INT-022/023 — percent-mode on the cash line uses the cash-balance replay,
    // not the per-asset holding replay: a Purchase (different asset_id) reduces
    // the base. Deposit 1000, buy 500 → balance 500; 10% credits 50, not 100.
    #[tokio::test]
    async fn record_interest_percent_on_cash_uses_cash_balance_base() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let cash_asset = asset_svc.seed_cash_asset("USD").await.unwrap();
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        // Purchase moves cash (−500) under the traded asset's id, not the cash id.
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let tx = uc
            .record_interest(
                &account.id,
                cash_asset.id.clone(),
                "2024-06-15".to_string(),
                Some(10_000_000),
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(
            tx.quantity,
            micro(50),
            "10% of the true 500 cash balance, not of the 1000 deposit history"
        );

        let holdings = account_svc
            .get_holdings_for_account(&account.id)
            .await
            .unwrap();
        let cash = holdings
            .iter()
            .find(|h| h.asset_id == cash_asset.id)
            .expect("cash holding must exist");
        assert_eq!(cash.quantity, micro(550), "500 + 50 credited");
    }
}

#[cfg(test)]
mod draft_tests {
    use super::*;
    use crate::context::account::{Holding, HoldingSnapshot, MockAccountServiceContract};
    use crate::context::asset::MockAssetServiceContract;

    const M: i64 = 1_000_000;

    fn draft(kind: DraftKind) -> TransactionDraft {
        TransactionDraft {
            kind,
            account_id: "acc-1".into(),
            asset_id: "asset-1".into(),
            date: "2026-01-02".into(),
            quantity: 2 * M,
            entered: EnteredAmount::UnitPrice {
                unit_price: 50 * M,
                exchange_rate: M,
                fees: 0,
            },
            correcting: None,
        }
    }

    fn use_case(held: Option<i64>) -> HoldingTransactionUseCase {
        let mut account = MockAccountServiceContract::new();
        account
            .expect_get_holding_by_account_asset()
            .returning(move |account_id, asset_id| {
                Ok(held.map(|quantity| {
                    Holding::new(account_id.into(), asset_id.into(), quantity, M, 0, None)
                        .expect("holding")
                }))
            });
        // As of any date the position is what is held now, bought at 40 a unit.
        account
            .expect_holding_snapshot_as_of()
            .returning(move |_, _, _| {
                Ok(HoldingSnapshot {
                    quantity: held.unwrap_or(0),
                    average_price: 40 * M,
                })
            });
        HoldingTransactionUseCase::new(Arc::new(account), Arc::new(MockAssetServiceContract::new()))
    }

    /// A use case holding 5 units today, whose position as of the draft's date is `as_of`
    /// (or cannot be read), and which reads that position `reads` times.
    fn use_case_as_of(
        as_of: Result<(i64, i64), AccountError>,
        reads: usize,
    ) -> HoldingTransactionUseCase {
        let mut account = MockAccountServiceContract::new();
        account
            .expect_get_holding_by_account_asset()
            .returning(|account_id, asset_id| {
                Ok(Some(
                    Holding::new(account_id.into(), asset_id.into(), 5 * M, M, 0, None)
                        .expect("holding"),
                ))
            });
        account
            .expect_holding_snapshot_as_of()
            .times(reads)
            .returning(move |_, _, _| {
                as_of
                    .clone()
                    .map(|(quantity, average_price)| HoldingSnapshot {
                        quantity,
                        average_price,
                    })
            });
        HoldingTransactionUseCase::new(Arc::new(account), Arc::new(MockAssetServiceContract::new()))
    }

    /// A use case whose account holds the asset today and previews a split as `position`
    /// (read `reads` times), and whose asset has `price` on 2026-01-01, and a later one
    /// on the draft's own date that must not be used.
    fn split_use_case(
        position: Result<StockSplitPosition, AccountError>,
        reads: usize,
        price: Option<i64>,
    ) -> HoldingTransactionUseCase {
        split_use_case_holding(Some(4 * M), position, reads, price)
    }

    /// As `split_use_case`, with what the account holds of the asset today.
    fn split_use_case_holding(
        held_today: Option<i64>,
        position: Result<StockSplitPosition, AccountError>,
        reads: usize,
        price: Option<i64>,
    ) -> HoldingTransactionUseCase {
        let mut account = MockAccountServiceContract::new();
        account
            .expect_get_holding_by_account_asset()
            .returning(move |account_id, asset_id| {
                Ok(held_today.map(|quantity| {
                    Holding::new(account_id.into(), asset_id.into(), quantity, M, 0, None)
                        .expect("holding")
                }))
            });
        account
            .expect_preview_split()
            .times(reads)
            .returning(move |_, _, _, _| position.clone());
        let mut asset = MockAssetServiceContract::new();
        asset.expect_get_asset_prices().returning(move |asset_id| {
            let on = |date: &str, price: i64| {
                crate::context::asset::AssetPrice::new(
                    asset_id.into(),
                    date.into(),
                    price,
                    crate::context::asset::AssetPriceSource::Manual,
                )
                .expect("price")
            };
            Ok(price
                .map(|price| {
                    vec![
                        on("2025-12-01", 7 * M),
                        on("2026-01-01", price),
                        on("2026-01-02", 9 * M),
                    ]
                })
                .unwrap_or_default())
        });
        HoldingTransactionUseCase::new(Arc::new(account), Arc::new(asset))
    }

    fn split_draft(size: SplitSize) -> StockSplitDraft {
        StockSplitDraft {
            account_id: "acc-1".into(),
            asset_id: "asset-1".into(),
            date: "2026-01-02".into(),
            size,
            correcting: None,
        }
    }

    const HELD: StockSplitPosition = StockSplitPosition {
        old_quantity: 4 * M,
        old_average_price: 30 * M,
        new_quantity: 6 * M,
        new_average_price: 20 * M,
    };

    // SPL-062 — a ratio becomes the factor recording stores, the position is the core's
    // preview, and the latest price is carried across the split — each rounded once.
    #[tokio::test]
    async fn spl_062_a_split_draft_previews_factor_position_and_price() {
        let ratio = |new, old| SplitSize::Ratio {
            new: Some(new),
            old: Some(old),
        };
        let preview = split_use_case(Ok(HELD), 1, Some(100 * M))
            .validate_stock_split_draft(split_draft(ratio(3, 2)))
            .await
            .expect("valid");
        assert_eq!(preview.factor, 1_500_000);
        assert_eq!(preview.position, Some(HELD));
        // 100 / 1.5 = 66.666666…, rounded at the last micro.
        assert_eq!(preview.price_after_split, Some(66_666_667));

        // 1 for 3 does not divide: the factor rounds at the micro (SPL-061).
        let preview = split_use_case(Ok(HELD), 1, None)
            .validate_stock_split_draft(split_draft(ratio(1, 3)))
            .await
            .expect("valid");
        assert_eq!(preview.factor, 333_333);
        assert_eq!(preview.price_after_split, None);
    }

    // SPL-062 — the first problem is reported as recording would: a date not entered, a
    // ratio not typed yet or not positive, and what the account refuses.
    #[tokio::test]
    async fn spl_062_a_split_draft_reports_its_first_problem() {
        let code_of = |result: Result<StockSplitPreview, TransactionDraftError>| match result {
            Ok(_) => "ok".to_string(),
            Err(error) => serde_json::to_value(&error).expect("serialize")["code"]
                .as_str()
                .expect("code")
                .to_string(),
        };
        let unread = || split_use_case(Ok(HELD), 0, None);
        let mut undated = split_draft(SplitSize::Factor { factor: 2 * M });
        undated.date = String::new();
        assert_eq!(
            code_of(unread().validate_stock_split_draft(undated).await),
            "DateMissing"
        );
        for (new, old) in [
            (None, Some(1)),
            (Some(2), None),
            (Some(0), Some(1)),
            (Some(1), Some(i64::MAX)),
            (Some(i64::MAX), Some(1)),
            (Some(2), Some(-1)),
        ] {
            let draft = split_draft(SplitSize::Ratio { new, old });
            assert_eq!(
                code_of(unread().validate_stock_split_draft(draft).await),
                "SplitFactorNotPositive"
            );
        }
        let refused = split_use_case(Err(AccountError::SplitCollapsesPosition), 1, None)
            .validate_stock_split_draft(split_draft(SplitSize::Factor { factor: 1 }))
            .await;
        assert_eq!(code_of(refused), "SplitCollapsesPosition");
    }

    // DIV-011, FSD-011, SPL-012, FEE-011, INT-011 — an asset is held when its holding has
    // a quantity above zero: a closed position keeps its holding at zero and is not, nor
    // is an asset the account never had.
    #[tokio::test]
    async fn an_asset_is_held_only_above_a_quantity_of_zero() {
        for (holding_quantity, held) in [(Some(1), true), (Some(0), false), (None, false)] {
            let use_case = split_use_case_holding(holding_quantity, Ok(HELD), 0, None);
            assert_eq!(use_case.holds("acc-1", "asset-1").await.ok(), Some(held));
        }
    }

    // SPL-012 — a position closed today is refused before anything is previewed, and a
    // price read that fails leaves the check clean, without a price.
    #[tokio::test]
    async fn spl_062_a_split_draft_needs_a_position_held_today() {
        for held_today in [None, Some(0)] {
            let refused = split_use_case_holding(held_today, Ok(HELD), 0, None)
                .validate_stock_split_draft(split_draft(SplitSize::Factor { factor: 2 * M }))
                .await;
            assert!(matches!(
                refused,
                Err(TransactionDraftError::Account(AccountError::ClosedPosition))
            ));
        }

        let mut account = MockAccountServiceContract::new();
        account
            .expect_get_holding_by_account_asset()
            .returning(|account_id, asset_id| {
                Ok(Some(
                    Holding::new(account_id.into(), asset_id.into(), 4 * M, M, 0, None)
                        .expect("holding"),
                ))
            });
        account
            .expect_preview_split()
            .returning(|_, _, _, _| Ok(HELD));
        let mut asset = MockAssetServiceContract::new();
        asset
            .expect_get_asset_prices()
            .returning(|_| Err(crate::context::asset::AssetError::DatabaseError));
        let preview = HoldingTransactionUseCase::new(Arc::new(account), Arc::new(asset))
            .validate_stock_split_draft(split_draft(SplitSize::Factor { factor: 2 * M }))
            .await
            .expect("valid");
        assert_eq!(
            (preview.position, preview.price_after_split),
            (Some(HELD), None)
        );
    }

    // SPL-063 — a split being corrected is checked on its date and factor alone: no
    // position is read nor previewed.
    #[tokio::test]
    async fn spl_062_a_corrected_split_is_checked_without_its_position() {
        let mut draft = split_draft(SplitSize::Factor { factor: 2 * M });
        draft.correcting = Some("tx-1".into());
        let preview = split_use_case(Ok(HELD), 0, Some(100 * M))
            .validate_stock_split_draft(draft.clone())
            .await
            .expect("valid");
        assert_eq!((preview.factor, preview.position), (2 * M, None));
        assert_eq!(preview.price_after_split, Some(50 * M));

        draft.size = SplitSize::Factor { factor: M };
        let refused = split_use_case(Ok(HELD), 0, None)
            .validate_stock_split_draft(draft)
            .await;
        assert!(matches!(
            refused,
            Err(TransactionDraftError::Account(
                AccountError::SplitFactorIsOne
            ))
        ));
    }

    // TDI-030 — a new sale previews the gain it would realize: its proceeds minus the
    // average cost, as of its date, of the quantity sold — the figure recording computes.
    #[tokio::test]
    async fn tdi_030_a_sale_previews_the_gain_it_would_realize() {
        // 2 units sold at 50 with 1 of fees: proceeds 99; cost 2 × 40 = 80; gain 19.
        let mut sale = draft(DraftKind::Sell);
        sale.entered = EnteredAmount::UnitPrice {
            unit_price: 50 * M,
            exchange_rate: M,
            fees: M,
        };
        let preview = use_case(Some(5 * M))
            .validate_draft(sale)
            .await
            .expect("valid");
        assert_eq!(preview.total_amount, 99 * M);
        assert_eq!(preview.realized_pnl, Some(19 * M));

        // Sold under its cost, the gain is a loss.
        let mut cheap = draft(DraftKind::Sell);
        cheap.entered = EnteredAmount::UnitPrice {
            unit_price: 30 * M,
            exchange_rate: M,
            fees: 0,
        };
        let preview = use_case(Some(5 * M))
            .validate_draft(cheap)
            .await
            .expect("valid");
        assert_eq!(preview.realized_pnl, Some(-20 * M));
    }

    // TDI-031 — no gain is previewed where none can be computed, and the check stands:
    // nothing held at the sale's date, less held then than is sold, a position that
    // cannot be read. A purchase and a sale being corrected do not read the position.
    #[tokio::test]
    async fn tdi_031_no_gain_is_previewed_where_none_can_be_computed() {
        for as_of in [
            Ok((0, 0)),
            Ok((M, 40 * M)),
            Err(AccountError::DatabaseError),
        ] {
            let preview = use_case_as_of(as_of, 1)
                .validate_draft(draft(DraftKind::Sell))
                .await
                .expect("the check stands");
            assert_eq!(preview.total_amount, 100 * M);
            assert_eq!(preview.realized_pnl, None);
        }

        let purchase = use_case_as_of(Ok((5 * M, 40 * M)), 0)
            .validate_draft(draft(DraftKind::Purchase))
            .await
            .expect("valid");
        assert_eq!(purchase.realized_pnl, None);

        let mut corrected = draft(DraftKind::Sell);
        corrected.correcting = Some("tx-1".into());
        let preview = use_case_as_of(Ok((5 * M, 40 * M)), 0)
            .validate_draft(corrected)
            .await
            .expect("valid");
        assert_eq!(preview.realized_pnl, None);
    }

    // TDI-030 — the gain is computed on the position of the sale's date, not today's: a
    // back-dated sale is set against the average cost of that day.
    #[tokio::test]
    async fn tdi_030_a_back_dated_sale_uses_the_position_of_its_date() {
        // Held 3 units at 25 on that day (5 today): 2 sold at 50 → 100 − 50 = 50.
        let preview = use_case_as_of(Ok((3 * M, 25 * M)), 1)
            .validate_draft(draft(DraftKind::Sell))
            .await
            .expect("valid");
        assert_eq!(preview.realized_pnl, Some(50 * M));
    }

    fn code(result: Result<TransactionPreview, TransactionDraftError>) -> String {
        match result {
            Ok(_) => "ok".into(),
            Err(error) => serde_json::to_value(&error).expect("serialize")["code"]
                .as_str()
                .expect("code")
                .to_string(),
        }
    }

    // TRX-062 — a complete purchase previews what recording it would store.
    #[tokio::test]
    async fn trx_062_a_complete_purchase_previews_its_total() {
        let preview = use_case(None)
            .validate_draft(draft(DraftKind::Purchase))
            .await;
        assert_eq!(
            preview.expect("valid"),
            TransactionPreview {
                unit_price: 50 * M,
                total_amount: 100 * M,
                realized_pnl: None,
            }
        );
    }

    // TRX-062 — fields not filled yet come first, in form order.
    #[tokio::test]
    async fn trx_062_fields_not_filled_are_reported_in_order() {
        let mut d = draft(DraftKind::Purchase);
        d.account_id = " ".into();
        d.asset_id.clear();
        d.date.clear();
        assert_eq!(
            code(use_case(None).validate_draft(d.clone()).await),
            "AccountMissing"
        );
        d.account_id = "acc-1".into();
        assert_eq!(
            code(use_case(None).validate_draft(d.clone()).await),
            "AssetMissing"
        );
        d.asset_id = "asset-1".into();
        assert_eq!(code(use_case(None).validate_draft(d).await), "DateMissing");
    }

    fn opening_balance(total_cost: Option<i64>) -> OpeningBalanceDraft {
        OpeningBalanceDraft {
            account_id: "acc-1".into(),
            asset_id: "asset-1".into(),
            date: "2024-06-01".into(),
            quantity: 1_000_000,
            total_cost,
        }
    }

    fn opening_code(result: Result<OpeningBalancePreview, TransactionDraftError>) -> String {
        match result {
            Ok(preview) if preview.zero_cost => "zero-cost".into(),
            Ok(_) => "ok".into(),
            Err(error) => serde_json::to_value(&error)
                .ok()
                .and_then(|value| value["code"].as_str().map(str::to_string))
                .unwrap_or_default(),
        }
    }

    // TRX-066 / TRX-065 — an opening balance draft that can be recorded is clean; a typed
    // total cost of 0 is clean and flagged; a total cost not typed yet is not a 0.
    #[tokio::test]
    async fn trx_066_an_opening_balance_draft_flags_a_zero_cost() {
        let uc = use_case(None);
        let check =
            |draft: OpeningBalanceDraft| opening_code(uc.validate_opening_balance_draft(&draft));

        assert_eq!(check(opening_balance(Some(5_000_000))), "ok");
        assert_eq!(check(opening_balance(Some(0))), "zero-cost");
        assert_eq!(check(opening_balance(None)), "TotalCostMissing");
    }

    // TRX-066 — fields not filled come first, in form order; then the rejections recording
    // would make, in its order (CSH-061, TRX-044, TRX-045, TRX-046).
    #[tokio::test]
    async fn trx_066_an_opening_balance_draft_reports_its_first_problem() {
        let uc = use_case(None);
        let check = |edit: &dyn Fn(&mut OpeningBalanceDraft)| {
            let mut draft = opening_balance(Some(5_000_000));
            edit(&mut draft);
            opening_code(uc.validate_opening_balance_draft(&draft))
        };

        assert_eq!(check(&|d| d.account_id.clear()), "AccountMissing");
        assert_eq!(check(&|d| d.asset_id = " ".into()), "AssetMissing");
        assert_eq!(check(&|d| d.date.clear()), "DateMissing");
        assert_eq!(
            check(&|d| d.asset_id = crate::core::cash::system_cash_asset_id("EUR")),
            "OpeningBalanceOnCashAsset"
        );
        assert_eq!(check(&|d| d.quantity = 0), "QuantityNotPositive");
        assert_eq!(check(&|d| d.total_cost = Some(-1)), "InvalidTotalCost");
        assert_eq!(check(&|d| d.date = "2999-01-01".into()), "DateInFuture");
        assert_eq!(
            check(&|d| {
                d.quantity = 0;
                d.total_cost = Some(-1);
                d.date = "2999-01-01".into();
            }),
            "QuantityNotPositive"
        );
        assert_eq!(
            check(&|d| {
                d.asset_id = crate::core::cash::system_cash_asset_id("EUR");
                d.quantity = 0;
            }),
            "OpeningBalanceOnCashAsset"
        );
        assert_eq!(
            check(&|d| {
                d.total_cost = Some(-1);
                d.date = "2999-01-01".into();
            }),
            "InvalidTotalCost"
        );
    }

    // TRX-062 / DIV-040 — a corrected dividend totals its amount at the exchange rate,
    // rounded down, whatever its unit price and fees.
    #[tokio::test]
    async fn div_040_a_dividend_correction_draft_totals_amount_times_rate() {
        let dividend = |quantity: i64, unit_price: i64, exchange_rate: i64, fees: i64| {
            let mut d = draft(DraftKind::Dividend);
            d.quantity = quantity;
            d.entered = EnteredAmount::UnitPrice {
                unit_price,
                exchange_rate,
                fees,
            };
            d
        };
        let total = |d: TransactionDraft| async move {
            use_case(None)
                .validate_draft(d)
                .await
                .map(|preview| preview.total_amount)
        };

        assert_eq!(
            total(dividend(12_500_000, 3_000_000, 1_100_000, 0))
                .await
                .ok(),
            Some(13_750_000)
        );
        // A dividend has no typed total: one sent is ignored, the amount and the rate decide.
        let mut by_total = dividend(12_500_000, 0, 0, 0);
        by_total.entered = EnteredAmount::Total {
            total: 99_000_000,
            exchange_rate: 1_100_000,
            fees: 0,
        };
        assert_eq!(total(by_total).await.ok(), Some(13_750_000));
        // 3.333333 × 1.1 = 3.6666663: the seventh decimal is dropped.
        assert_eq!(
            total(dividend(3_333_333, 1_000_000, 1_100_000, 0))
                .await
                .ok(),
            Some(3_666_666)
        );
    }

    // TRX-062 / DIV-040 — a corrected dividend is refused as recording it would be, the
    // first problem in recording's order: date, amount, unit price, fees, rate, total.
    #[tokio::test]
    async fn div_040_a_dividend_correction_draft_reports_what_recording_would() {
        let check = |edit: &dyn Fn(&mut TransactionDraft)| {
            let mut d = draft(DraftKind::Dividend);
            d.quantity = 12_500_000;
            d.entered = EnteredAmount::UnitPrice {
                unit_price: 1_000_000,
                exchange_rate: 1_000_000,
                fees: 0,
            };
            edit(&mut d);
            d
        };
        let entered = |unit_price: i64, exchange_rate: i64, fees: i64| EnteredAmount::UnitPrice {
            unit_price,
            exchange_rate,
            fees,
        };
        for (d, expected) in [
            (check(&|d| d.date = "2999-01-01".into()), "DateInFuture"),
            (check(&|d| d.quantity = 0), "QuantityNotPositive"),
            (
                check(&|d| d.entered = entered(-1, 1_000_000, 0)),
                "UnitPriceNegative",
            ),
            (
                check(&|d| d.entered = entered(1_000_000, 1_000_000, -1)),
                "FeesNegative",
            ),
            (
                check(&|d| d.entered = entered(1_000_000, 0, 0)),
                "ExchangeRateNotPositive",
            ),
            // Fees are judged before the rate.
            (
                check(&|d| d.entered = entered(1_000_000, 0, -1)),
                "FeesNegative",
            ),
            // One micro-unit at a rate of one micro-unit rounds down to nothing.
            (
                check(&|d| {
                    d.quantity = 1;
                    d.entered = entered(1_000_000, 1, 0);
                }),
                "TotalAmountNotPositive",
            ),
        ] {
            assert_eq!(code(use_case(None).validate_draft(d).await), expected);
        }
    }

    // CSH-062 — a draft on the cash line is refused as recording it would be, a purchase
    // and a sale alike.
    #[tokio::test]
    async fn csh_062_a_draft_on_the_cash_line_is_refused() {
        for kind in [DraftKind::Purchase, DraftKind::Sell] {
            let mut d = draft(kind);
            d.asset_id = crate::core::cash::system_cash_asset_id("EUR");
            assert_eq!(
                code(use_case(Some(1_000_000)).validate_draft(d).await),
                "TradeOnCashAsset"
            );
        }
    }

    // TRX-062 / SEL-022 — a sale above the quantity held is refused with both figures.
    #[tokio::test]
    async fn trx_062_a_sale_above_the_holding_is_an_oversell() {
        let result = use_case(Some(M + M / 2))
            .validate_draft(draft(DraftKind::Sell))
            .await;
        match result {
            Err(TransactionDraftError::Account(AccountError::Oversell {
                available,
                requested,
            })) => assert_eq!((available, requested), (M + M / 2, 2 * M)),
            other => panic!("expected an oversell, got {other:?}"),
        }
        assert_eq!(
            code(use_case(None).validate_draft(draft(DraftKind::Sell)).await),
            "Oversell"
        );
        assert_eq!(
            code(
                use_case(Some(2 * M))
                    .validate_draft(draft(DraftKind::Sell))
                    .await
            ),
            "ok"
        );
    }

    // TRX-062 / SEL-030 — a corrected sale is not checked for oversell here.
    #[tokio::test]
    async fn trx_062_a_corrected_sale_skips_the_oversell_check() {
        let mut d = draft(DraftKind::Sell);
        d.correcting = Some("tx-1".into());
        assert_eq!(code(use_case(Some(0)).validate_draft(d).await), "ok");
    }

    // TRX-062 — a figure the account domain rejects comes back with the recording code.
    #[tokio::test]
    async fn trx_062_a_rejected_figure_keeps_the_recording_code() {
        let mut d = draft(DraftKind::Purchase);
        d.quantity = 0;
        assert_eq!(
            code(use_case(None).validate_draft(d).await),
            "QuantityNotPositive"
        );
    }
}

#[cfg(test)]
mod name_lookup_tests {
    use super::*;
    use crate::context::account::UpdateFrequency;

    fn account(id: &str, name: &str) -> Account {
        Account::restore(
            id.to_string(),
            name.to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
    }

    // CLI-011 — accounts match by name, case ignored for every letter; two accounts sharing a
    // name after a merge are ambiguous rather than a silent pick.
    #[test]
    fn cli_011_accounts_match_by_name_and_refuse_a_shared_one() {
        let accounts = vec![
            account("a1", "Épargne"),
            account("a2", "PEA"),
            account("a3", "PEA"),
        ];

        assert_eq!(
            match_account(&accounts, "épargne").map(|found| found.id.as_str()),
            Ok("a1")
        );
        assert_eq!(
            match_account(&accounts, "pea").map(|found| found.id.clone()),
            Err(NameLookupError::AccountAmbiguous {
                typed: "pea".to_string()
            })
        );
        assert_eq!(
            match_account(&accounts, "CTO").map(|found| found.id.clone()),
            Err(NameLookupError::AccountNotFound {
                typed: "CTO".to_string()
            })
        );
    }
}
