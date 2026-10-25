use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;
use serde::Serialize;
use sqlx::{
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteConnection, SqlitePoolOptions},
    ConnectOptions, Connection, Pool, Sqlite,
};

use crate::core::logger::BACKEND;

const DATABASE_FILENAME: &str = "portfolio";

/// UPD-032 — the folder beside the portfolio that holds the copies made before a migration.
const BACKUPS_DIRNAME: &str = "backups";

/// UPD-033 — how many copies are kept.
const BACKUPS_KEPT: usize = 5;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// UPD-034 — a migration failed and the copy made before it was put back.
#[derive(Debug)]
pub struct MigrationRestored {
    /// The folder that holds the copies.
    pub backups_dir: PathBuf,
    source: sqlx::migrate::MigrateError,
}

impl std::fmt::Display for MigrationRestored {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Failed to run database migrations")
    }
}

impl std::error::Error for MigrationRestored {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// UPD-018 / UPD-035 — what the window is told when the portfolio could not be opened.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct MigrationFailure {
    /// The error, as the log has it.
    pub detail: String,
    /// The folder that holds the copies, when the portfolio was put back; null otherwise.
    pub backups_dir: Option<String>,
}

impl MigrationFailure {
    /// The failure of `error`, naming the copies' folder when the portfolio was put back.
    pub fn from_error(error: &anyhow::Error) -> Self {
        Self {
            detail: format!("{error:#}"),
            backups_dir: error
                .downcast_ref::<MigrationRestored>()
                .map(|restored| restored.backups_dir.to_string_lossy().into_owned()),
        }
    }
}

/// Manages the SQLite database connection and migrations.
pub struct Database {
    /// The underlying SQLx connection pool.
    pub pool: Pool<Sqlite>,
}

impl Database {
    /// Whether a portfolio has been created in `app_data_dir` (CLI-032).
    pub fn exists_in(app_data_dir: &std::path::Path) -> bool {
        app_data_dir.join(DATABASE_FILENAME).is_file()
    }

    /// Opens the existing database for reading only (CLI-018): it is neither created nor
    /// migrated and no data is written, so it can be read while the window owns it. SQLite
    /// may leave its own empty side files beside it.
    pub async fn open_read_only(app_data_dir: &std::path::Path) -> anyhow::Result<Self> {
        let db_path = app_data_dir.join(DATABASE_FILENAME);
        let connect_options = SqliteConnectOptions::new()
            .filename(&db_path)
            .read_only(true)
            .busy_timeout(std::time::Duration::from_secs(5))
            .disable_statement_logging();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(connect_options)
            .await
            .with_context(|| format!("Failed to open SQLite read-only at {:?}", db_path))?;
        Ok(Database { pool })
    }

    /// Initializes the database at the specified path and runs pending migrations.
    pub async fn new(app_data_dir: PathBuf) -> anyhow::Result<Self> {
        Self::open_with(app_data_dir, &MIGRATOR).await
    }

    async fn open_with(app_data_dir: PathBuf, migrator: &Migrator) -> anyhow::Result<Self> {
        let is_db_reset = reset_requested(
            cfg!(debug_assertions),
            std::env::var("RESET_DATABASE").ok().as_deref(),
        );

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

        // UPD-032 / UPD-034 — the migrations run on one connection of their own, closed before
        // anything else: a pool may still be closing a connection in the background, whose
        // last checkpoint would write the failed run into the copy put back.
        let mut migration_conn = connect_options
            .connect()
            .await
            .with_context(|| format!("Failed to connect to SQLite at {:?}", db_path))?;
        let backups_dir = app_data_dir.join(BACKUPS_DIRNAME);
        let copy = if has_pending_migrations(&mut migration_conn, migrator).await? {
            Some(back_up(&mut migration_conn, &backups_dir).await?)
        } else {
            None
        };
        let migrated = migrator.run(&mut migration_conn).await;
        let closed = migration_conn.close().await;

        if let Err(source) = migrated {
            let Some(copy) = copy else {
                return Err(anyhow::Error::new(source).context("Failed to run database migrations"));
            };
            if let Err(e) = closed {
                tracing::warn!(target: BACKEND, err = %e, "Migration connection not closed cleanly");
            }
            restore(&db_path, &copy)?;
            tracing::warn!(target: BACKEND, copy = %copy.display(), "Migration failed: portfolio put back (UPD-034)");
            return Err(MigrationRestored {
                backups_dir,
                source,
            }
            .into());
        }
        closed.context("Failed to close the migration connection")?;

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(connect_options)
            .await
            .with_context(|| format!("Failed to connect to SQLite at {:?}", db_path))?;

        Ok(Database { pool })
    }
}

/// UPD-032 — an existing portfolio with a migration not yet applied. A new portfolio
/// (no migration table yet) has nothing to copy.
async fn has_pending_migrations(
    conn: &mut SqliteConnection,
    migrator: &Migrator,
) -> anyhow::Result<bool> {
    let (tracked,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_one(&mut *conn)
    .await
    .context("Failed to read the migration table")?;
    if tracked == 0 {
        return Ok(false);
    }
    let applied: Vec<(i64,)> =
        sqlx::query_as("SELECT version FROM _sqlx_migrations WHERE success = 1")
            .fetch_all(&mut *conn)
            .await
            .context("Failed to read the applied migrations")?;
    Ok(migrator
        .iter()
        .filter(|migration| migration.migration_type.is_up_migration())
        .any(|migration| {
            !applied
                .iter()
                .any(|(version,)| *version == migration.version)
        }))
}

/// UPD-032 — a consistent copy of the open portfolio into `backups_dir`, named by the
/// time of the copy, then only the newest `BACKUPS_KEPT` kept.
async fn back_up(conn: &mut SqliteConnection, backups_dir: &Path) -> anyhow::Result<PathBuf> {
    fs::create_dir_all(backups_dir)
        .with_context(|| format!("Failed to create {:?}", backups_dir))?;
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%3fZ");
    let copy = backups_dir.join(format!("{DATABASE_FILENAME}-{stamp}"));
    sqlx::query("VACUUM INTO ?")
        .bind(copy.to_string_lossy().into_owned())
        .execute(&mut *conn)
        .await
        .with_context(|| format!("Failed to copy the portfolio to {:?}", copy))?;
    tracing::info!(target: BACKEND, copy = %copy.display(), "Portfolio copied before migrating (UPD-032)");
    // A copy that cannot be pruned is harmless; it never stops the migration.
    if let Err(e) = prune_backups(backups_dir, BACKUPS_KEPT, &copy) {
        tracing::warn!(target: BACKEND, err = %e, "Older portfolio copies not pruned (UPD-033)");
    }
    Ok(copy)
}

/// UPD-033 — deletes all but the newest `kept` copies (their names sort by time).
fn prune_backups(backups_dir: &Path, kept: usize, current: &Path) -> anyhow::Result<()> {
    let prefix = format!("{DATABASE_FILENAME}-");
    let mut copies: Vec<PathBuf> = fs::read_dir(backups_dir)
        .with_context(|| format!("Failed to list {:?}", backups_dir))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix))
        })
        .collect();
    copies.sort();
    let excess = copies.len().saturating_sub(kept);
    // The copy just made is never pruned, even when the clock went back.
    for old in copies.iter().take(excess).filter(|old| *old != current) {
        fs::remove_file(old).with_context(|| format!("Failed to delete {:?}", old))?;
    }
    Ok(())
}

/// The WAL side files of the portfolio at `db_path`.
fn side_files(db_path: &Path) -> [PathBuf; 2] {
    ["-wal", "-shm"].map(|suffix| PathBuf::from(format!("{}{suffix}", db_path.to_string_lossy())))
}

/// UPD-034 — puts `copy` back in place of the closed portfolio at `db_path`.
fn restore(db_path: &Path, copy: &Path) -> anyhow::Result<()> {
    for side in side_files(db_path) {
        match fs::remove_file(&side) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err(e).with_context(|| format!("Failed to delete {:?}", side));
            }
            _ => {}
        }
    }
    // Copied beside the portfolio, then renamed over it: a crash leaves either file whole.
    let restoring = db_path.with_extension("restoring");
    fs::copy(copy, &restoring)
        .with_context(|| format!("Failed to copy {:?} beside the portfolio", copy))?;
    fs::rename(&restoring, db_path)
        .with_context(|| format!("Failed to put {:?} back in place of the portfolio", copy))?;
    Ok(())
}

/// Whether `RESET_DATABASE` asks for the database to be deleted before it is opened.
/// Only a debug build listens: a release binary never deletes the user's portfolio.
fn reset_requested(debug_build: bool, variable: Option<&str>) -> bool {
    debug_build && variable.is_some_and(|value| value.to_lowercase() == "true" || value == "1")
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO-040 — `just dev --reset-db` sets the variable; a debug build obeys it.
    #[test]
    fn a_debug_build_obeys_the_reset_variable() {
        assert!(reset_requested(true, Some("true")));
        assert!(reset_requested(true, Some("TRUE")));
        assert!(reset_requested(true, Some("1")));
        assert!(!reset_requested(true, Some("false")));
        assert!(!reset_requested(true, None));
    }

    // TODO-040 — a release binary ignores the variable, whatever it holds.
    #[test]
    fn a_release_build_ignores_the_reset_variable() {
        assert!(!reset_requested(false, Some("true")));
        assert!(!reset_requested(false, Some("1")));
    }

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

    /// A migrator over `migrations` (file name, SQL), written into a fresh folder.
    async fn migrator_of(migrations: &[(&str, &str)]) -> (tempfile::TempDir, Migrator) {
        let dir = tempfile::tempdir().expect("migrations dir");
        for (name, sql) in migrations {
            fs::write(dir.path().join(name), sql).expect("write migration");
        }
        let migrator = Migrator::new(dir.path()).await.expect("migrator");
        (dir, migrator)
    }

    const FIRST: (&str, &str) = (
        "1_first.sql",
        "CREATE TABLE kept (name TEXT); INSERT INTO kept VALUES ('history');",
    );
    const SECOND: (&str, &str) = ("2_second.sql", "CREATE TABLE added (name TEXT);");
    const BROKEN: (&str, &str) = ("3_broken.sql", "INSERT INTO missing VALUES (1);");

    fn copies_in(app_data_dir: &Path) -> Vec<PathBuf> {
        let Ok(entries) = fs::read_dir(app_data_dir.join(BACKUPS_DIRNAME)) else {
            return Vec::new();
        };
        entries.map(|entry| entry.expect("entry").path()).collect()
    }

    async fn table_exists(db: &Database, name: &str) -> bool {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?")
                .bind(name)
                .fetch_one(&db.pool)
                .await
                .expect("table query");
        count == 1
    }

    // UPD-032 — a new portfolio, and a launch with nothing to apply, copy nothing.
    #[tokio::test]
    async fn nothing_to_migrate_copies_nothing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let (_m, first) = migrator_of(&[FIRST]).await;
        Database::open_with(dir.path().to_path_buf(), &first)
            .await
            .expect("new")
            .pool
            .close()
            .await;
        Database::open_with(dir.path().to_path_buf(), &first)
            .await
            .expect("again")
            .pool
            .close()
            .await;
        assert!(copies_in(dir.path()).is_empty());
    }

    // UPD-032 — a migration to apply is preceded by a copy of the portfolio as it was.
    #[tokio::test]
    async fn a_migration_to_apply_is_preceded_by_a_copy() {
        let dir = tempfile::tempdir().expect("temp dir");
        let (_m1, first) = migrator_of(&[FIRST]).await;
        Database::open_with(dir.path().to_path_buf(), &first)
            .await
            .expect("v1")
            .pool
            .close()
            .await;

        let (_m2, second) = migrator_of(&[FIRST, SECOND]).await;
        let db = Database::open_with(dir.path().to_path_buf(), &second)
            .await
            .expect("v2");
        assert!(table_exists(&db, "added").await);

        let copies = copies_in(dir.path());
        assert_eq!(copies.len(), 1);
        let copy_dir = tempfile::tempdir().expect("copy dir");
        fs::copy(&copies[0], copy_dir.path().join(DATABASE_FILENAME)).expect("copy");
        let copy = Database::open_read_only(copy_dir.path())
            .await
            .expect("open copy");
        assert!(table_exists(&copy, "kept").await);
        assert!(!table_exists(&copy, "added").await);
    }

    // UPD-034 — a failed migration puts the copy back: what the run had applied is gone,
    // the history is there, and the error says where the copies are.
    #[tokio::test]
    async fn a_failed_migration_puts_the_portfolio_back() {
        let dir = tempfile::tempdir().expect("temp dir");
        let (_m1, first) = migrator_of(&[FIRST]).await;
        Database::open_with(dir.path().to_path_buf(), &first)
            .await
            .expect("v1")
            .pool
            .close()
            .await;

        let (_m2, broken) = migrator_of(&[FIRST, SECOND, BROKEN]).await;
        let error = Database::open_with(dir.path().to_path_buf(), &broken)
            .await
            .err()
            .expect("the broken migration fails");
        let failure = MigrationFailure::from_error(&error);
        assert_eq!(
            failure.backups_dir.as_deref(),
            Some(dir.path().join(BACKUPS_DIRNAME).to_string_lossy().as_ref())
        );

        let db = Database::open_with(dir.path().to_path_buf(), &first)
            .await
            .expect("v1 again");
        assert!(table_exists(&db, "kept").await);
        assert!(!table_exists(&db, "added").await);
        let (name,): (String,) = sqlx::query_as("SELECT name FROM kept")
            .fetch_one(&db.pool)
            .await
            .expect("history");
        assert_eq!(name, "history");
    }

    // UPD-035 — a failure with no copy (a new portfolio) names no folder.
    #[tokio::test]
    async fn a_failure_without_a_copy_names_no_folder() {
        let dir = tempfile::tempdir().expect("temp dir");
        let (_m, broken) = migrator_of(&[FIRST, BROKEN]).await;
        let error = Database::open_with(dir.path().to_path_buf(), &broken)
            .await
            .err()
            .expect("the broken migration fails");
        assert_eq!(MigrationFailure::from_error(&error).backups_dir, None);
    }

    // UPD-033 — the copy just made is kept even when the clock went back.
    #[test]
    fn the_copy_just_made_is_never_pruned() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = |stamp: u32| {
            dir.path()
                .join(format!("{DATABASE_FILENAME}-2026102{stamp}T000000000Z"))
        };
        for stamp in 2..=3 {
            fs::write(path(stamp), "").expect("copy");
        }

        prune_backups(dir.path(), 1, &path(2)).expect("prune");

        assert!(path(2).exists(), "the copy just made is kept");
    }

    // UPD-033 — only the newest copies are kept.
    #[test]
    fn only_the_newest_copies_are_kept() {
        let dir = tempfile::tempdir().expect("temp dir");
        for stamp in 1..=7 {
            fs::write(
                dir.path()
                    .join(format!("{DATABASE_FILENAME}-2026102{stamp}T000000000Z")),
                "",
            )
            .expect("copy");
        }
        fs::write(dir.path().join("notes.txt"), "").expect("other file");

        let newest = dir
            .path()
            .join(format!("{DATABASE_FILENAME}-20261027T000000000Z"));
        prune_backups(dir.path(), 5, &newest).expect("prune");

        let mut names: Vec<String> = fs::read_dir(dir.path())
            .expect("list")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        let expected: Vec<String> = std::iter::once("notes.txt".to_string())
            .chain((3..=7).map(|stamp| format!("{DATABASE_FILENAME}-2026102{stamp}T000000000Z")))
            .collect();
        assert_eq!(names, expected);
    }
}
