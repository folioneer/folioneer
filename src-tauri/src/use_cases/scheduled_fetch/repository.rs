//! Persistence for the scheduled-fetch use case (SPF-011, SPF-050). A deliberate
//! divergence from "use cases orchestrate, contexts own persistence" (spec
//! Context): `ScheduledFetchConfiguration` and `ScheduledFetchRun` are
//! operational records of the orchestration itself, belonging to no existing
//! bounded context — recorded in `docs/ddd-divergences.md` at implementation.

use super::error::ScheduledFetchError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::str::FromStr;

/// The device-wide configuration of the daily download (SPF-010). Exactly one
/// configuration exists (singleton row, migration-seeded).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct ScheduledFetchConfiguration {
    /// Whether the daily download is active on this device. Off by default (SPF-010).
    pub enabled: bool,
    /// Local wall-clock time of day the download runs, "HH:MM" (SPF-014).
    pub trigger_time: String,
}

impl ScheduledFetchConfiguration {
    /// Validates `trigger_time` is a well-formed "HH:MM" time of day (hours
    /// 00–23, minutes 00–59) before constructing the configuration (SPF-019).
    pub fn new(enabled: bool, trigger_time: String) -> Result<Self, ScheduledFetchError> {
        let well_formed = trigger_time.len() == 5
            && trigger_time.as_bytes().get(2) == Some(&b':')
            && trigger_time
                .get(..2)
                .is_some_and(|hours| hours.parse::<u8>().is_ok_and(|value| value <= 23))
            && trigger_time
                .get(3..)
                .is_some_and(|minutes| minutes.parse::<u8>().is_ok_and(|value| value <= 59));
        if !well_formed {
            return Err(ScheduledFetchError::InvalidTriggerTime);
        }
        Ok(Self {
            enabled,
            trigger_time,
        })
    }

    /// Restores a configuration from storage without validation (B7 — already
    /// validated at write time).
    pub fn restore(enabled: bool, trigger_time: String) -> Self {
        Self {
            enabled,
            trigger_time,
        }
    }
}

/// Outcome of one scheduled-fetch run (SPF-050).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Type,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum ScheduledFetchOutcome {
    /// The run completed its sweep (including a zero-update empty scope, SPF-042).
    Succeeded,
    /// The provider was unreachable after the bounded retry budget (SPF-051).
    Failed,
    /// The once-per-day guard exited the run before any external call (SPF-021).
    SkippedAlreadyRun,
}

/// The record of one execution of the scheduled download (SPF-050). Runs
/// accumulate as an auditable history and power the once-per-day guard
/// (SPF-021) and the settings status line (SPF-052).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct ScheduledFetchRun {
    /// When the run actually executed — may be later than the trigger when
    /// catching up (SPF-022).
    pub executed_at: String,
    /// The calendar day this run settles — always the latest pending trigger
    /// at execution time (SPF-021, SPF-022).
    pub trigger_date: String,
    /// Whether the run succeeded, failed, or was guard-skipped.
    pub outcome: ScheduledFetchOutcome,
    /// Number of assets whose prices were written by this run (SPF-050).
    pub updated_count: u32,
    /// Number of in-scope assets the run could not price (SPF-041).
    pub skipped_count: u32,
}

impl ScheduledFetchRun {
    /// Builds a run record. No cross-field validation exists for this
    /// use-case-owned operational record — every field is orchestrator-derived.
    pub fn new(
        executed_at: String,
        trigger_date: String,
        outcome: ScheduledFetchOutcome,
        updated_count: u32,
        skipped_count: u32,
    ) -> Self {
        Self {
            executed_at,
            trigger_date,
            outcome,
            updated_count,
            skipped_count,
        }
    }

    /// Restores a run from storage (B7 — no validation).
    pub fn restore(
        executed_at: String,
        trigger_date: String,
        outcome: ScheduledFetchOutcome,
        updated_count: u32,
        skipped_count: u32,
    ) -> Self {
        Self::new(
            executed_at,
            trigger_date,
            outcome,
            updated_count,
            skipped_count,
        )
    }
}

/// Wire-facing status returned by `get_scheduled_fetch_status` (SPF-052).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct ScheduledFetchStatus {
    /// The current device configuration.
    pub configuration: ScheduledFetchConfiguration,
    /// The most recent run, or `None` when no run has ever executed (SPF-052 — fresh install).
    pub last_run: Option<ScheduledFetchRun>,
}

/// Persistence for the scheduled-fetch use case (SPF-011, SPF-021, SPF-050).
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait ScheduledFetchRepository: Send + Sync {
    /// Reads the singleton configuration (always present — migration-seeded).
    async fn get_configuration(&self) -> anyhow::Result<ScheduledFetchConfiguration>;
    /// Persists the singleton configuration (SPF-011 — lives with the app data).
    async fn save_configuration(&self, enabled: bool, trigger_time: &str) -> anyhow::Result<()>;
    /// Returns the most recent run of any outcome, or `None` on a fresh install (SPF-052).
    async fn last_run(&self) -> anyhow::Result<Option<ScheduledFetchRun>>;
    /// Returns the most recent *successful* run — the once-per-day guard and
    /// backfill-window anchor (SPF-021, SPF-031).
    async fn last_successful_run(&self) -> anyhow::Result<Option<ScheduledFetchRun>>;
    /// Records the outcome of a run (SPF-050 — every path is recorded).
    async fn record_run(&self, run: ScheduledFetchRun) -> anyhow::Result<()>;
}

#[derive(sqlx::FromRow)]
struct ScheduledFetchConfigurationRow {
    enabled: bool,
    trigger_time: String,
}

impl From<ScheduledFetchConfigurationRow> for ScheduledFetchConfiguration {
    fn from(row: ScheduledFetchConfigurationRow) -> Self {
        ScheduledFetchConfiguration::restore(row.enabled, row.trigger_time)
    }
}

#[derive(sqlx::FromRow)]
struct ScheduledFetchRunRow {
    executed_at: String,
    trigger_date: String,
    outcome: String,
    updated_count: i64,
    skipped_count: i64,
}

impl From<ScheduledFetchRunRow> for ScheduledFetchRun {
    fn from(row: ScheduledFetchRunRow) -> Self {
        let outcome = ScheduledFetchOutcome::from_str(&row.outcome).unwrap_or_else(|_| {
            tracing::warn!(
                target: crate::core::logger::BACKEND,
                value = %row.outcome,
                "unknown scheduled_fetch_runs.outcome value, falling back to Failed"
            );
            ScheduledFetchOutcome::Failed
        });
        ScheduledFetchRun::restore(
            row.executed_at,
            row.trigger_date,
            outcome,
            row.updated_count as u32,
            row.skipped_count as u32,
        )
    }
}

/// SQLite implementation of [`ScheduledFetchRepository`].
pub struct SqliteScheduledFetchRepository {
    pool: sqlx::Pool<sqlx::Sqlite>,
}

impl SqliteScheduledFetchRepository {
    /// Creates a new repository backed by the given connection pool.
    pub fn new(pool: sqlx::Pool<sqlx::Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ScheduledFetchRepository for SqliteScheduledFetchRepository {
    async fn get_configuration(&self) -> anyhow::Result<ScheduledFetchConfiguration> {
        let row = sqlx::query_as!(
            ScheduledFetchConfigurationRow,
            r#"SELECT enabled AS "enabled: bool", trigger_time
               FROM scheduled_fetch_configuration WHERE id = 1"#,
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into())
    }

    async fn save_configuration(&self, enabled: bool, trigger_time: &str) -> anyhow::Result<()> {
        sqlx::query!(
            "UPDATE scheduled_fetch_configuration SET enabled = ?, trigger_time = ? WHERE id = 1",
            enabled,
            trigger_time,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn last_run(&self) -> anyhow::Result<Option<ScheduledFetchRun>> {
        let row = sqlx::query_as!(
            ScheduledFetchRunRow,
            r#"SELECT executed_at, trigger_date, outcome,
                      updated_count AS "updated_count: i64",
                      skipped_count AS "skipped_count: i64"
               FROM scheduled_fetch_runs
               ORDER BY executed_at DESC, rowid DESC LIMIT 1"#,
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn last_successful_run(&self) -> anyhow::Result<Option<ScheduledFetchRun>> {
        let row = sqlx::query_as!(
            ScheduledFetchRunRow,
            r#"SELECT executed_at, trigger_date, outcome,
                      updated_count AS "updated_count: i64",
                      skipped_count AS "skipped_count: i64"
               FROM scheduled_fetch_runs
               WHERE outcome = 'Succeeded'
               ORDER BY executed_at DESC, rowid DESC LIMIT 1"#,
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn record_run(&self, run: ScheduledFetchRun) -> anyhow::Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        let outcome = run.outcome.to_string();
        let updated_count = i64::from(run.updated_count);
        let skipped_count = i64::from(run.skipped_count);
        sqlx::query!(
            "INSERT INTO scheduled_fetch_runs
                 (id, executed_at, trigger_date, outcome, updated_count, skipped_count)
             VALUES (?, ?, ?, ?, ?, ?)",
            id,
            run.executed_at,
            run.trigger_date,
            outcome,
            updated_count,
            skipped_count,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn make_pool() -> sqlx::Pool<sqlx::Sqlite> {
        let opts = SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(false);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .expect("test pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations");
        pool
    }

    // -------------------------------------------------------------------------
    // ScheduledFetchConfiguration::new — SPF-019
    // -------------------------------------------------------------------------

    // SPF-019 — a well-formed "HH:MM" trigger time is accepted.
    #[test]
    fn new_accepts_a_well_formed_trigger_time() {
        let configuration = ScheduledFetchConfiguration::new(true, "22:15".to_string()).unwrap();
        assert_eq!(configuration.trigger_time, "22:15");
        assert!(configuration.enabled);
    }

    // SPF-019 — hour 24 is out of range and rejected.
    #[test]
    fn new_rejects_hour_out_of_range() {
        let err = ScheduledFetchConfiguration::new(true, "24:00".to_string()).unwrap_err();
        assert!(
            matches!(err, ScheduledFetchError::InvalidTriggerTime),
            "got: {err:?}"
        );
    }

    // SPF-019 — a single-digit minute ("9:5") is not well-formed "HH:MM".
    #[test]
    fn new_rejects_malformed_single_digit_time() {
        let err = ScheduledFetchConfiguration::new(true, "9:5".to_string()).unwrap_err();
        assert!(
            matches!(err, ScheduledFetchError::InvalidTriggerTime),
            "got: {err:?}"
        );
    }

    // SPF-019 — a non-numeric time is rejected.
    #[test]
    fn new_rejects_non_numeric_time() {
        let err = ScheduledFetchConfiguration::new(true, "aa:bb".to_string()).unwrap_err();
        assert!(
            matches!(err, ScheduledFetchError::InvalidTriggerTime),
            "got: {err:?}"
        );
    }

    // -------------------------------------------------------------------------
    // SqliteScheduledFetchRepository — real SQLite (Tier 2)
    // -------------------------------------------------------------------------

    // SPF-018 — a fresh migrated database seeds the default configuration:
    // disabled, trigger_time = 22:15.
    #[tokio::test]
    async fn get_configuration_returns_the_seeded_default_on_a_fresh_database() {
        let pool = make_pool().await;
        let repo = SqliteScheduledFetchRepository::new(pool);
        let configuration = repo.get_configuration().await.unwrap();
        assert!(!configuration.enabled, "must default to disabled (SPF-010)");
        assert_eq!(
            configuration.trigger_time, "22:15",
            "must default to 22:15 (SPF-018)"
        );
    }

    // SPF-011 — save_configuration then get_configuration round-trips the new values.
    #[tokio::test]
    async fn save_configuration_then_get_configuration_round_trips() {
        let pool = make_pool().await;
        let repo = SqliteScheduledFetchRepository::new(pool);
        repo.save_configuration(true, "19:00").await.unwrap();
        let configuration = repo.get_configuration().await.unwrap();
        assert!(configuration.enabled);
        assert_eq!(configuration.trigger_time, "19:00");
    }

    // SPF-052 — last_run returns None on a fresh database (no runs recorded yet).
    #[tokio::test]
    async fn last_run_returns_none_on_a_fresh_database() {
        let pool = make_pool().await;
        let repo = SqliteScheduledFetchRepository::new(pool);
        assert_eq!(repo.last_run().await.unwrap(), None);
    }

    // SPF-050 — record_run then last_run returns the recorded run.
    #[tokio::test]
    async fn record_run_then_last_run_returns_the_recorded_run() {
        let pool = make_pool().await;
        let repo = SqliteScheduledFetchRepository::new(pool);
        let run = ScheduledFetchRun::new(
            "2026-06-08T22:15:00".to_string(),
            "2026-06-08".to_string(),
            ScheduledFetchOutcome::Succeeded,
            12,
            2,
        );
        repo.record_run(run.clone()).await.unwrap();
        let last = repo.last_run().await.unwrap().expect("a run must exist");
        assert_eq!(last, run);
    }

    // SPF-021 — last_successful_run only returns Succeeded runs, skipping a
    // more recent SkippedAlreadyRun/Failed record.
    #[tokio::test]
    async fn last_successful_run_ignores_non_succeeded_runs() {
        let pool = make_pool().await;
        let repo = SqliteScheduledFetchRepository::new(pool);
        let succeeded = ScheduledFetchRun::new(
            "2026-06-07T22:15:00".to_string(),
            "2026-06-07".to_string(),
            ScheduledFetchOutcome::Succeeded,
            5,
            0,
        );
        let failed = ScheduledFetchRun::new(
            "2026-06-08T22:15:00".to_string(),
            "2026-06-08".to_string(),
            ScheduledFetchOutcome::Failed,
            0,
            0,
        );
        repo.record_run(succeeded.clone()).await.unwrap();
        repo.record_run(failed).await.unwrap();

        let last_successful = repo
            .last_successful_run()
            .await
            .unwrap()
            .expect("a successful run must exist");
        assert_eq!(last_successful, succeeded);
    }
}
