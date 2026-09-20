use anyhow::{Context, Result};
use sqlx::{Pool, Sqlite, SqliteConnection};
use std::str::FromStr;
use std::sync::Arc;

use crate::context::account::domain::{
    Account, AccountChange, AccountRepository, FeeCatchUpPosition, FeeFrequency, FeeSchedule,
    Holding, HoldingNote, ThresholdDirection, Transaction, TransactionType, UpdateFrequency,
};
use crate::context::account::repository::fee_schedule::schedule_content;
use crate::core::logger::BACKEND;
use crate::shared::domain::{
    ChangeDraft, LogicalTimestamp, Operation, Origin, Rank, RecordIdentity, RecordKind,
    SyncedChild, SyncedRecord,
};
use crate::shared::infrastructure::change_recorder::{
    rank_from_columns, ChangeRecorder, NoopChangeRecorder, RankColumns,
};

#[derive(sqlx::FromRow)]
struct AccountRow {
    id: String,
    name: String,
    bank_name: String,
    currency: String,
    update_frequency: String,
    management_fees_enabled: i64,
}

impl From<AccountRow> for Account {
    fn from(row: AccountRow) -> Self {
        let update_frequency = UpdateFrequency::from_str(&row.update_frequency).unwrap_or_else(|_| {
            tracing::warn!(target: BACKEND, value = %row.update_frequency, "unknown update_frequency value, falling back to default");
            UpdateFrequency::default()
        });
        Account::restore(
            row.id,
            row.name,
            row.bank_name,
            row.currency,
            update_frequency,
            row.management_fees_enabled != 0,
        )
    }
}

#[derive(sqlx::FromRow)]
struct HoldingRow {
    id: String,
    account_id: String,
    asset_id: String,
    quantity: i64,
    average_price: i64,
    total_realized_pnl: i64,
    last_sold_date: Option<String>,
}

impl From<HoldingRow> for Holding {
    fn from(row: HoldingRow) -> Self {
        Holding::restore(
            row.id,
            row.account_id,
            row.asset_id,
            row.quantity,
            row.average_price,
            row.total_realized_pnl,
            row.last_sold_date,
        )
    }
}

#[derive(sqlx::FromRow)]
struct TransactionRow {
    id: String,
    account_id: String,
    asset_id: String,
    transaction_type: String,
    date: String,
    quantity: i64,
    unit_price: i64,
    exchange_rate: i64,
    fees: i64,
    total_amount: i64,
    note: Option<String>,
    realized_pnl: Option<i64>,
    created_at: String,
}

impl TryFrom<TransactionRow> for Transaction {
    type Error = anyhow::Error;

    fn try_from(row: TransactionRow) -> Result<Self> {
        let transaction_type = TransactionType::from_str(&row.transaction_type).map_err(|_| {
            anyhow::anyhow!("unknown transaction_type in DB: '{}'", row.transaction_type)
        })?;
        Ok(Transaction::restore(
            row.id,
            row.account_id,
            row.asset_id,
            transaction_type,
            row.date,
            row.quantity,
            row.unit_price,
            row.exchange_rate,
            row.fees,
            row.total_amount,
            row.note,
            row.realized_pnl,
            row.created_at,
        ))
    }
}

/// The three rank columns of one synced row (CFR-014), read alongside its identity.
#[derive(sqlx::FromRow)]
struct RankRow {
    identity: String,
    sync_logical_timestamp: Option<String>,
    sync_origin: Option<String>,
    sync_device_id: Option<String>,
}

impl RankRow {
    fn rank(self) -> Option<Rank> {
        rank_from_columns(
            self.sync_logical_timestamp,
            self.sync_origin,
            self.sync_device_id,
        )
    }
}

/// How `persist_pending` treats the transactions it writes: a local write records each
/// through the change recorder (SYN-020); an applied change records nothing and stamps the
/// incoming rank on the transaction it carried (CFR-014).
enum Capture {
    Record,
    Applied(Option<(String, Rank)>),
}

/// SQLite implementation of the AccountRepository.
#[derive(Clone)]
pub struct SqliteAccountRepository {
    pool: Pool<Sqlite>,
    change_recorder: Arc<dyn ChangeRecorder>,
}

impl SqliteAccountRepository {
    /// Creates a new SqliteAccountRepository.
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self {
            pool,
            change_recorder: Arc::new(NoopChangeRecorder),
        }
    }

    /// Attaches the change recorder every write appends through (SYN-020).
    pub fn with_change_recorder(mut self, change_recorder: Arc<dyn ChangeRecorder>) -> Self {
        self.change_recorder = change_recorder;
        self
    }

    async fn record_account(
        &self,
        conn: &mut SqliteConnection,
        account_id: &str,
        draft: ChangeDraft,
    ) -> Result<()> {
        let rank = self.change_recorder.record(conn, draft).await?;
        if let Some(rank) = rank {
            stamp_account_rank(conn, account_id, &rank).await?;
        }
        Ok(())
    }

    async fn record_transaction(
        &self,
        conn: &mut SqliteConnection,
        transaction_id: &str,
        draft: ChangeDraft,
    ) -> Result<()> {
        let rank = self.change_recorder.record(conn, draft).await?;
        if let Some(rank) = rank {
            stamp_transaction_rank(conn, transaction_id, &rank).await?;
        }
        Ok(())
    }

    /// Writes the aggregate's pending changes on `conn`, recording or stamping per
    /// `capture`, and clears them afterward.
    async fn persist_pending(
        &self,
        conn: &mut SqliteConnection,
        account: &mut Account,
        capture: Capture,
    ) -> Result<()> {
        for change in account.pending_changes() {
            match change {
                AccountChange::TransactionInserted(tx) => {
                    insert_transaction(conn, tx).await?;
                    self.capture_transaction(conn, tx, Operation::Created, Origin::User, &capture)
                        .await?;
                }
                AccountChange::TransactionGenerated(tx) => {
                    insert_transaction(conn, tx).await?;
                    self.capture_transaction(
                        conn,
                        tx,
                        Operation::Created,
                        Origin::Application,
                        &capture,
                    )
                    .await?;
                }
                AccountChange::TransactionUpdated(tx) => {
                    let tx_type = tx.transaction_type.to_string();
                    // created_at is immutable after creation (SEL-024)
                    sqlx::query!(
                        r#"UPDATE transactions SET account_id = ?, asset_id = ?, transaction_type = ?,
                               date = ?, quantity = ?, unit_price = ?, exchange_rate = ?, fees = ?,
                               total_amount = ?, note = ?, realized_pnl = ?
                           WHERE id = ?"#,
                        tx.account_id, tx.asset_id, tx_type, tx.date, tx.quantity,
                        tx.unit_price, tx.exchange_rate, tx.fees, tx.total_amount,
                        tx.note, tx.realized_pnl, tx.id
                    )
                    .execute(&mut *conn)
                    .await
                    .with_context(|| format!("Failed to update transaction {}", tx.id))?;
                    self.capture_transaction(conn, tx, Operation::Updated, Origin::User, &capture)
                        .await?;
                }
                AccountChange::TransactionDeleted(id) => {
                    let based_on = match capture {
                        Capture::Record => current_transaction_timestamp(conn, id).await?,
                        Capture::Applied(_) => None,
                    };
                    let deleted = sqlx::query!(r#"DELETE FROM transactions WHERE id = ?"#, id)
                        .execute(&mut *conn)
                        .await
                        .with_context(|| format!("Failed to delete transaction {}", id))?;
                    if deleted.rows_affected() > 0 && matches!(capture, Capture::Record) {
                        let draft = ChangeDraft::new(
                            RecordKind::Transaction,
                            transaction_identity(id),
                            Operation::Removed,
                            Origin::User,
                            based_on,
                            None,
                        );
                        self.change_recorder.record(&mut *conn, draft).await?;
                    }
                }
                AccountChange::HoldingUpserted(h) => {
                    sqlx::query!(
                        r#"INSERT INTO holdings (id, account_id, asset_id, quantity, average_price,
                               total_realized_pnl, last_sold_date)
                           VALUES (?, ?, ?, ?, ?, ?, ?)
                           ON CONFLICT(account_id, asset_id) DO UPDATE SET
                               quantity = excluded.quantity,
                               average_price = excluded.average_price,
                               total_realized_pnl = excluded.total_realized_pnl,
                               last_sold_date = excluded.last_sold_date"#,
                        h.id,
                        h.account_id,
                        h.asset_id,
                        h.quantity,
                        h.average_price,
                        h.total_realized_pnl,
                        h.last_sold_date
                    )
                    .execute(&mut *conn)
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to upsert holding for account {} asset {}",
                            h.account_id, h.asset_id
                        )
                    })?;
                }
                AccountChange::HoldingDeleted {
                    account_id,
                    asset_id,
                } => {
                    sqlx::query!(
                        r#"DELETE FROM holdings WHERE account_id = ? AND asset_id = ?"#,
                        account_id,
                        asset_id
                    )
                    .execute(&mut *conn)
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to delete holding for account {} asset {}",
                            account_id, asset_id
                        )
                    })?;
                }
            }
        }
        account.pending_changes.clear();
        Ok(())
    }

    /// Records a transaction write (SYN-020) or stamps the applied rank on it (CFR-014).
    async fn capture_transaction(
        &self,
        conn: &mut SqliteConnection,
        tx: &Transaction,
        operation: Operation,
        origin: Origin,
        capture: &Capture,
    ) -> Result<()> {
        match capture {
            Capture::Record => {
                let based_on = match operation {
                    Operation::Created => None,
                    Operation::Updated | Operation::Removed => {
                        current_transaction_timestamp(conn, &tx.id).await?
                    }
                };
                let draft = ChangeDraft::new(
                    RecordKind::Transaction,
                    transaction_identity(&tx.id),
                    operation,
                    origin,
                    based_on,
                    Some(serde_json::to_string(tx)?),
                );
                self.record_transaction(conn, &tx.id, draft).await
            }
            Capture::Applied(Some((applied_id, rank))) if applied_id == &tx.id => {
                stamp_transaction_rank(conn, &tx.id, rank).await
            }
            Capture::Applied(_) => Ok(()),
        }
    }
}

fn account_identity(account_id: &str) -> RecordIdentity {
    RecordIdentity::canonical(RecordKind::Account, &[account_id])
}

fn transaction_identity(transaction_id: &str) -> RecordIdentity {
    RecordIdentity::canonical(RecordKind::Transaction, &[transaction_id])
}

fn holding_identity(kind: RecordKind, account_id: &str, asset_id: &str) -> String {
    RecordIdentity::canonical(kind, &[account_id, asset_id])
        .as_str()
        .to_string()
}

/// Splits an `account:asset` identity (CFR-012) into its two keys.
fn split_holding_identity(identity: &str) -> Result<(&str, &str)> {
    identity
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("malformed holding identity: '{identity}'"))
}

async fn stamp_account_rank(
    conn: &mut SqliteConnection,
    account_id: &str,
    rank: &Rank,
) -> Result<()> {
    let columns = RankColumns::from(rank.clone());
    sqlx::query!(
        r#"UPDATE accounts SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
           WHERE id = ?"#,
        columns.logical_timestamp,
        columns.origin,
        columns.device_id,
        account_id
    )
    .execute(conn)
    .await
    .with_context(|| format!("Failed to stamp rank on account {}", account_id))?;
    Ok(())
}

async fn stamp_transaction_rank(
    conn: &mut SqliteConnection,
    transaction_id: &str,
    rank: &Rank,
) -> Result<()> {
    let columns = RankColumns::from(rank.clone());
    sqlx::query!(
        r#"UPDATE transactions SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
           WHERE id = ?"#,
        columns.logical_timestamp,
        columns.origin,
        columns.device_id,
        transaction_id
    )
    .execute(conn)
    .await
    .with_context(|| format!("Failed to stamp rank on transaction {}", transaction_id))?;
    Ok(())
}

/// CFR-011 — the logical timestamp of the account's current state, the `based_on` of the
/// next local change to it; `None` while the row has never been ranked.
async fn current_account_timestamp(
    conn: &mut SqliteConnection,
    account_id: &str,
) -> Result<Option<LogicalTimestamp>> {
    let stored = sqlx::query_scalar!(
        r#"SELECT sync_logical_timestamp AS "sync_logical_timestamp?: String" FROM accounts WHERE id = ?"#,
        account_id
    )
    .fetch_optional(conn)
    .await
    .with_context(|| format!("Failed to read the rank of account {}", account_id))?;
    Ok(stored
        .flatten()
        .and_then(|timestamp| LogicalTimestamp::from_wire(&timestamp)))
}

/// CFR-011 — the logical timestamp of the transaction's current state.
async fn current_transaction_timestamp(
    conn: &mut SqliteConnection,
    transaction_id: &str,
) -> Result<Option<LogicalTimestamp>> {
    let stored = sqlx::query_scalar!(
        r#"SELECT sync_logical_timestamp AS "sync_logical_timestamp?: String" FROM transactions WHERE id = ?"#,
        transaction_id
    )
    .fetch_optional(conn)
    .await
    .with_context(|| format!("Failed to read the rank of transaction {}", transaction_id))?;
    Ok(stored
        .flatten()
        .and_then(|timestamp| LogicalTimestamp::from_wire(&timestamp)))
}

async fn insert_transaction(conn: &mut SqliteConnection, tx: &Transaction) -> Result<()> {
    let tx_type = tx.transaction_type.to_string();
    sqlx::query!(
        r#"INSERT INTO transactions (id, account_id, asset_id, transaction_type, date, quantity,
               unit_price, exchange_rate, fees, total_amount, note, realized_pnl, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        tx.id,
        tx.account_id,
        tx.asset_id,
        tx_type,
        tx.date,
        tx.quantity,
        tx.unit_price,
        tx.exchange_rate,
        tx.fees,
        tx.total_amount,
        tx.note,
        tx.realized_pnl,
        tx.created_at
    )
    .execute(conn)
    .await
    .with_context(|| format!("Failed to insert transaction {}", tx.id))?;
    Ok(())
}

/// Loads the full aggregate on `conn`: account + all holdings + all transactions, the
/// transactions in the replay order every device shares (CFR-041).
async fn load_aggregate_on(conn: &mut SqliteConnection, id: &str) -> Result<Option<Account>> {
    let account_row = sqlx::query_as!(
        AccountRow,
        r#"SELECT id, name, bank_name, currency, update_frequency, management_fees_enabled FROM accounts WHERE id = ?"#,
        id
    )
    .fetch_optional(&mut *conn)
    .await
    .with_context(|| format!("Failed to fetch account {}", id))?;

    let account_row = match account_row {
        Some(r) => r,
        None => return Ok(None),
    };

    let base = Account::from(account_row);

    let holding_rows = sqlx::query_as!(
        HoldingRow,
        r#"SELECT id, account_id, asset_id, quantity, average_price, total_realized_pnl, last_sold_date
           FROM holdings WHERE account_id = ?"#,
        id
    )
    .fetch_all(&mut *conn)
    .await
    .with_context(|| format!("Failed to fetch holdings for account {}", id))?;

    let tx_rows = sqlx::query_as!(
        TransactionRow,
        r#"SELECT id, account_id, asset_id, transaction_type, date, quantity, unit_price,
                  exchange_rate, fees, total_amount, note, realized_pnl, created_at
           FROM transactions WHERE account_id = ?
           ORDER BY date ASC, created_at ASC, id ASC"#,
        id
    )
    .fetch_all(&mut *conn)
    .await
    .with_context(|| format!("Failed to fetch transactions for account {}", id))?;

    let holdings = holding_rows.into_iter().map(Holding::from).collect();
    let transactions = tx_rows
        .into_iter()
        .map(Transaction::try_from)
        .collect::<Result<Vec<_>>>()?;

    Ok(Some(Account::restore_with_positions(
        base.id,
        base.name,
        base.bank_name,
        base.currency,
        base.update_frequency,
        base.management_fees_enabled,
        holdings,
        transactions,
    )))
}

#[async_trait::async_trait]
impl AccountRepository for SqliteAccountRepository {
    async fn stamp_sync_rank(&self, conn: &mut SqliteConnection, rank: &Rank) -> Result<u64> {
        let columns = RankColumns::from(rank.clone());
        let (timestamp, origin, device_id) = (
            &columns.logical_timestamp,
            &columns.origin,
            &columns.device_id,
        );
        let mut stamped = 0;
        stamped += sqlx::query!(
            "UPDATE accounts SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
             WHERE sync_logical_timestamp IS NULL",
            timestamp,
            origin,
            device_id
        )
        .execute(&mut *conn)
        .await
        .context("Failed to stamp unranked accounts")?
        .rows_affected();
        stamped += sqlx::query!(
            "UPDATE transactions SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
             WHERE sync_logical_timestamp IS NULL",
            timestamp,
            origin,
            device_id
        )
        .execute(&mut *conn)
        .await
        .context("Failed to stamp unranked transactions")?
        .rows_affected();
        stamped += sqlx::query!(
            "UPDATE fee_schedules SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
             WHERE sync_logical_timestamp IS NULL",
            timestamp,
            origin,
            device_id
        )
        .execute(&mut *conn)
        .await
        .context("Failed to stamp unranked fee schedules")?
        .rows_affected();
        stamped += sqlx::query!(
            "UPDATE fee_catch_up_positions SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
             WHERE sync_logical_timestamp IS NULL",
            timestamp,
            origin,
            device_id
        )
        .execute(&mut *conn)
        .await
        .context("Failed to stamp unranked fee catch-up positions")?
        .rows_affected();
        stamped += sqlx::query!(
            "UPDATE holding_notes SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
             WHERE sync_logical_timestamp IS NULL",
            timestamp,
            origin,
            device_id
        )
        .execute(&mut *conn)
        .await
        .context("Failed to stamp unranked holding notes")?
        .rows_affected();
        Ok(stamped)
    }

    async fn get_all(&self) -> Result<Vec<Account>> {
        let rows = sqlx::query_as!(
            AccountRow,
            r#"SELECT id, name, bank_name, currency, update_frequency, management_fees_enabled FROM accounts"#
        )
        .fetch_all(&self.pool)
        .await
        .with_context(|| "Failed to fetch accounts")?;

        Ok(rows.into_iter().map(Account::from).collect())
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Account>> {
        let row = sqlx::query_as!(
            AccountRow,
            r#"SELECT id, name, bank_name, currency, update_frequency, management_fees_enabled FROM accounts WHERE id = ?"#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .with_context(|| format!("Failed to fetch account {}", id))?;

        Ok(row.map(Account::from))
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<Account>> {
        let row = sqlx::query_as!(
            AccountRow,
            r#"SELECT id, name, bank_name, currency, update_frequency, management_fees_enabled FROM accounts WHERE LOWER(name) = LOWER(?)"#,
            name
        )
        .fetch_optional(&self.pool)
        .await
        .with_context(|| format!("Failed to find account by name {}", name))?;

        Ok(row.map(Account::from))
    }

    async fn create(&self, account: Account) -> Result<Account> {
        let update_freq_str = account.update_frequency.to_string();
        let management_fees_enabled = account.management_fees_enabled as i64;
        let mut db_tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin DB transaction for account create")?;
        sqlx::query!(
            r#"INSERT INTO accounts (id, name, bank_name, currency, update_frequency, management_fees_enabled)
               VALUES (?, ?, ?, ?, ?, ?)"#,
            account.id,
            account.name,
            account.bank_name,
            account.currency,
            update_freq_str,
            management_fees_enabled
        )
        .execute(&mut *db_tx)
        .await
        .with_context(|| format!("Failed to create account {}", account.name))?;
        let draft = ChangeDraft::new(
            RecordKind::Account,
            account_identity(&account.id),
            Operation::Created,
            Origin::User,
            None,
            Some(serde_json::to_string(&account)?),
        );
        self.record_account(&mut db_tx, &account.id, draft).await?;
        db_tx
            .commit()
            .await
            .context("Failed to commit account create")?;

        Ok(account)
    }

    async fn update(&self, account: Account) -> Result<Account> {
        let update_freq_str = account.update_frequency.to_string();
        let management_fees_enabled = account.management_fees_enabled as i64;
        let mut db_tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin DB transaction for account update")?;
        let based_on = current_account_timestamp(&mut db_tx, &account.id).await?;
        let written = sqlx::query!(
            r#"UPDATE accounts
               SET name = ?, bank_name = ?, currency = ?, update_frequency = ?, management_fees_enabled = ?
               WHERE id = ?"#,
            account.name,
            account.bank_name,
            account.currency,
            update_freq_str,
            management_fees_enabled,
            account.id
        )
        .execute(&mut *db_tx)
        .await
        .with_context(|| format!("Failed to update account {}", account.id))?;
        if written.rows_affected() > 0 {
            let draft = ChangeDraft::new(
                RecordKind::Account,
                account_identity(&account.id),
                Operation::Updated,
                Origin::User,
                based_on,
                Some(serde_json::to_string(&account)?),
            );
            self.record_account(&mut db_tx, &account.id, draft).await?;
        }
        db_tx
            .commit()
            .await
            .context("Failed to commit account update")?;

        Ok(account)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let mut db_tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin DB transaction for account delete")?;
        let based_on = current_account_timestamp(&mut db_tx, id).await?;
        let deleted = sqlx::query!(r#"DELETE FROM accounts WHERE id = ?"#, id)
            .execute(&mut *db_tx)
            .await
            .with_context(|| format!("Failed to delete account {}", id))?;
        if deleted.rows_affected() > 0 {
            let draft = ChangeDraft::new(
                RecordKind::Account,
                account_identity(id),
                Operation::Removed,
                Origin::User,
                based_on,
                None,
            );
            self.change_recorder.record(&mut db_tx, draft).await?;
        }
        db_tx
            .commit()
            .await
            .context("Failed to commit account delete")?;

        Ok(())
    }

    async fn get_with_holdings_and_transactions(&self, id: &str) -> Result<Option<Account>> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .context("Failed to acquire a connection for the aggregate load")?;
        load_aggregate_on(&mut conn, id).await
    }

    async fn save(&self, account: &mut Account) -> Result<()> {
        let mut db_tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin DB transaction for account save")?;
        self.persist_pending(&mut db_tx, account, Capture::Record)
            .await?;
        db_tx
            .commit()
            .await
            .context("Failed to commit account save")?;
        Ok(())
    }

    async fn synced_record(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> Result<Option<SyncedRecord>> {
        match kind {
            RecordKind::Account => {
                let row = sqlx::query!(
                    r#"SELECT id, name, bank_name, currency, update_frequency, management_fees_enabled,
                              sync_logical_timestamp, sync_origin, sync_device_id
                       FROM accounts WHERE id = ?"#,
                    identity
                )
                .fetch_optional(conn)
                .await
                .with_context(|| format!("Failed to read synced account {}", identity))?;
                row.map(|row| {
                    let account = Account::from(AccountRow {
                        id: row.id,
                        name: row.name,
                        bank_name: row.bank_name,
                        currency: row.currency,
                        update_frequency: row.update_frequency,
                        management_fees_enabled: row.management_fees_enabled,
                    });
                    Ok(SyncedRecord {
                        rank: rank_from_columns(
                            row.sync_logical_timestamp,
                            row.sync_origin,
                            row.sync_device_id,
                        ),
                        content: serde_json::to_string(&account)?,
                    })
                })
                .transpose()
            }
            RecordKind::Transaction => {
                let row = sqlx::query!(
                    r#"SELECT id, account_id, asset_id, transaction_type, date, quantity, unit_price,
                              exchange_rate, fees, total_amount, note, realized_pnl, created_at,
                              sync_logical_timestamp, sync_origin, sync_device_id
                       FROM transactions WHERE id = ?"#,
                    identity
                )
                .fetch_optional(conn)
                .await
                .with_context(|| format!("Failed to read synced transaction {}", identity))?;
                row.map(|row| {
                    let rank = rank_from_columns(
                        row.sync_logical_timestamp,
                        row.sync_origin,
                        row.sync_device_id,
                    );
                    let transaction = Transaction::try_from(TransactionRow {
                        id: row.id,
                        account_id: row.account_id,
                        asset_id: row.asset_id,
                        transaction_type: row.transaction_type,
                        date: row.date,
                        quantity: row.quantity,
                        unit_price: row.unit_price,
                        exchange_rate: row.exchange_rate,
                        fees: row.fees,
                        total_amount: row.total_amount,
                        note: row.note,
                        realized_pnl: row.realized_pnl,
                        created_at: row.created_at,
                    })?;
                    Ok(SyncedRecord {
                        rank,
                        content: serde_json::to_string(&transaction)?,
                    })
                })
                .transpose()
            }
            RecordKind::HoldingNote => {
                let (account_id, asset_id) = split_holding_identity(identity)?;
                let row = sqlx::query!(
                    r#"SELECT account_id, asset_id, text, threshold_price, threshold_direction,
                              sync_logical_timestamp, sync_origin, sync_device_id
                       FROM holding_notes WHERE account_id = ? AND asset_id = ?"#,
                    account_id,
                    asset_id
                )
                .fetch_optional(conn)
                .await
                .with_context(|| format!("Failed to read synced holding note {}", identity))?;
                row.map(|row| {
                    let threshold_direction = row
                        .threshold_direction
                        .map(|direction| {
                            ThresholdDirection::from_str(&direction).map_err(|_| {
                                anyhow::anyhow!("unknown threshold direction in DB: '{direction}'")
                            })
                        })
                        .transpose()?;
                    let note = HoldingNote::from_storage(
                        row.account_id,
                        row.asset_id,
                        row.text,
                        row.threshold_price,
                        threshold_direction,
                    );
                    Ok(SyncedRecord {
                        rank: rank_from_columns(
                            row.sync_logical_timestamp,
                            row.sync_origin,
                            row.sync_device_id,
                        ),
                        content: serde_json::to_string(&note)?,
                    })
                })
                .transpose()
            }
            RecordKind::FeeSchedule => {
                let (account_id, asset_id) = split_holding_identity(identity)?;
                let row = sqlx::query!(
                    r#"SELECT id, account_id, asset_id, annual_rate_micros, frequency, start_date,
                              end_date, active, sync_logical_timestamp, sync_origin, sync_device_id
                       FROM fee_schedules WHERE account_id = ? AND asset_id = ?"#,
                    account_id,
                    asset_id
                )
                .fetch_optional(conn)
                .await
                .with_context(|| format!("Failed to read synced fee schedule {}", identity))?;
                row.map(|row| {
                    let frequency = FeeFrequency::from_str(&row.frequency).map_err(|_| {
                        anyhow::anyhow!("unknown fee frequency in DB: '{}'", row.frequency)
                    })?;
                    let schedule = FeeSchedule::restore(
                        row.id,
                        row.account_id,
                        row.asset_id,
                        row.annual_rate_micros,
                        frequency,
                        row.start_date,
                        row.end_date,
                        row.active != 0,
                        None,
                    );
                    Ok(SyncedRecord {
                        rank: rank_from_columns(
                            row.sync_logical_timestamp,
                            row.sync_origin,
                            row.sync_device_id,
                        ),
                        content: schedule_content(&schedule)?,
                    })
                })
                .transpose()
            }
            RecordKind::FeeCatchUpPosition => {
                let (account_id, asset_id) = split_holding_identity(identity)?;
                let row = sqlx::query!(
                    r#"SELECT account_id, asset_id, last_applied_period,
                              sync_logical_timestamp, sync_origin, sync_device_id
                       FROM fee_catch_up_positions WHERE account_id = ? AND asset_id = ?"#,
                    account_id,
                    asset_id
                )
                .fetch_optional(conn)
                .await
                .with_context(|| format!("Failed to read synced catch-up position {}", identity))?;
                row.map(|row| {
                    let position = FeeCatchUpPosition {
                        account_id: row.account_id,
                        asset_id: row.asset_id,
                        last_applied_period: row.last_applied_period,
                    };
                    Ok(SyncedRecord {
                        rank: rank_from_columns(
                            row.sync_logical_timestamp,
                            row.sync_origin,
                            row.sync_device_id,
                        ),
                        content: serde_json::to_string(&position)?,
                    })
                })
                .transpose()
            }
            RecordKind::Category
            | RecordKind::Asset
            | RecordKind::AssetPrice
            | RecordKind::CurrencyPair
            | RecordKind::CurrencyRate => Ok(None),
        }
    }

    async fn synced_children(
        &self,
        conn: &mut SqliteConnection,
        account_id: &str,
    ) -> Result<Vec<SyncedChild>> {
        let mut children = Vec::new();
        let transactions = sqlx::query_as!(
            RankRow,
            r#"SELECT id AS identity, sync_logical_timestamp, sync_origin, sync_device_id
               FROM transactions WHERE account_id = ?"#,
            account_id
        )
        .fetch_all(&mut *conn)
        .await
        .with_context(|| format!("Failed to list the transactions of account {}", account_id))?;
        children.extend(transactions.into_iter().map(|row| SyncedChild {
            record_kind: RecordKind::Transaction,
            record_identity: row.identity.clone(),
            rank: row.rank(),
        }));
        let notes = sqlx::query_as!(
            RankRow,
            r#"SELECT asset_id AS identity, sync_logical_timestamp, sync_origin, sync_device_id
               FROM holding_notes WHERE account_id = ?"#,
            account_id
        )
        .fetch_all(&mut *conn)
        .await
        .with_context(|| format!("Failed to list the holding notes of account {}", account_id))?;
        children.extend(notes.into_iter().map(|row| SyncedChild {
            record_kind: RecordKind::HoldingNote,
            record_identity: holding_identity(RecordKind::HoldingNote, account_id, &row.identity),
            rank: row.rank(),
        }));
        let schedules = sqlx::query_as!(
            RankRow,
            r#"SELECT asset_id AS identity, sync_logical_timestamp, sync_origin, sync_device_id
               FROM fee_schedules WHERE account_id = ?"#,
            account_id
        )
        .fetch_all(&mut *conn)
        .await
        .with_context(|| format!("Failed to list the fee schedules of account {}", account_id))?;
        children.extend(schedules.into_iter().map(|row| SyncedChild {
            record_kind: RecordKind::FeeSchedule,
            record_identity: holding_identity(RecordKind::FeeSchedule, account_id, &row.identity),
            rank: row.rank(),
        }));
        let positions = sqlx::query_as!(
            RankRow,
            r#"SELECT asset_id AS identity, sync_logical_timestamp, sync_origin, sync_device_id
               FROM fee_catch_up_positions WHERE account_id = ?"#,
            account_id
        )
        .fetch_all(&mut *conn)
        .await
        .with_context(|| {
            format!(
                "Failed to list the catch-up positions of account {}",
                account_id
            )
        })?;
        children.extend(positions.into_iter().map(|row| SyncedChild {
            record_kind: RecordKind::FeeCatchUpPosition,
            record_identity: holding_identity(
                RecordKind::FeeCatchUpPosition,
                account_id,
                &row.identity,
            ),
            rank: row.rank(),
        }));
        Ok(children)
    }

    async fn clashing_name_rank(
        &self,
        conn: &mut SqliteConnection,
        account_id: &str,
        name: &str,
    ) -> Result<Option<Rank>> {
        let row = sqlx::query_as!(
            RankRow,
            r#"SELECT id AS identity, sync_logical_timestamp, sync_origin, sync_device_id
               FROM accounts WHERE LOWER(name) = LOWER(?) AND id <> ?
               ORDER BY sync_origin, sync_logical_timestamp, sync_device_id, id
               LIMIT 1"#,
            name,
            account_id
        )
        .fetch_optional(conn)
        .await
        .with_context(|| format!("Failed to look up accounts named {}", name))?;
        Ok(row.and_then(RankRow::rank))
    }

    async fn apply_account(
        &self,
        conn: &mut SqliteConnection,
        account: &Account,
        rank: &Rank,
    ) -> Result<()> {
        let update_freq_str = account.update_frequency.to_string();
        let management_fees_enabled = account.management_fees_enabled as i64;
        let columns = RankColumns::from(rank.clone());
        sqlx::query!(
            r#"INSERT INTO accounts (id, name, bank_name, currency, update_frequency, management_fees_enabled,
                                     sync_logical_timestamp, sync_origin, sync_device_id)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(id) DO UPDATE SET
                   name = excluded.name,
                   bank_name = excluded.bank_name,
                   currency = excluded.currency,
                   update_frequency = excluded.update_frequency,
                   management_fees_enabled = excluded.management_fees_enabled,
                   sync_logical_timestamp = excluded.sync_logical_timestamp,
                   sync_origin = excluded.sync_origin,
                   sync_device_id = excluded.sync_device_id"#,
            account.id,
            account.name,
            account.bank_name,
            account.currency,
            update_freq_str,
            management_fees_enabled,
            columns.logical_timestamp,
            columns.origin,
            columns.device_id
        )
        .execute(conn)
        .await
        .with_context(|| format!("Failed to apply account {}", account.id))?;
        Ok(())
    }

    async fn remove_account(&self, conn: &mut SqliteConnection, id: &str) -> Result<()> {
        for statement in [
            "DELETE FROM holdings WHERE account_id = ?",
            "DELETE FROM transactions WHERE account_id = ?",
            "DELETE FROM holding_notes WHERE account_id = ?",
            "DELETE FROM fee_schedules WHERE account_id = ?",
            "DELETE FROM fee_catch_up_positions WHERE account_id = ?",
            "DELETE FROM accounts WHERE id = ?",
        ] {
            sqlx::query(statement)
                .bind(id)
                .execute(&mut *conn)
                .await
                .with_context(|| format!("Failed to remove account {}", id))?;
        }
        Ok(())
    }

    async fn load_aggregate(
        &self,
        conn: &mut SqliteConnection,
        id: &str,
    ) -> Result<Option<Account>> {
        load_aggregate_on(conn, id).await
    }

    async fn save_applied(
        &self,
        conn: &mut SqliteConnection,
        account: &mut Account,
        stamp: Option<(String, Rank)>,
    ) -> Result<()> {
        self.persist_pending(conn, account, Capture::Applied(stamp))
            .await
    }

    async fn account_id_of_transaction(
        &self,
        conn: &mut SqliteConnection,
        transaction_id: &str,
    ) -> Result<Option<String>> {
        sqlx::query_scalar!(
            r#"SELECT account_id AS "account_id: String" FROM transactions WHERE id = ?"#,
            transaction_id
        )
        .fetch_optional(conn)
        .await
        .with_context(|| {
            format!(
                "Failed to read the account of transaction {}",
                transaction_id
            )
        })
    }

    async fn apply_holding_note(
        &self,
        conn: &mut SqliteConnection,
        note: &HoldingNote,
        rank: &Rank,
    ) -> Result<()> {
        let threshold_direction = note.threshold_direction.map(|d| d.to_string());
        let columns = RankColumns::from(rank.clone());
        sqlx::query!(
            r#"INSERT INTO holding_notes (account_id, asset_id, text, threshold_price, threshold_direction,
                                          sync_logical_timestamp, sync_origin, sync_device_id)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(account_id, asset_id) DO UPDATE SET
                   text = excluded.text,
                   threshold_price = excluded.threshold_price,
                   threshold_direction = excluded.threshold_direction,
                   sync_logical_timestamp = excluded.sync_logical_timestamp,
                   sync_origin = excluded.sync_origin,
                   sync_device_id = excluded.sync_device_id"#,
            note.account_id,
            note.asset_id,
            note.text,
            note.threshold_price,
            threshold_direction,
            columns.logical_timestamp,
            columns.origin,
            columns.device_id
        )
        .execute(conn)
        .await
        .context("Failed to apply holding note")?;
        Ok(())
    }

    async fn remove_holding_note(
        &self,
        conn: &mut SqliteConnection,
        account_id: &str,
        asset_id: &str,
    ) -> Result<()> {
        sqlx::query!(
            "DELETE FROM holding_notes WHERE account_id = ? AND asset_id = ?",
            account_id,
            asset_id
        )
        .execute(conn)
        .await
        .context("Failed to remove holding note")?;
        Ok(())
    }

    async fn apply_fee_schedule(
        &self,
        conn: &mut SqliteConnection,
        schedule: &FeeSchedule,
        rank: &Rank,
    ) -> Result<()> {
        let frequency = schedule.frequency.to_string();
        let active = schedule.active as i64;
        let columns = RankColumns::from(rank.clone());
        sqlx::query!(
            r#"INSERT INTO fee_schedules (id, account_id, asset_id, annual_rate_micros, frequency, start_date,
                                          end_date, active, sync_logical_timestamp, sync_origin, sync_device_id)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(account_id, asset_id) DO UPDATE SET
                   id = excluded.id,
                   annual_rate_micros = excluded.annual_rate_micros,
                   frequency = excluded.frequency,
                   start_date = excluded.start_date,
                   end_date = excluded.end_date,
                   active = excluded.active,
                   sync_logical_timestamp = excluded.sync_logical_timestamp,
                   sync_origin = excluded.sync_origin,
                   sync_device_id = excluded.sync_device_id"#,
            schedule.id,
            schedule.account_id,
            schedule.asset_id,
            schedule.annual_rate_percent_micros,
            frequency,
            schedule.start_date,
            schedule.end_date,
            active,
            columns.logical_timestamp,
            columns.origin,
            columns.device_id
        )
        .execute(conn)
        .await
        .context("Failed to apply fee schedule")?;
        Ok(())
    }

    async fn remove_fee_schedule(
        &self,
        conn: &mut SqliteConnection,
        account_id: &str,
        asset_id: &str,
    ) -> Result<()> {
        sqlx::query!(
            "DELETE FROM fee_schedules WHERE account_id = ? AND asset_id = ?",
            account_id,
            asset_id
        )
        .execute(conn)
        .await
        .context("Failed to remove fee schedule")?;
        Ok(())
    }

    async fn apply_catch_up_position(
        &self,
        conn: &mut SqliteConnection,
        position: &FeeCatchUpPosition,
        rank: &Rank,
    ) -> Result<()> {
        let columns = RankColumns::from(rank.clone());
        sqlx::query!(
            r#"INSERT INTO fee_catch_up_positions (account_id, asset_id, last_applied_period,
                                                   sync_logical_timestamp, sync_origin, sync_device_id)
               VALUES (?, ?, ?, ?, ?, ?)
               ON CONFLICT(account_id, asset_id) DO UPDATE SET
                   last_applied_period = MAX(last_applied_period, excluded.last_applied_period),
                   sync_logical_timestamp = excluded.sync_logical_timestamp,
                   sync_origin = excluded.sync_origin,
                   sync_device_id = excluded.sync_device_id"#,
            position.account_id,
            position.asset_id,
            position.last_applied_period,
            columns.logical_timestamp,
            columns.origin,
            columns.device_id
        )
        .execute(conn)
        .await
        .context("Failed to apply fee catch-up position")?;
        Ok(())
    }

    async fn remove_catch_up_position(
        &self,
        conn: &mut SqliteConnection,
        account_id: &str,
        asset_id: &str,
    ) -> Result<()> {
        sqlx::query!(
            "DELETE FROM fee_catch_up_positions WHERE account_id = ? AND asset_id = ?",
            account_id,
            asset_id
        )
        .execute(conn)
        .await
        .context("Failed to remove fee catch-up position")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::domain::{Account, AccountChange, UpdateFrequency};
    use crate::context::sync::SqliteChangeRecorder;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::sync::Arc;

    async fn make_pool() -> Pool<Sqlite> {
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

    async fn seed_sync_device(pool: &Pool<Sqlite>) {
        sqlx::query!(
            r#"INSERT INTO sync_device
               (id, device_id, device_name, folder, joined_at, paused, portfolio_created_at,
                logical_clock, derived_key, data_format_version)
               VALUES (1, 'desktop-device', 'Desktop', '/tmp/sync', '2026-08-22T00:00:00Z', 0,
                       '2026-08-22T00:00:00Z', 0, X'00', 1)"#
        )
        .execute(pool)
        .await
        .expect("seed sync_device");
    }

    async fn seed_asset(pool: &Pool<Sqlite>, id: &str) {
        sqlx::query!(
            "INSERT INTO assets (id, name, reference, asset_class, currency, risk_level, category_id, is_archived)
             VALUES (?, 'Test Asset', 'REF', 'Stocks', 'EUR', 3, 'default-uncategorized', 0)",
            id,
        )
        .execute(pool)
        .await
        .expect("seed asset");
    }

    async fn changes_with_operation(pool: &Pool<Sqlite>, operation: &str) -> i64 {
        sqlx::query_scalar!(
            "SELECT COUNT(*) FROM changes WHERE operation = ?",
            operation
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    fn new_account() -> Account {
        Account::new(
            "CTO".to_string(),
            "Boursorama".to_string(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .unwrap()
    }

    fn recorded_tx(id: &str, account_id: &str, asset_id: &str) -> Transaction {
        Transaction::restore(
            id.to_string(),
            account_id.to_string(),
            asset_id.to_string(),
            TransactionType::Purchase,
            "2026-08-01".to_string(),
            10,
            100_000_000,
            1_000_000,
            0,
            1_000_000_000,
            None,
            None,
            "2026-08-01T00:00:00Z".to_string(),
        )
    }

    // SYN-020/021 — create records exactly one Created change, rank-stamped (CFR-014).
    #[tokio::test]
    async fn create_records_one_created_change_with_rank_stamped() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAccountRepository::new(pool.clone()).with_change_recorder(recorder);

        let account = new_account();
        repo.create(account.clone()).await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Created").await, 1);
        let row = sqlx::query!(
            "SELECT sync_logical_timestamp, sync_origin FROM accounts WHERE id = ?",
            account.id
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            row.sync_logical_timestamp.is_some(),
            "CFR-014: the row's rank columns are stamped"
        );
        assert_eq!(row.sync_origin.as_deref(), Some("User"));
    }

    // CFR-014/D6 — stamp_sync_rank ranks only the rows that were never ranked; a row the
    // recorder already ranked keeps its own rank.
    #[tokio::test]
    async fn stamp_sync_rank_stamps_only_unranked_rows() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let unranked = new_account();
        SqliteAccountRepository::new(pool.clone())
            .create(unranked.clone())
            .await
            .unwrap();
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAccountRepository::new(pool.clone()).with_change_recorder(recorder);
        let mut ranked = new_account();
        ranked.id = "acc-ranked".to_string();
        ranked.name = "Ranked".to_string();
        repo.create(ranked.clone()).await.unwrap();

        let rank = crate::shared::domain::Rank {
            origin: Origin::User,
            logical_timestamp: crate::shared::domain::LogicalTimestamp::new(99),
            device_id: "desktop-device".to_string(),
        };
        let mut conn = pool.acquire().await.unwrap();
        let stamped = repo.stamp_sync_rank(&mut conn, &rank).await.unwrap();
        drop(conn);
        assert_eq!(stamped, 1, "only the unranked account is stamped");

        let rows = sqlx::query!("SELECT id, sync_logical_timestamp FROM accounts ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
        let stamp_of = |id: &str| {
            rows.iter()
                .find(|row| row.id == id)
                .and_then(|row| row.sync_logical_timestamp.clone())
        };
        assert_eq!(
            stamp_of(&unranked.id).as_deref(),
            Some("00000000000000000099")
        );
        assert_ne!(
            stamp_of(&ranked.id).as_deref(),
            Some("00000000000000000099"),
            "a row the recorder ranked keeps its rank"
        );
    }

    // SYN-020 — update records exactly one Updated change.
    #[tokio::test]
    async fn update_records_one_updated_change() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let setup_repo = SqliteAccountRepository::new(pool.clone());
        let account = new_account();
        setup_repo.create(account.clone()).await.unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAccountRepository::new(pool.clone()).with_change_recorder(recorder);
        let mut renamed = account.clone();
        renamed.name = "CTO Fortuneo".to_string();
        repo.update(renamed).await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Updated").await, 1);
    }

    // CFR-011 — every local write's `based_on` is the record's current rank at the time of
    // the write, so a device receiving the change can classify concurrency correctly. Two
    // sequential updates to the same account: the second change's `based_on` must equal the
    // first change's own `logical_timestamp`.
    #[tokio::test]
    async fn update_populates_based_on_from_the_records_current_rank() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let account = new_account();
        SqliteAccountRepository::new(pool.clone())
            .create(account.clone())
            .await
            .unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAccountRepository::new(pool.clone()).with_change_recorder(recorder);
        let mut first_rename = account.clone();
        first_rename.name = "CTO Fortuneo".to_string();
        repo.update(first_rename).await.unwrap();
        let mut second_rename = account.clone();
        second_rename.name = "CTO Fortuneo Renamed".to_string();
        repo.update(second_rename).await.unwrap();

        let logical_timestamps: Vec<String> = sqlx::query_scalar(
            "SELECT logical_timestamp FROM changes WHERE record_kind = 'Account' ORDER BY sequence ASC",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        let based_ons: Vec<Option<String>> = sqlx::query_scalar(
            "SELECT based_on FROM changes WHERE record_kind = 'Account' ORDER BY sequence ASC",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            based_ons[1].as_deref(),
            Some(logical_timestamps[0].as_str()),
            "CFR-011: the second update's based_on must equal the first update's own \
             logical_timestamp, not be absent"
        );
    }

    // SYN-020/024 — delete records exactly one Removed change and leaves a tombstone (CFR-015).
    #[tokio::test]
    async fn delete_records_one_removed_change_and_tombstone() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let setup_repo = SqliteAccountRepository::new(pool.clone());
        let account = new_account();
        setup_repo.create(account.clone()).await.unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAccountRepository::new(pool.clone()).with_change_recorder(recorder);
        repo.delete(&account.id).await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Removed").await, 1);
        let tombstone = sqlx::query!(
            "SELECT record_identity FROM tombstones WHERE record_kind = 'Account' AND record_identity = ?",
            account.id
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert!(tombstone.is_some(), "CFR-015: a removal leaves a tombstone");
    }

    // SYN-020 — a failed write records no change (rollback: the row and its change exist
    // together or not at all).
    #[tokio::test]
    async fn create_rolls_back_change_when_the_write_fails() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAccountRepository::new(pool.clone()).with_change_recorder(recorder);
        let account = new_account();
        repo.create(account.clone()).await.unwrap();

        // Same id — PRIMARY KEY violation, the whole write must fail atomically.
        let result = repo.create(account.clone()).await;
        assert!(result.is_err());

        assert_eq!(
            changes_with_operation(&pool, "Created").await,
            1,
            "only the first (successful) create recorded a change; the failed second create recorded none"
        );
    }

    // SYN-020 — AccountChange::TransactionInserted (via save) records one Created change.
    #[tokio::test]
    async fn save_transaction_inserted_records_one_created_change() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let setup_repo = SqliteAccountRepository::new(pool.clone());
        let account = new_account();
        setup_repo.create(account.clone()).await.unwrap();
        seed_asset(&pool, "asset-1").await;

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAccountRepository::new(pool.clone()).with_change_recorder(recorder);
        let mut loaded = account.clone();
        loaded
            .pending_changes
            .push(AccountChange::TransactionInserted(recorded_tx(
                "tx-1",
                &account.id,
                "asset-1",
            )));
        repo.save(&mut loaded).await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Created").await, 1);
    }

    // SYN-020 — AccountChange::TransactionUpdated (via save) records one Updated change.
    #[tokio::test]
    async fn save_transaction_updated_records_one_updated_change() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let setup_repo = SqliteAccountRepository::new(pool.clone());
        let account = new_account();
        setup_repo.create(account.clone()).await.unwrap();
        seed_asset(&pool, "asset-1").await;
        let mut seeded = account.clone();
        seeded
            .pending_changes
            .push(AccountChange::TransactionInserted(recorded_tx(
                "tx-1",
                &account.id,
                "asset-1",
            )));
        setup_repo.save(&mut seeded).await.unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAccountRepository::new(pool.clone()).with_change_recorder(recorder);
        let mut updated = account.clone();
        updated
            .pending_changes
            .push(AccountChange::TransactionUpdated(recorded_tx(
                "tx-1",
                &account.id,
                "asset-1",
            )));
        repo.save(&mut updated).await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Updated").await, 1);
    }

    // SYN-020/024 — AccountChange::TransactionDeleted (via save) records one Removed change
    // and leaves a tombstone.
    #[tokio::test]
    async fn save_transaction_deleted_records_one_removed_change_and_tombstone() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let setup_repo = SqliteAccountRepository::new(pool.clone());
        let account = new_account();
        setup_repo.create(account.clone()).await.unwrap();
        seed_asset(&pool, "asset-1").await;
        let mut seeded = account.clone();
        seeded
            .pending_changes
            .push(AccountChange::TransactionInserted(recorded_tx(
                "tx-1",
                &account.id,
                "asset-1",
            )));
        setup_repo.save(&mut seeded).await.unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAccountRepository::new(pool.clone()).with_change_recorder(recorder);
        let mut deleted = account.clone();
        deleted
            .pending_changes
            .push(AccountChange::TransactionDeleted("tx-1".to_string()));
        repo.save(&mut deleted).await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Removed").await, 1);
        let tombstone = sqlx::query!(
            "SELECT record_identity FROM tombstones WHERE record_kind = 'Transaction' AND record_identity = 'tx-1'"
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert!(tombstone.is_some());
    }

    // CFR-041 — transactions load in `date, created_at, id` order: two transactions dated
    // and created identically load in id order (the total-order fix this PR adds).
    #[tokio::test]
    async fn get_with_holdings_and_transactions_orders_by_date_created_at_id() {
        let pool = make_pool().await;
        let setup_repo = SqliteAccountRepository::new(pool.clone());
        let account = new_account();
        setup_repo.create(account.clone()).await.unwrap();
        seed_asset(&pool, "asset-1").await;

        // Same date, same created_at — only `id` decides the order (CFR-041).
        for id in ["tx-b", "tx-a"] {
            sqlx::query!(
                r#"INSERT INTO transactions (id, account_id, asset_id, transaction_type, date, quantity,
                       unit_price, exchange_rate, fees, total_amount, note, realized_pnl, created_at)
                   VALUES (?, ?, 'asset-1', 'Purchase', '2026-08-20', 10, 100000000, 1000000, 0, 1000000000, NULL, NULL, '2026-08-20T00:00:00Z')"#,
                id,
                account.id,
            )
            .execute(&pool)
            .await
            .unwrap();
        }

        let loaded = setup_repo
            .get_with_holdings_and_transactions(&account.id)
            .await
            .unwrap()
            .unwrap();
        let ids: Vec<&str> = loaded.transactions.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["tx-a", "tx-b"],
            "CFR-041: equal date and created_at fall back to transaction id order"
        );
    }
}
