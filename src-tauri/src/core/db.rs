use std::{fs, path::PathBuf};

use anyhow::Context;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    ConnectOptions, Pool, Sqlite,
};

use crate::core::logger::BACKEND;

const DATABASE_FILENAME: &str = "portfolio";

/// Manages the SQLite database connection and migrations.
pub struct Database {
    /// The underlying SQLx connection pool.
    pub pool: Pool<Sqlite>,
}

impl Database {
    /// Initializes the database at the specified path and runs pending migrations.
    pub async fn new(app_data_dir: PathBuf) -> anyhow::Result<Self> {
        // Check if database reset is requested
        let is_db_reset = std::env::var("RESET_DATABASE")
            .map(|val| val.to_lowercase() == "true" || val == "1")
            .unwrap_or_default();

        let db_path = app_data_dir.join(DATABASE_FILENAME);
        if !db_path.exists() {
            fs::File::create(&db_path)
                .with_context(|| format!("Failed to create database file {:?}", db_path))?;
        }

        // Handle database reset if requested
        if is_db_reset {
            tracing::warn!("RESET_DATABASE is set - deleting existing database");
            if db_path.exists() {
                fs::remove_file(&db_path).with_context(|| "Failed to delete database")?;
                tracing::info!("Database deleted successfully");
            } else {
                tracing::info!("Database does not exist, skipping delete");
            }
        }

        tracing::trace!(target: BACKEND, "Connecting to database: {}", db_path.to_string_lossy());

        // WAL + busy_timeout: the OS-scheduled headless run (SPF-020/023) can
        // execute while the interactive app holds the same database — WAL lets
        // the two processes interleave reads/writes, busy_timeout absorbs
        // short write-lock contention instead of erroring.
        let connect_options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_secs(5))
            .disable_statement_logging();

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(connect_options)
            .await
            .with_context(|| format!("Failed to connect to SQLite at {:?}", db_path))?;

        let db = Database { pool };

        // Initialize tables via migrations
        sqlx::migrate!("./migrations")
            .run(&db.pool)
            .await
            .with_context(|| "Failed to run database migrations")?;

        Ok(db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // SPF-023 — the OS-scheduled headless run and the interactive app share
    // this database as two processes; WAL is the config that makes their
    // interleaved reads/writes safe. Locks the pragma in so a connect-options
    // change can't silently drop it.
    #[tokio::test]
    async fn database_opens_in_wal_journal_mode() {
        let dir = tempfile::tempdir().expect("temp dir");
        let db = Database::new(dir.path().to_path_buf())
            .await
            .expect("database");
        let row: (String,) = sqlx::query_as("PRAGMA journal_mode")
            .fetch_one(&db.pool)
            .await
            .expect("pragma query");
        assert_eq!(row.0.to_lowercase(), "wal");
    }
}
