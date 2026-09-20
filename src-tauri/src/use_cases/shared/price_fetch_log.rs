//! This installation's price fetch log (MKT-201): written by the in-app fetch task and
//! the scheduled download, read by the price freshness use case — owned by none of them.

use anyhow::Context;
use async_trait::async_trait;
use chrono::NaiveDateTime;

/// Injectable source of the local wall-clock "now" so tests can fix the recorded moment.
pub type FetchMomentClock = std::sync::Arc<dyn Fn() -> NaiveDateTime + Send + Sync>;

/// Remembers when this installation last completed a price fetch (MKT-201).
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait PriceFetchLogRepository: Send + Sync {
    /// Records `completed_at`, a local wall-clock moment, as the moment a fetch completed.
    /// An earlier moment never replaces a later one.
    async fn record_completed(&self, completed_at: NaiveDateTime) -> anyhow::Result<()>;
    /// The moment the last fetch completed (`YYYY-MM-DDTHH:MM:SS`), if any ever did.
    async fn last_completed_at(&self) -> anyhow::Result<Option<String>>;
}

/// SQLite implementation of [`PriceFetchLogRepository`].
pub struct SqlitePriceFetchLogRepository {
    pool: sqlx::Pool<sqlx::Sqlite>,
}

impl SqlitePriceFetchLogRepository {
    /// Creates a new repository backed by the given connection pool.
    pub fn new(pool: sqlx::Pool<sqlx::Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PriceFetchLogRepository for SqlitePriceFetchLogRepository {
    async fn record_completed(&self, completed_at: NaiveDateTime) -> anyhow::Result<()> {
        // The format scheduled runs use, so the moments the migration seeds compare as text.
        let completed_at = completed_at.format("%Y-%m-%dT%H:%M:%S").to_string();
        sqlx::query!(
            "INSERT INTO price_fetch_log (id, last_completed_at) VALUES (1, ?)
             ON CONFLICT (id) DO UPDATE
             SET last_completed_at = MAX(last_completed_at, excluded.last_completed_at)",
            completed_at,
        )
        .execute(&self.pool)
        .await
        .context("recording the price fetch moment")?;
        Ok(())
    }

    async fn last_completed_at(&self) -> anyhow::Result<Option<String>> {
        let row = sqlx::query_scalar!("SELECT last_completed_at FROM price_fetch_log WHERE id = 1")
            .fetch_optional(&self.pool)
            .await
            .context("reading the price fetch moment")?;
        Ok(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moment(month: u32, day: u32, hour: u32, minute: u32, second: u32) -> NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, month, day)
            .and_then(|date| date.and_hms_opt(hour, minute, second))
            .expect("valid moment")
    }

    async fn make_pool() -> sqlx::Pool<sqlx::Sqlite> {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
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

    // MKT-201 — nothing is recorded until a fetch completes.
    #[tokio::test]
    async fn reports_no_moment_before_any_fetch() {
        let repo = SqlitePriceFetchLogRepository::new(make_pool().await);
        assert_eq!(repo.last_completed_at().await.unwrap(), None);
    }

    // MKT-201 — a later fetch replaces the recorded moment; the table keeps one row.
    #[tokio::test]
    async fn keeps_only_the_latest_recorded_moment() {
        let pool = make_pool().await;
        let repo = SqlitePriceFetchLogRepository::new(pool.clone());
        repo.record_completed(moment(9, 18, 8, 0, 0)).await.unwrap();
        repo.record_completed(moment(9, 19, 8, 14, 0))
            .await
            .unwrap();
        assert_eq!(
            repo.last_completed_at().await.unwrap().as_deref(),
            Some("2026-09-19T08:14:00")
        );
        let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM price_fetch_log")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows, 1);
    }

    // MKT-201 — the in-app task and the scheduled download both write here; whichever
    // writes last, the later moment stays.
    #[tokio::test]
    async fn an_earlier_moment_never_replaces_a_later_one() {
        let repo = SqlitePriceFetchLogRepository::new(make_pool().await);
        repo.record_completed(moment(9, 19, 8, 14, 0))
            .await
            .unwrap();
        repo.record_completed(moment(9, 18, 22, 15, 3))
            .await
            .unwrap();
        assert_eq!(
            repo.last_completed_at().await.unwrap().as_deref(),
            Some("2026-09-19T08:14:00")
        );
    }

    // MKT-201 — the migration starts a device from its last successful scheduled download,
    // and running it again changes nothing.
    #[tokio::test]
    async fn the_migration_seeds_the_log_from_the_last_successful_scheduled_run() {
        let pool = make_pool().await;
        for (id, executed_at, outcome) in [
            ("run-1", "2026-08-27T22:15:02", "Succeeded"),
            ("run-2", "2026-08-28T09:30:28", "Succeeded"),
            ("run-3", "2026-08-29T22:15:01", "Failed"),
        ] {
            sqlx::query(
                "INSERT INTO scheduled_fetch_runs
                 (id, executed_at, trigger_date, outcome, updated_count, skipped_count)
                 VALUES (?, ?, '2026-08-28', ?, 0, 0)",
            )
            .bind(id)
            .bind(executed_at)
            .bind(outcome)
            .execute(&pool)
            .await
            .unwrap();
        }
        let migration = include_str!("../../../migrations/202609190001_create_price_fetch_log.sql");
        let repo = SqlitePriceFetchLogRepository::new(pool.clone());

        sqlx::raw_sql(migration).execute(&pool).await.unwrap();
        assert_eq!(
            repo.last_completed_at().await.unwrap().as_deref(),
            Some("2026-08-28T09:30:28")
        );

        repo.record_completed(moment(9, 19, 8, 14, 0))
            .await
            .unwrap();
        sqlx::raw_sql(migration).execute(&pool).await.unwrap();
        assert_eq!(
            repo.last_completed_at().await.unwrap().as_deref(),
            Some("2026-09-19T08:14:00")
        );
    }
}
