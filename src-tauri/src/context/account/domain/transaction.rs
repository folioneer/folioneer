use crate::context::account::error::AccountError;
use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::result::Result as StdResult;
use uuid::Uuid;

/// Type of financial transaction.
#[derive(
    Debug,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Type,
    PartialEq,
    Eq,
    Default,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum TransactionType {
    /// A purchase (acquisition) of an asset.
    #[default]
    Purchase,
    /// A sale of a previously purchased asset.
    Sell,
    /// Seeds a holding directly from a known quantity and total cost, without full transaction history (TRX-042).
    OpeningBalance,
    /// A cash inflow from outside the application's tracked world (CSH-022).
    Deposit,
    /// A cash outflow to outside the application's tracked world (CSH-032).
    Withdrawal,
    /// A cash dividend paid by a held asset; credited to the Cash Holding (DIV-023).
    Dividend,
    /// Shares of a held asset received at no cost; quantity rises, cost basis unchanged (FSD-022).
    FreeShares,
    /// Management fee deduction: shares removed at zero cost; cost basis unchanged, VWAP concentrates (FEE-012).
    ManagementFee,
    /// A stock split rescaling the position; the micro-scaled factor rides in
    /// `quantity`, no money moves (SPL-010/020).
    Split,
    /// Interest credited at zero cost: quantity rises, cost basis unchanged (INT-024);
    /// on the account's cash line it credits the balance directly (INT-023).
    Interest,
}

/// A single financial event affecting an asset's quantity and cost basis within an account.
/// All financial fields are stored as i64 micro-units (ADR-001, TRX-024).
#[derive(Debug, Serialize, Deserialize, Clone, Type)]
pub struct Transaction {
    /// Unique identifier.
    pub id: String,
    /// The account where the transaction occurred.
    pub account_id: String,
    /// The financial asset involved.
    pub asset_id: String,
    /// Type of transaction: Purchase, Sell, or OpeningBalance.
    pub transaction_type: TransactionType,
    /// Date when the transaction was executed (ISO 8601, "YYYY-MM-DD").
    pub date: String,
    /// Number of units traded (micro-units: value × 10^6). Must be > 0.
    pub quantity: i64,
    /// Price per unit in asset's native currency (micro-units). Can be 0 (gifted assets).
    pub unit_price: i64,
    /// Conversion rate from asset currency to account currency (micro-units).
    pub exchange_rate: i64,
    /// Transaction fees in account currency (micro-units).
    pub fees: i64,
    /// Total cost (Purchase) or proceeds (Sell) in account currency (micro-units).
    /// Must be > 0, except an OpeningBalance position may be 0 (zero-cost, TRX-045).
    pub total_amount: i64,
    /// Optional user comment.
    pub note: Option<String>,
    /// Realized P&L for Sell transactions (micro-units, SEL-024). NULL for Purchase.
    pub realized_pnl: Option<i64>,
    /// ISO 8601 timestamp of record creation — used for same-date tie-breaking (SEL-024).
    pub created_at: String,
}

impl Transaction {
    /// Creates a new Transaction with a generated ID.
    /// Validates TRX-020 and TRX-026. Returns a typed `AccountError`
    /// so callers can propagate it through typed unions (e.g. the application-
    /// layer `AccountError` composed by `AccountService::record_deposit`).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        account_id: String,
        asset_id: String,
        transaction_type: TransactionType,
        date: String,
        quantity: i64,
        unit_price: i64,
        exchange_rate: i64,
        fees: i64,
        total_amount: i64,
        note: Option<String>,
        realized_pnl: Option<i64>,
    ) -> StdResult<Self, AccountError> {
        Self::validate(
            &transaction_type,
            &date,
            quantity,
            unit_price,
            exchange_rate,
            fees,
            total_amount,
        )?;
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            account_id,
            asset_id,
            transaction_type,
            date,
            quantity,
            unit_price,
            exchange_rate,
            fees,
            total_amount,
            note,
            realized_pnl,
            // Microsecond precision keeps the ORDER BY tiebreaker deterministic even when
            // multiple Transactions land within the same second — important for the
            // chronological cash replay (CSH-080).
            created_at: chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.6fZ")
                .to_string(),
        })
    }

    /// Creates a Transaction with a provided ID (used for updates, TRX-033).
    /// Applies the same validation as new().
    #[allow(clippy::too_many_arguments)]
    pub fn with_id(
        id: String,
        account_id: String,
        asset_id: String,
        transaction_type: TransactionType,
        date: String,
        quantity: i64,
        unit_price: i64,
        exchange_rate: i64,
        fees: i64,
        total_amount: i64,
        note: Option<String>,
        realized_pnl: Option<i64>,
        created_at: String,
    ) -> StdResult<Self, AccountError> {
        Self::validate(
            &transaction_type,
            &date,
            quantity,
            unit_price,
            exchange_rate,
            fees,
            total_amount,
        )?;
        Ok(Self {
            id,
            account_id,
            asset_id,
            transaction_type,
            date,
            quantity,
            unit_price,
            exchange_rate,
            fees,
            total_amount,
            note,
            realized_pnl,
            created_at,
        })
    }

    /// Factory: builds a Deposit transaction with cash-specific defaults
    /// (`unit_price = 1.0` micros, `exchange_rate = 1.0` micros, `fees = 0`,
    /// `total_amount = amount`).
    ///
    /// CSH-021 — `amount <= 0` is rejected here as `AmountNotPositive` so the
    /// FE sees the cash-specific error code. The check fires BEFORE the
    /// generic `Transaction::new` validator (which would otherwise raise the
    /// less-specific `QuantityNotPositive`).
    pub fn new_deposit(
        account_id: String,
        cash_asset_id: String,
        date: String,
        amount: i64,
        note: Option<String>,
    ) -> StdResult<Self, AccountError> {
        if amount <= 0 {
            return Err(AccountError::AmountNotPositive);
        }
        Self::new(
            account_id,
            cash_asset_id,
            TransactionType::Deposit,
            date,
            amount,
            1_000_000,
            1_000_000,
            0,
            amount,
            note,
            None,
        )
    }

    /// Factory: builds a Withdrawal transaction with cash-specific defaults.
    /// Same shape as `new_deposit` but with `TransactionType::Withdrawal`.
    ///
    /// CSH-031 — `amount <= 0` is rejected here as `AmountNotPositive`
    /// (mirrors `new_deposit`). CSH-080 (insufficient cash) is enforced by
    /// `Account::apply_withdrawal`, not here — this factory only validates
    /// the transaction itself.
    pub fn new_withdrawal(
        account_id: String,
        cash_asset_id: String,
        date: String,
        amount: i64,
        note: Option<String>,
    ) -> StdResult<Self, AccountError> {
        if amount <= 0 {
            return Err(AccountError::AmountNotPositive);
        }
        Self::new(
            account_id,
            cash_asset_id,
            TransactionType::Withdrawal,
            date,
            amount,
            1_000_000,
            1_000_000,
            0,
            amount,
            note,
            None,
        )
    }

    /// Factory: builds a Dividend transaction (DIV-023).
    ///
    /// `total_amount = floor(amount_micros × exchange_rate / MICRO)` in account currency.
    /// Carries `transaction_type = Dividend`, `asset_id = paying asset` (not the Cash Asset),
    /// `fees = 0`, `realized_pnl = None` (income, not a capital gain — DIV-024).
    ///
    /// DIV-021 — `amount_micros <= 0` is rejected as `AmountNotPositive` before the
    /// generic validator so the FE sees the cash-specific code.
    /// DIV-022 — `exchange_rate <= 0` is rejected as `ExchangeRateNotPositive`.
    pub fn new_dividend(
        account_id: String,
        paying_asset_id: String,
        date: String,
        amount_micros: i64,
        exchange_rate: i64,
        note: Option<String>,
    ) -> StdResult<Self, AccountError> {
        // DIV-021/022 — cash-specific codes fire before the generic validator.
        if amount_micros <= 0 {
            return Err(AccountError::AmountNotPositive);
        }
        if exchange_rate <= 0 {
            return Err(AccountError::ExchangeRateNotPositive);
        }
        // total_amount in account currency = amount × rate (i128 intermediate, ADR-001).
        let total_amount = ((amount_micros as i128 * exchange_rate as i128) / 1_000_000) as i64;
        Self::new(
            account_id,
            paying_asset_id,
            TransactionType::Dividend,
            date,
            amount_micros,
            1_000_000,
            exchange_rate,
            0,
            total_amount,
            note,
            None,
        )
    }

    /// Factory: builds a FreeShares transaction (FSD-022/023).
    ///
    /// Zero-cost convention: `unit_price = 0`, `exchange_rate = 1.0` micros,
    /// `fees = 0`, `total_amount = 0` (no money moves), `realized_pnl = None`.
    /// `asset_id` is the distributing asset.
    ///
    /// FSD-021 — validates the date bounds and `quantity > 0` directly; the
    /// generic validator does not apply because it rejects `total_amount = 0`
    /// for free-shares' `FreeShares` type (only `OpeningBalance` allows 0, TRX-045),
    /// which is exactly this type's convention.
    pub fn free_shares(
        account_id: String,
        asset_id: String,
        date: String,
        quantity: i64,
        note: Option<String>,
    ) -> StdResult<Self, AccountError> {
        Self::validate_date(&date)?;
        if quantity <= 0 {
            return Err(AccountError::QuantityNotPositive);
        }
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            account_id,
            asset_id,
            transaction_type: TransactionType::FreeShares,
            date,
            quantity,
            unit_price: 0,
            exchange_rate: 1_000_000,
            fees: 0,
            total_amount: 0,
            note,
            realized_pnl: None,
            created_at: chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.6fZ")
                .to_string(),
        })
    }

    /// Factory: rebuilds a FreeShares transaction with a caller-supplied ID and
    /// `created_at` (FSD-040 correction). Same zero-cost packing and FSD-021
    /// validation as `free_shares`, but preserves the transaction's identity —
    /// the type-specific sibling of `with_id` (which rejects `total_amount = 0`
    /// for the `FreeShares` type — `with_id` allows 0 only for `OpeningBalance`, TRX-045).
    pub fn free_shares_with_id(
        id: String,
        account_id: String,
        asset_id: String,
        date: String,
        quantity: i64,
        note: Option<String>,
        created_at: String,
    ) -> StdResult<Self, AccountError> {
        let mut tx = Self::free_shares(account_id, asset_id, date, quantity, note)?;
        tx.id = id;
        tx.created_at = created_at;
        Ok(tx)
    }

    /// Factory: builds a Split transaction (SPL-010/011).
    ///
    /// The micro-scaled split factor rides in `quantity` (20-for-1 →
    /// 20_000_000); `unit_price = 0`, `exchange_rate = 1.0` micros, `fees = 0`,
    /// `total_amount = 0` (a split moves no money), `realized_pnl = None`.
    ///
    /// SPL-011 — validates the date bounds, `factor > 0` and `factor ≠ ×1`
    /// directly; the generic validator does not apply because it rejects
    /// `total_amount = 0` for the `Split` type (only `OpeningBalance` allows 0,
    /// TRX-045), which is exactly this type's convention.
    pub fn split(
        account_id: String,
        asset_id: String,
        date: String,
        factor: i64,
        note: Option<String>,
    ) -> StdResult<Self, AccountError> {
        Self::validate_date(&date)?;
        if factor <= 0 {
            return Err(AccountError::SplitFactorNotPositive);
        }
        if factor == 1_000_000 {
            return Err(AccountError::SplitFactorIsOne);
        }
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            account_id,
            asset_id,
            transaction_type: TransactionType::Split,
            date,
            quantity: factor,
            unit_price: 0,
            exchange_rate: 1_000_000,
            fees: 0,
            total_amount: 0,
            note,
            realized_pnl: None,
            created_at: chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.6fZ")
                .to_string(),
        })
    }

    /// Factory: rebuilds a Split transaction with a caller-supplied ID and
    /// `created_at` (SPL-030 correction). Same packing and SPL-011 validation
    /// as `split`, preserving the transaction's identity.
    pub fn split_with_id(
        id: String,
        account_id: String,
        asset_id: String,
        date: String,
        factor: i64,
        note: Option<String>,
        created_at: String,
    ) -> StdResult<Self, AccountError> {
        let mut tx = Self::split(account_id, asset_id, date, factor, note)?;
        tx.id = id;
        tx.created_at = created_at;
        Ok(tx)
    }

    /// Factory: builds a ManagementFee transaction (FEE-012/021).
    ///
    /// Zero-cost-removal convention: `unit_price = 0`, `exchange_rate = 1_000_000`,
    /// `fees = 0`, `total_amount = 0` (no cash moves), `realized_pnl = None`.
    /// `asset_id` is the charged asset; `quantity` is the number of shares removed.
    ///
    /// FEE-021 — validates the date bounds and `quantity > 0` directly; the generic
    /// validator does not apply because it rejects `total_amount = 0` for the
    /// `ManagementFee` type (only `OpeningBalance` allows 0, TRX-045).
    pub fn management_fee(
        account_id: String,
        asset_id: String,
        date: String,
        quantity: i64,
        note: Option<String>,
    ) -> StdResult<Self, AccountError> {
        Self::validate_date(&date)?;
        if quantity <= 0 {
            return Err(AccountError::QuantityNotPositive);
        }
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            account_id,
            asset_id,
            transaction_type: TransactionType::ManagementFee,
            date,
            quantity,
            unit_price: 0,
            exchange_rate: 1_000_000,
            fees: 0,
            total_amount: 0,
            note,
            realized_pnl: None,
            created_at: chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.6fZ")
                .to_string(),
        })
    }

    /// Factory: rebuilds a ManagementFee transaction with a caller-supplied ID and
    /// `created_at` (correction path). Same zero-cost packing and FEE-021 validation
    /// as `management_fee`, but preserves the transaction's identity.
    pub fn management_fee_with_id(
        id: String,
        account_id: String,
        asset_id: String,
        date: String,
        quantity: i64,
        note: Option<String>,
        created_at: String,
    ) -> StdResult<Self, AccountError> {
        let mut tx = Self::management_fee(account_id, asset_id, date, quantity, note)?;
        tx.id = id;
        tx.created_at = created_at;
        Ok(tx)
    }

    /// Factory: builds a generated ManagementFee deduction (FEE-040) under the
    /// deterministic identity FEE-048 assigns it, so two devices generating the same
    /// period converge on one transaction. Same zero-cost packing and FEE-021
    /// validation as `management_fee`.
    pub fn generated_management_fee(
        id: String,
        account_id: String,
        asset_id: String,
        date: String,
        quantity: i64,
    ) -> StdResult<Self, AccountError> {
        let created_at = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.6fZ")
            .to_string();
        Self::management_fee_with_id(id, account_id, asset_id, date, quantity, None, created_at)
    }

    /// Factory: builds an Interest transaction (INT-021/024).
    ///
    /// Zero-cost convention: `unit_price = 0`, `exchange_rate = 1_000_000`,
    /// `fees = 0`, `total_amount = 0` (no money moves), `realized_pnl = None`.
    /// `asset_id` is the credited asset — a held asset or the account's Cash Asset
    /// (INT-023); for the cash line `quantity` is the credited balance in
    /// account-currency micros.
    ///
    /// INT-021 — validates the date bounds and `quantity > 0` directly; the generic
    /// validator does not apply because it rejects `total_amount = 0` for the
    /// `Interest` type (only `OpeningBalance` allows 0, TRX-045).
    pub fn interest(
        account_id: String,
        asset_id: String,
        date: String,
        quantity: i64,
        note: Option<String>,
    ) -> StdResult<Self, AccountError> {
        Self::validate_date(&date)?;
        if quantity <= 0 {
            return Err(AccountError::QuantityNotPositive);
        }
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            account_id,
            asset_id,
            transaction_type: TransactionType::Interest,
            date,
            quantity,
            unit_price: 0,
            exchange_rate: 1_000_000,
            fees: 0,
            total_amount: 0,
            note,
            realized_pnl: None,
            created_at: chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.6fZ")
                .to_string(),
        })
    }

    /// Factory: rebuilds an Interest transaction with a caller-supplied ID and
    /// `created_at` (INT-040 correction). Same zero-cost packing and INT-021
    /// validation as `interest`, but preserves the transaction's identity.
    pub fn interest_with_id(
        id: String,
        account_id: String,
        asset_id: String,
        date: String,
        quantity: i64,
        note: Option<String>,
        created_at: String,
    ) -> StdResult<Self, AccountError> {
        let mut tx = Self::interest(account_id, asset_id, date, quantity, note)?;
        tx.id = id;
        tx.created_at = created_at;
        Ok(tx)
    }

    /// Reconstructs a Transaction from storage without validation.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: String,
        account_id: String,
        asset_id: String,
        transaction_type: TransactionType,
        date: String,
        quantity: i64,
        unit_price: i64,
        exchange_rate: i64,
        fees: i64,
        total_amount: i64,
        note: Option<String>,
        realized_pnl: Option<i64>,
        created_at: String,
    ) -> Self {
        Self {
            id,
            account_id,
            asset_id,
            transaction_type,
            date,
            quantity,
            unit_price,
            exchange_rate,
            fees,
            total_amount,
            note,
            realized_pnl,
            created_at,
        }
    }

    /// Validates business rules (TRX-020).
    /// total_amount is computed by the orchestrator (TRX-026) before this is called —
    /// no formula check here.
    fn validate(
        transaction_type: &TransactionType,
        date: &str,
        quantity: i64,
        unit_price: i64,
        exchange_rate: i64,
        fees: i64,
        total_amount: i64,
    ) -> StdResult<(), AccountError> {
        Self::validate_date(date)?;

        // TRX-020 — quantity must be strictly positive
        if quantity <= 0 {
            return Err(AccountError::QuantityNotPositive);
        }

        // TRX-020 — unit_price must be >= 0
        if unit_price < 0 {
            return Err(AccountError::UnitPriceNegative);
        }

        // SEL-020 — fees must be zero or positive
        if fees < 0 {
            return Err(AccountError::FeesNegative);
        }

        // TRX-020 — exchange_rate must be strictly positive
        if exchange_rate <= 0 {
            return Err(AccountError::ExchangeRateNotPositive);
        }

        // TRX-020 — total_amount must be > 0, EXCEPT an OpeningBalance position
        // may have a zero total cost (TRX-045 — a mined / gifted / airdropped
        // position seeded at zero cost). Negative is always rejected.
        let total_amount_ok = match transaction_type {
            TransactionType::OpeningBalance => total_amount >= 0,
            _ => total_amount > 0,
        };
        if !total_amount_ok {
            return Err(AccountError::TotalAmountNotPositive);
        }

        Ok(())
    }

    /// TRX-020 — date must be parseable, not in the future, not before 1900-01-01.
    pub(super) fn validate_date(date: &str) -> StdResult<(), AccountError> {
        let parsed_date =
            NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| AccountError::InvalidDate)?;
        let today = chrono::Local::now().date_naive();
        if parsed_date > today {
            return Err(AccountError::DateInFuture);
        }
        let min_date = NaiveDate::from_ymd_opt(1900, 1, 1).expect("hardcoded valid date");
        if parsed_date < min_date {
            return Err(AccountError::DateTooOld);
        }
        Ok(())
    }
}

/// Interface for transaction persistence.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait TransactionRepository: Send + Sync {
    /// Fetches a transaction by ID.
    async fn get_by_id(&self, id: &str) -> Result<Option<Transaction>>;
    /// Fetches all transactions for a given account and asset, ordered chronologically (TRX-036).
    async fn get_by_account_asset(
        &self,
        account_id: &str,
        asset_id: &str,
    ) -> Result<Vec<Transaction>>;
    /// Fetches every transaction for an account across all assets (including cash),
    /// ordered chronologically by `(date, created_at)` (PRF-021).
    async fn get_all_for_account(&self, account_id: &str) -> Result<Vec<Transaction>>;
    /// Returns distinct asset IDs that have transactions for the given account (TXL-013).
    async fn get_asset_ids_for_account(&self, account_id: &str) -> Result<Vec<String>>;
    /// Returns sum of realized_pnl grouped by asset_id for Sell transactions in the account (SEL-038).
    async fn get_realized_pnl_by_account(&self, account_id: &str) -> Result<Vec<(String, i64)>>;
    /// Deletes a transaction by ID.
    async fn delete(&self, id: &str) -> Result<()>;
    /// Returns true if any transaction references this asset (across all accounts).
    async fn has_transactions_for_asset(&self, asset_id: &str) -> Result<bool>;
    /// Counts all transactions for a given account (ACC-020).
    async fn count_by_account(&self, account_id: &str) -> Result<u32>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_transaction(
        quantity: i64,
        unit_price: i64,
        exchange_rate: i64,
        fees: i64,
        total_amount: i64,
    ) -> StdResult<Transaction, AccountError> {
        Transaction::new(
            "account-1".to_string(),
            "asset-1".to_string(),
            TransactionType::Purchase,
            "2020-01-01".to_string(),
            quantity,
            unit_price,
            exchange_rate,
            fees,
            total_amount,
            None,
            None,
        )
    }

    // TRX-020 — quantity must be > 0
    #[test]
    fn rejects_zero_quantity() {
        let micro = 1_000_000i64;
        let result = make_transaction(0, micro, micro, 0, 0);
        assert!(result.is_err());
    }

    // TRX-020 — unit_price can be 0 (gifted assets, OQ-1)
    #[test]
    fn accepts_zero_unit_price() {
        // qty=1_000_000 (1.0), price=0, rate=1_000_000, fees=1_000_000 (1.0), total=1_000_000
        // expected = (1_000_000 * 0 / 1_000_000) * 1_000_000 / 1_000_000 + 1_000_000 = 1_000_000 ✓
        let micro = 1_000_000i64;
        let result = make_transaction(micro, 0, micro, micro, micro);
        assert!(result.is_ok(), "got: {:?}", result.err());
    }

    // TRX-045 — an OpeningBalance may have total_amount = 0 (zero-cost position),
    // on both the create (`new`) and edit (`with_id`) paths.
    #[test]
    fn new_allows_zero_total_amount_for_opening_balance() {
        let micro = 1_000_000i64;
        let result = Transaction::new(
            "account-1".to_string(),
            "asset-1".to_string(),
            TransactionType::OpeningBalance,
            "2020-01-01".to_string(),
            micro, // quantity
            0,     // unit_price
            micro, // exchange_rate
            0,     // fees
            0,     // total_amount = 0 (zero-cost)
            None,
            None,
        );
        assert!(result.is_ok(), "got: {:?}", result.err());
    }

    #[test]
    fn with_id_allows_zero_total_amount_for_opening_balance() {
        let micro = 1_000_000i64;
        let result = Transaction::with_id(
            "tx-1".to_string(),
            "account-1".to_string(),
            "asset-1".to_string(),
            TransactionType::OpeningBalance,
            "2020-01-01".to_string(),
            micro,
            0,
            micro,
            0,
            0, // total_amount = 0 (zero-cost, edit path)
            None,
            None,
            "2020-01-01T00:00:00.000000Z".to_string(),
        );
        assert!(result.is_ok(), "got: {:?}", result.err());
    }

    // TRX-045 — the carve-out is OpeningBalance-only: a Purchase with total_amount 0
    // is still rejected (buy/sell invariant untouched).
    #[test]
    fn new_still_rejects_zero_total_amount_for_purchase() {
        let micro = 1_000_000i64;
        let result = make_transaction(micro, 0, micro, 0, 0); // Purchase, total_amount = 0
        assert!(matches!(result, Err(AccountError::TotalAmountNotPositive)));
    }

    // TRX-020 — date before 1900-01-01 is rejected
    #[test]
    fn rejects_date_before_1900() {
        let micro = 1_000_000i64;
        let result = Transaction::new(
            "a".to_string(),
            "b".to_string(),
            TransactionType::Purchase,
            "1899-12-31".to_string(),
            micro,
            micro,
            micro,
            0,
            micro,
            None,
            None,
        );
        assert!(result.is_err());
    }

    // TRX-020 — future date is rejected
    #[test]
    fn rejects_future_date() {
        let micro = 1_000_000i64;
        let result = Transaction::new(
            "a".to_string(),
            "b".to_string(),
            TransactionType::Purchase,
            "2099-01-01".to_string(),
            micro,
            micro,
            micro,
            0,
            micro,
            None,
            None,
        );
        assert!(result.is_err());
    }

    // TRX-020 — exchange_rate must be strictly positive
    #[test]
    fn rejects_zero_exchange_rate() {
        let micro = 1_000_000i64;
        // total_amount=0 also fails (TRX-020) but exchange_rate=0 is caught first
        let result = make_transaction(micro, micro, 0, 0, 0);
        assert!(result.is_err());
    }

    // SEL-020 — fees cannot be negative
    #[test]
    fn rejects_negative_fees() {
        let micro = 1_000_000i64;
        let result = make_transaction(micro, micro, micro, -1, micro);
        assert!(result.is_err());
    }

    // TRX-042 — OpeningBalance round-trips through strum Display → from_str
    #[test]
    fn opening_balance_round_trips_through_strum() {
        use std::str::FromStr;
        let original = TransactionType::OpeningBalance;
        let as_str = original.to_string();
        let parsed = TransactionType::from_str(&as_str).expect("strum parse");
        assert_eq!(parsed, TransactionType::OpeningBalance);
    }

    // CSH-022 — Transaction::new_deposit sets the cash-specific defaults
    // (price=1.0 micros, rate=1.0 micros, fees=0, total=amount, type=Deposit).
    #[test]
    fn new_deposit_sets_cash_defaults() {
        let tx = Transaction::new_deposit(
            "acc-1".to_string(),
            "asset-cash-USD".to_string(),
            "2020-01-01".to_string(),
            500_000_000,
            None,
        )
        .unwrap();
        assert_eq!(tx.transaction_type, TransactionType::Deposit);
        assert_eq!(tx.account_id, "acc-1");
        assert_eq!(tx.asset_id, "asset-cash-USD");
        assert_eq!(tx.quantity, 500_000_000);
        assert_eq!(tx.unit_price, 1_000_000);
        assert_eq!(tx.exchange_rate, 1_000_000);
        assert_eq!(tx.fees, 0);
        assert_eq!(tx.total_amount, 500_000_000);
        assert!(tx.realized_pnl.is_none());
    }

    // CSH-021 — zero amount surfaces the cash-specific `AmountNotPositive`
    // (raised by `new_deposit`'s own check) before the generic
    // `QuantityNotPositive` from `Transaction::new` could fire.
    #[test]
    fn new_deposit_rejects_zero_amount_as_amount_not_positive() {
        let err = Transaction::new_deposit(
            "acc-1".to_string(),
            "asset-cash-USD".to_string(),
            "2020-01-01".to_string(),
            0,
            None,
        )
        .unwrap_err();
        assert!(matches!(err, AccountError::AmountNotPositive));
    }

    // CSH-032 — Transaction::new_withdrawal sets the cash-specific defaults
    // with TransactionType::Withdrawal. CSH-080 is enforced by the aggregate
    // (Account::apply_withdrawal), not by this factory.
    #[test]
    fn new_withdrawal_sets_cash_defaults() {
        let tx = Transaction::new_withdrawal(
            "acc-1".to_string(),
            "asset-cash-USD".to_string(),
            "2020-01-01".to_string(),
            250_000_000,
            Some("ATM".to_string()),
        )
        .unwrap();
        assert_eq!(tx.transaction_type, TransactionType::Withdrawal);
        assert_eq!(tx.quantity, 250_000_000);
        assert_eq!(tx.unit_price, 1_000_000);
        assert_eq!(tx.exchange_rate, 1_000_000);
        assert_eq!(tx.fees, 0);
        assert_eq!(tx.total_amount, 250_000_000);
        assert_eq!(tx.note.as_deref(), Some("ATM"));
    }

    // TRX-020 propagates through new_withdrawal unchanged.
    #[test]
    fn new_withdrawal_propagates_date_validation() {
        let err = Transaction::new_withdrawal(
            "acc-1".to_string(),
            "asset-cash-USD".to_string(),
            "1899-12-31".to_string(),
            100,
            None,
        )
        .unwrap_err();
        assert!(matches!(err, AccountError::DateTooOld));
    }

    // -------------------------------------------------------------------------
    // DIV-023 / DIV-024 — Transaction::new_dividend factory
    // -------------------------------------------------------------------------

    // DIV-023 — new_dividend sets transaction_type = Dividend, asset_id = paying
    // asset (not cash asset), realized_pnl = None.
    #[test]
    fn new_dividend_sets_dividend_defaults() {
        let total_amount = 500_000_000i64; // 500.0 account-currency
        let exchange_rate = 1_000_000i64; // 1.0 (same currency)
        let tx = Transaction::new_dividend(
            "acc-1".to_string(),
            "asset-aapl".to_string(),
            "2024-06-15".to_string(),
            500_000_000, // amount_micros in asset currency
            exchange_rate,
            None,
        )
        .unwrap();
        assert_eq!(tx.transaction_type, TransactionType::Dividend);
        assert_eq!(tx.account_id, "acc-1");
        assert_eq!(tx.asset_id, "asset-aapl");
        assert_eq!(tx.exchange_rate, exchange_rate);
        assert_eq!(tx.total_amount, total_amount);
        assert_eq!(tx.fees, 0);
        assert!(
            tx.realized_pnl.is_none(),
            "dividend realized_pnl must be None"
        );
    }

    // DIV-022 — total_amount = amount_micros × exchange_rate / MICRO.
    #[test]
    fn new_dividend_converts_amount_at_exchange_rate() {
        // amount = 100 USD, rate = 0.9 EUR/USD → total = 90 EUR
        let amount_micros = 100_000_000i64; // 100.0 asset ccy
        let exchange_rate = 900_000i64; // 0.9 account ccy per asset ccy
        let tx = Transaction::new_dividend(
            "acc-1".to_string(),
            "asset-aapl".to_string(),
            "2024-06-15".to_string(),
            amount_micros,
            exchange_rate,
            None,
        )
        .unwrap();
        // total_amount = floor(100_000_000 × 900_000 / 1_000_000) = 90_000_000
        assert_eq!(tx.total_amount, 90_000_000);
    }

    // DIV-021 — amount_micros ≤ 0 is rejected as AmountNotPositive.
    #[test]
    fn new_dividend_rejects_zero_amount() {
        let err = Transaction::new_dividend(
            "acc-1".to_string(),
            "asset-aapl".to_string(),
            "2024-06-15".to_string(),
            0,
            1_000_000,
            None,
        )
        .unwrap_err();
        assert!(
            matches!(err, AccountError::AmountNotPositive),
            "expected AmountNotPositive, got: {err:?}"
        );
    }

    // DIV-021 — negative amount_micros is rejected as AmountNotPositive.
    #[test]
    fn new_dividend_rejects_negative_amount() {
        let err = Transaction::new_dividend(
            "acc-1".to_string(),
            "asset-aapl".to_string(),
            "2024-06-15".to_string(),
            -1_000_000,
            1_000_000,
            None,
        )
        .unwrap_err();
        assert!(
            matches!(err, AccountError::AmountNotPositive),
            "expected AmountNotPositive, got: {err:?}"
        );
    }

    // DIV-022 — exchange_rate ≤ 0 is rejected as ExchangeRateNotPositive.
    #[test]
    fn new_dividend_rejects_zero_exchange_rate() {
        let err = Transaction::new_dividend(
            "acc-1".to_string(),
            "asset-aapl".to_string(),
            "2024-06-15".to_string(),
            100_000_000,
            0,
            None,
        )
        .unwrap_err();
        assert!(
            matches!(err, AccountError::ExchangeRateNotPositive),
            "expected ExchangeRateNotPositive, got: {err:?}"
        );
    }

    // DIV-021 — future date is rejected.
    #[test]
    fn new_dividend_rejects_future_date() {
        let err = Transaction::new_dividend(
            "acc-1".to_string(),
            "asset-aapl".to_string(),
            "2099-01-01".to_string(),
            100_000_000,
            1_000_000,
            None,
        )
        .unwrap_err();
        assert!(
            matches!(err, AccountError::DateInFuture),
            "expected DateInFuture, got: {err:?}"
        );
    }

    // DIV-021 — date before 1900-01-01 is rejected as DateTooOld.
    #[test]
    fn new_dividend_rejects_date_too_old() {
        let err = Transaction::new_dividend(
            "acc-1".to_string(),
            "asset-aapl".to_string(),
            "1899-12-31".to_string(),
            100_000_000,
            1_000_000,
            None,
        )
        .unwrap_err();
        assert!(
            matches!(err, AccountError::DateTooOld),
            "expected DateTooOld, got: {err:?}"
        );
    }

    // DIV-021 — invalid date string is rejected as InvalidDate.
    #[test]
    fn new_dividend_rejects_invalid_date_string() {
        let err = Transaction::new_dividend(
            "acc-1".to_string(),
            "asset-aapl".to_string(),
            "not-a-date".to_string(),
            100_000_000,
            1_000_000,
            None,
        )
        .unwrap_err();
        assert!(
            matches!(err, AccountError::InvalidDate),
            "expected InvalidDate, got: {err:?}"
        );
    }

    // DIV-024 — new_dividend carries no realized_pnl (income, not capital gain).
    #[test]
    fn new_dividend_realized_pnl_is_none() {
        let tx = Transaction::new_dividend(
            "acc-1".to_string(),
            "asset-aapl".to_string(),
            "2024-06-15".to_string(),
            100_000_000,
            1_000_000,
            Some("Q2 dividend".to_string()),
        )
        .unwrap();
        assert!(tx.realized_pnl.is_none());
        assert_eq!(tx.note.as_deref(), Some("Q2 dividend"));
    }

    // -------------------------------------------------------------------------
    // FSD-022/023 — Transaction::free_shares factory
    // -------------------------------------------------------------------------

    // FSD-022/023 — free_shares packs the contract convention exactly:
    // transaction_type = FreeShares, unit_price = 0, exchange_rate = 1_000_000,
    // fees = 0, total_amount = 0, realized_pnl = None; asset_id = distributing asset.
    #[test]
    fn free_shares_factory_packs_contract_convention() {
        // FSD-022 — Transaction::free_shares must exist and set the zero-cost convention.
        let tx = Transaction::free_shares(
            "acc-1".to_string(),
            "asset-xyz".to_string(),
            "2024-06-15".to_string(),
            5_000_000, // 5 shares in micros
            None,
        )
        .unwrap();
        assert_eq!(
            tx.transaction_type,
            TransactionType::FreeShares,
            "transaction_type must be FreeShares"
        );
        assert_eq!(tx.account_id, "acc-1");
        assert_eq!(
            tx.asset_id, "asset-xyz",
            "asset_id must be the distributing asset"
        );
        assert_eq!(tx.quantity, 5_000_000);
        // FSD-023 — zero-cost convention: no money moves
        assert_eq!(
            tx.unit_price, 0,
            "unit_price must be 0 (no acquisition cost)"
        );
        assert_eq!(
            tx.exchange_rate, 1_000_000,
            "exchange_rate must be 1_000_000 (no FX leg)"
        );
        assert_eq!(tx.fees, 0, "fees must be 0");
        assert_eq!(
            tx.total_amount, 0,
            "total_amount must be 0 (no money moved)"
        );
        assert!(
            tx.realized_pnl.is_none(),
            "realized_pnl must be None (not a capital gain)"
        );
    }

    // FSD-022 — note is preserved on the returned transaction.
    #[test]
    fn free_shares_factory_preserves_note() {
        let tx = Transaction::free_shares(
            "acc-1".to_string(),
            "asset-xyz".to_string(),
            "2024-06-15".to_string(),
            1_000_000,
            Some("Bonus issue 1:10".to_string()),
        )
        .unwrap();
        assert_eq!(tx.note.as_deref(), Some("Bonus issue 1:10"));
    }

    // FSD-021 — quantity ≤ 0 must be rejected as QuantityNotPositive.
    #[test]
    fn split_factory_rejects_non_positive_factor() {
        // SPL-011 — factor must be strictly positive
        for factor in [0, -5_000_000] {
            let err = Transaction::split(
                "acc-1".to_string(),
                "asset-xyz".to_string(),
                "2024-06-15".to_string(),
                factor,
                None,
            )
            .unwrap_err();
            assert!(
                matches!(err, AccountError::SplitFactorNotPositive),
                "expected SplitFactorNotPositive, got: {err:?}"
            );
        }
    }

    // SPL-011 — a ×1 split is a no-op data-entry error.
    #[test]
    fn split_factory_rejects_identity_factor() {
        let err = Transaction::split(
            "acc-1".to_string(),
            "asset-xyz".to_string(),
            "2024-06-15".to_string(),
            1_000_000,
            None,
        )
        .unwrap_err();
        assert!(
            matches!(err, AccountError::SplitFactorIsOne),
            "expected SplitFactorIsOne, got: {err:?}"
        );
    }

    // SPL-010 — the zero-money packing: factor in quantity, no money fields.
    #[test]
    fn split_factory_packs_factor_in_quantity_with_no_money() {
        let tx = Transaction::split(
            "acc-1".to_string(),
            "asset-xyz".to_string(),
            "2024-06-15".to_string(),
            20_000_000,
            None,
        )
        .unwrap();
        assert_eq!(tx.transaction_type, TransactionType::Split);
        assert_eq!(tx.quantity, 20_000_000);
        assert_eq!(tx.unit_price, 0);
        assert_eq!(tx.fees, 0);
        assert_eq!(tx.total_amount, 0);
        assert_eq!(tx.exchange_rate, 1_000_000);
    }

    #[test]
    fn free_shares_factory_rejects_zero_quantity() {
        // FSD-021 — quantity must be strictly positive
        let err = Transaction::free_shares(
            "acc-1".to_string(),
            "asset-xyz".to_string(),
            "2024-06-15".to_string(),
            0,
            None,
        )
        .unwrap_err();
        assert!(
            matches!(err, AccountError::QuantityNotPositive),
            "expected QuantityNotPositive, got: {err:?}"
        );
    }

    // FSD-021 — future date is rejected as DateInFuture.
    #[test]
    fn free_shares_factory_rejects_future_date() {
        // FSD-021 — date bounds: not in future
        let err = Transaction::free_shares(
            "acc-1".to_string(),
            "asset-xyz".to_string(),
            "2099-01-01".to_string(),
            1_000_000,
            None,
        )
        .unwrap_err();
        assert!(
            matches!(err, AccountError::DateInFuture),
            "expected DateInFuture, got: {err:?}"
        );
    }

    // FSD-021 — date before 1900-01-01 is rejected as DateTooOld.
    #[test]
    fn free_shares_factory_rejects_date_too_old() {
        // FSD-021 — date bounds: not older than lower bound
        let err = Transaction::free_shares(
            "acc-1".to_string(),
            "asset-xyz".to_string(),
            "1899-12-31".to_string(),
            1_000_000,
            None,
        )
        .unwrap_err();
        assert!(
            matches!(err, AccountError::DateTooOld),
            "expected DateTooOld, got: {err:?}"
        );
    }

    // FSD-021 — malformed date string is rejected as InvalidDate.
    #[test]
    fn free_shares_factory_rejects_invalid_date_string() {
        // FSD-021 — date must be a well-formed ISO 8601 calendar date
        let err = Transaction::free_shares(
            "acc-1".to_string(),
            "asset-xyz".to_string(),
            "not-a-date".to_string(),
            1_000_000,
            None,
        )
        .unwrap_err();
        assert!(
            matches!(err, AccountError::InvalidDate),
            "expected InvalidDate, got: {err:?}"
        );
    }

    // -------------------------------------------------------------------------
    // INT-021/024 — Transaction::interest factory
    // -------------------------------------------------------------------------

    // INT-024 — interest packs the zero-cost convention exactly:
    // transaction_type = Interest, unit_price = 0, exchange_rate = 1_000_000,
    // fees = 0, total_amount = 0, realized_pnl = None; asset_id = credited asset.
    #[test]
    fn interest_factory_packs_contract_convention() {
        let tx = Transaction::interest(
            "acc-1".to_string(),
            "asset-xyz".to_string(),
            "2024-06-15".to_string(),
            5_000_000, // 5 units in micros
            None,
        )
        .unwrap();
        assert_eq!(
            tx.transaction_type,
            TransactionType::Interest,
            "transaction_type must be Interest"
        );
        assert_eq!(tx.account_id, "acc-1");
        assert_eq!(
            tx.asset_id, "asset-xyz",
            "asset_id must be the credited asset"
        );
        assert_eq!(tx.quantity, 5_000_000);
        // INT-024 — zero-cost convention: no money moves
        assert_eq!(
            tx.unit_price, 0,
            "unit_price must be 0 (no acquisition cost)"
        );
        assert_eq!(
            tx.exchange_rate, 1_000_000,
            "exchange_rate must be 1_000_000 (no FX leg)"
        );
        assert_eq!(tx.fees, 0, "fees must be 0");
        assert_eq!(
            tx.total_amount, 0,
            "total_amount must be 0 (no money moved)"
        );
        assert!(
            tx.realized_pnl.is_none(),
            "realized_pnl must be None (not a capital gain)"
        );
    }

    // INT-021 — quantity ≤ 0 must be rejected as QuantityNotPositive.
    #[test]
    fn interest_factory_rejects_zero_quantity() {
        let err = Transaction::interest(
            "acc-1".to_string(),
            "asset-xyz".to_string(),
            "2024-06-15".to_string(),
            0,
            None,
        )
        .unwrap_err();
        assert!(
            matches!(err, AccountError::QuantityNotPositive),
            "expected QuantityNotPositive, got: {err:?}"
        );
    }

    // INT-021 — future date is rejected as DateInFuture.
    #[test]
    fn interest_factory_rejects_future_date() {
        let err = Transaction::interest(
            "acc-1".to_string(),
            "asset-xyz".to_string(),
            "2099-01-01".to_string(),
            1_000_000,
            None,
        )
        .unwrap_err();
        assert!(
            matches!(err, AccountError::DateInFuture),
            "expected DateInFuture, got: {err:?}"
        );
    }

    // INT-040 — interest_with_id preserves the caller-supplied identity while
    // applying the same zero-cost packing and INT-021 validation.
    #[test]
    fn interest_with_id_preserves_identity() {
        let tx = Transaction::interest_with_id(
            "tx-1".to_string(),
            "acc-1".to_string(),
            "asset-xyz".to_string(),
            "2024-06-15".to_string(),
            2_000_000,
            Some("2024 rate".to_string()),
            "2024-06-15T00:00:00.000001Z".to_string(),
        )
        .unwrap();
        assert_eq!(tx.id, "tx-1");
        assert_eq!(tx.created_at, "2024-06-15T00:00:00.000001Z");
        assert_eq!(tx.transaction_type, TransactionType::Interest);
        assert_eq!(tx.total_amount, 0, "total_amount must remain 0 (INT-024)");
    }

    // INT-024 — Interest round-trips through strum Display → from_str
    // (the variant must be persisted as TEXT and deserialized back without error).
    #[test]
    fn interest_variant_round_trips_through_strum() {
        use std::str::FromStr;
        let original = TransactionType::Interest;
        let as_str = original.to_string();
        let parsed = TransactionType::from_str(&as_str).expect("strum parse must succeed");
        assert_eq!(
            parsed,
            TransactionType::Interest,
            "Interest must round-trip through strum"
        );
    }

    // FSD-022 — FreeShares round-trips through strum Display → from_str
    // (the variant must be persisted as TEXT and deserialized back without error).
    #[test]
    fn free_shares_variant_round_trips_through_strum() {
        // FSD-022 — strum serialization must handle FreeShares without error
        use std::str::FromStr;
        let original = TransactionType::FreeShares;
        let as_str = original.to_string();
        let parsed = TransactionType::from_str(&as_str).expect("strum parse must succeed");
        assert_eq!(
            parsed,
            TransactionType::FreeShares,
            "FreeShares must round-trip through strum"
        );
    }
}
