//! Unit of work (ADR-006, B26): one database transaction several bounded contexts write
//! through, committed once by the use case that opened it. The use case holds it as an
//! opaque handle and passes it to the services; only repositories reach its connection.

use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::{Pool, Sqlite, SqliteConnection, Transaction};

/// One open transaction across bounded contexts. Committed by `commit`; dropped without
/// it, everything written through it is rolled back.
pub struct UnitOfWork {
    transaction: Transaction<'static, Sqlite>,
}

impl UnitOfWork {
    /// The connection a repository writes on — for repositories only, never a use case.
    pub fn connection(&mut self) -> &mut SqliteConnection {
        &mut self.transaction
    }

    /// Makes every write of the unit permanent, together.
    pub async fn commit(self) -> Result<()> {
        self.transaction
            .commit()
            .await
            .context("Failed to commit the unit of work")
    }
}

/// Opens units of work; injected into the use cases that write across bounded contexts.
#[async_trait]
pub trait TransactionManager: Send + Sync {
    /// A new unit of work, nothing written yet.
    async fn begin(&self) -> Result<UnitOfWork>;
}

/// `TransactionManager` over the application's SQLite pool.
pub struct SqliteTransactionManager {
    pool: Pool<Sqlite>,
}

impl SqliteTransactionManager {
    /// A transaction manager over `pool`.
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TransactionManager for SqliteTransactionManager {
    async fn begin(&self) -> Result<UnitOfWork> {
        let transaction = self
            .pool
            .begin()
            .await
            .context("Failed to begin a unit of work")?;
        Ok(UnitOfWork { transaction })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool_with_table() -> Pool<Sqlite> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool");
        sqlx::query("CREATE TABLE t (v INTEGER)")
            .execute(&pool)
            .await
            .expect("table");
        pool
    }

    async fn rows(pool: &Pool<Sqlite>) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM t")
            .fetch_one(pool)
            .await
            .expect("count")
    }

    // ADR-006 — what a committed unit wrote stays.
    #[tokio::test]
    async fn a_committed_unit_keeps_its_writes() {
        let pool = pool_with_table().await;
        let manager = SqliteTransactionManager::new(pool.clone());

        let mut unit = manager.begin().await.expect("begin");
        sqlx::query("INSERT INTO t VALUES (1)")
            .execute(unit.connection())
            .await
            .expect("insert");
        unit.commit().await.expect("commit");

        assert_eq!(rows(&pool).await, 1);
    }

    // ADR-006 — a unit dropped without its commit leaves nothing.
    #[tokio::test]
    async fn a_unit_dropped_without_commit_writes_nothing() {
        let pool = pool_with_table().await;
        let manager = SqliteTransactionManager::new(pool.clone());

        let mut unit = manager.begin().await.expect("begin");
        sqlx::query("INSERT INTO t VALUES (1)")
            .execute(unit.connection())
            .await
            .expect("insert");
        drop(unit);

        assert_eq!(rows(&pool).await, 0);
    }
}
