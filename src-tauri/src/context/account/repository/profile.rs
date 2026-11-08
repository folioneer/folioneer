//! PFL — the profiles' storage; each write records its change for the sync (PFL-060).

use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::{Pool, Sqlite, SqliteConnection};
use std::sync::Arc;

use crate::context::account::domain::{
    Profile, ProfileRepository, ProfileSummary, DEFAULT_PROFILE_ID,
};
use crate::shared::domain::{
    ChangeDraft, LogicalTimestamp, Operation, Origin, RecordIdentity, RecordKind,
};
use crate::shared::infrastructure::change_recorder::{
    ChangeRecorder, NoopChangeRecorder, RankColumns,
};

/// The profiles' SQLite storage.
pub struct SqliteProfileRepository {
    pool: Pool<Sqlite>,
    change_recorder: Arc<dyn ChangeRecorder>,
}

impl SqliteProfileRepository {
    /// A repository over `pool` that records nothing until a recorder is given.
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self {
            pool,
            change_recorder: Arc::new(NoopChangeRecorder),
        }
    }

    /// The repository recording each write for the sync (PFL-060).
    pub fn with_change_recorder(mut self, change_recorder: Arc<dyn ChangeRecorder>) -> Self {
        self.change_recorder = change_recorder;
        self
    }
}

/// CFR-012 — a profile is identified by its identifier.
pub(crate) fn profile_identity(id: &str) -> RecordIdentity {
    RecordIdentity::canonical(RecordKind::Profile, &[id])
}

async fn current_timestamp(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<Option<LogicalTimestamp>> {
    let stored = sqlx::query_scalar!(
        r#"SELECT sync_logical_timestamp AS "sync_logical_timestamp?: String" FROM profiles WHERE id = ?"#,
        id
    )
    .fetch_optional(conn)
    .await
    .context("read profile rank")?;
    Ok(stored
        .flatten()
        .and_then(|timestamp| LogicalTimestamp::from_wire(&timestamp)))
}

/// PFL-012 / PFL-014 — the profile in use, read on `pool`: the chosen one while it exists,
/// the default profile otherwise. The one query both repositories of the context use.
pub(super) async fn profile_in_use(pool: &Pool<Sqlite>) -> Result<String> {
    let row = sqlx::query!(
        r#"SELECT COALESCE(
               (SELECT p.id FROM profiles p JOIN profile_in_use u ON u.profile_id = p.id),
               ?) AS "id!: String""#,
        DEFAULT_PROFILE_ID
    )
    .fetch_one(pool)
    .await
    .context("read the profile in use")?;
    Ok(row.id)
}

#[async_trait]
impl ProfileRepository for SqliteProfileRepository {
    async fn list(&self) -> Result<Vec<ProfileSummary>> {
        // PFL-023 / PFL-061 — an account whose profile was removed elsewhere counts in the
        // default profile ('default-profile', DEFAULT_PROFILE_ID).
        let rows = sqlx::query!(
            r#"SELECT p.id AS "id!", p.name, COUNT(a.id) AS "account_count!: i64"
               FROM profiles p LEFT JOIN accounts a
                 ON COALESCE((SELECT q.id FROM profiles q WHERE q.id = a.profile_id),
                             'default-profile') = p.id
               GROUP BY p.id, p.name"#
        )
        .fetch_all(&self.pool)
        .await
        .context("list profiles")?;
        Ok(rows
            .into_iter()
            .map(|row| ProfileSummary {
                profile: Profile::restore(row.id, row.name),
                account_count: u32::try_from(row.account_count).unwrap_or(u32::MAX),
            })
            .collect())
    }

    async fn get(&self, id: &str) -> Result<Option<Profile>> {
        let row = sqlx::query!(r#"SELECT id AS "id!", name FROM profiles WHERE id = ?"#, id)
            .fetch_optional(&self.pool)
            .await
            .context("get profile")?;
        Ok(row.map(|row| Profile::restore(row.id, row.name)))
    }

    async fn name_taken(&self, name: &str, except_id: Option<String>) -> Result<bool> {
        let except = except_id.unwrap_or_default();
        let found = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!: i64" FROM profiles
               WHERE name IS NOT NULL AND LOWER(name) = LOWER(?) AND id != ?"#,
            name,
            except
        )
        .fetch_one(&self.pool)
        .await
        .context("profile name taken")?;
        Ok(found > 0)
    }

    async fn save(&self, profile: &Profile) -> Result<()> {
        let mut tx = self.pool.begin().await.context("begin save profile")?;
        let existing = sqlx::query_scalar!("SELECT id FROM profiles WHERE id = ?", profile.id)
            .fetch_optional(&mut *tx)
            .await
            .context("lookup profile")?;
        let based_on = current_timestamp(&mut tx, &profile.id).await?;
        sqlx::query!(
            r#"INSERT INTO profiles (id, name) VALUES (?, ?)
               ON CONFLICT(id) DO UPDATE SET name = excluded.name"#,
            profile.id,
            profile.name
        )
        .execute(&mut *tx)
        .await
        .context("save profile")?;
        let operation = if existing.is_some() {
            Operation::Updated
        } else {
            Operation::Created
        };
        let draft = ChangeDraft::new(
            RecordKind::Profile,
            profile_identity(&profile.id),
            operation,
            Origin::User,
            based_on,
            Some(serde_json::to_string(profile)?),
        );
        if let Some(rank) = self.change_recorder.record(&mut tx, draft).await? {
            let columns = RankColumns::from(rank);
            sqlx::query!(
                r#"UPDATE profiles
                   SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
                   WHERE id = ?"#,
                columns.logical_timestamp,
                columns.origin,
                columns.device_id,
                profile.id
            )
            .execute(&mut *tx)
            .await
            .context("stamp profile rank")?;
        }
        tx.commit().await.context("commit save profile")?;
        Ok(())
    }

    async fn in_use(&self) -> Result<String> {
        profile_in_use(&self.pool).await
    }

    async fn set_in_use(&self, id: &str) -> Result<()> {
        sqlx::query!(
            "INSERT INTO profile_in_use (id, profile_id) VALUES (1, ?)
             ON CONFLICT(id) DO UPDATE SET profile_id = excluded.profile_id",
            id
        )
        .execute(&self.pool)
        .await
        .context("choose the profile in use")?;
        Ok(())
    }

    async fn delete_if_empty(&self, id: &str) -> Result<bool> {
        let mut tx = self.pool.begin().await.context("begin delete profile")?;
        let based_on = current_timestamp(&mut tx, id).await?;
        // PFL-011 / PFL-040 — never the default profile ('default-profile', DEFAULT_PROFILE_ID),
        // never one an account belongs to: checked in the statement that removes.
        let deleted = sqlx::query!(
            r#"DELETE FROM profiles
               WHERE id = ? AND id != 'default-profile'
                 AND NOT EXISTS (SELECT 1 FROM accounts WHERE accounts.profile_id = profiles.id)"#,
            id
        )
        .execute(&mut *tx)
        .await
        .context("delete profile")?;
        let removed = deleted.rows_affected() > 0;
        if removed {
            let draft = ChangeDraft::new(
                RecordKind::Profile,
                profile_identity(id),
                Operation::Removed,
                Origin::User,
                based_on,
                None,
            );
            self.change_recorder.record(&mut tx, draft).await?;
        }
        tx.commit().await.context("commit delete profile")?;
        Ok(removed)
    }
}
