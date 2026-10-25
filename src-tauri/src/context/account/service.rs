use super::domain::EnteredAmount;
use super::domain::{
    generated_deduction_id, Account, AccountJournal, AccountRepository, AgentRecording,
    FeeCatchUpPosition, FeeCatchUpRepository, FeeSchedule, FeeScheduleRepository, Holding,
    HoldingNote, HoldingNoteRepository, HoldingRepository, HoldingSnapshot, JournalFilter,
    ManagementFeeRemoval, SessionRecording, StockSplitPosition, ThresholdDirection, Transaction,
    TransactionRepository, TransactionType, TransferView, WriteOffReason,
};
use super::error::AccountError;
use crate::core::{logger::BACKEND, Event, SideEffectEventBus};
use crate::shared::domain::{Rank, RecordKind, SyncedChild, SyncedRecord};
use crate::shared::infrastructure::change_recorder::LocalWriteOutranked;
use crate::shared::infrastructure::uow::UnitOfWork;
use crate::use_cases::holding_transaction::OpenHoldingError;
use async_trait::async_trait;
use chrono::NaiveDate;
use sqlx::SqliteConnection;
use std::result::Result as StdResult;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

/// Orchestrates business logic for the Account bounded context.
pub struct AccountService {
    account_repo: Box<dyn AccountRepository>,
    holding_repo: Box<dyn HoldingRepository>,
    transaction_repo: Box<dyn TransactionRepository>,
    event_bus: Option<Arc<SideEffectEventBus>>,
    fee_schedule_repo: Option<Box<dyn FeeScheduleRepository>>,
    fee_catch_up_repo: Option<Box<dyn FeeCatchUpRepository>>,
    holding_note_repo: Option<Box<dyn HoldingNoteRepository>>,
}

/// A purchase or a sale as recording takes it (TRX-027, SEL-012).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeOp {
    /// The asset traded.
    pub asset_id: String,
    /// The date (YYYY-MM-DD).
    pub date: String,
    /// The quantity (micro-units).
    pub quantity: i64,
    /// The unit price (micro-units).
    pub unit_price: i64,
    /// The exchange rate (micro-units).
    pub exchange_rate: i64,
    /// The fees (micro-units).
    pub fees: i64,
    /// A typed all-in total, which the unit price then follows.
    pub total_amount: Option<i64>,
    /// The note.
    pub note: Option<String>,
}

/// A recording an agent previews (AGT-072): what the account's recording takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordingOp {
    /// A purchase.
    Buy(TradeOp),
    /// A sale.
    Sell(TradeOp),
    /// A deposit of `amount` into the account's cash (CSH-022).
    Deposit {
        /// The date.
        date: String,
        /// The amount (micro-units).
        amount: i64,
        /// The note.
        note: Option<String>,
    },
    /// A withdrawal of `amount` from the account's cash (CSH-032).
    Withdrawal {
        /// The date.
        date: String,
        /// The amount (micro-units).
        amount: i64,
        /// The note.
        note: Option<String>,
    },
    /// A dividend paid by `asset_id` (DIV-023).
    Dividend {
        /// The paying asset.
        asset_id: String,
        /// The date.
        date: String,
        /// The amount in the asset's currency (micro-units).
        amount: i64,
        /// The exchange rate to the account's currency (micro-units).
        exchange_rate: i64,
        /// The note.
        note: Option<String>,
    },
    /// Free shares of `asset_id` received (FSD-022).
    FreeShares {
        /// The distributing asset.
        asset_id: String,
        /// The date.
        date: String,
        /// The quantity received (micro-units).
        quantity: i64,
        /// The note.
        note: Option<String>,
    },
    /// A split of `factor` (micro-scaled).
    Split {
        /// The asset split.
        asset_id: String,
        /// The date.
        date: String,
        /// The factor (micro-units).
        factor: i64,
        /// The note.
        note: Option<String>,
    },
    /// A write-off of `quantity` of `asset_id` (WOF-020).
    WriteOff {
        /// The asset written off.
        asset_id: String,
        /// The date.
        date: String,
        /// The quantity written off (micro-units).
        quantity: i64,
        /// Why it left.
        reason: WriteOffReason,
        /// The note.
        note: Option<String>,
    },
}

/// What a transfer would make of its two accounts, nothing written (AGT-065A, AGT-072).
#[derive(Debug, Clone)]
pub struct TransferPreview {
    /// The parts as they would be recorded, the fee in kind first when there is one.
    pub parts: Vec<Transaction>,
    /// The sending account's transactions without the transfer.
    pub sending_before: Vec<Transaction>,
    /// The sending account's transactions with it.
    pub sending_after: Vec<Transaction>,
    /// The receiving account's transactions without the transfer.
    pub receiving_before: Vec<Transaction>,
    /// The receiving account's transactions with it.
    pub receiving_after: Vec<Transaction>,
}

/// What a recording would make of the account, nothing written (AGT-072).
#[derive(Debug, Clone)]
pub struct RecordingPreview {
    /// The transaction as it would be recorded.
    pub transaction: Transaction,
    /// The account's transactions without it.
    pub before: Vec<Transaction>,
    /// The account's transactions with it.
    pub after: Vec<Transaction>,
}

/// One recording to judge in turn, nothing written (IMP-035).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordingStep {
    /// The recording to judge.
    pub op: RecordingOp,
    /// DIV-011 — a dividend's asset must be held, the rows before it included.
    pub needs_held: bool,
}

/// What the account makes of one recording step (IMP-035, IMP-036, IMP-038).
#[derive(Debug, Clone)]
pub enum RecordingJudgment {
    /// Manual entry takes it; `funding` is the deposit a purchase the cash cannot fund
    /// needs (IMP-038).
    Ready {
        /// The deposit added before the purchase, in the account's currency (micro-units).
        funding: Option<i64>,
    },
    /// The account holds a transaction it matches (IMP-036).
    AlreadyRecorded,
    /// Manual entry refuses it, with its reason.
    Refused(AccountError),
    /// DIV-011 — the dividend's asset is not held.
    NotHeld,
}

impl AccountService {
    /// Creates a new AccountService.
    pub fn new(
        account_repo: Box<dyn AccountRepository>,
        holding_repo: Box<dyn HoldingRepository>,
        transaction_repo: Box<dyn TransactionRepository>,
    ) -> Self {
        Self {
            account_repo,
            holding_repo,
            transaction_repo,
            event_bus: None,
            fee_schedule_repo: None,
            fee_catch_up_repo: None,
            holding_note_repo: None,
        }
    }

    /// Attaches an event bus for side-effect notifications.
    pub fn with_event_bus(mut self, bus: Arc<SideEffectEventBus>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    /// Attaches the fee-schedule repository (FEE-030) — required for the
    /// `*_fee_schedule` methods; absent in constructions that never touch them.
    pub fn with_fee_schedule_repo(mut self, repo: Box<dyn FeeScheduleRepository>) -> Self {
        self.fee_schedule_repo = Some(repo);
        self
    }

    /// Returns the wired fee-schedule repository or a `DatabaseError` if absent
    /// (a wiring bug — the repo must be attached via `with_fee_schedule_repo`).
    fn fee_schedule_repo(&self) -> StdResult<&dyn FeeScheduleRepository, AccountError> {
        self.fee_schedule_repo.as_deref().ok_or_else(|| {
            tracing::error!(target: BACKEND, "fee_schedule_repo not wired on AccountService");
            AccountError::DatabaseError
        })
    }

    /// Attaches the fee catch-up position repository (FEE-043, CFR-044) — required
    /// by `advance_fee_schedule_cursor`; absent in constructions that never touch it.
    pub fn with_fee_catch_up_repo(mut self, repo: Box<dyn FeeCatchUpRepository>) -> Self {
        self.fee_catch_up_repo = Some(repo);
        self
    }

    /// Returns the wired fee catch-up repository or a `DatabaseError` if absent
    /// (a wiring bug — the repo must be attached via `with_fee_catch_up_repo`).
    fn fee_catch_up_repo(&self) -> StdResult<&dyn FeeCatchUpRepository, AccountError> {
        self.fee_catch_up_repo.as_deref().ok_or_else(|| {
            tracing::error!(target: BACKEND, "fee_catch_up_repo not wired on AccountService");
            AccountError::DatabaseError
        })
    }

    /// Attaches the holding-note repository (HNO-010) — required for the
    /// `*_holding_note` methods; absent in constructions that never touch them.
    pub fn with_holding_note_repo(mut self, repo: Box<dyn HoldingNoteRepository>) -> Self {
        self.holding_note_repo = Some(repo);
        self
    }

    /// Returns the wired holding-note repository or a `DatabaseError` if absent
    /// (a wiring bug — the repo must be attached via `with_holding_note_repo`).
    fn holding_note_repo(&self) -> StdResult<&dyn HoldingNoteRepository, AccountError> {
        self.holding_note_repo.as_deref().ok_or_else(|| {
            tracing::error!(target: BACKEND, "holding_note_repo not wired on AccountService");
            AccountError::DatabaseError
        })
    }

    // -------------------------------------------------------------------------
    // Account CRUD
    // -------------------------------------------------------------------------

    /// Retrieves all non-deleted accounts.
    pub async fn get_all(&self) -> StdResult<Vec<Account>, AccountError> {
        self.account_repo.get_all().await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "get_all: repository failure");
            AccountError::DatabaseError
        })
    }

    /// CLI-018 — every account, ordered by name with case ignored.
    pub async fn get_all_by_name(&self) -> StdResult<Vec<Account>, AccountError> {
        let mut accounts = self.get_all().await?;
        accounts.sort_by_cached_key(|account| account.name.to_lowercase());
        Ok(accounts)
    }

    /// Retrieves an account by ID.
    pub async fn get_by_id(&self, id: &str) -> StdResult<Option<Account>, AccountError> {
        self.account_repo.get_by_id(id).await.map_err(|e| {
            tracing::error!(target: BACKEND, account_id = %id, err = ?e, "get_by_id: repository failure");
            AccountError::DatabaseError
        })
    }

    /// Creates a new account.
    pub async fn create(
        &self,
        name: String,
        bank_name: String,
        currency: String,
        management_fees_enabled: bool,
    ) -> Result<Account, AccountError> {
        let account = Account::new(name, bank_name, currency, management_fees_enabled)?;
        self.create_account(account).await
    }

    /// ACC-041 — creates a new account with its country and wrapper, both optional.
    // The account's own fields, one argument each as `create`/`update` take them, plus
    // its country and wrapper (ACC-041).
    #[allow(clippy::too_many_arguments)]
    pub async fn create_classified(
        &self,
        name: String,
        bank_name: String,
        currency: String,
        management_fees_enabled: bool,
        country: Option<String>,
        wrapper: Option<String>,
    ) -> Result<Account, AccountError> {
        let account = Account::new(name, bank_name, currency, management_fees_enabled)?
            .classified(country, wrapper, None)?;
        self.create_account(account).await
    }

    /// AGT-069 — the mark of every account an agent created on this computer, by account id.
    pub async fn agent_creations(
        &self,
    ) -> Result<std::collections::HashMap<String, AgentRecording>, AccountError> {
        let marks = self.account_repo.agent_creations().await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "agent_creations: repository failure");
            AccountError::DatabaseError
        })?;
        Ok(marks.into_iter().collect())
    }

    /// ACC-003 — stores a valid new account whose name no other account has.
    async fn create_account(&self, account: Account) -> Result<Account, AccountError> {
        if find_account_by_name(&*self.account_repo, &account.name)
            .await?
            .is_some()
        {
            return Err(AccountError::NameAlreadyExists);
        }
        info!(target: BACKEND, account_id = %account.id, name = %account.name, "creating account");
        let created = self.account_repo.create(account).await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "create: repository failure");
            AccountError::DatabaseError
        })?;
        self.emit_account_updated();
        Ok(created)
    }

    /// ACC-025 — stores the valid new `account` with its 0-balance Cash Holding (CSH-012) and,
    /// when an agent created it, its mark (AGT-069), inside `unit` (ADR-006): the account
    /// exists with its cash line or not at all. Its name is checked first (ACC-003). Nothing
    /// is published — the caller commits, then calls `notify_account_updated`.
    pub async fn create_in(
        &self,
        unit: &mut UnitOfWork,
        mut account: Account,
        mark: Option<AgentRecording>,
    ) -> Result<Account, AccountError> {
        let clashing = self
            .account_repo
            .find_by_name_in(unit, &account.name)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, name = %account.name, err = ?e, "find_by_name: repository failure");
                AccountError::DatabaseError
            })?;
        if clashing.is_some() {
            return Err(AccountError::NameAlreadyExists);
        }
        info!(target: BACKEND, account_id = %account.id, name = %account.name, by_agent = mark.is_some(), "creating account");
        account.seed_cash_holding();
        self.account_repo
            .create_in(unit, &mut account, mark)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "create_in: repository failure");
                AccountError::DatabaseError
            })?;
        Ok(account)
    }

    /// Publishes `AccountUpdated` for writes a unit of work committed (ADR-006).
    pub fn notify_account_updated(&self) {
        self.emit_account_updated();
    }

    /// Seeds the account's 0-balance Cash Holding (CSH-012). Called by the
    /// account-creation use case after the Cash Asset has been ensured (FK).
    /// Idempotent — `Account::seed_cash_holding` is a no-op if one already exists.
    ///
    /// Uses inline load/save logic rather than the module-level
    /// `load_account`/`save_account` helpers.
    pub async fn seed_cash_holding(&self, account_id: &str) -> Result<(), AccountError> {
        let mut account = match self
            .account_repo
            .get_with_holdings_and_transactions(account_id)
            .await
        {
            Ok(Some(acc)) => acc,
            Ok(None) => {
                return Err(AccountError::AccountNotFound {
                    account_id: account_id.to_string(),
                });
            }
            Err(e) => {
                tracing::error!(target: BACKEND, account_id = %account_id, err = ?e, "seed_cash_holding: load failure");
                return Err(AccountError::DatabaseError);
            }
        };
        account.seed_cash_holding();
        self.account_repo.save(&mut account).await.map_err(|e| {
            tracing::error!(target: BACKEND, account_id = %account_id, err = ?e, "seed_cash_holding: save failure");
            AccountError::DatabaseError
        })?;
        self.emit_account_updated();
        Ok(())
    }

    /// Updates an existing account; its country and wrapper (ACC-041) stay as they are.
    pub async fn update(
        &self,
        id: String,
        name: String,
        bank_name: String,
        currency: String,
        management_fees_enabled: bool,
    ) -> Result<Account, AccountError> {
        let account = Account::with_id(id, name, bank_name, currency, management_fees_enabled)?;
        let account = match self.get_by_id(&account.id).await? {
            Some(current) => account.with_stored_classification(current.country, current.wrapper),
            None => account,
        };
        self.update_account(account).await
    }

    /// ACC-041 — updates an existing account, its country and wrapper included.
    // The account's own fields, one argument each as `create`/`update` take them, plus
    // its country and wrapper (ACC-041).
    #[allow(clippy::too_many_arguments)]
    pub async fn update_classified(
        &self,
        id: String,
        name: String,
        bank_name: String,
        currency: String,
        management_fees_enabled: bool,
        country: Option<String>,
        wrapper: Option<String>,
    ) -> Result<Account, AccountError> {
        // ACC-043 — read apart from the write: a sync landing in between is a single-user race
        // whose worst case keeps a wrapper another device set, which ACC-040 allows anyway.
        let stored_wrapper = self.get_by_id(&id).await?.and_then(|stored| stored.wrapper);
        let account = Account::with_id(id, name, bank_name, currency, management_fees_enabled)?
            .classified(country, wrapper, stored_wrapper.as_deref())?;
        self.update_account(account).await
    }

    /// ACC-003/CFR-035 — stores a valid account under its id, refusing a rename into another
    /// account's name.
    async fn update_account(&self, account: Account) -> Result<Account, AccountError> {
        // CFR-035 — the uniqueness rule (ACC-003) binds the name being set: a name that
        // already clashes after a merge is accepted unchanged, a rename into another
        // account's name is rejected.
        let name_changes = self
            .get_by_id(&account.id)
            .await?
            .is_none_or(|current| !current.name.eq_ignore_ascii_case(&account.name));
        if name_changes {
            if let Some(existing) = find_account_by_name(&*self.account_repo, &account.name).await?
            {
                if existing.id != account.id {
                    return Err(AccountError::NameAlreadyExists);
                }
            }
        }
        info!(target: BACKEND, account_id = %account.id, name = %account.name, "updating account");
        let updated = self.account_repo.update(account).await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "update: repository failure");
            AccountError::DatabaseError
        })?;
        self.emit_account_updated();
        Ok(updated)
    }

    /// Permanently deletes an account and cascades to its holdings (R5).
    ///
    /// SYN-024/CFR-030 — every synced child (transactions, holding notes, fee schedules,
    /// catch-up positions) is removed through its own repository first, so each leaves
    /// its own change and tombstone instead of vanishing under the database cascade.
    pub async fn delete(&self, id: &str) -> StdResult<(), AccountError> {
        info!(target: BACKEND, account_id = %id, "deleting account");
        self.remove_children(id).await.map_err(|e| {
            tracing::error!(target: BACKEND, account_id = %id, err = ?e, "delete: child removal failure");
            AccountError::DatabaseError
        })?;
        self.account_repo.delete(id).await.map_err(|e| {
            tracing::error!(target: BACKEND, account_id = %id, err = ?e, "delete: repository failure");
            AccountError::DatabaseError
        })?;
        self.emit_account_updated();
        Ok(())
    }

    async fn remove_children(&self, account_id: &str) -> anyhow::Result<()> {
        for transaction in self
            .transaction_repo
            .get_all_for_account(account_id)
            .await?
        {
            self.transaction_repo.delete(&transaction.id).await?;
        }
        if let Some(repo) = self.holding_note_repo.as_deref() {
            for note in repo.get_for_account(account_id).await? {
                repo.delete(&note.account_id, &note.asset_id).await?;
            }
        }
        if let Some(repo) = self.fee_schedule_repo.as_deref() {
            for schedule in repo.get_by_account(account_id).await? {
                repo.delete_by_account_asset(&schedule.account_id, &schedule.asset_id)
                    .await?;
            }
        }
        if let Some(repo) = self.fee_catch_up_repo.as_deref() {
            for position in repo.get_by_account(account_id).await? {
                repo.delete_by_account_asset(&position.account_id, &position.asset_id)
                    .await?;
            }
        }
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Holding reads
    // -------------------------------------------------------------------------

    /// Retrieves all holdings for a given account (ACD-022, B24).
    pub async fn get_holdings_for_account(
        &self,
        account_id: &str,
    ) -> StdResult<Vec<Holding>, AccountError> {
        self.holding_repo.get_by_account(account_id).await.map_err(|e| {
            tracing::error!(target: BACKEND, account_id = %account_id, err = ?e, "get_holdings_for_account: repository failure");
            AccountError::DatabaseError
        })
    }

    /// Retrieves a single holding by account/asset pair, or None (B19).
    pub async fn get_holding_by_account_asset(
        &self,
        account_id: &str,
        asset_id: &str,
    ) -> StdResult<Option<Holding>, AccountError> {
        self.holding_repo
            .get_by_account_asset(account_id, asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, err = ?e, "get_holding_by_account_asset: repository failure");
                AccountError::DatabaseError
            })
    }

    // -------------------------------------------------------------------------
    // Transaction reads
    // -------------------------------------------------------------------------

    /// Retrieves a transaction by ID.
    pub async fn get_transaction_by_id(
        &self,
        id: &str,
    ) -> StdResult<Option<Transaction>, AccountError> {
        self.transaction_repo.get_by_id(id).await.map_err(|e| {
            tracing::error!(target: BACKEND, transaction_id = %id, err = ?e, "get_transaction_by_id: repository failure");
            AccountError::DatabaseError
        })
    }

    /// Retrieves all transactions for an account/asset pair in chronological order (TRX-036).
    pub async fn get_transactions(
        &self,
        account_id: &str,
        asset_id: &str,
    ) -> StdResult<Vec<Transaction>, AccountError> {
        self.transaction_repo
            .get_by_account_asset(account_id, asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, err = ?e, "get_transactions: repository failure");
                AccountError::DatabaseError
            })
    }

    /// TDI-010 — Computes the (account, asset) holding's quantity and VWAP
    /// average cost as of `date` by replaying the pair's transactions dated on or
    /// before it. Unknown account/asset yields an empty snapshot (`0`/`0`); an
    /// unparseable `date` is rejected with `InvalidDate` (TDI-012).
    pub async fn holding_snapshot_as_of(
        &self,
        account_id: &str,
        asset_id: &str,
        date: &str,
    ) -> StdResult<HoldingSnapshot, AccountError> {
        NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| AccountError::InvalidDate)?;
        // The cash line is moved by every asset's purchases, sales and dividends: its
        // balance needs the whole account, a holding only its own pair.
        let transactions = if crate::core::cash::is_cash_asset(asset_id) {
            self.transaction_repo.get_all_for_account(account_id).await
        } else {
            self.transaction_repo
                .get_by_account_asset(account_id, asset_id)
                .await
        }
        .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, err = ?e, "holding_snapshot_as_of: repository failure");
                AccountError::DatabaseError
            })?;
        Ok(Account::holding_snapshot_as_of(
            &transactions,
            asset_id,
            date,
        ))
    }

    /// SPL-062 — what a split of `factor` on `date` would make of the (account, asset)
    /// position, or the refusal recording it would give. Loads the account as recording
    /// does; saves nothing.
    pub async fn preview_split(
        &self,
        account_id: &str,
        asset_id: &str,
        date: &str,
        factor: i64,
    ) -> StdResult<StockSplitPosition, AccountError> {
        load_account(&*self.account_repo, account_id)
            .await?
            .preview_split(asset_id, date, factor)
            .map_err(to_holding_tx_error)
    }

    /// Retrieves every transaction for an account across all assets, ordered
    /// chronologically by `(date, created_at)` (PRF-021). Used by the
    /// account-performance use case to replay holdings and cash as of any past date.
    pub async fn get_all_transactions_for_account(
        &self,
        account_id: &str,
    ) -> StdResult<Vec<Transaction>, AccountError> {
        self.transaction_repo
            .get_all_for_account(account_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, err = ?e, "get_all_transactions_for_account: repository failure");
                AccountError::DatabaseError
            })
    }

    /// TXL-060 — the account journal of an account: its transactions in date order with
    /// their cash columns, the rows the filter keeps. An unknown account has an empty one.
    pub async fn get_account_journal(
        &self,
        account_id: &str,
        filter: &JournalFilter,
    ) -> StdResult<AccountJournal, AccountError> {
        let transactions = self.get_all_transactions_for_account(account_id).await?;
        let mut journal = AccountJournal::from_transactions(transactions, filter);
        if journal.rows.is_empty() {
            return Ok(journal);
        }
        // AGT-045 — each row says which agent session recorded it, if one did.
        let mut marks: std::collections::HashMap<String, AgentRecording> = self
            .transaction_repo
            .agent_recordings_for_account(account_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, err = ?e, "get_account_journal: agent recordings not read");
                AccountError::DatabaseError
            })?
            .into_iter()
            .collect();
        for row in &mut journal.rows {
            row.recorded_by = marks.remove(&row.transaction.id);
        }
        // TRF-062 — a part of a transfer names the transfer's other account.
        if journal
            .rows
            .iter()
            .any(|row| row.transaction.transfer_id.is_some())
        {
            let others: std::collections::HashMap<String, String> = self
                .transaction_repo
                .transfer_other_accounts(account_id)
                .await
                .map_err(|e| {
                    tracing::error!(target: BACKEND, account_id = %account_id, err = ?e, "get_account_journal: transfers' other accounts not read");
                    AccountError::DatabaseError
                })?
                .into_iter()
                .collect();
            for row in &mut journal.rows {
                row.transfer_account_id = row
                    .transaction
                    .transfer_id
                    .as_ref()
                    .and_then(|transfer_id| others.get(transfer_id).cloned());
            }
        }
        Ok(journal)
    }

    /// AGT-050 — the mark of a transaction, when an agent recorded it.
    pub async fn agent_recording_of(
        &self,
        transaction_id: &str,
    ) -> StdResult<Option<AgentRecording>, AccountError> {
        self.transaction_repo
            .agent_recording_of(transaction_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, transaction_id = %transaction_id, err = ?e, "agent_recording_of: repository failure");
                AccountError::DatabaseError
            })
    }

    /// AGT-053 — the transactions a session recorded and that still exist, the last recorded first.
    pub async fn recorded_in_session(
        &self,
        session: &str,
    ) -> StdResult<Vec<SessionRecording>, AccountError> {
        self.transaction_repo
            .recorded_in_session(session)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "recorded_in_session: repository failure");
                AccountError::DatabaseError
            })
    }

    /// AGT-045 — marks a transaction as recorded by an agent session, and tells the window
    /// the transaction changed.
    pub async fn mark_agent_recording(
        &self,
        transaction_id: &str,
        recording: &AgentRecording,
    ) -> StdResult<(), AccountError> {
        self.transaction_repo
            .mark_agent_recording(transaction_id, recording)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, transaction_id = %transaction_id, err = ?e, "mark_agent_recording: repository failure");
                AccountError::DatabaseError
            })?;
        self.emit_transaction_updated();
        Ok(())
    }

    /// Returns distinct asset IDs that have transactions for the given account (TXL-013).
    pub async fn get_asset_ids_for_account(
        &self,
        account_id: &str,
    ) -> StdResult<Vec<String>, AccountError> {
        self.transaction_repo
            .get_asset_ids_for_account(account_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, err = ?e, "get_asset_ids_for_account: repository failure");
                AccountError::DatabaseError
            })
    }

    // -------------------------------------------------------------------------
    // Aggregate operations (B21 — thin orchestrators)
    // -------------------------------------------------------------------------

    /// Records a purchase of an asset into the account (TRX-020, TRX-026, TRX-060).
    ///
    /// Loads the Account aggregate, delegates to `Account::buy_holding`, saves
    /// atomically. Returns a typed `AccountError` — same composite as
    /// the cash methods, since cash deposit/withdrawal and asset buy/sell are
    /// all kinds of holding transaction.
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
        info!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, "buy_holding");
        let mut account = load_account(&*self.account_repo, account_id).await?;
        let tx = apply(
            &mut account,
            RecordingOp::Buy(TradeOp {
                asset_id,
                date,
                quantity,
                unit_price,
                exchange_rate,
                fees,
                total_amount,
                note,
            }),
        )?;
        save_account(&*self.account_repo, &mut account).await?;
        self.emit_transaction_updated();
        Ok(tx)
    }

    /// Records a sale of an asset from the account (SEL-012, SEL-021, SEL-023, SEL-024, SEL-050).
    ///
    /// Loads the Account aggregate, delegates to `Account::sell_holding`, saves atomically.
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
        info!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, "sell_holding");
        let mut account = load_account(&*self.account_repo, account_id).await?;
        let tx = apply(
            &mut account,
            RecordingOp::Sell(TradeOp {
                asset_id,
                date,
                quantity,
                unit_price,
                exchange_rate,
                fees,
                total_amount,
                note,
            }),
        )?;
        save_account(&*self.account_repo, &mut account).await?;
        self.emit_transaction_updated();
        Ok(tx)
    }

    /// Corrects an existing transaction and recalculates the affected holding
    /// (TRX-031, SEL-031, TRX-061, SEL-051).
    ///
    /// Loads the Account aggregate, delegates to `Account::correct_transaction`, saves atomically.
    #[allow(clippy::too_many_arguments)]
    pub async fn correct_transaction(
        &self,
        account_id: &str,
        tx_id: &str,
        date: String,
        quantity: i64,
        unit_price: i64,
        exchange_rate: i64,
        fees: i64,
        total_amount: Option<i64>,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        info!(target: BACKEND, account_id = %account_id, tx_id = %tx_id, "correct_transaction");
        let (_, mut account, tx) = self
            .corrected(
                account_id,
                tx_id,
                date,
                quantity,
                unit_price,
                exchange_rate,
                fees,
                total_amount,
                note,
            )
            .await?;
        save_account(&*self.account_repo, &mut account).await?;
        self.emit_transaction_updated();
        Ok(tx)
    }

    /// Corrects a write-off's date, quantity, reason and note (WOF-030), the holding replayed.
    pub async fn correct_write_off(
        &self,
        account_id: &str,
        tx_id: &str,
        date: String,
        quantity: i64,
        reason: WriteOffReason,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        info!(target: BACKEND, account_id = %account_id, tx_id = %tx_id, reason = %reason, "correct_write_off");
        let mut account = load_account(&*self.account_repo, account_id).await?;
        let tx = account
            .correct_write_off(tx_id, date, quantity, reason, note)
            .map_err(to_holding_tx_error)?;
        save_account(&*self.account_repo, &mut account).await?;
        self.emit_transaction_updated();
        Ok(tx)
    }

    /// The transaction as `correct_transaction` would make it, with its refusals, and the
    /// account's transactions without and with the correction; nothing written: the
    /// corrected account is dropped (AGT-072, AGT-087).
    #[allow(clippy::too_many_arguments)]
    pub async fn preview_correction(
        &self,
        account_id: &str,
        tx_id: &str,
        date: String,
        quantity: i64,
        unit_price: i64,
        exchange_rate: i64,
        fees: i64,
        total_amount: Option<i64>,
        note: Option<String>,
    ) -> Result<RecordingPreview, AccountError> {
        let (before, account, transaction) = self
            .corrected(
                account_id,
                tx_id,
                date,
                quantity,
                unit_price,
                exchange_rate,
                fees,
                total_amount,
                note,
            )
            .await?;
        Ok(RecordingPreview {
            transaction,
            before,
            after: account.transactions,
        })
    }

    /// The account's transactions before the correction, the account with the transaction
    /// corrected in memory, and the transaction as corrected: what `correct_transaction`
    /// saves and `preview_correction` drops.
    #[allow(clippy::too_many_arguments)]
    async fn corrected(
        &self,
        account_id: &str,
        tx_id: &str,
        date: String,
        quantity: i64,
        unit_price: i64,
        exchange_rate: i64,
        fees: i64,
        total_amount: Option<i64>,
        note: Option<String>,
    ) -> Result<(Vec<Transaction>, Account, Transaction), AccountError> {
        let mut account = load_account(&*self.account_repo, account_id).await?;
        let before = account.transactions.clone();
        let tx = account
            .correct_transaction(
                tx_id,
                date,
                quantity,
                unit_price,
                exchange_rate,
                fees,
                total_amount,
                note,
            )
            .map_err(to_holding_tx_error)?
            .clone();
        Ok((before, account, tx))
    }

    /// AGT-072 — what recording `op` would make of the account: the account's own recording
    /// applied to it in memory, with the same refusals, and nothing written.
    pub async fn preview_recording(
        &self,
        account_id: &str,
        op: RecordingOp,
    ) -> Result<RecordingPreview, AccountError> {
        let mut account = load_account(&*self.account_repo, account_id).await?;
        let before = account.transactions.clone();
        let transaction = apply(&mut account, op)?;
        Ok(RecordingPreview {
            transaction,
            before,
            after: account.transactions.clone(),
        })
    }

    /// IMP-035 — recording steps judged in order, as manual entry would judge each
    /// typed after the ones accepted before it, nothing written: one matching a transaction
    /// the account holds is already recorded (IMP-036), a purchase the cash cannot fund is
    /// ready with the deposit it needs (IMP-038).
    pub async fn judge_recordings(
        &self,
        account_id: &str,
        steps: Vec<RecordingStep>,
    ) -> Result<Vec<RecordingJudgment>, AccountError> {
        let mut account = load_account(&*self.account_repo, account_id).await?;
        Ok(judge_all(&mut account, steps, true))
    }

    /// IMP-050 — recording steps judged as `judge_recordings` judges them, then every one
    /// taken recorded in one write, raising one `TransactionUpdated`. With `add_deposits`, a
    /// purchase the cash cannot fund is recorded after the deposit it needs, marked as added
    /// (IMP-051); without, it is refused as manual entry refuses it (CSH-041).
    pub async fn record_recordings(
        &self,
        account_id: &str,
        steps: Vec<RecordingStep>,
        add_deposits: bool,
    ) -> Result<Vec<RecordingJudgment>, AccountError> {
        info!(target: BACKEND, account_id = %account_id, steps = steps.len(), "record_recordings");
        let mut account = load_account(&*self.account_repo, account_id).await?;
        let judgments = judge_all(&mut account, steps, add_deposits);
        if judgments
            .iter()
            .any(|judgment| matches!(judgment, RecordingJudgment::Ready { .. }))
        {
            save_account(&*self.account_repo, &mut account).await?;
            self.emit_transaction_updated();
        }
        Ok(judgments)
    }

    /// AGT-072 — what recording an opening balance would make of the account, nothing
    /// written: `open_holding` applied in memory, with its refusals.
    pub async fn preview_opening(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        total_cost: i64,
    ) -> Result<RecordingPreview, OpenHoldingError> {
        let mut account = load_account_for_open_holding(&*self.account_repo, account_id).await?;
        let before = account.transactions.clone();
        let transaction = apply_opening(&mut account, asset_id, date, quantity, total_cost)?;
        Ok(RecordingPreview {
            transaction,
            before,
            after: account.transactions.clone(),
        })
    }

    /// Deletes a transaction and recalculates (or removes) the associated holding (TRX-034).
    ///
    /// Loads the Account aggregate, delegates to `Account::cancel_transaction`, saves atomically.
    pub async fn cancel_transaction(
        &self,
        account_id: &str,
        tx_id: &str,
    ) -> Result<(), AccountError> {
        info!(target: BACKEND, account_id = %account_id, tx_id = %tx_id, "cancel_transaction");
        let mut account = load_account(&*self.account_repo, account_id).await?;
        // TRF-040 — a part of a transfer is removed with the whole transfer.
        if let Some(transfer_id) = account
            .transactions
            .iter()
            .find(|t| t.id == tx_id)
            .and_then(|t| t.transfer_id.clone())
        {
            return self.cancel_transfer(&transfer_id).await;
        }
        account
            .cancel_transaction(tx_id)
            .map_err(to_holding_tx_error)?;
        save_account(&*self.account_repo, &mut account).await?;
        self.emit_transaction_updated();
        Ok(())
    }

    /// Seeds a holding directly from a quantity and total cost (TRX-042, TRX-047).
    ///
    /// Asset existence and archived-status checks are the caller's responsibility
    /// (handled by `HoldingTransactionUseCase::open_holding` — TRX-050, TRX-056).
    /// Returns the use-case-owned `OpenHoldingError`; the service-internal slice
    /// (load + aggregate + save) raises `Application(AccountNotFound | DatabaseError)`,
    /// `Validation(InvalidTotalCost)`, or `TxValidation(...)`. Cross-BC asset
    /// rejections never reach this method — the orchestrator raises them before
    /// delegating.
    pub async fn open_holding(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        total_cost: i64,
    ) -> Result<Transaction, OpenHoldingError> {
        info!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, "open_holding");
        let mut account = load_account_for_open_holding(&*self.account_repo, account_id).await?;
        let tx = apply_opening(&mut account, asset_id, date, quantity, total_cost)?;
        save_account_for_open_holding(&*self.account_repo, &mut account).await?;
        self.emit_transaction_updated();
        Ok(tx)
    }

    /// Records a Deposit (CSH-022) — cash inflow into the account.
    ///
    /// Application-layer composition: loads the Account, builds the Transaction
    /// via `Transaction::new_deposit` (TRX-020 enforced by the factory), applies
    /// it via `Account::apply_deposit` (CSH-080 enforced by the aggregate),
    /// then saves atomically. Returns a typed `AccountError` — no
    /// `anyhow` at this boundary; the caller (orchestrator / api) propagates
    /// the typed enum directly.
    pub async fn record_deposit(
        &self,
        account_id: &str,
        date: String,
        amount: i64,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        info!(target: BACKEND, account_id = %account_id, amount = amount, "record_deposit");
        let mut account = load_account(&*self.account_repo, account_id).await?;
        let tx = apply(&mut account, RecordingOp::Deposit { date, amount, note })?;
        save_account(&*self.account_repo, &mut account).await?;
        self.emit_transaction_updated();
        Ok(tx)
    }

    /// Records a Withdrawal (CSH-032) — cash outflow from the account.
    ///
    /// Application-layer composition mirroring `record_deposit`. Raises
    /// `InsufficientCash` (CSH-080) when no Cash Holding exists or its balance
    /// is below `amount` — the check lives inside `Account::apply_withdrawal`.
    pub async fn record_withdrawal(
        &self,
        account_id: &str,
        date: String,
        amount: i64,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        info!(target: BACKEND, account_id = %account_id, amount = amount, "record_withdrawal");
        let mut account = load_account(&*self.account_repo, account_id).await?;
        let tx = apply(&mut account, RecordingOp::Withdrawal { date, amount, note })?;
        save_account(&*self.account_repo, &mut account).await?;
        self.emit_transaction_updated();
        Ok(tx)
    }

    // -------------------------------------------------------------------------
    // Cross-BC guard queries (called by use cases)
    // -------------------------------------------------------------------------

    /// Returns true if any account holds a non-zero quantity of the given asset.
    /// Used by the archive_asset use case to enforce OQ-6. Translates raw
    /// infra failure into `AccountError::DatabaseError` per the
    /// gold infra-translation rule.
    pub async fn has_active_holdings_for_asset(
        &self,
        asset_id: &str,
    ) -> StdResult<bool, AccountError> {
        self.holding_repo
            .has_active_holdings_for_asset(asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, asset_id = %asset_id, err = ?e, "has_active_holdings_for_asset: repository failure");
                AccountError::DatabaseError
            })
    }

    /// Returns true if any transaction references the given asset.
    /// Used by the delete_asset use case to block hard-deletion when history
    /// exists. Translates raw infra failure into
    /// `AccountError::DatabaseError` per the gold infra-translation rule.
    pub async fn has_holding_entries_for_asset(
        &self,
        asset_id: &str,
    ) -> StdResult<bool, AccountError> {
        self.transaction_repo
            .has_transactions_for_asset(asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, asset_id = %asset_id, err = ?e, "has_holding_entries_for_asset: repository failure");
                AccountError::DatabaseError
            })
    }

    /// Returns the count of active holdings and total transactions for an account (ACC-020).
    pub async fn get_deletion_summary(
        &self,
        account_id: &str,
    ) -> StdResult<(u32, u32), AccountError> {
        tokio::try_join!(
            self.holding_repo.count_active_for_account(account_id),
            self.transaction_repo.count_by_account(account_id),
        )
        .map_err(|e| {
            tracing::error!(target: BACKEND, account_id = %account_id, err = ?e, "get_deletion_summary: repository failure");
            AccountError::DatabaseError
        })
    }

    /// Records a cash Dividend attributed to a held paying asset (DIV-023).
    ///
    /// Application-layer composition: loads the Account, builds the Transaction
    /// via `Transaction::new_dividend` (DIV-021/022 enforced by the factory),
    /// applies it via `Account::apply_dividend` (credit-only; no InsufficientCash),
    /// then saves atomically. Returns a typed `AccountError`.
    pub async fn record_dividend(
        &self,
        account_id: &str,
        paying_asset_id: String,
        date: String,
        amount_micros: i64,
        exchange_rate: i64,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        info!(target: BACKEND, account_id = %account_id, asset_id = %paying_asset_id, amount = amount_micros, "record_dividend");
        let mut account = load_account(&*self.account_repo, account_id).await?;
        let tx = apply(
            &mut account,
            RecordingOp::Dividend {
                asset_id: paying_asset_id,
                date,
                amount: amount_micros,
                exchange_rate,
                note,
            },
        )?;
        save_account(&*self.account_repo, &mut account).await?;
        self.emit_transaction_updated();
        Ok(tx)
    }

    /// Records a FreeShares distribution attributed to a held distributing asset
    /// (FSD-022).
    ///
    /// Application-layer composition: loads the Account, builds the Transaction
    /// via `Transaction::free_shares` (FSD-021 enforced by the factory), applies
    /// it via `Account::apply_free_shares` (holding quantity rises at zero cost,
    /// no cash leg), then saves atomically. Returns a typed
    /// `AccountError`.
    pub async fn record_free_shares(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        info!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, quantity = quantity, "record_free_shares");
        let mut account = load_account(&*self.account_repo, account_id).await?;
        let tx = apply(
            &mut account,
            RecordingOp::FreeShares {
                asset_id,
                date,
                quantity,
                note,
            },
        )?;
        save_account(&*self.account_repo, &mut account).await?;
        self.emit_transaction_updated();
        Ok(tx)
    }

    /// Records a write-off of a held asset (WOF-010/020/021): the quantity leaves at its
    /// cost, no cash moves, its realized P&L is the cost taken out, negated.
    pub async fn record_write_off(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        reason: WriteOffReason,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        info!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, quantity = quantity, reason = %reason, "record_write_off");
        let mut account = load_account(&*self.account_repo, account_id).await?;
        let tx = apply(
            &mut account,
            RecordingOp::WriteOff {
                asset_id,
                date,
                quantity,
                reason,
                note,
            },
        )?;
        save_account(&*self.account_repo, &mut account).await?;
        self.emit_transaction_updated();
        Ok(tx)
    }

    /// TRF-020/021/022 — records a transfer of `asset_id` from one account to another, its
    /// fee in kind included when less is received than sent, at the rate the caller
    /// resolved (micro-units; one between accounts of one currency). Both accounts are
    /// written together; asset existence and archiving are the caller's (TRF-010).
    #[allow(clippy::too_many_arguments)]
    pub async fn record_transfer(
        &self,
        from_account_id: &str,
        to_account_id: &str,
        asset_id: String,
        date: String,
        quantity_sent: i64,
        quantity_received: i64,
        exchange_rate: i64,
        note: Option<String>,
    ) -> Result<Vec<Transaction>, AccountError> {
        info!(target: BACKEND, from = %from_account_id, to = %to_account_id, asset_id = %asset_id, "record_transfer");
        let transfer_id = Uuid::new_v4().to_string();
        let parts = self
            .place_transfer(
                &transfer_id,
                from_account_id,
                to_account_id,
                TransferFigures {
                    asset_id,
                    date,
                    quantity_sent,
                    quantity_received,
                    exchange_rate,
                    note,
                },
                None,
            )
            .await?;
        self.emit_transaction_updated();
        Ok(parts)
    }

    /// AGT-045 / AGT-049 — records a transfer as `record_transfer` does, every part marked
    /// with the agent's session in the same write: a mark that cannot be written leaves
    /// nothing recorded.
    #[allow(clippy::too_many_arguments)]
    pub async fn record_transfer_by_agent(
        &self,
        from_account_id: &str,
        to_account_id: &str,
        asset_id: String,
        date: String,
        quantity_sent: i64,
        quantity_received: i64,
        exchange_rate: i64,
        note: Option<String>,
        mark: &AgentRecording,
    ) -> Result<Vec<Transaction>, AccountError> {
        info!(target: BACKEND, from = %from_account_id, to = %to_account_id, asset_id = %asset_id, "record_transfer_by_agent");
        let transfer_id = Uuid::new_v4().to_string();
        let parts = self
            .place_transfer(
                &transfer_id,
                from_account_id,
                to_account_id,
                TransferFigures {
                    asset_id,
                    date,
                    quantity_sent,
                    quantity_received,
                    exchange_rate,
                    note,
                },
                Some(mark),
            )
            .await?;
        self.emit_transaction_updated();
        Ok(parts)
    }

    /// TRF-030 — corrects the transfer `transfer_id` as one operation: its date, quantities,
    /// rate and note; its accounts and asset stay. Checked again on the history without it.
    #[allow(clippy::too_many_arguments)]
    pub async fn correct_transfer(
        &self,
        transfer_id: &str,
        date: String,
        quantity_sent: i64,
        quantity_received: i64,
        exchange_rate: i64,
        note: Option<String>,
    ) -> Result<Vec<Transaction>, AccountError> {
        info!(target: BACKEND, transfer_id = %transfer_id, "correct_transfer");
        let (from, to, asset_id) = self.transfer_sides(transfer_id).await?;
        let parts = self
            .place_transfer(
                transfer_id,
                &from,
                &to,
                TransferFigures {
                    asset_id,
                    date,
                    quantity_sent,
                    quantity_received,
                    exchange_rate,
                    note,
                },
                None,
            )
            .await?;
        self.emit_transaction_updated();
        Ok(parts)
    }

    /// The sending account, the receiving account and the asset of the transfer
    /// `transfer_id`; a transfer unknown or left without a side is not found (TRF-030).
    pub async fn transfer_sides(
        &self,
        transfer_id: &str,
    ) -> Result<(String, String, String), AccountError> {
        let accounts = self
            .account_repo
            .transfer_accounts(transfer_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, transfer_id = %transfer_id, err = ?e, "transfer_sides: repository failure");
                AccountError::DatabaseError
            })?;
        let [from, to] = accounts.as_slice() else {
            return Err(AccountError::TransferNotFound);
        };
        let sending = load_account(&*self.account_repo, from).await?;
        let asset_id = sending
            .transfer_parts(transfer_id)
            .iter()
            .find(|t| t.transaction_type == TransactionType::TransferOut)
            .map(|t| t.asset_id.clone())
            .ok_or(AccountError::TransferNotFound)?;
        Ok((from.clone(), to.clone(), asset_id))
    }

    /// The parts of the transfer `transfer_id` as they stand, in every account holding one.
    pub async fn transfer_parts(
        &self,
        transfer_id: &str,
    ) -> Result<Vec<Transaction>, AccountError> {
        let mut parts = Vec::new();
        for account_id in self.transfer_account_ids(transfer_id).await? {
            let account = load_account(&*self.account_repo, &account_id).await?;
            parts.extend(account.transfer_parts(transfer_id).into_iter().cloned());
        }
        Ok(parts)
    }

    async fn transfer_account_ids(&self, transfer_id: &str) -> Result<Vec<String>, AccountError> {
        self.account_repo
            .transfer_accounts(transfer_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, transfer_id = %transfer_id, err = ?e, "transfer_accounts: repository failure");
                AccountError::DatabaseError
            })
    }

    /// TRF-060 — the transfer `transfer_id` read as one from its parts; an unknown transfer,
    /// or one that has lost a side, is `TransferNotFound`.
    pub async fn transfer_view(&self, transfer_id: &str) -> Result<TransferView, AccountError> {
        let parts = self.transfer_parts(transfer_id).await?;
        TransferView::from_parts(transfer_id, &parts).ok_or(AccountError::TransferNotFound)
    }

    /// TRF-040 — removes every part of the transfer `transfer_id`, in every account holding
    /// one, together; a later transaction left short in the receiving account refuses it.
    pub async fn cancel_transfer(&self, transfer_id: &str) -> Result<(), AccountError> {
        info!(target: BACKEND, transfer_id = %transfer_id, "cancel_transfer");
        let account_ids = self.transfer_account_ids(transfer_id).await?;
        if account_ids.is_empty() {
            return Err(AccountError::TransferNotFound);
        }
        let mut accounts = Vec::with_capacity(account_ids.len());
        for account_id in &account_ids {
            let mut account = load_account(&*self.account_repo, account_id).await?;
            account
                .remove_transfer_parts(transfer_id)
                .map_err(to_holding_tx_error)?;
            accounts.push(account);
        }
        let mut written: Vec<&mut Account> = accounts.iter_mut().collect();
        save_accounts(&*self.account_repo, &mut written).await?;
        self.emit_transaction_updated();
        Ok(())
    }

    /// AGT-065A — what recording a transfer would make of its two accounts: the checks of
    /// `record_transfer`, nothing written.
    #[allow(clippy::too_many_arguments)]
    pub async fn preview_transfer(
        &self,
        from_account_id: &str,
        to_account_id: &str,
        asset_id: String,
        date: String,
        quantity_sent: i64,
        quantity_received: i64,
        exchange_rate: i64,
        note: Option<String>,
    ) -> Result<TransferPreview, AccountError> {
        let transfer_id = Uuid::new_v4().to_string();
        let staged = self
            .stage_transfer(
                &transfer_id,
                from_account_id,
                to_account_id,
                TransferFigures {
                    asset_id,
                    date,
                    quantity_sent,
                    quantity_received,
                    exchange_rate,
                    note,
                },
            )
            .await?;
        Ok(staged.into_preview())
    }

    /// Places the parts of `transfer_id` in both accounts from `figures` (TRF-020 to
    /// TRF-022, TRF-030) and writes both, with what re-costing follows (TRF-025).
    async fn place_transfer(
        &self,
        transfer_id: &str,
        from_account_id: &str,
        to_account_id: &str,
        figures: TransferFigures,
        mark: Option<&AgentRecording>,
    ) -> Result<Vec<Transaction>, AccountError> {
        let mut staged = self
            .stage_transfer(transfer_id, from_account_id, to_account_id, figures)
            .await?;
        let ids: Vec<String> = staged.parts.iter().map(|part| part.id.clone()).collect();
        let mut written = vec![&mut staged.sending, &mut staged.receiving];
        save_accounts_marked(
            &*self.account_repo,
            &mut written,
            mark.map(|mark| (ids.as_slice(), mark)),
        )
        .await?;
        Ok(staged.parts)
    }

    /// Both accounts with the parts of `transfer_id` placed from `figures`, checked and
    /// replayed (TRF-010 to TRF-022, TRF-030), nothing written.
    async fn stage_transfer(
        &self,
        transfer_id: &str,
        from_account_id: &str,
        to_account_id: &str,
        figures: TransferFigures,
    ) -> Result<StagedTransfer, AccountError> {
        if from_account_id == to_account_id {
            return Err(AccountError::TransferToSameAccount);
        }
        let sending = load_account(&*self.account_repo, from_account_id).await?;
        let receiving = load_account(&*self.account_repo, to_account_id).await?;
        stage_on(transfer_id, sending, receiving, figures)
    }

    /// AGT-091 — both accounts with the transactions `replaced` taken out and a new transfer
    /// placed from `figures` in their stead, the cash replayed, checked as TRF-010 to TRF-022
    /// and CSH-080 check them; nothing written. No state between the two is checked: the
    /// accounts stand as if the transfer had been recorded instead from the start. Also
    /// answers the four as they were stored, in the order of `replaced`.
    async fn stage_replacement(
        &self,
        replaced: &[String],
        from_account_id: &str,
        to_account_id: &str,
        figures: TransferFigures,
    ) -> Result<(StagedTransfer, Vec<Transaction>), AccountError> {
        if from_account_id == to_account_id {
            return Err(AccountError::TransferToSameAccount);
        }
        let mut sending = load_account(&*self.account_repo, from_account_id).await?;
        let mut receiving = load_account(&*self.account_repo, to_account_id).await?;
        let stored = replaced
            .iter()
            .map(|id| {
                sending
                    .transactions
                    .iter()
                    .chain(&receiving.transactions)
                    .find(|t| &t.id == id)
                    .cloned()
                    .ok_or(AccountError::TransactionNotFound)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (sending_before, receiving_before) =
            (sending.transactions.clone(), receiving.transactions.clone());
        let ids_in = |account_id: &str| -> Vec<String> {
            stored
                .iter()
                .filter(|t| t.account_id == account_id)
                .map(|t| t.id.clone())
                .collect()
        };
        sending
            .take_out(&ids_in(from_account_id))
            .map_err(to_holding_tx_error)?;
        receiving
            .take_out(&ids_in(to_account_id))
            .map_err(to_holding_tx_error)?;
        let transfer_id = Uuid::new_v4().to_string();
        let mut staged = stage_on(&transfer_id, sending, receiving, figures)?;
        staged.sending.settle_cash().map_err(to_holding_tx_error)?;
        staged
            .receiving
            .settle_cash()
            .map_err(to_holding_tx_error)?;
        staged.sending_before = sending_before;
        staged.receiving_before = receiving_before;
        Ok((staged, stored))
    }

    /// AGT-090 — what replacing the transactions `replaced` with a transfer would make of its
    /// two accounts: the checks of `replace_with_transfer`, nothing written.
    #[allow(clippy::too_many_arguments)]
    pub async fn preview_replacement(
        &self,
        replaced: &[String],
        from_account_id: &str,
        to_account_id: &str,
        asset_id: String,
        date: String,
        quantity_sent: i64,
        quantity_received: i64,
        exchange_rate: i64,
    ) -> Result<TransferPreview, AccountError> {
        let (staged, _) = self
            .stage_replacement(
                replaced,
                from_account_id,
                to_account_id,
                TransferFigures {
                    asset_id,
                    date,
                    quantity_sent,
                    quantity_received,
                    exchange_rate,
                    note: None,
                },
            )
            .await?;
        Ok(staged.into_preview())
    }

    /// AGT-091 — replaces the transactions `shown` with a transfer in one write, as
    /// `stage_replacement` stages it, only while each stands as it was shown and the
    /// transfer moves `cost_shown`; `None` when not, nothing written.
    #[allow(clippy::too_many_arguments)]
    pub async fn replace_with_transfer(
        &self,
        shown: &[Transaction],
        from_account_id: &str,
        to_account_id: &str,
        asset_id: String,
        date: String,
        quantity_sent: i64,
        quantity_received: i64,
        exchange_rate: i64,
        cost_shown: i64,
    ) -> Result<Option<Vec<Transaction>>, AccountError> {
        info!(target: BACKEND, from = %from_account_id, to = %to_account_id, asset_id = %asset_id, "replace_with_transfer");
        let replaced: Vec<String> = shown.iter().map(|t| t.id.clone()).collect();
        let (mut staged, stored) = self
            .stage_replacement(
                &replaced,
                from_account_id,
                to_account_id,
                TransferFigures {
                    asset_id,
                    date,
                    quantity_sent,
                    quantity_received,
                    exchange_rate,
                    note: None,
                },
            )
            .await?;
        let cost = staged
            .parts
            .iter()
            .find(|t| t.transaction_type == TransactionType::TransferOut)
            .map_or(0, |t| t.total_amount);
        let stands = stored
            .iter()
            .zip(shown)
            .all(|(stored, shown)| stored.stands_as(shown));
        if !stands || cost != cost_shown {
            return Ok(None);
        }
        let mut written = vec![&mut staged.sending, &mut staged.receiving];
        save_accounts(&*self.account_repo, &mut written).await?;
        self.emit_transaction_updated();
        Ok(Some(staged.parts))
    }
}

/// Both accounts, as loaded, with the parts of `transfer_id` placed from `figures`, checked
/// and replayed (TRF-010 to TRF-022, TRF-030), nothing written.
fn stage_on(
    transfer_id: &str,
    mut sending: Account,
    mut receiving: Account,
    figures: TransferFigures,
) -> Result<StagedTransfer, AccountError> {
    let (from_account_id, to_account_id) = (sending.id.clone(), receiving.id.clone());
    let (from_account_id, to_account_id) = (from_account_id.as_str(), to_account_id.as_str());
    let sending_before = sending.transactions.clone();
    let receiving_before = receiving.transactions.clone();
    let TransferFigures {
        asset_id,
        date,
        quantity_sent,
        quantity_received,
        exchange_rate,
        note,
    } = figures;
    if quantity_sent <= 0 || quantity_received <= 0 {
        return Err(AccountError::QuantityNotPositive);
    }
    if quantity_received > quantity_sent {
        return Err(AccountError::TransferReceivedAboveSent);
    }
    if sending.currency == receiving.currency && exchange_rate != 1_000_000 {
        return Err(AccountError::TransferRateNotOne);
    }
    let previous: Vec<Transaction> = sending
        .transfer_parts(transfer_id)
        .into_iter()
        .chain(receiving.transfer_parts(transfer_id))
        .cloned()
        .collect();
    let kept = |kind: TransactionType| {
        previous
            .iter()
            .find(|t| t.transaction_type == kind)
            .map(|t| (t.id.clone(), t.created_at.clone()))
    };
    // A correction keeps each part's identity (TRF-030); a new transfer has none yet.
    let side = |kind: TransactionType,
                account_id: &str,
                date: &str,
                total_amount: i64,
                exchange_rate: i64| match kept(kind) {
        Some((id, created_at)) => Transaction::transfer_side_with_id(
            id,
            created_at,
            account_id.to_string(),
            asset_id.clone(),
            kind,
            date.to_string(),
            quantity_received,
            total_amount,
            exchange_rate,
            note.clone(),
            transfer_id.to_string(),
        ),
        None => Transaction::new_transfer_side(
            account_id.to_string(),
            asset_id.clone(),
            kind,
            date.to_string(),
            quantity_received,
            total_amount,
            exchange_rate,
            note.clone(),
            transfer_id.to_string(),
        ),
    };
    let mut sent_parts = Vec::new();
    if quantity_sent > quantity_received {
        let quantity = quantity_sent - quantity_received;
        let fee = match kept(TransactionType::WriteOff) {
            Some((id, created_at)) => Transaction::write_off_with_id(
                id,
                from_account_id.to_string(),
                asset_id.clone(),
                date.clone(),
                quantity,
                WriteOffReason::NetworkFee,
                note.clone(),
                created_at,
            ),
            None => Transaction::write_off(
                from_account_id.to_string(),
                asset_id.clone(),
                date.clone(),
                quantity,
                WriteOffReason::NetworkFee,
                note.clone(),
            ),
        }?
        .with_transfer_id(Some(transfer_id.to_string()));
        sent_parts.push(fee);
    }
    sent_parts.push(side(
        TransactionType::TransferOut,
        from_account_id,
        &date,
        0,
        1_000_000,
    )?);
    let placed_out = sending
        .place_transfer_parts(transfer_id, &asset_id, sent_parts)
        .map_err(to_holding_tx_error)?;
    let cost = placed_out
        .iter()
        .find(|t| t.transaction_type == TransactionType::TransferOut)
        .map(|t| t.total_amount)
        .unwrap_or(0);
    let incoming = side(
        TransactionType::TransferIn,
        to_account_id,
        &date,
        super::domain::converted_cost(cost, exchange_rate),
        exchange_rate,
    )?;
    let placed_in = receiving
        .place_transfer_parts(transfer_id, &asset_id, vec![incoming])
        .map_err(to_holding_tx_error)?;
    Ok(StagedTransfer {
        parts: placed_out.into_iter().chain(placed_in).collect(),
        sending,
        receiving,
        sending_before,
        receiving_before,
    })
}

impl AccountService {
    /// Records a stock split on a held asset (SPL-010/020).
    ///
    /// The micro-scaled factor rescales the position at the split date; the
    /// cost basis is preserved and no money moves. Rejects splits on the cash
    /// line, on closed positions, and rescales that collapse the position
    /// (SPL-011/012/021).
    pub async fn record_split(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        factor: i64,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        info!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, factor = factor, "record_split");
        let mut account = load_account(&*self.account_repo, account_id).await?;
        let tx = apply(
            &mut account,
            RecordingOp::Split {
                asset_id,
                date,
                factor,
                note,
            },
        )?;
        save_account(&*self.account_repo, &mut account).await?;
        self.emit_transaction_updated();
        Ok(tx)
    }

    // -------------------------------------------------------------------------
    // FEE-012/021/022/023/027 — management fee recording
    // -------------------------------------------------------------------------

    /// Records a one-off management fee deduction on a held asset (FEE-012).
    ///
    /// `percent_micros` is the fee in micro-percent (1% = 1_000_000). The number
    /// of shares removed is `floor(holding_qty_as_of(date) × percent_micros /
    /// 100_000_000)`. Cost basis is unchanged (VWAP concentrates — FEE-023).
    /// No cash leg. Raises `CascadingOversell` if a chronological replay of
    /// subsequent transactions would drive the holding negative (FEE-027).
    pub async fn record_management_fee(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        percent_micros: i64,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        info!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, percent_micros = percent_micros, "record_management_fee");
        self.apply_management_fee_deduction(
            account_id,
            asset_id,
            date,
            percent_micros,
            1,
            note,
            None,
        )
        .await
    }

    /// Records a generated management fee deduction (FEE-040) under the deterministic
    /// identity FEE-048 assigns it (`deduction_id`). It removes one period's share of the
    /// annual rate, `floor(held × annual_rate ÷ periods_per_year)`, floored once (FEE-041);
    /// otherwise identical to a one-off deduction without a note.
    pub async fn record_generated_management_fee(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        annual_rate_percent_micros: i64,
        periods_per_year: i64,
        deduction_id: String,
    ) -> Result<Transaction, AccountError> {
        info!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, annual_rate_percent_micros = annual_rate_percent_micros, periods_per_year = periods_per_year, deduction_id = %deduction_id, "record_generated_management_fee");
        self.apply_management_fee_deduction(
            account_id,
            asset_id,
            date,
            annual_rate_percent_micros,
            periods_per_year,
            None,
            Some(deduction_id),
        )
        .await
    }

    /// Records a one-off management fee entered either as a percentage of the holding or
    /// as the quantity the holding should hold after it — exactly one of the two (FEE-021).
    pub async fn record_management_fee_entry(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        percent_micros: Option<i64>,
        resulting_quantity: Option<i64>,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        match (percent_micros, resulting_quantity) {
            (Some(percent_micros), None) => {
                self.record_management_fee(account_id, asset_id, date, percent_micros, note)
                    .await
            }
            (None, Some(resulting_quantity)) => {
                self.record_management_fee_by_resulting_quantity(
                    account_id,
                    asset_id,
                    date,
                    resulting_quantity,
                    note,
                )
                .await
            }
            _ => Err(AccountError::ManagementFeeAmountInvalid),
        }
    }

    /// Records a one-off management fee entered as the quantity the holding should hold
    /// after it (FEE-028): the removal is the held quantity as of `date` minus
    /// `resulting_quantity`, exactly. Otherwise identical to `record_management_fee`.
    pub async fn record_management_fee_by_resulting_quantity(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        resulting_quantity: i64,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        info!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, resulting_quantity = resulting_quantity, "record_management_fee_by_resulting_quantity");
        let mut account = load_account(&*self.account_repo, account_id).await?;
        // FEE-077 — the % fee mechanism must be enabled on the account.
        account.ensure_management_fees_enabled()?;
        let removal = account.management_fee_removal(&asset_id, &date, resulting_quantity)?;
        let tx = Transaction::management_fee(
            account.id.clone(),
            asset_id,
            date,
            removal.removed_quantity,
            note,
        )?;
        let tx = account
            .apply_management_fee(tx)
            .map_err(to_holding_tx_error)?;
        save_account(&*self.account_repo, &mut account).await?;
        self.emit_transaction_updated();
        Ok(tx)
    }

    /// What `record_management_fee_by_resulting_quantity` would remove (FEE-029); writes
    /// nothing.
    pub async fn preview_management_fee(
        &self,
        account_id: &str,
        asset_id: &str,
        date: &str,
        resulting_quantity: i64,
    ) -> Result<ManagementFeeRemoval, AccountError> {
        let account = load_account(&*self.account_repo, account_id).await?;
        account.ensure_management_fees_enabled()?;
        account.management_fee_removal(asset_id, date, resulting_quantity)
    }

    // One recording for the one-off fee and the generated one: what differs between them
    // is passed in, not branched on by the callers.
    #[allow(clippy::too_many_arguments)]
    async fn apply_management_fee_deduction(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        percent_micros: i64,
        periods: i64,
        note: Option<String>,
        deduction_id: Option<String>,
    ) -> Result<Transaction, AccountError> {
        // FEE-021 — percentage is micro-percent: strictly positive, at most 100%.
        if percent_micros <= 0 {
            return Err(AccountError::PercentageNotPositive);
        }
        if percent_micros > 100_000_000 {
            return Err(AccountError::PercentageAboveHundred);
        }
        let mut account = load_account(&*self.account_repo, account_id).await?;
        // FEE-077 — the % fee mechanism must be enabled on the account.
        account.ensure_management_fees_enabled()?;
        // FEE-022a / FEE-041 — removed qty = floor(holding_qty_as_of(date) × percent /
        // 100% ÷ periods): one division, so one flooring. A one-off fee is one period.
        let quantity_as_of = account.holding_quantity_as_of(&asset_id, &date);
        let removed = (quantity_as_of as i128 * percent_micros as i128
            / (100_000_000 * periods.max(1) as i128)) as i64;
        // CFR-016 — a generated deduction is the application's own write; a one-off
        // deduction is the user's.
        let tx = match deduction_id {
            Some(id) => {
                let tx = Transaction::generated_management_fee(
                    id,
                    account.id.clone(),
                    asset_id,
                    date,
                    removed,
                )?;
                account
                    .apply_generated_management_fee(tx)
                    .map_err(to_holding_tx_error)?
            }
            None => {
                let tx =
                    Transaction::management_fee(account.id.clone(), asset_id, date, removed, note)?;
                account
                    .apply_management_fee(tx)
                    .map_err(to_holding_tx_error)?
            }
        };
        save_account(&*self.account_repo, &mut account).await?;
        self.emit_transaction_updated();
        Ok(tx)
    }

    // -------------------------------------------------------------------------
    // INT-021/022/023/024/025 — interest recording
    // -------------------------------------------------------------------------

    /// Records an Interest credit on a held asset or the account's cash line
    /// (INT-021/022/023/024).
    ///
    /// Exactly one of `percent_micros` / `quantity_micros` must be provided
    /// (INT-021). Percent mode: the credited quantity is
    /// `floor(holding_qty_as_of(date) × percent_micros / 100_000_000)` (INT-022);
    /// a computed credit of 0 (empty holding or rate too small) is rejected as
    /// `QuantityNotPositive` by the `Transaction::interest` factory. Cost basis
    /// is unchanged (VWAP dilutes — INT-024); on the Cash Asset the credit goes
    /// through the cash replay (INT-023). Not gated by the account's
    /// `management_fees_enabled` parameter (INT-050).
    pub async fn record_interest(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        percent_micros: Option<i64>,
        quantity_micros: Option<i64>,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        info!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, percent_micros = percent_micros, quantity_micros = quantity_micros, "record_interest");
        // INT-021 — exactly one of percent / quantity; bounds per mode.
        match (percent_micros, quantity_micros) {
            (Some(percent_micros), None) => {
                if percent_micros <= 0 {
                    return Err(AccountError::PercentageNotPositive);
                }
                if percent_micros > 100_000_000 {
                    return Err(AccountError::PercentageAboveHundred);
                }
            }
            (None, Some(quantity_micros)) => {
                if quantity_micros <= 0 {
                    return Err(AccountError::QuantityNotPositive);
                }
            }
            _ => return Err(AccountError::InterestAmountInvalid),
        }
        let mut account = load_account(&*self.account_repo, account_id).await?;
        let credited = match (percent_micros, quantity_micros) {
            // INT-022 — credited qty = floor(holding_qty_as_of(date) × percent / 100%).
            // The cash line's balance moves with Purchase/Sell/Dividend transactions
            // whose asset_id is NOT the cash asset, so its percent base comes from the
            // dedicated cash replay, not the per-asset holding replay (INT-023).
            (Some(percent_micros), None) => {
                let quantity_as_of = if crate::core::cash::is_cash_asset(&asset_id) {
                    Account::cash_balance_as_of(&account.transactions, &date)
                } else {
                    account.holding_quantity_as_of(&asset_id, &date)
                };
                (quantity_as_of as i128 * percent_micros as i128 / 100_000_000) as i64
            }
            (None, Some(quantity_micros)) => quantity_micros,
            // Unreachable — both/neither already rejected above (INT-021).
            _ => return Err(AccountError::InterestAmountInvalid),
        };
        let tx = Transaction::interest(account.id.clone(), asset_id, date, credited, note)?;
        let tx = account.apply_interest(tx).map_err(to_holding_tx_error)?;
        save_account(&*self.account_repo, &mut account).await?;
        self.emit_transaction_updated();
        Ok(tx)
    }

    // -------------------------------------------------------------------------
    // FEE-030/031/032/033/034/060/061/062 — fee schedule CRUD
    // -------------------------------------------------------------------------

    /// Creates a new fee schedule for the (account, asset) pair (FEE-030).
    ///
    /// FEE-031 — rejects if a schedule already exists for the pair.
    /// FEE-032 — validates rate > 0 (`RateNotPositive`), rate < 100 % (`RateAboveHundred`),
    /// end_date > start_date (`EndBeforeStart`).
    pub async fn create_fee_schedule(
        &self,
        account_id: &str,
        asset_id: String,
        annual_rate_percent_micros: i64,
        frequency: super::domain::FeeFrequency,
        start_date: String,
        end_date: Option<String>,
    ) -> Result<FeeSchedule, AccountError> {
        info!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, "create_fee_schedule");
        // FEE-077 — the % fee mechanism must be enabled on the account.
        self.get_by_id(account_id)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound {
                account_id: account_id.to_string(),
            })?
            .ensure_management_fees_enabled()?;
        let repo = self.fee_schedule_repo()?;
        // FEE-031 — at most one schedule per (account, asset).
        let existing = repo
            .get_by_account_asset(account_id, &asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "create_fee_schedule: lookup failed");
                AccountError::DatabaseError
            })?;
        if existing.is_some() {
            return Err(AccountError::ScheduleAlreadyExists);
        }
        // FEE-032 — validates rate (> 0, < 100 %) and end_date > start_date.
        let schedule = FeeSchedule::new(
            account_id.to_string(),
            asset_id,
            annual_rate_percent_micros,
            frequency,
            start_date,
            end_date,
        )?;
        repo.insert(&schedule).await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "create_fee_schedule: insert failed");
            AccountError::DatabaseError
        })?;
        self.emit_fee_schedule_updated();
        Ok(schedule)
    }

    /// Updates an existing fee schedule (FEE-060/061).
    ///
    /// FEE-060 — rejects with `ScheduleNotFound` if no schedule exists.
    /// Editable fields: `annual_rate_percent_micros`, `end_date`, `active`.
    /// `frequency` and `start_date` are immutable after creation.
    pub async fn update_fee_schedule(
        &self,
        account_id: &str,
        asset_id: &str,
        annual_rate_percent_micros: i64,
        end_date: Option<String>,
        active: bool,
    ) -> Result<FeeSchedule, AccountError> {
        let today = chrono::Local::now().date_naive();
        self.update_fee_schedule_on(
            account_id,
            asset_id,
            annual_rate_percent_micros,
            end_date,
            active,
            today,
        )
        .await
    }

    /// `update_fee_schedule` on a given day: the day a reactivation resumes from (FEE-061).
    pub(crate) async fn update_fee_schedule_on(
        &self,
        account_id: &str,
        asset_id: &str,
        annual_rate_percent_micros: i64,
        end_date: Option<String>,
        active: bool,
        today: chrono::NaiveDate,
    ) -> Result<FeeSchedule, AccountError> {
        info!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, "update_fee_schedule");
        let repo = self.fee_schedule_repo()?;
        let existing = repo
            .get_by_account_asset(account_id, asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "update_fee_schedule: lookup failed");
                AccountError::DatabaseError
            })?
            .ok_or(AccountError::ScheduleNotFound)?;
        let reactivated = !existing.active && active;
        let mut schedule = existing.update_from(annual_rate_percent_micros, end_date, active)?;
        if reactivated {
            // FEE-061 — the pause is not charged: generation resumes from the current
            // period, so the cursor passes every period completed while paused. It moves
            // before the schedule is saved as active: a failure between the two writes
            // leaves a paused schedule, never an active one that would charge the pause.
            let resumed = schedule
                .frequency
                .last_completed_period_end(today)
                .map(|boundary| boundary.format("%Y-%m-%d").to_string())
                .filter(|boundary| {
                    schedule
                        .last_applied_period
                        .as_deref()
                        .is_none_or(|cursor| cursor < boundary.as_str())
                });
            if let Some(boundary) = resumed {
                self.advance_fee_schedule_cursor(account_id, asset_id, boundary.clone())
                    .await?;
                schedule.last_applied_period = Some(boundary);
            }
        }
        repo.update(&schedule).await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "update_fee_schedule: persist failed");
            AccountError::DatabaseError
        })?;
        self.emit_fee_schedule_updated();
        Ok(schedule)
    }

    /// Deletes the fee schedule for the (account, asset) pair (FEE-062).
    ///
    /// Silent no-op if no schedule exists (mirrors `delete_account` precedent).
    pub async fn delete_fee_schedule(
        &self,
        account_id: &str,
        asset_id: &str,
    ) -> Result<(), AccountError> {
        info!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, "delete_fee_schedule");
        let repo = self.fee_schedule_repo()?;
        repo.delete_by_account_asset(account_id, asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "delete_fee_schedule: delete failed");
                AccountError::DatabaseError
            })?;
        self.emit_fee_schedule_updated();
        Ok(())
    }

    /// Returns the fee schedule for the (account, asset) pair, or None (FEE-030).
    pub async fn get_fee_schedule(
        &self,
        account_id: &str,
        asset_id: &str,
    ) -> Result<Option<FeeSchedule>, AccountError> {
        let repo = self.fee_schedule_repo()?;
        repo.get_by_account_asset(account_id, asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "get_fee_schedule: lookup failed");
                AccountError::DatabaseError
            })
    }

    /// AGT-066 — whether the holding's fee schedule has generated the deduction of the
    /// period `date` falls in (FEE-042, FEE-048). False without a schedule, and for a date
    /// that does not parse: the recording refuses that one itself.
    pub async fn has_generated_fee_for_period(
        &self,
        account_id: &str,
        asset_id: &str,
        date: &str,
    ) -> Result<bool, AccountError> {
        let Some(schedule) = self.get_fee_schedule(account_id, asset_id).await? else {
            return Ok(false);
        };
        let Some(boundary) = NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .ok()
            .and_then(|day| schedule.frequency.period_end_containing(day))
        else {
            return Ok(false);
        };
        let id = generated_deduction_id(
            account_id,
            asset_id,
            &boundary.format("%Y-%m-%d").to_string(),
        );
        let generated = self.transaction_repo.get_by_id(&id).await.map_err(|e| {
            tracing::error!(target: BACKEND, account_id = %account_id, err = ?e, "has_generated_fee_for_period: lookup failed");
            AccountError::DatabaseError
        })?;
        Ok(generated.is_some_and(|tx| tx.account_id == account_id))
    }

    /// Returns every active fee schedule across all accounts (FEE-040 catch-up).
    pub async fn list_active_fee_schedules(&self) -> Result<Vec<FeeSchedule>, AccountError> {
        let repo = self.fee_schedule_repo()?;
        repo.get_all_active().await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "list_active_fee_schedules: query failed");
            AccountError::DatabaseError
        })
    }

    /// Returns the active fee schedules of one account (FEE-074).
    pub async fn list_active_fee_schedules_for_account(
        &self,
        account_id: &str,
    ) -> Result<Vec<FeeSchedule>, AccountError> {
        let repo = self.fee_schedule_repo()?;
        repo.get_active_by_account(account_id).await.map_err(|e| {
            tracing::error!(target: BACKEND, account_id = %account_id, err = ?e, "list_active_fee_schedules_for_account: query failed");
            AccountError::DatabaseError
        })
    }

    /// Returns every fee schedule of one account, active or not (SYN-013/021).
    pub async fn list_fee_schedules_for_account(
        &self,
        account_id: &str,
    ) -> Result<Vec<FeeSchedule>, AccountError> {
        let repo = self.fee_schedule_repo()?;
        repo.get_by_account(account_id).await.map_err(|e| {
            tracing::error!(target: BACKEND, account_id = %account_id, err = ?e, "list_fee_schedules_for_account: query failed");
            AccountError::DatabaseError
        })
    }

    /// Returns every fee catch-up position of one account (SYN-013/021, CFR-044).
    pub async fn list_fee_catch_up_positions_for_account(
        &self,
        account_id: &str,
    ) -> Result<Vec<FeeCatchUpPosition>, AccountError> {
        let repo = self.fee_catch_up_repo()?;
        repo.get_by_account(account_id).await.map_err(|e| {
            tracing::error!(target: BACKEND, account_id = %account_id, err = ?e, "list_fee_catch_up_positions_for_account: query failed");
            AccountError::DatabaseError
        })
    }

    /// Stamps `rank` on every account-owned synced row that has never been ranked (CFR-014,
    /// D6), on the first publish's enrolment transaction (SYN-013). Returns how many rows
    /// were stamped.
    pub async fn stamp_sync_rank(
        &self,
        conn: &mut SqliteConnection,
        rank: &Rank,
    ) -> StdResult<u64, AccountError> {
        self.account_repo
            .stamp_sync_rank(conn, rank)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "stamp_sync_rank: repository failure");
                AccountError::DatabaseError
            })
    }

    /// Advances a schedule's catch-up cursor to `last_applied_period` (FEE-043) by
    /// merging the schedule's `FeeCatchUpPosition` by maximum (CFR-044).
    pub async fn advance_fee_schedule_cursor(
        &self,
        account_id: &str,
        asset_id: &str,
        last_applied_period: String,
    ) -> Result<(), AccountError> {
        let repo = self.fee_catch_up_repo()?;
        repo.upsert(FeeCatchUpPosition {
            account_id: account_id.to_string(),
            asset_id: asset_id.to_string(),
            last_applied_period,
        })
        .await
        .map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "advance_fee_schedule_cursor: persist failed");
            AccountError::DatabaseError
        })?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // HNO-010/011/020/021/040 — holding note CRUD
    // -------------------------------------------------------------------------

    /// Creates or fully replaces the note for the (account, asset) pair (HNO-020).
    ///
    /// HNO-011 — the account must exist (`AccountNotFound`), the asset must not
    /// be the cash line (`NoteOnCashAsset`), and the pair must have at least one
    /// transaction entry (`NoteOnUnheldAsset`); `HoldingNote::new` validates the
    /// text and alarm fields.
    pub async fn upsert_holding_note(
        &self,
        account_id: &str,
        asset_id: String,
        text: String,
        threshold_price: Option<i64>,
        threshold_direction: Option<ThresholdDirection>,
    ) -> Result<HoldingNote, AccountError> {
        info!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, "upsert_holding_note");
        self.get_by_id(account_id)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound {
                account_id: account_id.to_string(),
            })?;
        if crate::core::cash::is_cash_asset(&asset_id) {
            return Err(AccountError::NoteOnCashAsset);
        }
        if self
            .get_transactions(account_id, &asset_id)
            .await?
            .is_empty()
        {
            return Err(AccountError::NoteOnUnheldAsset);
        }
        let note = HoldingNote::new(
            account_id.to_string(),
            asset_id,
            text,
            threshold_price,
            threshold_direction,
        )?;
        self.holding_note_repo()?.upsert(&note).await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "upsert_holding_note: persist failed");
            AccountError::DatabaseError
        })?;
        self.emit_holding_note_updated();
        Ok(note)
    }

    /// Deletes the note for the (account, asset) pair (HNO-021).
    ///
    /// Silent no-op if no note exists (mirrors `delete_fee_schedule`).
    pub async fn delete_holding_note(
        &self,
        account_id: &str,
        asset_id: &str,
    ) -> Result<(), AccountError> {
        info!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, "delete_holding_note");
        self.holding_note_repo()?
            .delete(account_id, asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "delete_holding_note: delete failed");
                AccountError::DatabaseError
            })?;
        self.emit_holding_note_updated();
        Ok(())
    }

    /// Returns all holding notes of one account (HNO-040).
    pub async fn get_holding_notes(
        &self,
        account_id: &str,
    ) -> Result<Vec<HoldingNote>, AccountError> {
        self.holding_note_repo()?
            .get_for_account(account_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, err = ?e, "get_holding_notes: query failed");
                AccountError::DatabaseError
            })
    }

    // -------------------------------------------------------------------------
    // Apply entry points (CFR-017) — merge executor writes; no entry guards run
    // -------------------------------------------------------------------------

    /// The synced record of `kind` this device holds for `identity` (CFR-014), on the apply
    /// transaction's connection; `None` when it holds none.
    pub async fn synced_record(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> StdResult<Option<SyncedRecord>, AccountError> {
        self.account_repo
            .synced_record(conn, kind, identity)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, identity = %identity, err = ?e, "synced_record: repository failure");
                AccountError::DatabaseError
            })
    }

    /// Every synced child of `account_id` this device holds (CFR-030), on the apply
    /// transaction's connection.
    pub async fn synced_children(
        &self,
        conn: &mut SqliteConnection,
        account_id: &str,
    ) -> StdResult<Vec<SyncedChild>, AccountError> {
        self.account_repo
            .synced_children(conn, account_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, err = ?e, "synced_children: repository failure");
                AccountError::DatabaseError
            })
    }

    /// The rank of another account named `name` (CFR-035), on the apply transaction's
    /// connection; `None` when none clashes.
    pub async fn clashing_account_name(
        &self,
        conn: &mut SqliteConnection,
        account_id: &str,
        name: &str,
    ) -> StdResult<Option<Rank>, AccountError> {
        self.account_repo
            .clashing_name_rank(conn, account_id, name)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, name = %name, err = ?e, "clashing_account_name: repository failure");
                AccountError::DatabaseError
            })
    }

    /// Applies an incoming account change verbatim (CFR-017): no entry guards run — an
    /// account whose incoming state would fail today's validation is still written, since
    /// it was valid where it was recorded. Stamps `rank` on the row (CFR-014) and seeds the
    /// account's 0-balance cash holding (CSH-012); the cash asset must already exist
    /// (SYN-027 — the caller ensures it).
    pub async fn apply_account(
        &self,
        conn: &mut SqliteConnection,
        content: &str,
        rank: Rank,
    ) -> StdResult<(), AccountError> {
        let account: Account = synced_content(content)?;
        self.account_repo
            .apply_account(conn, &account, &rank)
            .await
            .map_err(|e| applied_write_error("apply_account", e))?;
        let mut aggregate = self.load_applied(conn, &account.id).await?;
        aggregate.seed_cash_holding();
        self.account_repo
            .save_applied(conn, &mut aggregate, None)
            .await
            .map_err(|e| applied_write_error("apply_account", e))?;
        Ok(())
    }

    /// Applies an incoming transaction verbatim (CFR-017): an oversell or an
    /// insufficient-cash combination is not rejected — CFR-042 surfaces it as a derived
    /// inconsistency instead. The transaction's asset must already exist (SYN-027 — the
    /// caller seeds the cash asset). Stamps `rank` on the row (CFR-014).
    pub async fn apply_transaction(
        &self,
        conn: &mut SqliteConnection,
        content: &str,
        rank: Rank,
    ) -> StdResult<(), AccountError> {
        let transaction: Transaction = synced_content(content)?;
        let mut account = self.load_applied(conn, &transaction.account_id).await?;
        let transaction_id = transaction.id.clone();
        account.apply_synced_transaction(transaction);
        self.account_repo
            .save_applied(conn, &mut account, Some((transaction_id, rank)))
            .await
            .map_err(|e| applied_write_error("apply_transaction", e))?;
        Ok(())
    }

    /// Applies an incoming holding note verbatim (CFR-017).
    pub async fn apply_holding_note(
        &self,
        conn: &mut SqliteConnection,
        content: &str,
        rank: Rank,
    ) -> StdResult<(), AccountError> {
        let note: HoldingNote = synced_content(content)?;
        self.account_repo
            .apply_holding_note(conn, &note, &rank)
            .await
            .map_err(|e| applied_write_error("apply_holding_note", e))?;
        Ok(())
    }

    /// Applies an incoming fee schedule verbatim (CFR-017) — a user record (CFR-016), so an
    /// incoming application-origin write can never outrank it once a user has touched it.
    pub async fn apply_fee_schedule(
        &self,
        conn: &mut SqliteConnection,
        content: &str,
        rank: Rank,
    ) -> StdResult<(), AccountError> {
        let schedule: FeeSchedule = synced_content(content)?;
        self.account_repo
            .apply_fee_schedule(conn, &schedule, &rank)
            .await
            .map_err(|e| applied_write_error("apply_fee_schedule", e))?;
        Ok(())
    }

    /// Applies an incoming fee catch-up position by maximum, never by rank (CFR-044).
    pub async fn apply_catch_up_position(
        &self,
        conn: &mut SqliteConnection,
        content: &str,
        rank: Rank,
    ) -> StdResult<(), AccountError> {
        let position: FeeCatchUpPosition = synced_content(content)?;
        self.account_repo
            .apply_catch_up_position(conn, &position, &rank)
            .await
            .map_err(|e| applied_write_error("apply_catch_up_position", e))
    }

    /// Applies an incoming removal of `kind`/`identity` (CFR-017/CFR-030): removing an
    /// account removes every child of it this device holds (its transactions, holding
    /// notes, fee schedules, and their catch-up positions); removing a transaction replays
    /// the holdings it leaves behind. A no-op for a record this device does not hold.
    pub async fn apply_removal(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> StdResult<(), AccountError> {
        match kind {
            RecordKind::Account => {
                self.account_repo
                    .remove_account(conn, identity)
                    .await
                    .map_err(|e| applied_write_error("apply_removal", e))?;
            }
            RecordKind::Transaction => {
                let account_id = self
                    .account_repo
                    .account_id_of_transaction(conn, identity)
                    .await
                    .map_err(|e| applied_write_error("apply_removal", e))?;
                let Some(account_id) = account_id else {
                    return Ok(());
                };
                let mut account = self.load_applied(conn, &account_id).await?;
                account.remove_synced_transaction(identity);
                self.account_repo
                    .save_applied(conn, &mut account, None)
                    .await
                    .map_err(|e| applied_write_error("apply_removal", e))?;
            }
            RecordKind::HoldingNote => {
                let (account_id, asset_id) = holding_keys(identity)?;
                self.account_repo
                    .remove_holding_note(conn, account_id, asset_id)
                    .await
                    .map_err(|e| applied_write_error("apply_removal", e))?;
            }
            RecordKind::FeeSchedule => {
                let (account_id, asset_id) = holding_keys(identity)?;
                self.account_repo
                    .remove_fee_schedule(conn, account_id, asset_id)
                    .await
                    .map_err(|e| applied_write_error("apply_removal", e))?;
            }
            RecordKind::FeeCatchUpPosition => {
                let (account_id, asset_id) = holding_keys(identity)?;
                self.account_repo
                    .remove_catch_up_position(conn, account_id, asset_id)
                    .await
                    .map_err(|e| applied_write_error("apply_removal", e))?;
            }
            RecordKind::Category
            | RecordKind::Asset
            | RecordKind::AssetPrice
            | RecordKind::ReportedSplit
            | RecordKind::CurrencyPair
            | RecordKind::CurrencyRate
            | RecordKind::PortfolioSetting
            | RecordKind::ColumnMapping => {}
        }
        Ok(())
    }

    /// Loads the aggregate an applied change targets, on the apply transaction's connection.
    async fn load_applied(
        &self,
        conn: &mut SqliteConnection,
        account_id: &str,
    ) -> StdResult<Account, AccountError> {
        match self.account_repo.load_aggregate(conn, account_id).await {
            Ok(Some(account)) => Ok(account),
            Ok(None) => Err(AccountError::AccountNotFound {
                account_id: account_id.to_string(),
            }),
            Err(e) => {
                tracing::error!(target: BACKEND, account_id = %account_id, err = ?e, "load_applied: repository failure");
                Err(AccountError::DatabaseError)
            }
        }
    }

    // -------------------------------------------------------------------------
    // Private helpers
    // -------------------------------------------------------------------------

    fn emit_account_updated(&self) {
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::AccountUpdated);
        }
    }

    fn emit_transaction_updated(&self) {
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::TransactionUpdated);
        }
    }

    fn emit_fee_schedule_updated(&self) {
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::FeeScheduleUpdated);
        }
    }

    fn emit_holding_note_updated(&self) {
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::HoldingNoteUpdated);
        }
    }
}

/// Account application surface consumed by cross-BC use-case orchestrators.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait AccountServiceContract: Send + Sync {
    /// Retrieves all non-deleted accounts.
    async fn get_all(&self) -> StdResult<Vec<Account>, AccountError>;
    /// Every account ordered by name, case ignored (CLI-018).
    async fn get_all_by_name(&self) -> StdResult<Vec<Account>, AccountError>;
    /// Marks a transaction as recorded by an agent session (AGT-045).
    async fn mark_agent_recording(
        &self,
        transaction_id: &str,
        recording: &AgentRecording,
    ) -> StdResult<(), AccountError>;
    /// The mark of a transaction, when an agent recorded it (AGT-050).
    async fn agent_recording_of(
        &self,
        transaction_id: &str,
    ) -> StdResult<Option<AgentRecording>, AccountError>;
    /// The transactions a session recorded and that still exist, the last recorded first (AGT-053).
    async fn recorded_in_session(
        &self,
        session: &str,
    ) -> StdResult<Vec<SessionRecording>, AccountError>;
    /// The account journal of an account, the rows the filter keeps (TXL-060).
    async fn get_account_journal(
        &self,
        account_id: &str,
        filter: &JournalFilter,
    ) -> StdResult<AccountJournal, AccountError>;
    /// Retrieves a transaction by ID.
    async fn get_transaction_by_id(&self, id: &str)
        -> StdResult<Option<Transaction>, AccountError>;
    /// Retrieves an account by ID.
    async fn get_by_id(&self, id: &str) -> StdResult<Option<Account>, AccountError>;
    /// Creates a new account.
    async fn create(
        &self,
        name: String,
        bank_name: String,
        currency: String,
        management_fees_enabled: bool,
    ) -> StdResult<Account, AccountError>;
    /// ACC-041 — creates a new account with its country and wrapper.
    // The account's own fields, one argument each as `create`/`update` take them, plus
    // its country and wrapper (ACC-041).
    #[allow(clippy::too_many_arguments)]
    async fn create_classified(
        &self,
        name: String,
        bank_name: String,
        currency: String,
        management_fees_enabled: bool,
        country: Option<String>,
        wrapper: Option<String>,
    ) -> StdResult<Account, AccountError>;
    /// AGT-069 — the mark of every account an agent created on this computer, by account id.
    async fn agent_creations(
        &self,
    ) -> StdResult<std::collections::HashMap<String, AgentRecording>, AccountError>;
    /// Seeds the account's 0-balance Cash Holding (CSH-012).
    async fn seed_cash_holding(&self, account_id: &str) -> StdResult<(), AccountError>;
    /// ACC-025 — stores a new account with its cash line inside `unit` (ADR-006).
    async fn create_in(
        &self,
        unit: &mut UnitOfWork,
        account: Account,
        mark: Option<AgentRecording>,
    ) -> StdResult<Account, AccountError>;
    /// Publishes `AccountUpdated` for writes a unit of work committed (ADR-006).
    fn notify_account_updated(&self);
    /// Retrieves all holdings for a given account (ACD-022, B24).
    async fn get_holdings_for_account(
        &self,
        account_id: &str,
    ) -> StdResult<Vec<Holding>, AccountError>;
    /// Retrieves a single holding by account/asset pair, or None (B19).
    async fn get_holding_by_account_asset(
        &self,
        account_id: &str,
        asset_id: &str,
    ) -> StdResult<Option<Holding>, AccountError>;
    /// What a split would make of a position, or its refusal (SPL-020/021).
    async fn preview_split(
        &self,
        account_id: &str,
        asset_id: &str,
        date: &str,
        factor: i64,
    ) -> StdResult<StockSplitPosition, AccountError>;
    /// The holding's quantity and average cost as of a date (TDI-010).
    async fn holding_snapshot_as_of(
        &self,
        account_id: &str,
        asset_id: &str,
        date: &str,
    ) -> StdResult<HoldingSnapshot, AccountError>;
    /// Retrieves every transaction for an account across all assets (PRF-021).
    async fn get_all_transactions_for_account(
        &self,
        account_id: &str,
    ) -> StdResult<Vec<Transaction>, AccountError>;
    /// Records a purchase of an asset into the account (TRX-020, TRX-026, TRX-060).
    #[allow(clippy::too_many_arguments)]
    async fn buy_holding(
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
    ) -> StdResult<Transaction, AccountError>;
    /// Records a sale of an asset from the account (SEL-012, SEL-021, SEL-023, SEL-024, SEL-050).
    #[allow(clippy::too_many_arguments)]
    async fn sell_holding(
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
    ) -> StdResult<Transaction, AccountError>;
    /// Corrects an existing transaction and recalculates the affected holding
    /// (TRX-031, SEL-031, TRX-061, SEL-051).
    #[allow(clippy::too_many_arguments)]
    async fn correct_transaction(
        &self,
        account_id: &str,
        tx_id: &str,
        date: String,
        quantity: i64,
        unit_price: i64,
        exchange_rate: i64,
        fees: i64,
        total_amount: Option<i64>,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError>;
    /// The transaction as `correct_transaction` would make it, nothing written (AGT-087).
    #[allow(clippy::too_many_arguments)]
    async fn preview_correction(
        &self,
        account_id: &str,
        tx_id: &str,
        date: String,
        quantity: i64,
        unit_price: i64,
        exchange_rate: i64,
        fees: i64,
        total_amount: Option<i64>,
        note: Option<String>,
    ) -> StdResult<RecordingPreview, AccountError>;
    /// AGT-072 — what recording `op` would make of the account, nothing written.
    async fn preview_recording(
        &self,
        account_id: &str,
        op: RecordingOp,
    ) -> StdResult<RecordingPreview, AccountError>;
    /// IMP-035 — recording steps judged in order, nothing written.
    async fn judge_recordings(
        &self,
        account_id: &str,
        steps: Vec<RecordingStep>,
    ) -> StdResult<Vec<RecordingJudgment>, AccountError>;
    /// IMP-050 — recording steps judged in order, the ones taken recorded in one write.
    async fn record_recordings(
        &self,
        account_id: &str,
        steps: Vec<RecordingStep>,
        add_deposits: bool,
    ) -> StdResult<Vec<RecordingJudgment>, AccountError>;
    /// AGT-072 — what recording an opening balance would make of the account, nothing written.
    async fn preview_opening(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        total_cost: i64,
    ) -> StdResult<RecordingPreview, OpenHoldingError>;
    /// Deletes a transaction and recalculates (or removes) the associated holding (TRX-034).
    async fn cancel_transaction(
        &self,
        account_id: &str,
        tx_id: &str,
    ) -> StdResult<(), AccountError>;
    /// Seeds a holding directly from a quantity and total cost (TRX-042, TRX-047).
    async fn open_holding(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        total_cost: i64,
    ) -> StdResult<Transaction, OpenHoldingError>;
    /// Records a Deposit (CSH-022) — cash inflow into the account.
    async fn record_deposit(
        &self,
        account_id: &str,
        date: String,
        amount: i64,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError>;
    /// Records a Withdrawal (CSH-032) — cash outflow from the account.
    async fn record_withdrawal(
        &self,
        account_id: &str,
        date: String,
        amount: i64,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError>;
    /// Records a cash Dividend attributed to a held paying asset (DIV-023).
    async fn record_dividend(
        &self,
        account_id: &str,
        paying_asset_id: String,
        date: String,
        amount_micros: i64,
        exchange_rate: i64,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError>;
    /// Records a FreeShares distribution attributed to a held distributing asset (FSD-022).
    async fn record_free_shares(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError>;
    /// Records a write-off of a held asset, at its cost and without cash (WOF-020).
    async fn record_write_off(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        reason: WriteOffReason,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError>;
    /// Corrects a write-off's date, quantity, reason and note, the holding replayed (WOF-030).
    async fn correct_write_off(
        &self,
        account_id: &str,
        tx_id: &str,
        date: String,
        quantity: i64,
        reason: WriteOffReason,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError>;
    /// Records a transfer between two accounts, at its cost and without cash (TRF-020).
    #[allow(clippy::too_many_arguments)]
    async fn record_transfer(
        &self,
        from_account_id: &str,
        to_account_id: &str,
        asset_id: String,
        date: String,
        quantity_sent: i64,
        quantity_received: i64,
        exchange_rate: i64,
        note: Option<String>,
    ) -> StdResult<Vec<Transaction>, AccountError>;
    /// AGT-045 / AGT-049 — records a transfer with every part marked, in one write.
    #[allow(clippy::too_many_arguments)]
    async fn record_transfer_by_agent(
        &self,
        from_account_id: &str,
        to_account_id: &str,
        asset_id: String,
        date: String,
        quantity_sent: i64,
        quantity_received: i64,
        exchange_rate: i64,
        note: Option<String>,
        mark: &AgentRecording,
    ) -> StdResult<Vec<Transaction>, AccountError>;
    /// Corrects a transfer's date, quantities, rate and note as one operation (TRF-030).
    async fn correct_transfer(
        &self,
        transfer_id: &str,
        date: String,
        quantity_sent: i64,
        quantity_received: i64,
        exchange_rate: i64,
        note: Option<String>,
    ) -> StdResult<Vec<Transaction>, AccountError>;
    /// Removes every part of a transfer together (TRF-040).
    async fn cancel_transfer(&self, transfer_id: &str) -> StdResult<(), AccountError>;
    /// What recording a transfer would make of its two accounts, nothing written (AGT-065A).
    #[allow(clippy::too_many_arguments)]
    async fn preview_transfer(
        &self,
        from_account_id: &str,
        to_account_id: &str,
        asset_id: String,
        date: String,
        quantity_sent: i64,
        quantity_received: i64,
        exchange_rate: i64,
        note: Option<String>,
    ) -> StdResult<TransferPreview, AccountError>;
    /// What replacing transactions with a transfer would make of its two accounts, nothing
    /// written (AGT-090).
    #[allow(clippy::too_many_arguments)]
    async fn preview_replacement(
        &self,
        replaced: &[String],
        from_account_id: &str,
        to_account_id: &str,
        asset_id: String,
        date: String,
        quantity_sent: i64,
        quantity_received: i64,
        exchange_rate: i64,
    ) -> StdResult<TransferPreview, AccountError>;
    /// Replaces transactions with a transfer in one write while each stands as shown and
    /// the transfer moves the cost shown; `None` when not (AGT-091).
    #[allow(clippy::too_many_arguments)]
    async fn replace_with_transfer(
        &self,
        shown: &[Transaction],
        from_account_id: &str,
        to_account_id: &str,
        asset_id: String,
        date: String,
        quantity_sent: i64,
        quantity_received: i64,
        exchange_rate: i64,
        cost_shown: i64,
    ) -> StdResult<Option<Vec<Transaction>>, AccountError>;
    /// A transfer read as one from its parts (TRF-060).
    async fn transfer_view(&self, transfer_id: &str) -> StdResult<TransferView, AccountError>;
    /// The sending account, the receiving account and the asset of a transfer (TRF-030).
    async fn transfer_sides(
        &self,
        transfer_id: &str,
    ) -> StdResult<(String, String, String), AccountError>;
    /// Records a stock split rescaling a held position (SPL-010/020).
    async fn record_split(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        factor: i64,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError>;
    /// Records a one-off management fee deduction on a held asset (FEE-012).
    async fn record_management_fee(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        percent_micros: i64,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError>;
    /// Records a one-off management fee entered as a percentage or as a resulting
    /// quantity — exactly one (FEE-021/028).
    async fn record_management_fee_entry(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        percent_micros: Option<i64>,
        resulting_quantity: Option<i64>,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError>;
    /// What a one-off management fee entered by its resulting quantity would remove
    /// (FEE-029).
    async fn preview_management_fee(
        &self,
        account_id: &str,
        asset_id: &str,
        date: &str,
        resulting_quantity: i64,
    ) -> StdResult<ManagementFeeRemoval, AccountError>;
    /// Records a generated management fee deduction under its FEE-048 identity (FEE-040).
    async fn record_generated_management_fee(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        annual_rate_percent_micros: i64,
        periods_per_year: i64,
        deduction_id: String,
    ) -> StdResult<Transaction, AccountError>;
    /// Records an Interest credit on a held asset or the account's cash line
    /// (INT-021/022/023/024).
    async fn record_interest(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        percent_micros: Option<i64>,
        quantity_micros: Option<i64>,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError>;
    /// Returns true if any account holds a non-zero quantity of the given asset (OQ-6).
    async fn has_active_holdings_for_asset(&self, asset_id: &str) -> StdResult<bool, AccountError>;
    /// Returns true if any transaction references the given asset.
    async fn has_holding_entries_for_asset(&self, asset_id: &str) -> StdResult<bool, AccountError>;
    /// Returns the count of active holdings and total transactions for an account (ACC-020).
    async fn get_deletion_summary(&self, account_id: &str) -> StdResult<(u32, u32), AccountError>;
    /// AGT-066 — whether the holding's schedule generated the fee of `date`'s period.
    async fn has_generated_fee_for_period(
        &self,
        account_id: &str,
        asset_id: &str,
        date: &str,
    ) -> StdResult<bool, AccountError>;
    /// Returns every active fee schedule across all accounts (FEE-040 catch-up).
    async fn list_active_fee_schedules(&self) -> StdResult<Vec<FeeSchedule>, AccountError>;
    /// Returns the active fee schedules of one account (FEE-074).
    async fn list_active_fee_schedules_for_account(
        &self,
        account_id: &str,
    ) -> StdResult<Vec<FeeSchedule>, AccountError>;
    /// Advances a schedule's catch-up cursor to `last_applied_period` (FEE-043).
    async fn advance_fee_schedule_cursor(
        &self,
        account_id: &str,
        asset_id: &str,
        last_applied_period: String,
    ) -> StdResult<(), AccountError>;
    /// Creates or fully replaces the note for the (account, asset) pair (HNO-020).
    async fn upsert_holding_note(
        &self,
        account_id: &str,
        asset_id: String,
        text: String,
        threshold_price: Option<i64>,
        threshold_direction: Option<ThresholdDirection>,
    ) -> StdResult<HoldingNote, AccountError>;
    /// Deletes the note for the (account, asset) pair; no-op if absent (HNO-021).
    async fn delete_holding_note(
        &self,
        account_id: &str,
        asset_id: &str,
    ) -> StdResult<(), AccountError>;
    /// Returns all holding notes of one account (HNO-040).
    async fn get_holding_notes(
        &self,
        account_id: &str,
    ) -> StdResult<Vec<HoldingNote>, AccountError>;
}

#[async_trait]
impl AccountServiceContract for AccountService {
    async fn get_all(&self) -> StdResult<Vec<Account>, AccountError> {
        AccountService::get_all(self).await
    }

    async fn get_all_by_name(&self) -> StdResult<Vec<Account>, AccountError> {
        AccountService::get_all_by_name(self).await
    }

    async fn mark_agent_recording(
        &self,
        transaction_id: &str,
        recording: &AgentRecording,
    ) -> StdResult<(), AccountError> {
        AccountService::mark_agent_recording(self, transaction_id, recording).await
    }

    async fn agent_recording_of(
        &self,
        transaction_id: &str,
    ) -> StdResult<Option<AgentRecording>, AccountError> {
        AccountService::agent_recording_of(self, transaction_id).await
    }

    async fn recorded_in_session(
        &self,
        session: &str,
    ) -> StdResult<Vec<SessionRecording>, AccountError> {
        AccountService::recorded_in_session(self, session).await
    }

    async fn get_account_journal(
        &self,
        account_id: &str,
        filter: &JournalFilter,
    ) -> StdResult<AccountJournal, AccountError> {
        AccountService::get_account_journal(self, account_id, filter).await
    }

    async fn get_transaction_by_id(
        &self,
        id: &str,
    ) -> StdResult<Option<Transaction>, AccountError> {
        AccountService::get_transaction_by_id(self, id).await
    }

    async fn get_by_id(&self, id: &str) -> StdResult<Option<Account>, AccountError> {
        AccountService::get_by_id(self, id).await
    }

    async fn create(
        &self,
        name: String,
        bank_name: String,
        currency: String,
        management_fees_enabled: bool,
    ) -> StdResult<Account, AccountError> {
        AccountService::create(self, name, bank_name, currency, management_fees_enabled).await
    }

    async fn agent_creations(
        &self,
    ) -> StdResult<std::collections::HashMap<String, AgentRecording>, AccountError> {
        AccountService::agent_creations(self).await
    }

    async fn create_classified(
        &self,
        name: String,
        bank_name: String,
        currency: String,
        management_fees_enabled: bool,
        country: Option<String>,
        wrapper: Option<String>,
    ) -> StdResult<Account, AccountError> {
        AccountService::create_classified(
            self,
            name,
            bank_name,
            currency,
            management_fees_enabled,
            country,
            wrapper,
        )
        .await
    }

    async fn seed_cash_holding(&self, account_id: &str) -> StdResult<(), AccountError> {
        AccountService::seed_cash_holding(self, account_id).await
    }

    async fn create_in(
        &self,
        unit: &mut UnitOfWork,
        account: Account,
        mark: Option<AgentRecording>,
    ) -> StdResult<Account, AccountError> {
        AccountService::create_in(self, unit, account, mark).await
    }

    fn notify_account_updated(&self) {
        AccountService::notify_account_updated(self)
    }

    async fn get_holdings_for_account(
        &self,
        account_id: &str,
    ) -> StdResult<Vec<Holding>, AccountError> {
        AccountService::get_holdings_for_account(self, account_id).await
    }

    async fn get_holding_by_account_asset(
        &self,
        account_id: &str,
        asset_id: &str,
    ) -> StdResult<Option<Holding>, AccountError> {
        AccountService::get_holding_by_account_asset(self, account_id, asset_id).await
    }

    async fn holding_snapshot_as_of(
        &self,
        account_id: &str,
        asset_id: &str,
        date: &str,
    ) -> StdResult<HoldingSnapshot, AccountError> {
        AccountService::holding_snapshot_as_of(self, account_id, asset_id, date).await
    }

    async fn preview_split(
        &self,
        account_id: &str,
        asset_id: &str,
        date: &str,
        factor: i64,
    ) -> StdResult<StockSplitPosition, AccountError> {
        AccountService::preview_split(self, account_id, asset_id, date, factor).await
    }

    async fn get_all_transactions_for_account(
        &self,
        account_id: &str,
    ) -> StdResult<Vec<Transaction>, AccountError> {
        AccountService::get_all_transactions_for_account(self, account_id).await
    }

    async fn buy_holding(
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
    ) -> StdResult<Transaction, AccountError> {
        AccountService::buy_holding(
            self,
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

    async fn sell_holding(
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
    ) -> StdResult<Transaction, AccountError> {
        AccountService::sell_holding(
            self,
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

    async fn correct_transaction(
        &self,
        account_id: &str,
        tx_id: &str,
        date: String,
        quantity: i64,
        unit_price: i64,
        exchange_rate: i64,
        fees: i64,
        total_amount: Option<i64>,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError> {
        AccountService::correct_transaction(
            self,
            account_id,
            tx_id,
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

    async fn preview_correction(
        &self,
        account_id: &str,
        tx_id: &str,
        date: String,
        quantity: i64,
        unit_price: i64,
        exchange_rate: i64,
        fees: i64,
        total_amount: Option<i64>,
        note: Option<String>,
    ) -> StdResult<RecordingPreview, AccountError> {
        AccountService::preview_correction(
            self,
            account_id,
            tx_id,
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

    async fn preview_recording(
        &self,
        account_id: &str,
        op: RecordingOp,
    ) -> StdResult<RecordingPreview, AccountError> {
        AccountService::preview_recording(self, account_id, op).await
    }

    async fn judge_recordings(
        &self,
        account_id: &str,
        steps: Vec<RecordingStep>,
    ) -> StdResult<Vec<RecordingJudgment>, AccountError> {
        AccountService::judge_recordings(self, account_id, steps).await
    }

    async fn record_recordings(
        &self,
        account_id: &str,
        steps: Vec<RecordingStep>,
        add_deposits: bool,
    ) -> StdResult<Vec<RecordingJudgment>, AccountError> {
        AccountService::record_recordings(self, account_id, steps, add_deposits).await
    }

    async fn preview_opening(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        total_cost: i64,
    ) -> StdResult<RecordingPreview, OpenHoldingError> {
        AccountService::preview_opening(self, account_id, asset_id, date, quantity, total_cost)
            .await
    }

    async fn cancel_transaction(
        &self,
        account_id: &str,
        tx_id: &str,
    ) -> StdResult<(), AccountError> {
        AccountService::cancel_transaction(self, account_id, tx_id).await
    }

    async fn open_holding(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        total_cost: i64,
    ) -> StdResult<Transaction, OpenHoldingError> {
        AccountService::open_holding(self, account_id, asset_id, date, quantity, total_cost).await
    }

    async fn record_deposit(
        &self,
        account_id: &str,
        date: String,
        amount: i64,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError> {
        AccountService::record_deposit(self, account_id, date, amount, note).await
    }

    async fn record_withdrawal(
        &self,
        account_id: &str,
        date: String,
        amount: i64,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError> {
        AccountService::record_withdrawal(self, account_id, date, amount, note).await
    }

    async fn record_dividend(
        &self,
        account_id: &str,
        paying_asset_id: String,
        date: String,
        amount_micros: i64,
        exchange_rate: i64,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError> {
        AccountService::record_dividend(
            self,
            account_id,
            paying_asset_id,
            date,
            amount_micros,
            exchange_rate,
            note,
        )
        .await
    }

    async fn record_free_shares(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError> {
        AccountService::record_free_shares(self, account_id, asset_id, date, quantity, note).await
    }

    async fn record_write_off(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        reason: WriteOffReason,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError> {
        AccountService::record_write_off(self, account_id, asset_id, date, quantity, reason, note)
            .await
    }

    async fn correct_write_off(
        &self,
        account_id: &str,
        tx_id: &str,
        date: String,
        quantity: i64,
        reason: WriteOffReason,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError> {
        AccountService::correct_write_off(self, account_id, tx_id, date, quantity, reason, note)
            .await
    }

    async fn record_transfer_by_agent(
        &self,
        from_account_id: &str,
        to_account_id: &str,
        asset_id: String,
        date: String,
        quantity_sent: i64,
        quantity_received: i64,
        exchange_rate: i64,
        note: Option<String>,
        mark: &AgentRecording,
    ) -> StdResult<Vec<Transaction>, AccountError> {
        AccountService::record_transfer_by_agent(
            self,
            from_account_id,
            to_account_id,
            asset_id,
            date,
            quantity_sent,
            quantity_received,
            exchange_rate,
            note,
            mark,
        )
        .await
    }

    async fn record_transfer(
        &self,
        from_account_id: &str,
        to_account_id: &str,
        asset_id: String,
        date: String,
        quantity_sent: i64,
        quantity_received: i64,
        exchange_rate: i64,
        note: Option<String>,
    ) -> StdResult<Vec<Transaction>, AccountError> {
        AccountService::record_transfer(
            self,
            from_account_id,
            to_account_id,
            asset_id,
            date,
            quantity_sent,
            quantity_received,
            exchange_rate,
            note,
        )
        .await
    }

    async fn correct_transfer(
        &self,
        transfer_id: &str,
        date: String,
        quantity_sent: i64,
        quantity_received: i64,
        exchange_rate: i64,
        note: Option<String>,
    ) -> StdResult<Vec<Transaction>, AccountError> {
        AccountService::correct_transfer(
            self,
            transfer_id,
            date,
            quantity_sent,
            quantity_received,
            exchange_rate,
            note,
        )
        .await
    }

    async fn cancel_transfer(&self, transfer_id: &str) -> StdResult<(), AccountError> {
        AccountService::cancel_transfer(self, transfer_id).await
    }

    async fn preview_transfer(
        &self,
        from_account_id: &str,
        to_account_id: &str,
        asset_id: String,
        date: String,
        quantity_sent: i64,
        quantity_received: i64,
        exchange_rate: i64,
        note: Option<String>,
    ) -> StdResult<TransferPreview, AccountError> {
        AccountService::preview_transfer(
            self,
            from_account_id,
            to_account_id,
            asset_id,
            date,
            quantity_sent,
            quantity_received,
            exchange_rate,
            note,
        )
        .await
    }

    async fn preview_replacement(
        &self,
        replaced: &[String],
        from_account_id: &str,
        to_account_id: &str,
        asset_id: String,
        date: String,
        quantity_sent: i64,
        quantity_received: i64,
        exchange_rate: i64,
    ) -> StdResult<TransferPreview, AccountError> {
        AccountService::preview_replacement(
            self,
            replaced,
            from_account_id,
            to_account_id,
            asset_id,
            date,
            quantity_sent,
            quantity_received,
            exchange_rate,
        )
        .await
    }

    async fn replace_with_transfer(
        &self,
        shown: &[Transaction],
        from_account_id: &str,
        to_account_id: &str,
        asset_id: String,
        date: String,
        quantity_sent: i64,
        quantity_received: i64,
        exchange_rate: i64,
        cost_shown: i64,
    ) -> StdResult<Option<Vec<Transaction>>, AccountError> {
        AccountService::replace_with_transfer(
            self,
            shown,
            from_account_id,
            to_account_id,
            asset_id,
            date,
            quantity_sent,
            quantity_received,
            exchange_rate,
            cost_shown,
        )
        .await
    }

    async fn transfer_view(&self, transfer_id: &str) -> StdResult<TransferView, AccountError> {
        AccountService::transfer_view(self, transfer_id).await
    }

    async fn transfer_sides(
        &self,
        transfer_id: &str,
    ) -> StdResult<(String, String, String), AccountError> {
        AccountService::transfer_sides(self, transfer_id).await
    }

    async fn record_split(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        factor: i64,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError> {
        AccountService::record_split(self, account_id, asset_id, date, factor, note).await
    }

    async fn record_management_fee_entry(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        percent_micros: Option<i64>,
        resulting_quantity: Option<i64>,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError> {
        AccountService::record_management_fee_entry(
            self,
            account_id,
            asset_id,
            date,
            percent_micros,
            resulting_quantity,
            note,
        )
        .await
    }

    async fn preview_management_fee(
        &self,
        account_id: &str,
        asset_id: &str,
        date: &str,
        resulting_quantity: i64,
    ) -> StdResult<ManagementFeeRemoval, AccountError> {
        AccountService::preview_management_fee(self, account_id, asset_id, date, resulting_quantity)
            .await
    }

    async fn record_management_fee(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        percent_micros: i64,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError> {
        AccountService::record_management_fee(
            self,
            account_id,
            asset_id,
            date,
            percent_micros,
            note,
        )
        .await
    }

    async fn record_generated_management_fee(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        annual_rate_percent_micros: i64,
        periods_per_year: i64,
        deduction_id: String,
    ) -> StdResult<Transaction, AccountError> {
        AccountService::record_generated_management_fee(
            self,
            account_id,
            asset_id,
            date,
            annual_rate_percent_micros,
            periods_per_year,
            deduction_id,
        )
        .await
    }

    async fn record_interest(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        percent_micros: Option<i64>,
        quantity_micros: Option<i64>,
        note: Option<String>,
    ) -> StdResult<Transaction, AccountError> {
        AccountService::record_interest(
            self,
            account_id,
            asset_id,
            date,
            percent_micros,
            quantity_micros,
            note,
        )
        .await
    }

    async fn has_active_holdings_for_asset(&self, asset_id: &str) -> StdResult<bool, AccountError> {
        AccountService::has_active_holdings_for_asset(self, asset_id).await
    }

    async fn has_holding_entries_for_asset(&self, asset_id: &str) -> StdResult<bool, AccountError> {
        AccountService::has_holding_entries_for_asset(self, asset_id).await
    }

    async fn get_deletion_summary(&self, account_id: &str) -> StdResult<(u32, u32), AccountError> {
        AccountService::get_deletion_summary(self, account_id).await
    }

    async fn has_generated_fee_for_period(
        &self,
        account_id: &str,
        asset_id: &str,
        date: &str,
    ) -> StdResult<bool, AccountError> {
        AccountService::has_generated_fee_for_period(self, account_id, asset_id, date).await
    }

    async fn list_active_fee_schedules(&self) -> StdResult<Vec<FeeSchedule>, AccountError> {
        AccountService::list_active_fee_schedules(self).await
    }

    async fn list_active_fee_schedules_for_account(
        &self,
        account_id: &str,
    ) -> StdResult<Vec<FeeSchedule>, AccountError> {
        AccountService::list_active_fee_schedules_for_account(self, account_id).await
    }

    async fn advance_fee_schedule_cursor(
        &self,
        account_id: &str,
        asset_id: &str,
        last_applied_period: String,
    ) -> StdResult<(), AccountError> {
        AccountService::advance_fee_schedule_cursor(self, account_id, asset_id, last_applied_period)
            .await
    }

    async fn upsert_holding_note(
        &self,
        account_id: &str,
        asset_id: String,
        text: String,
        threshold_price: Option<i64>,
        threshold_direction: Option<ThresholdDirection>,
    ) -> StdResult<HoldingNote, AccountError> {
        AccountService::upsert_holding_note(
            self,
            account_id,
            asset_id,
            text,
            threshold_price,
            threshold_direction,
        )
        .await
    }

    async fn delete_holding_note(
        &self,
        account_id: &str,
        asset_id: &str,
    ) -> StdResult<(), AccountError> {
        AccountService::delete_holding_note(self, account_id, asset_id).await
    }

    async fn get_holding_notes(
        &self,
        account_id: &str,
    ) -> StdResult<Vec<HoldingNote>, AccountError> {
        AccountService::get_holding_notes(self, account_id).await
    }
}

/// Loads an Account aggregate (with holdings + transactions) for the
/// holding-transaction family. Translates repository failures into typed
/// `AccountError` — `AccountNotFound` for `Ok(None)`, `DatabaseError` for any
/// anyhow error (logged at the same site).
async fn load_account(
    repo: &dyn AccountRepository,
    account_id: &str,
) -> Result<Account, AccountError> {
    match repo.get_with_holdings_and_transactions(account_id).await {
        Ok(Some(acc)) => Ok(acc),
        Ok(None) => Err(AccountError::AccountNotFound {
            account_id: account_id.to_string(),
        }),
        Err(e) => {
            tracing::error!(target: BACKEND, account_id = %account_id, err = ?e, "load_account: repository failure");
            Err(AccountError::DatabaseError)
        }
    }
}

/// A transfer placed in both accounts, not written yet.
struct StagedTransfer {
    parts: Vec<Transaction>,
    sending: Account,
    receiving: Account,
    sending_before: Vec<Transaction>,
    receiving_before: Vec<Transaction>,
}

impl StagedTransfer {
    /// What the staged transfer makes of its two accounts (AGT-065A, AGT-090).
    fn into_preview(self) -> TransferPreview {
        TransferPreview {
            parts: self.parts,
            sending_before: self.sending_before,
            sending_after: self.sending.transactions,
            receiving_before: self.receiving_before,
            receiving_after: self.receiving.transactions,
        }
    }
}

/// The figures a transfer is placed from (TRF entity).
struct TransferFigures {
    asset_id: String,
    date: String,
    quantity_sent: i64,
    quantity_received: i64,
    exchange_rate: i64,
    note: Option<String>,
}

/// TRF-025 — the accounts whose transfers in follow the transfers out `accounts` re-costed,
/// each re-costed in turn until no cost moves; the accounts already in `accounts` are
/// re-costed in place. A chain of transfers is followed to its end, at most once per
/// account and transfer.
async fn follow_transfers(
    repo: &dyn AccountRepository,
    accounts: &mut [&mut Account],
) -> Result<Vec<Account>, AccountError> {
    let mut followers: Vec<Account> = Vec::new();
    // Each pass re-costs what the last one moved; a cost moves at most once per transfer
    // and pass, so the passes end — bounded all the same against a corrupt ledger.
    for _ in 0..64 {
        let mut moved = Vec::new();
        for account in accounts.iter_mut() {
            moved.extend(account.restate_transfers_out());
        }
        for account in followers.iter_mut() {
            moved.extend(account.restate_transfers_out());
        }
        if moved.is_empty() {
            break;
        }
        for (transfer_id, cost) in moved {
            let holders = repo.transfer_accounts(&transfer_id).await.map_err(|e| {
                tracing::error!(target: BACKEND, transfer_id = %transfer_id, err = ?e, "follow_transfers: repository failure");
                AccountError::DatabaseError
            })?;
            for holder in holders {
                if let Some(account) = accounts.iter_mut().find(|a| a.id == holder) {
                    account
                        .recost_transfer_in(&transfer_id, cost)
                        .map_err(to_holding_tx_error)?;
                } else if let Some(account) = followers.iter_mut().find(|a| a.id == holder) {
                    account
                        .recost_transfer_in(&transfer_id, cost)
                        .map_err(to_holding_tx_error)?;
                } else {
                    let mut account = load_account(repo, &holder).await?;
                    if account
                        .recost_transfer_in(&transfer_id, cost)
                        .map_err(to_holding_tx_error)?
                    {
                        followers.push(account);
                    }
                }
            }
        }
    }
    Ok(followers)
}

/// Persists the pending changes of `accounts` and of the accounts their re-costed
/// transfers reach (TRF-025), in one database transaction.
async fn save_accounts(
    repo: &dyn AccountRepository,
    accounts: &mut [&mut Account],
) -> Result<(), AccountError> {
    save_accounts_marked(repo, accounts, None).await
}

/// `save_accounts`, the transactions of `marked` marked with its agent's session in the
/// same write (AGT-045, AGT-049).
async fn save_accounts_marked(
    repo: &dyn AccountRepository,
    accounts: &mut [&mut Account],
    marked: Option<(&[String], &AgentRecording)>,
) -> Result<(), AccountError> {
    let mut followers = follow_transfers(repo, accounts).await?;
    let mut written: Vec<&mut Account> = accounts
        .iter_mut()
        .map(|account| &mut **account)
        .chain(followers.iter_mut())
        .collect();
    let saved = match marked {
        Some((ids, mark)) => repo.save_all_marked(&mut written, ids, mark).await,
        None => repo.save_all(&mut written).await,
    };
    saved.map_err(|e| save_error("save_accounts", e))
}

/// Persists an Account aggregate's pending changes for the holding-transaction
/// family, with the transfers in its re-costed transfers out reach (TRF-025).
/// Translates repository failures into `AccountError::DatabaseError` after
/// logging the underlying error.
async fn save_account(
    repo: &dyn AccountRepository,
    account: &mut Account,
) -> Result<(), AccountError> {
    let mut followers = follow_transfers(repo, std::slice::from_mut(&mut &mut *account)).await?;
    if !followers.is_empty() {
        let mut written: Vec<&mut Account> = std::iter::once(&mut *account)
            .chain(followers.iter_mut())
            .collect();
        return repo
            .save_all(&mut written)
            .await
            .map_err(|e| save_error("save_account", e));
    }
    repo.save(account)
        .await
        .map_err(|e| save_error("save_account", e))
}

/// A failed save, translated: the change recorder refusing the application's own write is
/// `ApplicationWriteOutranked` (CFR-016) — the repository's transaction rolled back with it,
/// nothing to report as a failure; anything else is logged and is `DatabaseError`.
fn save_error(op: &'static str, e: anyhow::Error) -> AccountError {
    if let Some(refused) = e.downcast_ref::<LocalWriteOutranked>() {
        tracing::info!(target: BACKEND, op = %op, record_identity = %refused.record_identity, "application write outranked by the user's state");
        return AccountError::ApplicationWriteOutranked;
    }
    tracing::error!(target: BACKEND, op = %op, err = ?e, "repository failure");
    AccountError::DatabaseError
}

/// CRUD-family parallel to the load/save helpers above. Wraps the
/// `find_by_name` uniqueness pre-check used by `create` and `update`,
/// translating any repository failure into
/// `AccountError::DatabaseError`.
///
/// Unlike `load_account`, `Ok(None)` is the **success** path here (the name
/// is available); the caller decides what to do with `Some(existing)`.
async fn find_account_by_name(
    repo: &dyn AccountRepository,
    name: &str,
) -> Result<Option<Account>, AccountError> {
    repo.find_by_name(name).await.map_err(|e| {
        tracing::error!(target: BACKEND, name = %name, err = ?e, "find_by_name: repository failure");
        AccountError::DatabaseError
    })
}

/// Open-holding parallel to `load_account`. Same shape; targets `OpenHoldingError`.
async fn load_account_for_open_holding(
    repo: &dyn AccountRepository,
    account_id: &str,
) -> Result<Account, OpenHoldingError> {
    match repo.get_with_holdings_and_transactions(account_id).await {
        Ok(Some(acc)) => Ok(acc),
        Ok(None) => Err(AccountError::AccountNotFound {
            account_id: account_id.to_string(),
        }
        .into()),
        Err(e) => {
            tracing::error!(target: BACKEND, account_id = %account_id, err = ?e, "load_account_for_open_holding: repository failure");
            Err(AccountError::DatabaseError.into())
        }
    }
}

/// Open-holding parallel to `save_account`. Same shape; targets `OpenHoldingError`.
async fn save_account_for_open_holding(
    repo: &dyn AccountRepository,
    account: &mut Account,
) -> Result<(), OpenHoldingError> {
    // TRF-025 — a backdated opening balance re-costs the transfers out it precedes.
    save_account(repo, account).await.map_err(Into::into)
}

/// A recording applied to `account` in memory — what recording saves and
/// a preview drops (AGT-072).
/// IMP-036 — what a transaction must hold to be the one a step records: its type, date and
/// asset, with its quantity and total for a trade, its amount otherwise.
struct RecordedKey {
    transaction_type: TransactionType,
    date: String,
    asset_id: String,
    quantity: Option<i64>,
    total_amount: i64,
}

impl RecordedKey {
    fn matches(&self, transaction: &Transaction) -> bool {
        transaction.transaction_type == self.transaction_type
            && transaction.date == self.date
            && transaction.asset_id == self.asset_id
            && self
                .quantity
                .is_none_or(|quantity| quantity == transaction.quantity)
            && transaction.total_amount == self.total_amount
    }
}

/// The key of a step, its total computed as manual entry computes it (TRX-026, SEL-023);
/// `None` for a step that cannot compute one, which manual entry then refuses.
fn recorded_key(op: &RecordingOp, cash_asset_id: &str) -> Option<RecordedKey> {
    let trade = |transaction_type: TransactionType, trade: &TradeOp| {
        let entered = EnteredAmount::UnitPrice {
            unit_price: trade.unit_price,
            exchange_rate: trade.exchange_rate,
            fees: trade.fees,
        };
        let (_, total_amount) =
            Account::preview_trade(transaction_type, &trade.date, trade.quantity, entered).ok()?;
        Some(RecordedKey {
            transaction_type,
            date: trade.date.clone(),
            asset_id: trade.asset_id.clone(),
            quantity: Some(trade.quantity),
            total_amount,
        })
    };
    let cash = |transaction_type: TransactionType, date: &str, amount: i64| {
        Some(RecordedKey {
            transaction_type,
            date: date.to_string(),
            asset_id: cash_asset_id.to_string(),
            quantity: None,
            total_amount: amount,
        })
    };
    match op {
        RecordingOp::Buy(op) => trade(TransactionType::Purchase, op),
        RecordingOp::Sell(op) => trade(TransactionType::Sell, op),
        RecordingOp::Deposit { date, amount, .. } => cash(TransactionType::Deposit, date, *amount),
        RecordingOp::Withdrawal { date, amount, .. } => {
            cash(TransactionType::Withdrawal, date, *amount)
        }
        RecordingOp::Dividend {
            asset_id,
            date,
            amount,
            ..
        } => Some(RecordedKey {
            transaction_type: TransactionType::Dividend,
            date: date.clone(),
            asset_id: asset_id.clone(),
            quantity: None,
            total_amount: *amount,
        }),
        _ => None,
    }
}

/// DIV-011 — the account holds the step's asset today.
fn holds(account: &Account, op: &RecordingOp) -> bool {
    let RecordingOp::Dividend { asset_id, .. } = op else {
        return true;
    };
    account
        .holdings
        .iter()
        .any(|holding| holding.asset_id == *asset_id && holding.quantity > 0)
}

/// One step applied to a copy of `account`: the judgment, and the account it leaves when
/// manual entry takes it. A purchase the cash cannot fund takes, on its date, the smallest
/// deposit that funds it (IMP-038): a deposit of its own total always does, the account
/// having been funded without it.
/// IMP-035 / IMP-036 — each step judged in order on `account`, every one taken applied to
/// it: already recorded when it matches a transaction the account held, each matched once.
/// `fund`: a purchase the cash cannot fund is taken after the deposit it needs (IMP-038).
fn judge_all(
    account: &mut Account,
    steps: Vec<RecordingStep>,
    fund: bool,
) -> Vec<RecordingJudgment> {
    let mut recorded: Vec<Option<Transaction>> =
        account.transactions.iter().cloned().map(Some).collect();
    let cash_asset_id = account.cash_asset_id();
    let mut judgments = Vec::with_capacity(steps.len());
    for step in steps {
        let key = recorded_key(&step.op, &cash_asset_id);
        let same = recorded.iter_mut().find(|transaction| {
            transaction
                .as_ref()
                .is_some_and(|transaction| key.as_ref().is_some_and(|key| key.matches(transaction)))
        });
        if let Some(same) = same {
            *same = None;
            judgments.push(RecordingJudgment::AlreadyRecorded);
            continue;
        }
        if step.needs_held && !holds(account, &step.op) {
            judgments.push(RecordingJudgment::NotHeld);
            continue;
        }
        let (judgment, judged) = judge_step(account, step.op, fund);
        if let Some(judged) = judged {
            *account = judged;
        }
        judgments.push(judgment);
    }
    judgments
}

fn judge_step(
    account: &Account,
    op: RecordingOp,
    fund: bool,
) -> (RecordingJudgment, Option<Account>) {
    let mut judged = account.clone();
    let refusal = match apply(&mut judged, op.clone()) {
        Ok(_) => return (RecordingJudgment::Ready { funding: None }, Some(judged)),
        Err(refusal) => refusal,
    };
    let (true, RecordingOp::Buy(trade), AccountError::InsufficientCash { .. }) =
        (fund, &op, &refusal)
    else {
        return (RecordingJudgment::Refused(refusal), None);
    };
    // IMP-051 — the deposit a purchase needs, dated with it and marked as added.
    let funded = |funding: i64| {
        let mut funded = account.clone();
        Transaction::new_deposit(
            funded.id.clone(),
            funded.cash_asset_id(),
            trade.date.clone(),
            funding,
            None,
        )
        .map(|deposit| deposit.with_added_deposit(true))
        .and_then(|deposit| funded.apply_deposit(deposit))
        .and_then(|_| apply(&mut funded, op.clone()))
        .map(|_| funded)
    };
    let total = trade
        .total_amount
        .or_else(|| recorded_key(&op, "").map(|key| key.total_amount))
        .unwrap_or(0);
    let Ok(mut best) = funded(total) else {
        return (RecordingJudgment::Refused(refusal), None);
    };
    let (mut low, mut high) = (0, total);
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        match funded(middle) {
            Ok(account) => {
                high = middle;
                best = account;
            }
            Err(_) => low = middle,
        }
    }
    (
        RecordingJudgment::Ready {
            funding: Some(high),
        },
        Some(best),
    )
}

fn apply(account: &mut Account, op: RecordingOp) -> Result<Transaction, AccountError> {
    match op {
        RecordingOp::Buy(trade) => account
            .buy_holding(
                trade.asset_id,
                trade.date,
                trade.quantity,
                trade.unit_price,
                trade.exchange_rate,
                trade.fees,
                trade.total_amount,
                trade.note,
            )
            .map_err(to_holding_tx_error)
            .cloned(),
        RecordingOp::Sell(trade) => account
            .sell_holding(
                trade.asset_id,
                trade.date,
                trade.quantity,
                trade.unit_price,
                trade.exchange_rate,
                trade.fees,
                trade.total_amount,
                trade.note,
            )
            .map_err(to_holding_tx_error)
            .cloned(),
        RecordingOp::Deposit { date, amount, note } => {
            let tx = Transaction::new_deposit(
                account.id.clone(),
                account.cash_asset_id(),
                date,
                amount,
                note,
            )?;
            Ok(account.apply_deposit(tx)?)
        }
        RecordingOp::Withdrawal { date, amount, note } => {
            let tx = Transaction::new_withdrawal(
                account.id.clone(),
                account.cash_asset_id(),
                date,
                amount,
                note,
            )?;
            Ok(account.apply_withdrawal(tx)?)
        }
        RecordingOp::Dividend {
            asset_id,
            date,
            amount,
            exchange_rate,
            note,
        } => {
            let tx = Transaction::new_dividend(
                account.id.clone(),
                asset_id,
                date,
                amount,
                exchange_rate,
                note,
            )?;
            Ok(account.apply_dividend(tx)?)
        }
        RecordingOp::FreeShares {
            asset_id,
            date,
            quantity,
            note,
        } => {
            let tx = Transaction::free_shares(account.id.clone(), asset_id, date, quantity, note)?;
            account.apply_free_shares(tx).map_err(to_holding_tx_error)
        }
        RecordingOp::Split {
            asset_id,
            date,
            factor,
            note,
        } => {
            let tx = Transaction::split(account.id.clone(), asset_id, date, factor, note)?;
            account.apply_split(tx).map_err(to_holding_tx_error)
        }
        RecordingOp::WriteOff {
            asset_id,
            date,
            quantity,
            reason,
            note,
        } => {
            let tx =
                Transaction::write_off(account.id.clone(), asset_id, date, quantity, reason, note)?;
            account.apply_write_off(tx).map_err(to_holding_tx_error)
        }
    }
}

/// An opening balance applied to `account` in memory (TRX-042, AGT-072).
fn apply_opening(
    account: &mut Account,
    asset_id: String,
    date: String,
    quantity: i64,
    total_cost: i64,
) -> Result<Transaction, OpenHoldingError> {
    account
        .open_holding(asset_id, date, quantity, total_cost)
        .map_err(to_open_holding_error)
        .cloned()
}

/// Converts the `anyhow::Error` returned by the buy/sell/correct/cancel
/// aggregate methods into the typed `AccountError` it boxes. Errors that don't
/// downcast to `AccountError` are logged and surfaced as
/// `AccountError::DatabaseError` — the same translation target as the
/// load/save helpers above.
fn to_holding_tx_error(e: anyhow::Error) -> AccountError {
    match e.downcast::<AccountError>() {
        Ok(err) => err,
        Err(e) => {
            tracing::error!(target: BACKEND, err = ?e, "unexpected error in holding-tx service method");
            AccountError::DatabaseError
        }
    }
}

/// Reads a synced change's content into the record it carries (CFR-017). A payload this
/// build cannot read is an infrastructure failure: logged, surfaced as `DatabaseError`.
fn synced_content<T: serde::de::DeserializeOwned>(content: &str) -> Result<T, AccountError> {
    serde_json::from_str(content).map_err(|e| {
        tracing::error!(target: BACKEND, err = %e, "synced content: malformed payload");
        AccountError::DatabaseError
    })
}

/// The two keys of an `account:asset` identity (CFR-012).
fn holding_keys(identity: &str) -> Result<(&str, &str), AccountError> {
    identity.split_once(':').ok_or_else(|| {
        tracing::error!(target: BACKEND, identity = %identity, "synced identity: malformed holding key");
        AccountError::DatabaseError
    })
}

/// Translates a failed applied write into `DatabaseError` after logging it.
fn applied_write_error(context: &'static str, e: anyhow::Error) -> AccountError {
    tracing::error!(target: BACKEND, err = ?e, "{context}: repository failure");
    AccountError::DatabaseError
}

/// Bridge for the open_holding aggregate method, which still returns
/// `anyhow::Result` boxing an `AccountError` (`InvalidTotalCost` or any variant
/// reachable from `Transaction::new`). Same shape as `to_holding_tx_error`;
/// targets `OpenHoldingError` instead.
fn to_open_holding_error(e: anyhow::Error) -> OpenHoldingError {
    match e.downcast::<AccountError>() {
        Ok(err) => err.into(),
        Err(e) => {
            tracing::error!(target: BACKEND, err = ?e, "unexpected error in open_holding service method");
            AccountError::DatabaseError.into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // This module contains both SQLite-backed integration tests (real in-memory DB,
    // catch constraint violations) and mock-based unit tests (fast delegation checks).
    // SQLite tests are grouped first; mock-based unit tests follow after the section header.
    use crate::context::account::{
        AccountError, Holding, MockAccountRepository, MockFeeCatchUpRepository,
        MockHoldingRepository, MockTransactionRepository, SqliteAccountRepository,
        SqliteFeeScheduleRepository, SqliteHoldingNoteRepository, SqliteHoldingRepository,
        SqliteTransactionRepository,
    };
    use sqlx::sqlite::SqlitePoolOptions;

    #[derive(Debug, thiserror::Error)]
    #[error("simulated DB failure")]
    struct SimulatedSaveError;

    // to_holding_tx_error is the anyhow→typed bridge for the four
    // holding-tx aggregate methods (buy/sell/correct/cancel) that still return
    // `anyhow::Result`. One global test covers the three branches: known
    // domain leaves route to their typed variant; everything else translates
    // to Application(DatabaseError).
    #[test]
    fn to_holding_tx_error_maps_every_branch() {
        // AccountError leaf → Operation
        let op_err = AccountError::Oversell {
            available: 10,
            requested: 99,
        };
        assert!(matches!(
            to_holding_tx_error(anyhow::Error::new(op_err)),
            AccountError::Oversell {
                available: 10,
                requested: 99
            }
        ));

        // AccountError leaf → Validation
        assert!(matches!(
            to_holding_tx_error(anyhow::Error::new(AccountError::DateInFuture)),
            AccountError::DateInFuture
        ));

        // Anything else → Application(DatabaseError) (the catch-all path)
        assert!(matches!(
            to_holding_tx_error(anyhow::anyhow!("synthetic infra failure")),
            AccountError::DatabaseError
        ));
    }

    // to_open_holding_error is the anyhow→typed bridge for `Account::open_holding`
    // (which still returns `anyhow::Result`). One global test covers the three
    // branches: known domain leaves route to their typed variants; unrecognized
    // errors translate to Application(DatabaseError).
    #[test]
    fn to_open_holding_error_maps_every_branch() {
        use crate::context::account::AccountError;
        use OpenHoldingError;

        // AccountError leaf → Validation
        assert!(matches!(
            to_open_holding_error(anyhow::Error::new(AccountError::InvalidTotalCost)),
            OpenHoldingError::Account(AccountError::InvalidTotalCost)
        ));

        // AccountError leaf → TxValidation
        assert!(matches!(
            to_open_holding_error(anyhow::Error::new(AccountError::QuantityNotPositive)),
            OpenHoldingError::Account(AccountError::QuantityNotPositive)
        ));

        // AccountError leaf → cash-line guard (CSH-061)
        assert!(matches!(
            to_open_holding_error(anyhow::Error::new(AccountError::OpeningBalanceOnCashAsset)),
            OpenHoldingError::Account(AccountError::OpeningBalanceOnCashAsset)
        ));

        // Anything else → Application(DatabaseError) (the catch-all path)
        assert!(matches!(
            to_open_holding_error(anyhow::anyhow!("synthetic infra failure")),
            OpenHoldingError::Account(AccountError::DatabaseError)
        ));
    }

    async fn setup(pool: &sqlx::Pool<sqlx::Sqlite>) -> (AccountService, String) {
        let svc = AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        )
        .with_fee_schedule_repo(Box::new(SqliteFeeScheduleRepository::new(pool.clone())))
        .with_holding_note_repo(Box::new(SqliteHoldingNoteRepository::new(pool.clone())));
        let asset_id = "test-asset-id".to_string();
        sqlx::query(
            "INSERT INTO assets (id, name, reference, asset_class, category_id, currency, risk_level)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&asset_id)
        .bind("TestAsset")
        .bind("TST")
        .bind("Stocks")
        .bind("default-uncategorized")
        .bind("USD")
        .bind(1_i64)
        .execute(pool)
        .await
        .expect("seed asset row");
        (svc, asset_id)
    }

    async fn make_pool() -> sqlx::Pool<sqlx::Sqlite> {
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

    async fn setup_service() -> AccountService {
        let pool = make_pool().await;
        AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        )
    }

    // ACC-044 — an account is created and edited with its country and wrapper, read back as
    // stored; an edit that does not name them (fees, sync) keeps them.
    #[tokio::test]
    async fn acc_044_an_account_keeps_its_country_and_wrapper() {
        let svc = setup_service().await;
        let created = svc
            .create_classified(
                "PEA".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
                None,
                Some("fr_pea".to_string()),
            )
            .await
            .unwrap();
        let read = svc.get_by_id(&created.id).await.unwrap().unwrap();
        assert_eq!(
            (read.country.as_deref(), read.wrapper.as_deref()),
            (None, Some("fr_pea"))
        );

        let enabled = enable_management_fees(&svc, &read).await;
        assert_eq!(enabled.wrapper.as_deref(), Some("fr_pea"));

        let moved = svc
            .update_classified(
                created.id.clone(),
                "Depot".to_string(),
                String::new(),
                "EUR".to_string(),
                true,
                Some("DE".to_string()),
                Some("de_depot".to_string()),
            )
            .await
            .unwrap();
        let read = svc.get_by_id(&moved.id).await.unwrap().unwrap();
        assert_eq!(
            (read.country.as_deref(), read.wrapper.as_deref()),
            (Some("DE"), Some("de_depot"))
        );
    }

    // ACC-041/043 — a malformed country and a wrapper the lists do not know are refused with
    // their own codes, and nothing is created; a known wrapper fits an account of any country.
    #[tokio::test]
    async fn acc_043_any_account_takes_a_known_wrapper_and_an_unknown_one_is_refused() {
        let svc = setup_service().await;
        let create = |name: &str, country: Option<&str>, wrapper: Option<&str>| {
            svc.create_classified(
                name.to_string(),
                String::new(),
                "EUR".to_string(),
                false,
                country.map(str::to_string),
                wrapper.map(str::to_string),
            )
        };
        assert!(matches!(
            create("A", Some("fra"), None).await,
            Err(AccountError::InvalidCountry { .. })
        ));
        assert!(matches!(
            create("A", None, Some("isa")).await,
            Err(AccountError::WrapperNotAvailable { .. })
        ));
        assert!(svc.get_all().await.unwrap().is_empty());
        let french_in_spain = create("A", Some("ES"), Some("fr_pea")).await.unwrap();
        assert_eq!(french_in_spain.wrapper.as_deref(), Some("fr_pea"));
        let german_in_portfolio = create("B", None, Some("de_depot")).await.unwrap();
        assert_eq!(german_in_portfolio.wrapper.as_deref(), Some("de_depot"));
    }

    // ACC-043/044 — an edit keeps a stored wrapper this device's lists do not know (another
    // device's newer lists, ACC-040), and a change of country never empties the wrapper.
    #[tokio::test]
    async fn acc_043_an_edit_keeps_a_wrapper_the_lists_do_not_know() {
        let svc = setup_service().await;
        let created = svc
            .create_classified(
                "Plan".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
                Some("ES".to_string()),
                None,
            )
            .await
            .unwrap();
        svc.account_repo
            .update(
                created.clone().with_stored_classification(
                    Some("ES".to_string()),
                    Some("es_plan".to_string()),
                ),
            )
            .await
            .unwrap();

        let renamed = svc
            .update_classified(
                created.id.clone(),
                "Plan pensiones".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
                Some("DE".to_string()),
                Some("es_plan".to_string()),
            )
            .await
            .unwrap();
        assert_eq!(
            (renamed.country.as_deref(), renamed.wrapper.as_deref()),
            (Some("DE"), Some("es_plan"))
        );

        assert!(matches!(
            svc.update_classified(
                created.id.clone(),
                "Plan pensiones".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
                None,
                Some("it_pir".to_string()),
            )
            .await,
            Err(AccountError::WrapperNotAvailable { .. })
        ));
    }

    async fn enable_management_fees(svc: &AccountService, account: &Account) -> Account {
        svc.update(
            account.id.clone(),
            account.name.clone(),
            String::new(),
            account.currency.clone(),
            true,
        )
        .await
        .unwrap()
    }

    // FEE-077 — record_management_fee rejects when the account has the mechanism disabled
    #[tokio::test]
    async fn fee_077_record_management_fee_rejects_disabled_account() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create("Gated".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        let err = svc
            .record_management_fee(
                &account.id,
                asset_id,
                "2026-01-15".to_string(),
                1_000_000,
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::ManagementFeesDisabled),
            "got: {err:?}"
        );
    }

    // FEE-077 — create_fee_schedule rejects when the account has the mechanism disabled
    #[tokio::test]
    async fn fee_077_create_fee_schedule_rejects_disabled_account() {
        use crate::context::account::FeeFrequency;
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create("Gated".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        let err = svc
            .create_fee_schedule(
                &account.id,
                asset_id,
                1_000_000,
                FeeFrequency::Monthly,
                "2026-01-01".to_string(),
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::ManagementFeesDisabled),
            "got: {err:?}"
        );
    }

    // FEE-074 — the scoped query returns only the given account's active
    // schedules: another account's active schedule and the account's own
    // inactive schedule are both excluded.
    #[tokio::test]
    async fn list_active_fee_schedules_for_account_excludes_other_accounts_and_inactive() {
        use crate::context::account::FeeFrequency;
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let second_asset_id = "test-asset-id-2".to_string();
        sqlx::query(
            "INSERT INTO assets (id, name, reference, asset_class, category_id, currency, risk_level)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&second_asset_id)
        .bind("TestAsset2")
        .bind("TST2")
        .bind("Stocks")
        .bind("default-uncategorized")
        .bind("USD")
        .bind(1_i64)
        .execute(&pool)
        .await
        .expect("seed second asset row");

        let target = svc
            .create(
                "Target".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        let target = enable_management_fees(&svc, &target).await;
        let other = svc
            .create("Other".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        let other = enable_management_fees(&svc, &other).await;

        svc.create_fee_schedule(
            &target.id,
            asset_id.clone(),
            1_000_000,
            FeeFrequency::Monthly,
            "2026-01-01".to_string(),
            None,
        )
        .await
        .unwrap();
        svc.create_fee_schedule(
            &target.id,
            second_asset_id.clone(),
            2_000_000,
            FeeFrequency::Monthly,
            "2026-01-01".to_string(),
            None,
        )
        .await
        .unwrap();
        svc.update_fee_schedule(&target.id, &second_asset_id, 2_000_000, None, false)
            .await
            .unwrap();
        svc.create_fee_schedule(
            &other.id,
            asset_id.clone(),
            3_000_000,
            FeeFrequency::Monthly,
            "2026-01-01".to_string(),
            None,
        )
        .await
        .unwrap();

        let schedules = svc
            .list_active_fee_schedules_for_account(&target.id)
            .await
            .unwrap();
        assert_eq!(
            schedules.len(),
            1,
            "only the target account's active schedule must return, got: {schedules:?}"
        );
        assert_eq!(schedules[0].account_id, target.id);
        assert_eq!(schedules[0].asset_id, asset_id);
        assert!(schedules[0].active);
    }

    /// Seeds the system Cash Asset row + a large Deposit so existing buy/sell tests can
    /// satisfy CSH-041 (purchase eligibility). Bypasses `AssetService` because these tests
    /// only construct an `AccountService`.
    async fn seed_cash_for_account(
        pool: &sqlx::Pool<sqlx::Sqlite>,
        svc: &AccountService,
        account_id: &str,
        currency: &str,
    ) {
        let cash_asset_id = format!("system-cash-{}", currency.to_lowercase());
        sqlx::query(
            "INSERT OR IGNORE INTO categories (id, name, is_deleted) VALUES ('system-cash-category', 'cash', 0)",
        )
        .execute(pool)
        .await
        .expect("seed cash category");
        sqlx::query(
            "INSERT OR IGNORE INTO assets (id, name, reference, asset_class, category_id, currency, risk_level) \
             VALUES (?, ?, ?, 'Cash', 'system-cash-category', ?, 1)",
        )
        .bind(&cash_asset_id)
        .bind(format!("Cash {}", currency.to_uppercase()))
        .bind(currency.to_uppercase())
        .bind(currency)
        .execute(pool)
        .await
        .expect("seed cash asset");
        svc.record_deposit(
            account_id,
            "2020-01-01".to_string(),
            1_000_000_000_000,
            None,
        )
        .await
        .expect("seed cash deposit");
    }

    // R3 — duplicate name (case-insensitive) is rejected at creation
    #[tokio::test]
    async fn test_create_rejects_duplicate_name_case_insensitive() {
        let svc = setup_service().await;
        svc.create("Alpha".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        let err = svc
            .create("alpha".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::NameAlreadyExists),
            "got: {err:?}"
        );
    }

    // R3 — updating an account to a name used by another account is rejected
    #[tokio::test]
    async fn test_update_rejects_name_collision_with_other_account() {
        let svc = setup_service().await;
        svc.create("Alpha".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        let beta = svc
            .create("Beta".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        let err = svc
            .update(
                beta.id,
                "ALPHA".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::NameAlreadyExists),
            "got: {err:?}"
        );
    }

    // R3 — updating an account with its own name (same case) must succeed
    #[tokio::test]
    async fn test_update_allows_same_name_on_same_account() {
        let svc = setup_service().await;
        let account = svc
            .create("Alpha".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        let result = svc
            .update(
                account.id,
                "Alpha".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await;
        assert!(result.is_ok());
    }

    fn micro(v: i64) -> i64 {
        v * 1_000_000
    }

    // TRX-026 / TRX-030 — buy_holding persists transaction and updates holding VWAP
    #[tokio::test]
    async fn test_buy_holding_persists_transaction_and_holding() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create("Acc".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;
        let tx = svc
            .buy_holding(
                &account.id,
                asset_id.clone(),
                "2024-01-01".to_string(),
                micro(2),
                micro(100),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(tx.account_id, account.id);
        assert_eq!(tx.asset_id, asset_id);
        let holdings = svc.get_holdings_for_account(&account.id).await.unwrap();
        let asset_holding = holdings
            .iter()
            .find(|h| h.asset_id == asset_id)
            .expect("asset holding present");
        assert_eq!(asset_holding.quantity, micro(2));
        assert_eq!(asset_holding.average_price, micro(100));
    }

    // SEL-021 — sell_holding rejects oversell via AccountError
    #[tokio::test]
    async fn test_sell_holding_rejects_oversell() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create("Acc".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;
        svc.buy_holding(
            &account.id,
            asset_id.clone(),
            "2024-01-01".to_string(),
            micro(1),
            micro(100),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();
        let err = svc
            .sell_holding(
                &account.id,
                asset_id,
                "2024-06-01".to_string(),
                micro(2),
                micro(100),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::Oversell { .. }),
            "expected Oversell, got: {err:?}"
        );
    }

    // TRX-034 — cancel_transaction removes the holding when it was the last transaction
    #[tokio::test]
    async fn test_cancel_transaction_removes_holding_when_last() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create("Acc".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;
        let tx = svc
            .buy_holding(
                &account.id,
                asset_id.clone(),
                "2024-01-01".to_string(),
                micro(1),
                micro(100),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap();
        svc.cancel_transaction(&account.id, &tx.id).await.unwrap();
        let holdings = svc.get_holdings_for_account(&account.id).await.unwrap();
        assert!(
            holdings.iter().all(|h| h.asset_id != asset_id),
            "asset holding should be removed after cancel"
        );
        let txs = svc.get_transactions(&account.id, &asset_id).await.unwrap();
        assert!(
            txs.is_empty(),
            "transactions for the asset should be removed after cancel"
        );
    }

    // SEL-026 — full sell retains holding at quantity=0 with VWAP preserved
    #[tokio::test]
    async fn test_full_sell_retains_holding_at_zero_with_last_vwap() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create("Acc".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;
        svc.buy_holding(
            &account.id,
            asset_id.clone(),
            "2024-01-01".to_string(),
            micro(2),
            micro(100),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();
        svc.sell_holding(
            &account.id,
            asset_id.clone(),
            "2024-06-01".to_string(),
            micro(2),
            micro(120),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();
        let holdings = svc.get_holdings_for_account(&account.id).await.unwrap();
        let h = holdings
            .iter()
            .find(|h| h.asset_id == asset_id)
            .expect("holding should exist after full sell");
        assert_eq!(h.quantity, 0, "holding should be retained at qty=0");
        assert_eq!(h.average_price, micro(100), "VWAP should be preserved");
    }

    // SEL-032 — correcting a purchase to a lower qty that would cause a cascading oversell is rejected
    #[tokio::test]
    async fn test_correct_purchase_rejected_when_causes_cascading_oversell() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create("Acc".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;
        let buy = svc
            .buy_holding(
                &account.id,
                asset_id.clone(),
                "2024-01-01".to_string(),
                micro(3),
                micro(100),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap();
        svc.sell_holding(
            &account.id,
            asset_id.clone(),
            "2024-06-01".to_string(),
            micro(2),
            micro(120),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();
        let err = svc
            .correct_transaction(
                &account.id,
                &buy.id,
                "2024-01-01".to_string(),
                micro(1),
                micro(100),
                micro(1),
                0,
                None, // total_amount (typed-total mode unused here)
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::CascadingOversell),
            "expected CascadingOversell, got: {err:?}"
        );
    }

    // TRX-027 — buy_holding propagates save failure so no partial state is committed
    #[tokio::test]
    async fn test_buy_holding_returns_error_when_save_fails() {
        let mut mock_ar = MockAccountRepository::new();
        mock_ar
            .expect_get_with_holdings_and_transactions()
            .once()
            .returning(|_| {
                let mut acc =
                    Account::new("Test".to_string(), String::new(), "EUR".to_string(), false)
                        .unwrap();
                // Seed enough cash so CSH-041 doesn't short-circuit before save() is called.
                acc.record_deposit("2020-01-01".to_string(), 1_000_000_000_000, None)
                    .unwrap();
                acc.pending_changes.clear();
                Ok(Some(acc))
            });
        mock_ar
            .expect_save()
            .once()
            .returning(|_| Err(SimulatedSaveError.into()));

        let svc = AccountService::new(
            Box::new(mock_ar),
            Box::new(MockHoldingRepository::new()),
            Box::new(MockTransactionRepository::new()),
        );

        let result = svc
            .buy_holding(
                "any-account-id",
                "asset-1".to_string(),
                "2024-01-01".to_string(),
                micro(1),
                micro(100),
                micro(1),
                0,
                None,
                None,
            )
            .await;

        let err = result.unwrap_err();
        // The repo save error is opaqued at the service boundary — translated
        // to AccountError::DatabaseError; the hint is preserved
        // server-side via tracing::error! at the same site.
        assert!(
            matches!(err, AccountError::DatabaseError),
            "buy_holding must surface save failures as Application(DatabaseError), got: {err:?}"
        );
    }

    // TDI-012 — an unparseable date is rejected before any repository call.
    #[tokio::test]
    async fn holding_snapshot_as_of_rejects_an_unparseable_date() {
        let svc = AccountService::new(
            Box::new(MockAccountRepository::new()),
            Box::new(MockHoldingRepository::new()),
            Box::new(MockTransactionRepository::new()),
        );
        let err = svc
            .holding_snapshot_as_of("acc-1", "asset-1", "not-a-date")
            .await
            .unwrap_err();
        assert!(matches!(err, AccountError::InvalidDate));
    }

    // TDI-010 — a valid date over an empty history yields an empty snapshot.
    #[tokio::test]
    async fn holding_snapshot_as_of_returns_empty_for_an_account_with_no_transactions() {
        let mut mock_tr = MockTransactionRepository::new();
        mock_tr
            .expect_get_by_account_asset()
            .once()
            .returning(|_, _| Ok(vec![]));
        let svc = AccountService::new(
            Box::new(MockAccountRepository::new()),
            Box::new(MockHoldingRepository::new()),
            Box::new(mock_tr),
        );
        let snap = svc
            .holding_snapshot_as_of("acc-1", "asset-1", "2024-07-01")
            .await
            .expect("a valid date with no transactions yields an empty snapshot");
        assert_eq!(snap.quantity, 0);
        assert_eq!(snap.average_price, 0);
    }

    // The cash line's snapshot is read over the whole account: the purchase of
    // another asset, paid from the cash, counts.
    #[tokio::test]
    async fn holding_snapshot_as_of_reads_the_cash_line_over_the_whole_account() {
        let cash = crate::core::cash::system_cash_asset_id("EUR");
        let deposit = Transaction::new_deposit(
            "acc-1".to_string(),
            cash.clone(),
            "2024-06-01".to_string(),
            100_000_000,
            None,
        )
        .expect("deposit");
        let purchase = Transaction::new(
            "acc-1".to_string(),
            "asset-1".to_string(),
            crate::context::account::TransactionType::Purchase,
            "2024-06-05".to_string(),
            4_000_000,
            10_000_000,
            1_000_000,
            0,
            40_000_000,
            None,
            None,
        )
        .expect("purchase");
        let mut mock_tr = MockTransactionRepository::new();
        mock_tr
            .expect_get_all_for_account()
            .once()
            .returning(move |_| Ok(vec![deposit.clone(), purchase.clone()]));
        let svc = AccountService::new(
            Box::new(MockAccountRepository::new()),
            Box::new(MockHoldingRepository::new()),
            Box::new(mock_tr),
        );

        let snap = svc
            .holding_snapshot_as_of("acc-1", &cash, "2024-07-01")
            .await
            .expect("snapshot");

        assert_eq!(snap.quantity, 60_000_000);
        assert_eq!(snap.average_price, 1_000_000);
    }

    // A repository failure on the transaction fetch surfaces as DatabaseError.
    #[tokio::test]
    async fn holding_snapshot_as_of_surfaces_repository_failure_as_database_error() {
        let mut mock_tr = MockTransactionRepository::new();
        mock_tr
            .expect_get_by_account_asset()
            .once()
            .returning(|_, _| Err(anyhow::anyhow!("db down")));
        let svc = AccountService::new(
            Box::new(MockAccountRepository::new()),
            Box::new(MockHoldingRepository::new()),
            Box::new(mock_tr),
        );
        let err = svc
            .holding_snapshot_as_of("acc-1", "asset-1", "2024-07-01")
            .await
            .unwrap_err();
        assert!(matches!(err, AccountError::DatabaseError));
    }

    // -------------------------------------------------------------------------
    // open_holding service tests (TRX-042 through TRX-056)
    // -------------------------------------------------------------------------

    // open_holding propagates save failure as Application(DatabaseError) — mirrors
    // test_buy_holding_returns_error_when_save_fails for the typed-Result path
    // (save_account_for_open_holding Err branch).
    #[tokio::test]
    async fn test_open_holding_returns_database_error_when_save_fails() {
        let mut mock_ar = MockAccountRepository::new();
        mock_ar
            .expect_get_with_holdings_and_transactions()
            .once()
            .returning(|_| {
                let acc = Account::new("Test".to_string(), String::new(), "EUR".to_string(), false)
                    .unwrap();
                Ok(Some(acc))
            });
        mock_ar
            .expect_save()
            .once()
            .returning(|_| Err(SimulatedSaveError.into()));

        let svc = AccountService::new(
            Box::new(mock_ar),
            Box::new(MockHoldingRepository::new()),
            Box::new(MockTransactionRepository::new()),
        );

        let result = svc
            .open_holding(
                "any-account-id",
                "asset-1".to_string(),
                "2024-01-01".to_string(),
                micro(1),
                micro(100),
            )
            .await;

        let err = result.unwrap_err();
        use crate::use_cases::holding_transaction::OpenHoldingError;
        assert!(
            matches!(err, OpenHoldingError::Account(AccountError::DatabaseError)),
            "open_holding must surface save failures as Application(DatabaseError), got: {err:?}"
        );
    }

    // open_holding propagates repo load failure as Application(DatabaseError).
    // Distinct from test_open_holding_returns_account_not_found (which exercises
    // Ok(None) → Application(AccountNotFound)). This covers the Err branch of
    // load_account_for_open_holding.
    #[tokio::test]
    async fn test_open_holding_returns_database_error_when_load_fails() {
        let mut mock_ar = MockAccountRepository::new();
        mock_ar
            .expect_get_with_holdings_and_transactions()
            .once()
            .returning(|_| Err(SimulatedSaveError.into()));

        let svc = AccountService::new(
            Box::new(mock_ar),
            Box::new(MockHoldingRepository::new()),
            Box::new(MockTransactionRepository::new()),
        );

        let result = svc
            .open_holding(
                "any-account-id",
                "asset-1".to_string(),
                "2024-01-01".to_string(),
                micro(1),
                micro(100),
            )
            .await;

        let err = result.unwrap_err();
        use crate::use_cases::holding_transaction::OpenHoldingError;
        assert!(
            matches!(err, OpenHoldingError::Account(AccountError::DatabaseError)),
            "open_holding must surface load failures as Application(DatabaseError), got: {err:?}"
        );
    }

    // TRX-056 — open_holding returns AccountNotFound when account does not exist
    #[tokio::test]
    async fn test_open_holding_returns_account_not_found() {
        let svc = setup_service().await;
        let err = svc
            .open_holding(
                "nonexistent-account-id",
                "some-asset-id".to_string(),
                "2024-01-01".to_string(),
                micro(1),
                micro(100),
            )
            .await
            .unwrap_err();
        use OpenHoldingError;
        assert!(
            matches!(
                err,
                OpenHoldingError::Account(AccountError::AccountNotFound { .. })
            ),
            "expected Application(AccountNotFound), got: {err:?}"
        );
    }

    // TRX-044 — open_holding propagates QuantityNotPositive through the service
    #[tokio::test]
    async fn test_open_holding_propagates_quantity_not_positive() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create("Acc".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();

        let err = svc
            .open_holding(
                &account.id,
                asset_id,
                "2024-01-01".to_string(),
                0, // quantity ≤ 0
                micro(100),
            )
            .await
            .unwrap_err();

        use crate::context::account::AccountError;
        use OpenHoldingError;
        assert!(
            matches!(
                err,
                OpenHoldingError::Account(AccountError::QuantityNotPositive)
            ),
            "expected TxValidation(QuantityNotPositive), got: {err:?}"
        );
    }

    // CSH-061 — open_holding rejects the account's cash line through the service
    #[tokio::test]
    async fn open_holding_rejects_cash_line_through_the_service() {
        use crate::context::account::AccountError;
        use OpenHoldingError;
        let pool = make_pool().await;
        let (svc, _asset_id) = setup(&pool).await;
        let account = svc
            .create("Acc".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();

        let err = svc
            .open_holding(
                &account.id,
                crate::core::cash::system_cash_asset_id("EUR"),
                "2024-01-01".to_string(),
                micro(1),
                micro(100),
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                OpenHoldingError::Account(AccountError::OpeningBalanceOnCashAsset)
            ),
            "expected Account(OpeningBalanceOnCashAsset), got: {err:?}"
        );
    }

    // TRX-045 — open_holding propagates InvalidTotalCost through the service
    #[tokio::test]
    async fn test_open_holding_propagates_invalid_total_cost() {
        use crate::context::account::AccountError;
        use OpenHoldingError;
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create("Acc".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();

        let err = svc
            .open_holding(
                &account.id,
                asset_id,
                "2024-01-01".to_string(),
                micro(1),
                -1, // negative total_cost (zero is now valid, TRX-045)
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                OpenHoldingError::Account(AccountError::InvalidTotalCost)
            ),
            "expected Validation(InvalidTotalCost), got: {err:?}"
        );
    }

    // TRX-047 — open_holding persists transaction and holding with correct fields
    #[tokio::test]
    async fn test_open_holding_persists_transaction_and_holding() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create("Acc".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();

        let tx = svc
            .open_holding(
                &account.id,
                asset_id.clone(),
                "2024-01-01".to_string(),
                micro(2),
                micro(200),
            )
            .await
            .unwrap();

        use crate::context::account::TransactionType;
        assert_eq!(tx.transaction_type, TransactionType::OpeningBalance);
        assert_eq!(tx.total_amount, micro(200), "total_amount = total_cost");
        assert_eq!(tx.fees, 0, "fees = 0");
        assert_eq!(tx.exchange_rate, 1_000_000, "exchange_rate = 1.0");
        // unit_price = floor(200_000_000 * 1_000_000 / 2_000_000) = 100_000_000
        assert_eq!(
            tx.unit_price,
            micro(100),
            "unit_price = total_cost / quantity"
        );

        let holdings = svc.get_holdings_for_account(&account.id).await.unwrap();
        assert_eq!(holdings.len(), 1);
        assert_eq!(holdings[0].quantity, micro(2));
        assert_eq!(holdings[0].average_price, micro(100));
    }

    // TRX-048 — open_holding participates in VWAP alongside Purchase
    #[tokio::test]
    async fn test_open_holding_participates_in_vwap_alongside_purchase() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create("Acc".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;

        // OpeningBalance: 2 units, total_cost = 200
        svc.open_holding(
            &account.id,
            asset_id.clone(),
            "2024-01-01".to_string(),
            micro(2),
            micro(200),
        )
        .await
        .unwrap();

        // Purchase: 2 units @ 100 → total = 200
        svc.buy_holding(
            &account.id,
            asset_id.clone(),
            "2024-02-01".to_string(),
            micro(2),
            micro(100),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let holdings = svc.get_holdings_for_account(&account.id).await.unwrap();
        let h = holdings
            .iter()
            .find(|h| h.asset_id == asset_id)
            .expect("holding must exist after VWAP test operations");
        // VWAP = (200 + 200) / 4 = 100
        assert_eq!(h.quantity, micro(4));
        assert_eq!(
            h.average_price,
            micro(100),
            "VWAP must include OpeningBalance totals"
        );
    }

    // TRX-049 — multiple open_holding entries for same pair are all persisted
    #[tokio::test]
    async fn test_open_holding_allows_multiple_for_same_pair() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create("Acc".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();

        svc.open_holding(
            &account.id,
            asset_id.clone(),
            "2023-01-01".to_string(),
            micro(1),
            micro(100),
        )
        .await
        .unwrap();
        svc.open_holding(
            &account.id,
            asset_id.clone(),
            "2023-06-01".to_string(),
            micro(2),
            micro(200),
        )
        .await
        .unwrap();

        let txs = svc.get_transactions(&account.id, &asset_id).await.unwrap();
        assert_eq!(txs.len(), 2, "both opening balance rows must be persisted");

        let holdings = svc.get_holdings_for_account(&account.id).await.unwrap();
        assert_eq!(holdings[0].quantity, micro(3), "quantities must accumulate");
    }

    // TRX-031 — correct_transaction updates the persisted holding
    #[tokio::test]
    async fn test_correct_transaction_updates_holding() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create("Acc".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;
        let tx = svc
            .buy_holding(
                &account.id,
                asset_id.clone(),
                "2024-01-01".to_string(),
                micro(2),
                micro(100),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap();
        svc.correct_transaction(
            &account.id,
            &tx.id,
            "2024-01-01".to_string(),
            micro(2),
            micro(200),
            micro(1),
            0,
            None, // total_amount (typed-total mode unused here)
            None,
        )
        .await
        .unwrap();
        let holdings = svc.get_holdings_for_account(&account.id).await.unwrap();
        let asset_holding = holdings
            .iter()
            .find(|h| h.asset_id == asset_id)
            .expect("asset holding present");
        assert_eq!(
            asset_holding.average_price,
            micro(200),
            "VWAP should update to 200"
        );
    }

    // Note: pure delegate methods — read-side (get_all, get_by_id,
    // get_holdings_for_account, get_holding_by_account_asset,
    // get_transaction_by_id, get_transactions, get_asset_ids_for_account)
    // and write-side (delete) — are exercised end-to-end against a real
    // SQLite repository in tests/account_service_crud.rs
    // (B33 — avoid trivial mock-passthrough tests).

    // CSH-100 — record_deposit and record_withdrawal publish TransactionUpdated.
    // Frontend reactivity (ACD-039, MKT-036) re-fetches on this signal.
    //
    // Pattern: do all setup first, THEN subscribe to the bus. New subscribers
    // see the latest value but `changed()` only fires on subsequent updates,
    // so this avoids racing against events emitted during setup.
    #[tokio::test]
    async fn csh_100_record_deposit_publishes_transaction_updated_event() {
        let pool = make_pool().await;
        let bus = Arc::new(SideEffectEventBus::new());
        let svc = AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus));
        let account = svc
            .create("Acc".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;

        // Subscribe AFTER setup — `changed()` fires only on the next publish.
        let mut rx = bus.subscribe();
        svc.record_deposit(&account.id, "2020-02-01".to_string(), 50_000_000, None)
            .await
            .unwrap();
        let event = rx
            .drain()
            .pop()
            .expect("TransactionUpdated event not received within 200ms");
        assert_eq!(event, Event::TransactionUpdated);
    }

    #[tokio::test]
    async fn csh_100_record_withdrawal_publishes_transaction_updated_event() {
        let pool = make_pool().await;
        let bus = Arc::new(SideEffectEventBus::new());
        let svc = AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus));
        let account = svc
            .create("Acc".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;

        // Subscribe AFTER setup so we only observe the withdrawal's event.
        let mut rx = bus.subscribe();
        svc.record_withdrawal(&account.id, "2020-02-01".to_string(), 100_000_000, None)
            .await
            .unwrap();
        let event = rx
            .drain()
            .pop()
            .expect("TransactionUpdated event not received within 200ms");
        assert_eq!(event, Event::TransactionUpdated);
    }

    // -------------------------------------------------------------------------
    // Typed cash service error paths (B34) — mock-based unit tests for
    // record_deposit / record_withdrawal covering all four typed-Result
    // variants of AccountError. Happy paths are covered by the SQLite
    // csh_100_* tests above.
    // -------------------------------------------------------------------------

    fn mock_cash_svc(ar: MockAccountRepository) -> AccountService {
        AccountService::new(
            Box::new(ar),
            Box::new(MockHoldingRepository::new()),
            Box::new(MockTransactionRepository::new()),
        )
    }

    // CSH-021 — non-positive deposit amount surfaces from `Transaction::new_deposit`
    // (the cash factory's input validation, per Rule B').
    #[tokio::test]
    async fn record_deposit_returns_amount_not_positive_on_zero() {
        let mut mock_ar = MockAccountRepository::new();
        mock_ar
            .expect_get_with_holdings_and_transactions()
            .once()
            .returning(|_| {
                Ok(Some(
                    Account::new("Test".to_string(), String::new(), "EUR".to_string(), false)
                        .unwrap(),
                ))
            });
        let svc = mock_cash_svc(mock_ar);
        let err = svc
            .record_deposit("acc", "2020-01-01".to_string(), 0, None)
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::AmountNotPositive),
            "got: {err:?}"
        );
    }

    // load_account translates Ok(None) → AccountError::AccountNotFound.
    #[tokio::test]
    async fn record_deposit_returns_account_not_found_when_repo_returns_none() {
        let mut mock_ar = MockAccountRepository::new();
        mock_ar
            .expect_get_with_holdings_and_transactions()
            .once()
            .returning(|_| Ok(None));
        let svc = mock_cash_svc(mock_ar);
        let err = svc
            .record_deposit("missing", "2020-01-01".to_string(), 100, None)
            .await
            .unwrap_err();
        match err {
            AccountError::AccountNotFound { account_id } => {
                assert_eq!(account_id, "missing");
            }
            other => panic!("expected AccountNotFound{{missing}}, got: {other:?}"),
        }
    }

    // load_account translates a repo Err → AccountError::DatabaseError.
    #[tokio::test]
    async fn record_deposit_returns_infrastructure_when_load_fails() {
        let mut mock_ar = MockAccountRepository::new();
        mock_ar
            .expect_get_with_holdings_and_transactions()
            .once()
            .returning(|_| Err(SimulatedSaveError.into()));
        let svc = mock_cash_svc(mock_ar);
        let err = svc
            .record_deposit("acc", "2020-01-01".to_string(), 100, None)
            .await
            .unwrap_err();
        assert!(matches!(err, AccountError::DatabaseError), "got: {err:?}");
    }

    // save_account translates a repo Err → AccountError::DatabaseError.
    #[tokio::test]
    async fn record_deposit_returns_infrastructure_when_save_fails() {
        let mut mock_ar = MockAccountRepository::new();
        mock_ar
            .expect_get_with_holdings_and_transactions()
            .once()
            .returning(|_| {
                let acc = Account::new("Test".to_string(), String::new(), "EUR".to_string(), false)
                    .unwrap();
                Ok(Some(acc))
            });
        mock_ar
            .expect_save()
            .once()
            .returning(|_| Err(SimulatedSaveError.into()));
        let svc = mock_cash_svc(mock_ar);
        let err = svc
            .record_deposit("acc", "2020-01-01".to_string(), 100, None)
            .await
            .unwrap_err();
        assert!(matches!(err, AccountError::DatabaseError), "got: {err:?}");
    }

    // CSH-031 — non-positive withdrawal amount surfaces from
    // `Transaction::new_withdrawal` (the cash factory's input validation).
    #[tokio::test]
    async fn record_withdrawal_returns_amount_not_positive_on_zero() {
        let mut mock_ar = MockAccountRepository::new();
        mock_ar
            .expect_get_with_holdings_and_transactions()
            .once()
            .returning(|_| {
                Ok(Some(
                    Account::new("Test".to_string(), String::new(), "EUR".to_string(), false)
                        .unwrap(),
                ))
            });
        let svc = mock_cash_svc(mock_ar);
        let err = svc
            .record_withdrawal("acc", "2020-01-01".to_string(), 0, None)
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::AmountNotPositive),
            "got: {err:?}"
        );
    }

    // load_account translates Ok(None) → AccountError::AccountNotFound.
    #[tokio::test]
    async fn record_withdrawal_returns_account_not_found_when_repo_returns_none() {
        let mut mock_ar = MockAccountRepository::new();
        mock_ar
            .expect_get_with_holdings_and_transactions()
            .once()
            .returning(|_| Ok(None));
        let svc = mock_cash_svc(mock_ar);
        let err = svc
            .record_withdrawal("missing", "2020-01-01".to_string(), 100, None)
            .await
            .unwrap_err();
        match err {
            AccountError::AccountNotFound { account_id } => {
                assert_eq!(account_id, "missing");
            }
            other => panic!("expected AccountNotFound{{missing}}, got: {other:?}"),
        }
    }

    // load_account translates a repo Err → AccountError::DatabaseError.
    #[tokio::test]
    async fn record_withdrawal_returns_infrastructure_when_load_fails() {
        let mut mock_ar = MockAccountRepository::new();
        mock_ar
            .expect_get_with_holdings_and_transactions()
            .once()
            .returning(|_| Err(SimulatedSaveError.into()));
        let svc = mock_cash_svc(mock_ar);
        let err = svc
            .record_withdrawal("acc", "2020-01-01".to_string(), 100, None)
            .await
            .unwrap_err();
        assert!(matches!(err, AccountError::DatabaseError), "got: {err:?}");
    }

    // save_account translates a repo Err → AccountError::DatabaseError.
    #[tokio::test]
    async fn record_withdrawal_returns_infrastructure_when_save_fails() {
        let mut mock_ar = MockAccountRepository::new();
        mock_ar
            .expect_get_with_holdings_and_transactions()
            .once()
            .returning(|_| {
                // Seed both a Cash Holding AND a matching Deposit Transaction
                // via `Account::restore_with_positions`. Both are required:
                // `apply_withdrawal` first checks `cash_holding_quantity()` (which
                // reads from `holdings`), then runs `replay_cash_holding()` (which
                // rebuilds the running balance from `transactions`). A holding
                // without a corresponding deposit would pass the snapshot check
                // but trip the chronological replay.
                let acc = Account::new("Test".to_string(), String::new(), "EUR".to_string(), false)
                    .unwrap();
                // CSH-080 only fails when current cash < requested amount. Seed
                // micro(1_000) (≈ €1,000) — comfortably above the test's micro(100)
                // withdrawal. Exact value isn't load-bearing; only the inequality.
                let cash_holding = Holding::restore(
                    "h-cash".to_string(),
                    acc.id.clone(),
                    acc.cash_asset_id(),
                    micro(1_000),
                    1_000_000,
                    0,
                    None,
                );
                let seed_deposit = Transaction::new_deposit(
                    acc.id.clone(),
                    acc.cash_asset_id(),
                    "2020-01-01".to_string(),
                    micro(1_000),
                    None,
                )
                .expect("seed deposit must validate");
                Ok(Some(Account::restore_with_positions(
                    acc.id,
                    acc.name,
                    acc.bank_name,
                    acc.currency,
                    acc.management_fees_enabled,
                    vec![cash_holding],
                    vec![seed_deposit],
                )))
            });
        mock_ar
            .expect_save()
            .once()
            .returning(|_| Err(SimulatedSaveError.into()));
        let svc = mock_cash_svc(mock_ar);
        let err = svc
            .record_withdrawal("acc", "2020-02-01".to_string(), micro(100), None)
            .await
            .unwrap_err();
        assert!(matches!(err, AccountError::DatabaseError), "got: {err:?}");
    }

    // -------------------------------------------------------------------------
    // Account CRUD typed-error coverage (PR 5)
    // -------------------------------------------------------------------------

    // create surfaces find_by_name repo failure as Application(DatabaseError).
    #[tokio::test]
    async fn test_create_returns_database_error_when_find_by_name_fails() {
        let mut mock_ar = MockAccountRepository::new();
        mock_ar
            .expect_find_by_name()
            .once()
            .returning(|_| Err(SimulatedSaveError.into()));
        let svc = AccountService::new(
            Box::new(mock_ar),
            Box::new(MockHoldingRepository::new()),
            Box::new(MockTransactionRepository::new()),
        );
        let err = svc
            .create("Test".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap_err();
        assert!(matches!(err, AccountError::DatabaseError), "got: {err:?}");
    }

    // create surfaces repo.create failure (after passing the uniqueness
    // pre-check) as Application(DatabaseError).
    #[tokio::test]
    async fn test_create_returns_database_error_when_repo_create_fails() {
        let mut mock_ar = MockAccountRepository::new();
        mock_ar.expect_find_by_name().once().returning(|_| Ok(None));
        mock_ar
            .expect_create()
            .once()
            .returning(|_| Err(SimulatedSaveError.into()));
        let svc = AccountService::new(
            Box::new(mock_ar),
            Box::new(MockHoldingRepository::new()),
            Box::new(MockTransactionRepository::new()),
        );
        let err = svc
            .create("Test".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap_err();
        assert!(matches!(err, AccountError::DatabaseError), "got: {err:?}");
    }

    // delete surfaces repo failure as AccountError::DatabaseError.
    #[tokio::test]
    async fn test_delete_returns_database_error_when_repo_fails() {
        let mut mock_ar = MockAccountRepository::new();
        mock_ar
            .expect_delete()
            .once()
            .returning(|_| Err(SimulatedSaveError.into()));
        let mut mock_tr = MockTransactionRepository::new();
        mock_tr
            .expect_get_all_for_account()
            .returning(|_| Ok(vec![]));
        let svc = AccountService::new(
            Box::new(mock_ar),
            Box::new(MockHoldingRepository::new()),
            Box::new(mock_tr),
        );
        let err = svc.delete("any-id").await.unwrap_err();
        assert!(matches!(err, AccountError::DatabaseError), "got: {err:?}");
    }

    // get_by_id translates raw repo failure to AccountError::DatabaseError.
    #[tokio::test]
    async fn get_by_id_translates_repo_failure_to_database_error() {
        let mut mock_ar = MockAccountRepository::new();
        mock_ar
            .expect_get_by_id()
            .once()
            .returning(|_| Err(SimulatedSaveError.into()));
        let svc = AccountService::new(
            Box::new(mock_ar),
            Box::new(MockHoldingRepository::new()),
            Box::new(MockTransactionRepository::new()),
        );
        let err = svc.get_by_id("any-id").await.unwrap_err();
        assert!(matches!(err, AccountError::DatabaseError), "got: {err:?}");
    }

    // get_holdings_for_account translates raw repo failure to AccountError::DatabaseError.
    #[tokio::test]
    async fn get_holdings_for_account_translates_repo_failure_to_database_error() {
        let mut mock_hr = MockHoldingRepository::new();
        mock_hr
            .expect_get_by_account()
            .once()
            .returning(|_| Err(SimulatedSaveError.into()));
        let svc = AccountService::new(
            Box::new(MockAccountRepository::new()),
            Box::new(mock_hr),
            Box::new(MockTransactionRepository::new()),
        );
        let err = svc.get_holdings_for_account("any-id").await.unwrap_err();
        assert!(matches!(err, AccountError::DatabaseError), "got: {err:?}");
    }

    // -------------------------------------------------------------------------
    // FSD-022/026 — record_free_shares service method
    // -------------------------------------------------------------------------

    // FSD-022 — record_free_shares persists the transaction and updates the
    // holding: quantity increases, cost basis unchanged (VWAP dilutes).
    #[tokio::test]
    async fn fsd_022_record_free_shares_persists_transaction_and_updates_holding() {
        // FSD-022 — end-to-end through AccountService (real SQLite)
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "FSD Account".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;

        // Buy 10 units @ 100
        svc.buy_holding(
            &account.id,
            asset_id.clone(),
            "2024-01-01".to_string(),
            micro(10),
            micro(100),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let holdings_before = svc.get_holdings_for_account(&account.id).await.unwrap();
        let cost_basis_before = holdings_before
            .iter()
            .find(|h| h.asset_id == asset_id)
            .map(|h| h.quantity as i128 * h.average_price as i128 / 1_000_000)
            .unwrap();

        // Record 5 free shares
        let tx = svc
            .record_free_shares(
                &account.id,
                asset_id.clone(),
                "2024-06-15".to_string(),
                micro(5),
                None,
            )
            .await
            .unwrap();

        // FSD-022 — returned Transaction must carry the FreeShares type and correct fields
        assert_eq!(
            tx.transaction_type,
            crate::context::account::TransactionType::FreeShares
        );
        assert_eq!(tx.asset_id, asset_id);
        assert_eq!(tx.quantity, micro(5));
        assert_eq!(tx.unit_price, 0, "unit_price must be 0 (FSD-023)");
        assert_eq!(
            tx.exchange_rate, 1_000_000,
            "exchange_rate must be 1_000_000"
        );
        assert_eq!(tx.fees, 0, "fees must be 0");
        assert_eq!(tx.total_amount, 0, "total_amount must be 0 (FSD-023)");
        assert!(tx.realized_pnl.is_none(), "realized_pnl must be None");

        let holdings_after = svc.get_holdings_for_account(&account.id).await.unwrap();
        let holding_after = holdings_after
            .iter()
            .find(|h| h.asset_id == asset_id)
            .unwrap();

        // FSD-022a — quantity increased
        assert_eq!(
            holding_after.quantity,
            micro(15),
            "quantity must be 15 after 10 + 5 free"
        );
        // FSD-023 — underlying cost unchanged → VWAP dilutes to the exact floored
        // value (TRX-026 floor convention).
        let expected_diluted_vwap =
            (cost_basis_before * 1_000_000 / holding_after.quantity as i128) as i64;
        assert_eq!(
            holding_after.average_price, expected_diluted_vwap,
            "average price must equal floor(cost_basis / new_quantity) after free-share distribution"
        );
    }

    // FSD-026 — record_free_shares publishes TransactionUpdated event on success.
    #[tokio::test]
    async fn fsd_026_record_free_shares_publishes_transaction_updated_event() {
        // FSD-026 — TransactionUpdated must be emitted after a successful distribution
        let pool = make_pool().await;
        let bus = Arc::new(SideEffectEventBus::new());
        let svc = AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus));

        let (_, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "FSD Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;
        svc.buy_holding(
            &account.id,
            asset_id.clone(),
            "2024-01-01".to_string(),
            micro(10),
            micro(100),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        // Subscribe AFTER setup so only the free-shares event is observed.
        let mut rx = bus.subscribe();
        svc.record_free_shares(
            &account.id,
            asset_id.clone(),
            "2024-06-15".to_string(),
            micro(5),
            None,
        )
        .await
        .unwrap();

        let event = rx
            .drain()
            .pop()
            .expect("TransactionUpdated event not received within 200ms");
        use crate::core::event_bus::Event;
        assert_eq!(
            event,
            Event::TransactionUpdated,
            "record_free_shares must publish TransactionUpdated (FSD-026)"
        );
    }

    // INT-025/050 — record_interest publishes TransactionUpdated on success; the
    // account has management_fees_enabled = false, proving the interest path is
    // independent of the fee parameter (INT-050).
    #[tokio::test]
    async fn int_025_record_interest_publishes_transaction_updated_event() {
        let pool = make_pool().await;
        let bus = Arc::new(SideEffectEventBus::new());
        let svc = AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus));

        let (_, asset_id) = setup(&pool).await;
        // INT-050 — fees stay disabled; interest must work regardless.
        let account = svc
            .create(
                "INT Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;
        svc.buy_holding(
            &account.id,
            asset_id.clone(),
            "2024-01-01".to_string(),
            micro(10),
            micro(100),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        // Subscribe AFTER setup so only the interest event is observed.
        let mut rx = bus.subscribe();
        svc.record_interest(
            &account.id,
            asset_id.clone(),
            "2024-06-15".to_string(),
            None,
            Some(micro(1)),
            None,
        )
        .await
        .unwrap();

        let event = rx
            .drain()
            .pop()
            .expect("TransactionUpdated event not received within 200ms");
        use crate::core::event_bus::Event;
        assert_eq!(
            event,
            Event::TransactionUpdated,
            "record_interest must publish TransactionUpdated (INT-025)"
        );
    }

    // FSD-022 — record_free_shares propagates save failure as Application(DatabaseError).
    #[tokio::test]
    async fn fsd_022_record_free_shares_returns_database_error_when_save_fails() {
        // FSD-022 — Unit-of-Work failure surface
        let mut mock_ar = MockAccountRepository::new();
        mock_ar
            .expect_get_with_holdings_and_transactions()
            .once()
            .returning(|_| {
                // Seed a holding so the apply_free_shares call reaches save()
                let mut acc =
                    Account::new("Test".to_string(), String::new(), "EUR".to_string(), false)
                        .unwrap();
                // Seed cash first (CSH-041 — a buy needs sufficient cash), then a buy
                // so the holding exists.
                acc.record_deposit("2024-01-01".to_string(), micro(1_000), None)
                    .expect("seed deposit");
                acc.buy_holding(
                    "asset-1".to_string(),
                    "2024-01-01".to_string(),
                    micro(5),
                    micro(100),
                    micro(1),
                    0,
                    None,
                    None,
                )
                .expect("seed buy");
                acc.pending_changes.clear();
                Ok(Some(acc))
            });
        mock_ar
            .expect_save()
            .once()
            .returning(|_| Err(SimulatedSaveError.into()));

        let svc = AccountService::new(
            Box::new(mock_ar),
            Box::new(MockHoldingRepository::new()),
            Box::new(MockTransactionRepository::new()),
        );

        let err = svc
            .record_free_shares(
                "any-id",
                "asset-1".to_string(),
                "2024-06-15".to_string(),
                micro(3),
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                AccountError::DatabaseError
            ),
            "record_free_shares must surface save failures as Application(DatabaseError), got: {err:?}"
        );
    }

    // -------------------------------------------------------------------------
    // FEE-012/021/022/023/027 — record_management_fee service method
    // -------------------------------------------------------------------------

    // FEE-012 — record_management_fee persists the transaction and updates the holding:
    // quantity decreases, cost basis unchanged (VWAP concentrates).
    #[tokio::test]
    async fn fee_012_record_management_fee_persists_transaction_and_updates_holding() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "FEE Account".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&svc, &account).await;
        seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;

        // Buy 100 units @ 50 (total cost = 5000 in account currency).
        svc.buy_holding(
            &account.id,
            asset_id.clone(),
            "2024-01-01".to_string(),
            micro(100),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        // Record a 1% management fee (1_000_000 micro-percent).
        // Expected removed qty = floor(100_000_000 × 1_000_000 / 100_000_000) = 1_000_000 (1 unit).
        let tx = svc
            .record_management_fee(
                &account.id,
                asset_id.clone(),
                "2024-06-30".to_string(),
                1_000_000,
                None,
            )
            .await
            .unwrap();

        // FEE-012 — returned Transaction must carry ManagementFee type and zero-cost packing.
        assert_eq!(
            tx.transaction_type,
            crate::context::account::TransactionType::ManagementFee
        );
        assert_eq!(tx.asset_id, asset_id);
        assert_eq!(tx.unit_price, 0, "unit_price must be 0 (FEE-023)");
        assert_eq!(
            tx.exchange_rate, 1_000_000,
            "exchange_rate must be 1_000_000"
        );
        assert_eq!(tx.fees, 0, "fees must be 0");
        assert_eq!(tx.total_amount, 0, "total_amount must be 0 (FEE-023)");
        assert!(tx.realized_pnl.is_none(), "realized_pnl must be None");

        let holdings_after = svc.get_holdings_for_account(&account.id).await.unwrap();
        let holding_after = holdings_after
            .iter()
            .find(|h| h.asset_id == asset_id)
            .expect("holding for asset_id must exist after management fee deduction");

        // FEE-023 — VWAP concentrates: qty decreases, cost basis unchanged.
        assert!(
            holding_after.quantity < micro(100),
            "quantity must decrease after fee deduction"
        );
    }

    // FEE-050 — a fee deduction reduces quantity but leaves the recorded cost
    // basis UNCHANGED, so the average cost per share rises (VWAP concentrates).
    #[tokio::test]
    async fn fee_050_management_fee_preserves_cost_basis_and_raises_average_price() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "FEE-050".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&svc, &account).await;
        seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;

        // Buy 100 units @ 50 → cost basis = 100 × 50 = 5000 (account currency).
        svc.buy_holding(
            &account.id,
            asset_id.clone(),
            "2024-01-01".to_string(),
            micro(100),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let holdings_before = svc.get_holdings_for_account(&account.id).await.unwrap();
        let holding_before = holdings_before
            .iter()
            .find(|h| h.asset_id == asset_id)
            .expect("holding must exist after buy")
            .clone();
        let cost_basis_before =
            holding_before.quantity as i128 * holding_before.average_price as i128 / 1_000_000;

        // Record a 1% management fee — removes floor(100 × 1%) = 1 unit.
        svc.record_management_fee(
            &account.id,
            asset_id.clone(),
            "2024-06-30".to_string(),
            1_000_000,
            None,
        )
        .await
        .unwrap();

        let holdings_after = svc.get_holdings_for_account(&account.id).await.unwrap();
        let holding_after = holdings_after
            .iter()
            .find(|h| h.asset_id == asset_id)
            .expect("holding must still exist after fee deduction")
            .clone();
        let cost_basis_after =
            holding_after.quantity as i128 * holding_after.average_price as i128 / 1_000_000;

        // FEE-050 — quantity dropped …
        assert!(
            holding_after.quantity < holding_before.quantity,
            "quantity must decrease after a fee deduction (FEE-050)"
        );
        // … the recorded cost basis is left UNCHANGED — the average price absorbs the
        // change by concentrating to floor(cost_basis / new_quantity) (FEE-023, TRX-026
        // floor convention). The stored VWAP must equal that exact floored value.
        let expected_concentrated_vwap =
            (cost_basis_before * 1_000_000 / holding_after.quantity as i128) as i64;
        assert_eq!(
            holding_after.average_price, expected_concentrated_vwap,
            "average price must equal floor(cost_basis / new_quantity) — cost basis unchanged (FEE-050/023)"
        );
        // … so the average cost per share rises (VWAP concentrates).
        assert!(
            holding_after.average_price > holding_before.average_price,
            "average price must rise as the cost basis concentrates over fewer shares (FEE-050)"
        );
        // The cost basis is preserved up to the per-share floor error (< 1 micro-unit
        // per remaining share); it can only round DOWN, never up.
        let shares_after = holding_after.quantity / 1_000_000;
        assert!(
            cost_basis_after <= cost_basis_before
                && cost_basis_after > cost_basis_before - (shares_after as i128 + 1),
            "cost basis must be unchanged within the floor tolerance (FEE-050): before={cost_basis_before}, after={cost_basis_after}"
        );
    }

    // FEE-021 — PercentageNotPositive when percent_micros <= 0.
    #[tokio::test]
    async fn fee_021_record_management_fee_rejects_zero_percent() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "FEE Zero".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();

        let err = svc
            .record_management_fee(
                &account.id,
                asset_id.clone(),
                "2024-06-30".to_string(),
                0,
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::PercentageNotPositive),
            "expected PercentageNotPositive, got: {err:?}"
        );
    }

    // FEE-021 — PercentageAboveHundred when percent_micros > 100_000_000.
    #[tokio::test]
    async fn fee_021_record_management_fee_rejects_above_hundred_percent() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "FEE Above100".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();

        let err = svc
            .record_management_fee(
                &account.id,
                asset_id.clone(),
                "2024-06-30".to_string(),
                100_000_001,
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::PercentageAboveHundred),
            "expected PercentageAboveHundred, got: {err:?}"
        );
    }

    // FEE-022 — AccountNotFound when the account does not exist.
    #[tokio::test]
    async fn fee_022_record_management_fee_rejects_unknown_account() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;

        let err = svc
            .record_management_fee(
                "nonexistent-account",
                asset_id.clone(),
                "2024-06-30".to_string(),
                1_000_000,
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::AccountNotFound { .. }),
            "expected AccountNotFound, got: {err:?}"
        );
    }

    // FEE-027 — record_management_fee propagates save failure as DatabaseError.
    #[tokio::test]
    async fn fee_027_record_management_fee_returns_database_error_when_save_fails() {
        let mut mock_ar = MockAccountRepository::new();
        mock_ar
            .expect_get_with_holdings_and_transactions()
            .once()
            .returning(|_| {
                let mut acc =
                    Account::new("Test".to_string(), String::new(), "EUR".to_string(), false)
                        .unwrap();
                acc.management_fees_enabled = true;
                acc.record_deposit("2024-01-01".to_string(), micro(1_000), None)
                    .expect("seed deposit");
                acc.buy_holding(
                    "asset-1".to_string(),
                    "2024-01-01".to_string(),
                    micro(10),
                    micro(100),
                    micro(1),
                    0,
                    None,
                    None,
                )
                .expect("seed buy");
                acc.pending_changes.clear();
                Ok(Some(acc))
            });
        mock_ar
            .expect_save()
            .once()
            .returning(|_| Err(SimulatedSaveError.into()));

        let svc = AccountService::new(
            Box::new(mock_ar),
            Box::new(MockHoldingRepository::new()),
            Box::new(MockTransactionRepository::new()),
        );

        let err = svc
            .record_management_fee(
                "any-id",
                "asset-1".to_string(),
                "2024-06-30".to_string(),
                1_000_000,
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::DatabaseError),
            "record_management_fee must surface save failures as DatabaseError, got: {err:?}"
        );
    }

    // FEE-027 — a one-off fee dated before a later Sell is rejected when the
    // removal would starve that Sell in chronological replay (CascadingOversell).
    #[tokio::test]
    async fn fee_027_record_management_fee_rejects_one_off_downstream_oversell() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "FEE-027".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&svc, &account).await;
        seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;

        // Buy 100 on 2024-01-01, then sell all 100 on 2024-03-01 (closes the position).
        svc.buy_holding(
            &account.id,
            asset_id.clone(),
            "2024-01-01".to_string(),
            micro(100),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();
        svc.sell_holding(
            &account.id,
            asset_id.clone(),
            "2024-03-01".to_string(),
            micro(100),
            micro(60),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        // A 10% fee on 2024-02-01 would remove 10 units, leaving 90 — the later
        // 2024-03-01 sell of 100 can then no longer source its quantity.
        let err = svc
            .record_management_fee(
                &account.id,
                asset_id.clone(),
                "2024-02-01".to_string(),
                10_000_000,
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::CascadingOversell),
            "a one-off fee that starves a later sell must be rejected with CascadingOversell, got: {err:?}"
        );
    }

    /// An account with management fees enabled, cash, and `quantity` of the asset bought
    /// on 2024-01-01.
    async fn account_holding(
        pool: &sqlx::Pool<sqlx::Sqlite>,
        svc: &AccountService,
        asset_id: &str,
        name: &str,
        quantity: i64,
    ) -> Account {
        let account = svc
            .create(name.to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        let account = enable_management_fees(svc, &account).await;
        seed_cash_for_account(pool, svc, &account.id, "EUR").await;
        svc.buy_holding(
            &account.id,
            asset_id.to_string(),
            "2024-01-01".to_string(),
            quantity,
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();
        account
    }

    // SPL-012 — a one-off fee that removes the whole position before a later split leaves
    // that split nothing to rescale: the fee is refused with the split's own code.
    #[tokio::test]
    async fn spl_012_a_fee_that_empties_the_position_before_a_split_is_refused() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = account_holding(&pool, &svc, &asset_id, "SPL-012-fee", micro(100)).await;
        svc.record_split(
            &account.id,
            asset_id.clone(),
            "2024-03-01".to_string(),
            2_000_000,
            None,
        )
        .await
        .unwrap();

        let err = svc
            .record_management_fee(
                &account.id,
                asset_id.clone(),
                "2024-02-01".to_string(),
                100_000_000,
                None,
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AccountError::ClosedPosition), "{err:?}");
    }

    // SPL-021 — a one-off fee, or a back-dated sale, that leaves a later reverse split a
    // position it floors to nothing is refused with the split's own code.
    #[tokio::test]
    async fn spl_021_a_fee_or_a_sale_before_a_split_that_then_collapses_is_refused() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = account_holding(&pool, &svc, &asset_id, "SPL-021-fee", 3).await;
        svc.record_split(
            &account.id,
            asset_id.clone(),
            "2024-03-01".to_string(),
            400_000,
            None,
        )
        .await
        .unwrap();

        let fee = svc
            .record_management_fee_by_resulting_quantity(
                &account.id,
                asset_id.clone(),
                "2024-02-01".to_string(),
                1,
                None,
            )
            .await
            .unwrap_err();
        let sale = svc
            .sell_holding(
                &account.id,
                asset_id.clone(),
                "2024-02-01".to_string(),
                1,
                micro(60),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap_err();

        assert!(
            matches!(fee, AccountError::SplitCollapsesPosition),
            "{fee:?}"
        );
        assert!(
            matches!(sale, AccountError::SplitCollapsesPosition),
            "{sale:?}"
        );
    }

    // DIV-021 — a dividend whose amount, converted, floors to nothing is refused.
    #[tokio::test]
    async fn div_021_a_dividend_that_converts_to_nothing_is_refused() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = account_holding(&pool, &svc, &asset_id, "DIV-021", micro(100)).await;

        let err = svc
            .record_dividend(
                &account.id,
                asset_id.clone(),
                "2024-02-01".to_string(),
                1,
                500_000,
                None,
            )
            .await
            .unwrap_err();

        assert!(
            matches!(err, AccountError::TotalAmountNotPositive),
            "{err:?}"
        );
    }

    // -------------------------------------------------------------------------
    // INT-021/022/023/024 — record_interest service method
    // -------------------------------------------------------------------------

    // INT-022 — percent mode: credited qty = floor(holding_qty_as_of(date) × percent / 100%);
    // the holding gains the credit at zero cost (VWAP dilutes, INT-024).
    #[tokio::test]
    async fn int_022_record_interest_percent_mode_credits_and_dilutes() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "INT Account".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;

        // Buy 100 units @ 50.
        svc.buy_holding(
            &account.id,
            asset_id.clone(),
            "2024-01-01".to_string(),
            micro(100),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let holdings_before = svc.get_holdings_for_account(&account.id).await.unwrap();
        let holding_before = holdings_before
            .iter()
            .find(|h| h.asset_id == asset_id)
            .unwrap()
            .clone();
        let cost_basis_before =
            holding_before.quantity as i128 * holding_before.average_price as i128 / 1_000_000;

        // Record a 10% interest (10_000_000 micro-percent).
        // Expected credited qty = floor(100_000_000 × 10_000_000 / 100_000_000) = 10 units.
        let tx = svc
            .record_interest(
                &account.id,
                asset_id.clone(),
                "2024-12-31".to_string(),
                Some(10_000_000),
                None,
                None,
            )
            .await
            .unwrap();

        // INT-024 — returned Transaction must carry the Interest type and zero-cost packing.
        assert_eq!(
            tx.transaction_type,
            crate::context::account::TransactionType::Interest
        );
        assert_eq!(tx.asset_id, asset_id);
        assert_eq!(tx.quantity, micro(10), "credited qty must be 10% of 100");
        assert_eq!(tx.unit_price, 0, "unit_price must be 0 (INT-024)");
        assert_eq!(
            tx.exchange_rate, 1_000_000,
            "exchange_rate must be 1_000_000"
        );
        assert_eq!(tx.fees, 0, "fees must be 0");
        assert_eq!(tx.total_amount, 0, "total_amount must be 0 (INT-024)");
        assert!(tx.realized_pnl.is_none(), "realized_pnl must be None");

        let holdings_after = svc.get_holdings_for_account(&account.id).await.unwrap();
        let holding_after = holdings_after
            .iter()
            .find(|h| h.asset_id == asset_id)
            .unwrap();
        assert_eq!(
            holding_after.quantity,
            micro(110),
            "quantity must be 110 after 100 + 10 interest"
        );
        // INT-024 — cost basis unchanged → VWAP dilutes to floor(cost_basis / new_quantity).
        let expected_diluted_vwap =
            (cost_basis_before * 1_000_000 / holding_after.quantity as i128) as i64;
        assert_eq!(
            holding_after.average_price, expected_diluted_vwap,
            "average price must equal floor(cost_basis / new_quantity) after the interest credit"
        );
    }

    // INT-021 — quantity mode: the provided quantity is credited directly.
    #[tokio::test]
    async fn int_021_record_interest_quantity_mode_credits_units() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "INT Qty".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;
        svc.buy_holding(
            &account.id,
            asset_id.clone(),
            "2024-01-01".to_string(),
            micro(100),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let tx = svc
            .record_interest(
                &account.id,
                asset_id.clone(),
                "2024-12-31".to_string(),
                None,
                Some(micro(5)),
                None,
            )
            .await
            .unwrap();
        assert_eq!(tx.quantity, micro(5));

        let holdings_after = svc.get_holdings_for_account(&account.id).await.unwrap();
        let holding_after = holdings_after
            .iter()
            .find(|h| h.asset_id == asset_id)
            .unwrap();
        assert_eq!(
            holding_after.quantity,
            micro(105),
            "quantity must be 105 after 100 + 5 interest"
        );
    }

    // INT-021 — both or neither of percent / quantity → InterestAmountInvalid.
    #[tokio::test]
    async fn int_021_record_interest_rejects_both_and_neither() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "INT XOR".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();

        let err = svc
            .record_interest(
                &account.id,
                asset_id.clone(),
                "2024-12-31".to_string(),
                Some(1_000_000),
                Some(micro(5)),
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::InterestAmountInvalid),
            "both provided must be rejected, got: {err:?}"
        );

        let err = svc
            .record_interest(
                &account.id,
                asset_id.clone(),
                "2024-12-31".to_string(),
                None,
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::InterestAmountInvalid),
            "neither provided must be rejected, got: {err:?}"
        );
    }

    // INT-023 — quantity-mode interest on the Cash Asset credits the cash balance:
    // deposit 1000, interest 50 → 1050.
    #[tokio::test]
    async fn int_023_record_interest_credits_cash_line() {
        let pool = make_pool().await;
        let (svc, _asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "INT Cash".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        // Seed the system Cash Asset row (FK target), then deposit 1000.
        sqlx::query(
            "INSERT OR IGNORE INTO categories (id, name, is_deleted) VALUES ('system-cash-category', 'cash', 0)",
        )
        .execute(&pool)
        .await
        .expect("seed cash category");
        sqlx::query(
            "INSERT OR IGNORE INTO assets (id, name, reference, asset_class, category_id, currency, risk_level) \
             VALUES ('system-cash-eur', 'Cash EUR', 'EUR', 'Cash', 'system-cash-category', 'EUR', 1)",
        )
        .execute(&pool)
        .await
        .expect("seed cash asset");
        svc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();

        svc.record_interest(
            &account.id,
            "system-cash-eur".to_string(),
            "2024-06-15".to_string(),
            None,
            Some(micro(50)),
            None,
        )
        .await
        .unwrap();

        let holdings = svc.get_holdings_for_account(&account.id).await.unwrap();
        let cash = holdings
            .iter()
            .find(|h| h.asset_id == "system-cash-eur")
            .expect("cash holding must exist");
        assert_eq!(
            cash.quantity,
            micro(1_050),
            "cash balance must be 1000 + 50 after the interest credit (INT-023)"
        );
    }

    // -------------------------------------------------------------------------
    // FEE-030/031/032/033/034 — create_fee_schedule service method
    // -------------------------------------------------------------------------

    // FEE-030 — create_fee_schedule persists the schedule and returns it.
    #[tokio::test]
    async fn fee_030_create_fee_schedule_returns_schedule() {
        use crate::context::account::FeeFrequency;
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "Schedule Acct".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&svc, &account).await;

        let schedule = svc
            .create_fee_schedule(
                &account.id,
                asset_id.clone(),
                1_000_000, // 1%
                FeeFrequency::Monthly,
                "2024-01-01".to_string(),
                None,
            )
            .await
            .unwrap();

        assert_eq!(schedule.account_id, account.id);
        assert_eq!(schedule.asset_id, asset_id);
        assert_eq!(schedule.annual_rate_percent_micros, 1_000_000);
        assert!(matches!(schedule.frequency, FeeFrequency::Monthly));
        assert!(schedule.active);
        assert!(schedule.last_applied_period.is_none());
    }

    // FEE-033 — creating a schedule persists it but removes NO shares; the holding
    // quantity is unchanged after create_fee_schedule (deductions come only from
    // generation, FEE-04x).
    #[tokio::test]
    async fn fee_033_create_fee_schedule_removes_no_shares() {
        use crate::context::account::FeeFrequency;
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "FEE-033".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&svc, &account).await;
        seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;

        // Establish a holding of 100 units.
        svc.buy_holding(
            &account.id,
            asset_id.clone(),
            "2024-01-01".to_string(),
            micro(100),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let quantity_before = svc
            .get_holdings_for_account(&account.id)
            .await
            .unwrap()
            .iter()
            .find(|h| h.asset_id == asset_id)
            .expect("holding must exist after buy")
            .quantity;

        // Create a schedule with a start_date well in the past — if creation itself
        // removed shares (or eagerly generated), the quantity would drop here.
        svc.create_fee_schedule(
            &account.id,
            asset_id.clone(),
            1_000_000, // 1%
            FeeFrequency::Monthly,
            "2024-01-01".to_string(),
            None,
        )
        .await
        .unwrap();

        let quantity_after = svc
            .get_holdings_for_account(&account.id)
            .await
            .unwrap()
            .iter()
            .find(|h| h.asset_id == asset_id)
            .expect("holding must still exist after create_fee_schedule")
            .quantity;

        // FEE-033 — schedule creation removes no shares.
        assert_eq!(
            quantity_after, quantity_before,
            "create_fee_schedule must not remove any shares (FEE-033)"
        );
    }

    // FEE-031 — ScheduleAlreadyExists when a schedule exists for the (account, asset) pair.
    #[tokio::test]
    async fn fee_031_create_fee_schedule_rejects_duplicate() {
        use crate::context::account::FeeFrequency;
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "Dup Sched".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&svc, &account).await;

        svc.create_fee_schedule(
            &account.id,
            asset_id.clone(),
            1_000_000,
            FeeFrequency::Monthly,
            "2024-01-01".to_string(),
            None,
        )
        .await
        .unwrap();

        let err = svc
            .create_fee_schedule(
                &account.id,
                asset_id.clone(),
                2_000_000,
                FeeFrequency::Quarterly,
                "2024-01-01".to_string(),
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::ScheduleAlreadyExists),
            "expected ScheduleAlreadyExists, got: {err:?}"
        );
    }

    // FEE-032 — RateNotPositive when annual_rate_percent_micros <= 0.
    #[tokio::test]
    async fn fee_032_create_fee_schedule_rejects_zero_rate() {
        use crate::context::account::FeeFrequency;
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "Rate Zero".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&svc, &account).await;

        let err = svc
            .create_fee_schedule(
                &account.id,
                asset_id.clone(),
                0,
                FeeFrequency::Monthly,
                "2024-01-01".to_string(),
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::RateNotPositive),
            "expected RateNotPositive, got: {err:?}"
        );
    }

    // FEE-032 — RateAboveHundred when rate > 100_000_000.
    #[tokio::test]
    async fn fee_032_create_fee_schedule_rejects_rate_above_hundred() {
        use crate::context::account::FeeFrequency;
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "Rate High".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&svc, &account).await;

        let err = svc
            .create_fee_schedule(
                &account.id,
                asset_id.clone(),
                100_000_001,
                FeeFrequency::Monthly,
                "2024-01-01".to_string(),
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::RateAboveHundred),
            "expected RateAboveHundred, got: {err:?}"
        );
    }

    // FEE-032 — EndBeforeStart when end_date <= start_date.
    #[tokio::test]
    async fn fee_032_create_fee_schedule_rejects_end_before_start() {
        use crate::context::account::FeeFrequency;
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "End Before".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&svc, &account).await;

        let err = svc
            .create_fee_schedule(
                &account.id,
                asset_id.clone(),
                1_000_000,
                FeeFrequency::Monthly,
                "2024-12-01".to_string(),
                Some("2024-01-01".to_string()),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::EndBeforeStart),
            "expected EndBeforeStart, got: {err:?}"
        );
    }

    // -------------------------------------------------------------------------
    // FEE-060/061 — update_fee_schedule service method
    // -------------------------------------------------------------------------

    // FEE-060 — update_fee_schedule returns the updated schedule.
    #[tokio::test]
    async fn fee_060_update_fee_schedule_returns_updated_schedule() {
        use crate::context::account::FeeFrequency;
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "Update Sched".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&svc, &account).await;

        svc.create_fee_schedule(
            &account.id,
            asset_id.clone(),
            1_000_000,
            FeeFrequency::Monthly,
            "2024-01-01".to_string(),
            None,
        )
        .await
        .unwrap();

        let updated = svc
            .update_fee_schedule(
                &account.id,
                &asset_id,
                2_000_000,
                Some("2025-12-31".to_string()),
                true,
            )
            .await
            .unwrap();

        assert_eq!(updated.annual_rate_percent_micros, 2_000_000);
        assert_eq!(updated.end_date, Some("2025-12-31".to_string()));
        assert!(updated.active);
        // FEE-061 — frequency and start_date are NOT changed by update.
        assert!(matches!(updated.frequency, FeeFrequency::Monthly));
        assert_eq!(updated.start_date, "2024-01-01");
    }

    // FEE-060 — ScheduleNotFound when no schedule exists for the pair.
    #[tokio::test]
    async fn fee_060_update_fee_schedule_rejects_missing_schedule() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "NoSched".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();

        let err = svc
            .update_fee_schedule(&account.id, &asset_id, 1_000_000, None, true)
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::ScheduleNotFound),
            "expected ScheduleNotFound, got: {err:?}"
        );
    }

    // -------------------------------------------------------------------------
    // FEE-062 — delete_fee_schedule service method
    // -------------------------------------------------------------------------

    // FEE-062 — delete_fee_schedule succeeds and the schedule is gone.
    #[tokio::test]
    async fn fee_062_delete_fee_schedule_removes_schedule() {
        use crate::context::account::FeeFrequency;
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "Del Sched".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&svc, &account).await;

        svc.create_fee_schedule(
            &account.id,
            asset_id.clone(),
            1_000_000,
            FeeFrequency::Monthly,
            "2024-01-01".to_string(),
            None,
        )
        .await
        .unwrap();

        svc.delete_fee_schedule(&account.id, &asset_id)
            .await
            .unwrap();

        let found = svc.get_fee_schedule(&account.id, &asset_id).await.unwrap();
        assert!(found.is_none(), "schedule must be absent after delete");
    }

    // FEE-062 — delete_fee_schedule is a no-op when no schedule exists (silent).
    #[tokio::test]
    async fn fee_062_delete_fee_schedule_is_silent_on_missing() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "Del None".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();

        // Should not error when no schedule exists.
        svc.delete_fee_schedule(&account.id, &asset_id)
            .await
            .unwrap();
    }

    // -------------------------------------------------------------------------
    // FEE-030 (read) — get_fee_schedule service method
    // -------------------------------------------------------------------------

    // FEE-030 — get_fee_schedule returns None when no schedule exists.
    #[tokio::test]
    async fn fee_030_get_fee_schedule_returns_none_when_absent() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "Get None".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();

        let found = svc.get_fee_schedule(&account.id, &asset_id).await.unwrap();
        assert!(found.is_none());
    }

    // FEE-030 — get_fee_schedule returns the schedule when it exists.
    #[tokio::test]
    async fn fee_030_get_fee_schedule_returns_schedule_when_present() {
        use crate::context::account::FeeFrequency;
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "Get Present".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&svc, &account).await;

        svc.create_fee_schedule(
            &account.id,
            asset_id.clone(),
            1_500_000,
            FeeFrequency::Quarterly,
            "2024-01-01".to_string(),
            None,
        )
        .await
        .unwrap();

        let found = svc.get_fee_schedule(&account.id, &asset_id).await.unwrap();
        assert!(found.is_some());
        let schedule = found.unwrap();
        assert_eq!(schedule.annual_rate_percent_micros, 1_500_000);
        assert!(matches!(schedule.frequency, FeeFrequency::Quarterly));
    }

    // -------------------------------------------------------------------------
    // HNO-010/011/020/021/040 — holding note CRUD
    // -------------------------------------------------------------------------

    /// Creates an account, funds it, and buys the setup asset so the
    /// (account, asset) pair has transaction history (HNO-011 eligibility).
    async fn make_account_with_held_asset(
        pool: &sqlx::Pool<sqlx::Sqlite>,
        svc: &AccountService,
        asset_id: &str,
    ) -> Account {
        let account = svc
            .create("Noted".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        seed_cash_for_account(pool, svc, &account.id, "EUR").await;
        svc.buy_holding(
            &account.id,
            asset_id.to_string(),
            "2024-01-15".to_string(),
            1_000_000,
            100_000_000,
            1_000_000,
            0,
            None,
            None,
        )
        .await
        .unwrap();
        account
    }

    // HNO-020/021/040 — upsert → get → full replace → delete → delete-again no-op
    #[tokio::test]
    async fn hno_020_holding_note_round_trip() {
        use crate::context::account::ThresholdDirection;
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = make_account_with_held_asset(&pool, &svc, &asset_id).await;

        // Upsert stores the trimmed text (HNO-011).
        let note = svc
            .upsert_holding_note(
                &account.id,
                asset_id.clone(),
                "  buy 7 below 150  ".to_string(),
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(note.text, "buy 7 below 150");

        let notes = svc.get_holding_notes(&account.id).await.unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].asset_id, asset_id);
        assert_eq!(notes[0].text, "buy 7 below 150");
        assert!(notes[0].threshold_price.is_none());
        assert!(notes[0].threshold_direction.is_none());

        // HNO-020 — upsert fully replaces the note for the pair.
        svc.upsert_holding_note(
            &account.id,
            asset_id.clone(),
            "sell above 200".to_string(),
            Some(200_000_000),
            Some(ThresholdDirection::Above),
        )
        .await
        .unwrap();
        let notes = svc.get_holding_notes(&account.id).await.unwrap();
        assert_eq!(notes.len(), 1, "upsert must replace, not add");
        assert_eq!(notes[0].text, "sell above 200");
        assert_eq!(notes[0].threshold_price, Some(200_000_000));
        assert_eq!(
            notes[0].threshold_direction,
            Some(ThresholdDirection::Above)
        );

        // HNO-021 — delete removes the note; a second delete is a no-op success.
        svc.delete_holding_note(&account.id, &asset_id)
            .await
            .unwrap();
        assert!(svc.get_holding_notes(&account.id).await.unwrap().is_empty());
        svc.delete_holding_note(&account.id, &asset_id)
            .await
            .expect("deleting a non-existent note must be a no-op success");
    }

    // HNO-020 — writing a note announces it, so an open view refreshes on the note's own
    // signal rather than on a broader one.
    #[tokio::test]
    async fn hno_020_upsert_holding_note_publishes_holding_note_updated() {
        let pool = make_pool().await;
        let bus = Arc::new(SideEffectEventBus::new());
        let (svc, asset_id) = setup(&pool).await;
        let svc = svc.with_event_bus(Arc::clone(&bus));
        let account = make_account_with_held_asset(&pool, &svc, &asset_id).await;

        // Subscribe AFTER setup — `changed()` fires only on the next publish.
        let mut rx = bus.subscribe();
        svc.upsert_holding_note(&account.id, asset_id, "watch".to_string(), None, None)
            .await
            .unwrap();
        let event = rx
            .drain()
            .pop()
            .expect("HoldingNoteUpdated not received within 200ms");
        assert_eq!(event, Event::HoldingNoteUpdated);
    }

    // HNO-021 — deleting a note announces it the same way.
    #[tokio::test]
    async fn hno_021_delete_holding_note_publishes_holding_note_updated() {
        let pool = make_pool().await;
        let bus = Arc::new(SideEffectEventBus::new());
        let (svc, asset_id) = setup(&pool).await;
        let svc = svc.with_event_bus(Arc::clone(&bus));
        let account = make_account_with_held_asset(&pool, &svc, &asset_id).await;
        svc.upsert_holding_note(
            &account.id,
            asset_id.clone(),
            "watch".to_string(),
            None,
            None,
        )
        .await
        .unwrap();

        let mut rx = bus.subscribe();
        svc.delete_holding_note(&account.id, &asset_id)
            .await
            .unwrap();
        let event = rx
            .drain()
            .pop()
            .expect("HoldingNoteUpdated not received within 200ms");
        assert_eq!(event, Event::HoldingNoteUpdated);
    }

    // SYN-064 — applying a note publishes nothing: the sync run announces everything it
    // applied with one SyncCompleted after its transaction commits.
    #[tokio::test]
    async fn syn_064_apply_holding_note_publishes_no_event() {
        let pool = make_pool().await;
        let bus = Arc::new(SideEffectEventBus::new());
        let (svc, asset_id) = setup(&pool).await;
        let svc = svc.with_event_bus(Arc::clone(&bus));
        let account = make_account_with_held_asset(&pool, &svc, &asset_id).await;
        let incoming = HoldingNote::new(
            account.id.clone(),
            asset_id.clone(),
            "from the laptop".to_string(),
            None,
            None,
        )
        .unwrap();
        let content = serde_json::to_string(&incoming).unwrap();

        // Subscribe AFTER setup — `changed()` fires only on the next publish.
        let mut rx = bus.subscribe();
        let mut conn = pool.acquire().await.unwrap();
        svc.apply_holding_note(&mut conn, &content, incoming_rank("laptop", 100))
            .await
            .expect("CFR-017: applying an incoming note must succeed");
        drop(conn);

        assert!(
            rx.drain().is_empty(),
            "SYN-064: an applied note must not raise its own event"
        );
        let notes = svc.get_holding_notes(&account.id).await.unwrap();
        assert_eq!(notes.len(), 1, "the applied note is on the account");
        assert_eq!(notes[0].text, "from the laptop");
    }

    // SYN-064 — applying a note's removal publishes nothing either; the view drops the note
    // on the sync run's SyncCompleted.
    #[tokio::test]
    async fn syn_064_apply_removal_of_a_note_publishes_no_event() {
        let pool = make_pool().await;
        let bus = Arc::new(SideEffectEventBus::new());
        let (svc, asset_id) = setup(&pool).await;
        let svc = svc.with_event_bus(Arc::clone(&bus));
        let account = make_account_with_held_asset(&pool, &svc, &asset_id).await;
        svc.upsert_holding_note(
            &account.id,
            asset_id.clone(),
            "gone soon".to_string(),
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(svc.get_holding_notes(&account.id).await.unwrap().len(), 1);

        // Subscribe AFTER setup — `changed()` fires only on the next publish.
        let mut rx = bus.subscribe();
        let mut conn = pool.acquire().await.unwrap();
        svc.apply_removal(
            &mut conn,
            RecordKind::HoldingNote,
            &format!("{}:{}", account.id, asset_id),
        )
        .await
        .expect("CFR-017: applying a note removal must succeed");
        drop(conn);

        assert!(
            rx.drain().is_empty(),
            "SYN-064: an applied removal must not raise its own event"
        );
        assert!(
            svc.get_holding_notes(&account.id).await.unwrap().is_empty(),
            "the applied removal took the note with it"
        );
    }

    // HNO-011 — the cash line cannot carry a note
    #[tokio::test]
    async fn hno_011_upsert_holding_note_rejects_cash_line() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = make_account_with_held_asset(&pool, &svc, &asset_id).await;

        let err = svc
            .upsert_holding_note(
                &account.id,
                "system-cash-eur".to_string(),
                "note on cash".to_string(),
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AccountError::NoteOnCashAsset), "got: {err:?}");
    }

    // HNO-011 — a pair without any transaction entry cannot carry a note
    #[tokio::test]
    async fn hno_011_upsert_holding_note_rejects_unheld_pair() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        let account = svc
            .create(
                "Unheld".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();

        let err = svc
            .upsert_holding_note(
                &account.id,
                asset_id.clone(),
                "never traded".to_string(),
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, AccountError::NoteOnUnheldAsset),
            "got: {err:?}"
        );
    }

    // HNO-011 — the account must exist
    #[tokio::test]
    async fn hno_011_upsert_holding_note_rejects_unknown_account() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;

        let err = svc
            .upsert_holding_note(
                "nonexistent-account",
                asset_id.clone(),
                "orphan".to_string(),
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(&err, AccountError::AccountNotFound { account_id } if account_id == "nonexistent-account"),
            "got: {err:?}"
        );
    }

    // D5/FEE-043 — advance_fee_schedule_cursor writes the FeeCatchUpRepository (a record of
    // its own, CFR-044) instead of mutating the FeeSchedule row; the schedule's own rank
    // stays a user record (CFR-016) untouched by generation.
    #[tokio::test]
    async fn advance_fee_schedule_cursor_writes_the_fee_catch_up_repository() {
        let mut mock_catch_up = MockFeeCatchUpRepository::new();
        mock_catch_up
            .expect_upsert()
            .withf(|position: &FeeCatchUpPosition| {
                position.account_id == "acc-1"
                    && position.asset_id == "asset-1"
                    && position.last_applied_period == "2026-08-31"
            })
            .once()
            .returning(Ok);

        let svc = AccountService::new(
            Box::new(MockAccountRepository::new()),
            Box::new(MockHoldingRepository::new()),
            Box::new(MockTransactionRepository::new()),
        )
        .with_fee_catch_up_repo(Box::new(mock_catch_up));

        svc.advance_fee_schedule_cursor("acc-1", "asset-1", "2026-08-31".to_string())
            .await
            .unwrap();
    }

    // SYN-024/CFR-030 — deleting an account must enumerate its children (transactions here;
    // holding notes and fee schedules follow the same shape) BEFORE the cascade delete, so
    // each child records its own Removed change and tombstone (SYN-020) rather than silently
    // vanishing under the database's `ON DELETE CASCADE` — invisible to Rust, and therefore
    // invisible to the change log.
    #[tokio::test]
    async fn delete_enumerates_transactions_before_deleting_the_account() {
        let mut mock_ar = MockAccountRepository::new();
        mock_ar.expect_delete().returning(|_| Ok(()));

        let mut mock_tr = MockTransactionRepository::new();
        mock_tr
            .expect_get_all_for_account()
            .withf(|account_id: &str| account_id == "acc-1")
            .once()
            .returning(|_| Ok(vec![]));

        let svc = AccountService::new(
            Box::new(mock_ar),
            Box::new(MockHoldingRepository::new()),
            Box::new(mock_tr),
        );

        svc.delete("acc-1").await.unwrap();
    }

    // -------------------------------------------------------------------------
    // CFR-017 — apply entry points bypass entry guards
    // -------------------------------------------------------------------------

    use crate::shared::domain::{LogicalTimestamp, Origin};

    fn incoming_rank(device_id: &str, timestamp: u64) -> Rank {
        Rank {
            origin: Origin::User,
            logical_timestamp: LogicalTimestamp::new(timestamp),
            device_id: device_id.to_string(),
        }
    }

    /// Seeds the cash asset an applied account's cash holding refers to (SYN-027 — in
    /// production the apply executor seeds it before the account is written).
    async fn seed_cash_asset_row(pool: &sqlx::Pool<sqlx::Sqlite>, currency: &str) {
        sqlx::query(
            "INSERT INTO assets (id, name, reference, asset_class, category_id, currency, risk_level)
             VALUES (?, ?, ?, 'Cash', 'default-uncategorized', ?, 1)",
        )
        .bind(crate::core::cash::system_cash_asset_id(currency))
        .bind(format!("Cash {currency}"))
        .bind(currency)
        .bind(currency)
        .execute(pool)
        .await
        .expect("seed cash asset row");
    }

    // CFR-017 — apply_account writes the incoming content verbatim and stamps the rank,
    // creating the account when this device has never seen it.
    #[tokio::test]
    async fn apply_account_creates_the_record_and_stamps_the_incoming_rank() {
        let pool = make_pool().await;
        seed_cash_asset_row(&pool, "EUR").await;
        let svc = AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        );
        let content = r#"{"id":"account-from-laptop","name":"Livret A","bank_name":"","currency":"EUR","update_frequency":"ManualMonth","management_fees_enabled":false}"#;
        let mut conn = pool.acquire().await.unwrap();
        svc.apply_account(&mut conn, content, incoming_rank("laptop", 100))
            .await
            .expect("CFR-017: applying a never-seen account must succeed");
        drop(conn);

        let stamped: (String,) =
            sqlx::query_as("SELECT sync_device_id FROM accounts WHERE id = 'account-from-laptop'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            stamped.0, "laptop",
            "CFR-014: the incoming rank must be stamped"
        );
    }

    // CFR-017 — apply_transaction runs no entry guards: a sell that oversells the position
    // is applied as-is (the invariant break is CFR-042's derived inconsistency, never a
    // rejection at apply time).
    #[tokio::test]
    async fn apply_transaction_allows_an_oversell() {
        let pool = make_pool().await;
        let (svc, asset_id) = setup(&pool).await;
        seed_cash_asset_row(&pool, "EUR").await;
        let account = svc
            .create("Oversold".into(), String::new(), "EUR".into(), false)
            .await
            .unwrap();
        let content = format!(
            r#"{{"id":"tx-oversell","account_id":"{}","asset_id":"{asset_id}",
                "transaction_type":"Sell","date":"2026-08-20","quantity":10000000,
                "unit_price":50000000,"exchange_rate":1000000,"fees":0,
                "total_amount":500000000,"note":null,"realized_pnl":null,
                "created_at":"2026-08-20T10:00:00Z"}}"#,
            account.id
        );
        let mut conn = pool.acquire().await.unwrap();
        svc.apply_transaction(&mut conn, &content, incoming_rank("laptop", 200))
            .await
            .expect("CFR-017: an oversell must be applied, never rejected");
        drop(conn);

        let holding = svc
            .get_holding_by_account_asset(&account.id, &asset_id)
            .await
            .unwrap()
            .expect("the oversold holding is kept, negative (CFR-042)");
        assert_eq!(holding.quantity, -10_000_000);
    }

    // CFR-017/CFR-030 — apply_removal of an account removes it and (via the cascade) every
    // child this device holds, without re-running the delete guards a local delete would.
    #[tokio::test]
    async fn apply_removal_removes_the_account() {
        let pool = make_pool().await;
        let svc = AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        );
        let account = svc
            .create("To Remove".into(), String::new(), "EUR".into(), false)
            .await
            .unwrap();
        let mut conn = pool.acquire().await.unwrap();
        svc.apply_removal(&mut conn, RecordKind::Account, &account.id)
            .await
            .expect("CFR-017: an incoming account removal must apply");
        drop(conn);
        assert!(
            svc.get_by_id(&account.id).await.unwrap().is_none(),
            "the account must be gone after the removal applies"
        );
    }

    // -------------------------------------------------------------------------
    // CFR-035 — the name-uniqueness guard binds the name being set, not names that
    // already clash after a multi-device merge
    // -------------------------------------------------------------------------

    // CFR-035 — editing other fields of an account whose name already clashes (post-merge)
    // must be accepted: the guard binds the name being *set*, not any name that already
    // clashes. This test isolates the service guard on a mock repository; the schema side
    // (duplicate names coexisting) is covered by the repository tests.
    #[tokio::test]
    async fn update_accepts_editing_other_fields_when_the_unchanged_name_already_clashes() {
        let mut mock_repo = MockAccountRepository::new();
        mock_repo.expect_get_by_id().returning(|_| {
            Ok(Some(Account::restore(
                "account-1".into(),
                "Livret A".into(),
                String::new(),
                "EUR".into(),
                false,
            )))
        });
        // Post-merge state: another account ("account-clash") already holds this exact
        // name. The name being submitted is unchanged from the edited account's own name.
        mock_repo.expect_find_by_name().returning(|_| {
            Ok(Some(Account::restore(
                "account-clash".into(),
                "Livret A".into(),
                String::new(),
                "EUR".into(),
                false,
            )))
        });
        mock_repo.expect_update().returning(Ok);
        let svc = AccountService::new(
            Box::new(mock_repo),
            Box::new(MockHoldingRepository::new()),
            Box::new(MockTransactionRepository::new()),
        );

        let result = svc
            .update(
                "account-1".into(),
                "Livret A".into(), // unchanged name — still clashes with account-clash
                "Fortuneo".into(), // only the bank name changes
                "EUR".into(),
                false,
            )
            .await;
        assert!(
            result.is_ok(),
            "CFR-035: editing other fields of a clashing-named account must be accepted, got {result:?}"
        );
    }

    // CFR-035 — changing the name TO one another account already holds is still rejected:
    // the guard binds the name being set.
    #[tokio::test]
    async fn update_still_rejects_changing_the_name_to_one_another_account_holds() {
        let pool = make_pool().await;
        let svc = AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        );
        svc.create("Existing".into(), String::new(), "EUR".into(), false)
            .await
            .unwrap();
        let renaming = svc
            .create("Renaming".into(), String::new(), "EUR".into(), false)
            .await
            .unwrap();

        let result = svc
            .update(
                renaming.id,
                "Existing".into(),
                String::new(),
                "EUR".into(),
                false,
            )
            .await;
        assert!(matches!(result, Err(AccountError::NameAlreadyExists)));
    }
}
