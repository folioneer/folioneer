use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::{Pool, Sqlite};
use std::str::FromStr;
use std::sync::Arc;

use crate::context::account::domain::{Transaction, TransactionRepository, TransactionType};
use crate::shared::domain::{
    ChangeDraft, LogicalTimestamp, Operation, Origin, RecordIdentity, RecordKind,
};
use crate::shared::infrastructure::change_recorder::{ChangeRecorder, NoopChangeRecorder};

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

/// SQLite implementation of the TransactionRepository.
#[derive(Clone)]
pub struct SqliteTransactionRepository {
    pool: Pool<Sqlite>,
    change_recorder: Arc<dyn ChangeRecorder>,
}

impl SqliteTransactionRepository {
    /// Creates a new SqliteTransactionRepository.
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
}

#[async_trait]
impl TransactionRepository for SqliteTransactionRepository {
    async fn get_by_id(&self, id: &str) -> Result<Option<Transaction>> {
        let row = sqlx::query_as!(
            TransactionRow,
            r#"SELECT id, account_id, asset_id, transaction_type, date, quantity, unit_price, exchange_rate, fees, total_amount, note, realized_pnl, created_at FROM transactions WHERE id = ?"#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .with_context(|| format!("Failed to fetch transaction {}", id))?;

        Ok(row.map(Transaction::try_from).transpose()?)
    }

    async fn get_by_account_asset(
        &self,
        account_id: &str,
        asset_id: &str,
    ) -> Result<Vec<Transaction>> {
        let rows = sqlx::query_as!(
            TransactionRow,
            r#"SELECT id, account_id, asset_id, transaction_type, date, quantity, unit_price, exchange_rate, fees, total_amount, note, realized_pnl, created_at
               FROM transactions
               WHERE account_id = ? AND asset_id = ?
               ORDER BY date ASC, created_at ASC, id ASC"#,
            account_id,
            asset_id
        )
        .fetch_all(&self.pool)
        .await
        .with_context(|| {
            format!(
                "Failed to fetch transactions for account {} asset {}",
                account_id, asset_id
            )
        })?;

        rows.into_iter()
            .map(Transaction::try_from)
            .collect::<Result<Vec<_>>>()
    }

    async fn get_all_for_account(&self, account_id: &str) -> Result<Vec<Transaction>> {
        let rows = sqlx::query_as!(
            TransactionRow,
            r#"SELECT id, account_id, asset_id, transaction_type, date, quantity, unit_price, exchange_rate, fees, total_amount, note, realized_pnl, created_at
               FROM transactions
               WHERE account_id = ?
               ORDER BY date ASC, created_at ASC, id ASC"#,
            account_id
        )
        .fetch_all(&self.pool)
        .await
        .with_context(|| format!("Failed to fetch all transactions for account {}", account_id))?;

        rows.into_iter()
            .map(Transaction::try_from)
            .collect::<Result<Vec<_>>>()
    }

    async fn get_realized_pnl_by_account(&self, account_id: &str) -> Result<Vec<(String, i64)>> {
        #[derive(sqlx::FromRow)]
        struct PnlRow {
            asset_id: String,
            total_pnl: Option<i64>,
        }
        let rows = sqlx::query_as!(
            PnlRow,
            r#"SELECT asset_id, SUM(realized_pnl) as "total_pnl: i64"
               FROM transactions
               WHERE account_id = ? AND transaction_type = 'Sell'
               GROUP BY asset_id"#,
            account_id
        )
        .fetch_all(&self.pool)
        .await
        .with_context(|| format!("Failed to fetch realized P&L for account {}", account_id))?;

        Ok(rows
            .into_iter()
            .map(|r| (r.asset_id, r.total_pnl.unwrap_or(0)))
            .collect())
    }

    async fn get_asset_ids_for_account(&self, account_id: &str) -> Result<Vec<String>> {
        let rows: Vec<String> = sqlx::query_scalar!(
            r#"SELECT DISTINCT asset_id as "asset_id: String" FROM transactions WHERE account_id = ? ORDER BY asset_id"#,
            account_id
        )
        .fetch_all(&self.pool)
        .await
        .with_context(|| format!("Failed to fetch asset IDs for account {}", account_id))?;

        Ok(rows)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let mut db_tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin DB transaction for transaction delete")?;
        // CFR-011 — the removal is based on the state this device holds.
        let based_on = sqlx::query_scalar!(
            r#"SELECT sync_logical_timestamp AS "sync_logical_timestamp?: String" FROM transactions WHERE id = ?"#,
            id
        )
        .fetch_optional(&mut *db_tx)
        .await
        .with_context(|| format!("Failed to read the rank of transaction {}", id))?
        .flatten()
        .and_then(|timestamp| LogicalTimestamp::from_wire(&timestamp));
        let deleted = sqlx::query!(r#"DELETE FROM transactions WHERE id = ?"#, id)
            .execute(&mut *db_tx)
            .await
            .with_context(|| format!("Failed to delete transaction {}", id))?;
        if deleted.rows_affected() > 0 {
            let draft = ChangeDraft::new(
                RecordKind::Transaction,
                RecordIdentity::canonical(RecordKind::Transaction, &[id]),
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
            .context("Failed to commit transaction delete")?;

        Ok(())
    }

    async fn has_transactions_for_asset(&self, asset_id: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count: i64" FROM transactions WHERE asset_id = ? LIMIT 1"#,
            asset_id
        )
        .fetch_one(&self.pool)
        .await
        .with_context(|| format!("Failed to check transactions for asset {}", asset_id))?;

        Ok(count > 0)
    }

    async fn count_by_account(&self, account_id: &str) -> Result<u32> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count: i64" FROM transactions WHERE account_id = ?"#,
            account_id
        )
        .fetch_one(&self.pool)
        .await
        .with_context(|| format!("Failed to count transactions for account {}", account_id))?;

        Ok(count as u32)
    }
}
