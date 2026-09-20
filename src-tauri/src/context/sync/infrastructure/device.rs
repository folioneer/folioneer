//! `SqliteSyncStateRepository` — the SQLite-backed `SyncStateRepository` (D2, PR-B): the
//! `sync_device` singleton, `sync_cursors`, `held_back_changes`, and `conflict_notices`.

use sqlx::{Pool, Sqlite, SqliteConnection};

use crate::context::sync::domain::{
    ConflictNotice, HeldBackChange, StoredDevice, SyncCursor, SyncDevice, SyncStateRepository,
};
use crate::context::sync::error::SyncError;
use crate::shared::domain::RecordKind;

/// SQLite-backed `SyncStateRepository`.
pub struct SqliteSyncStateRepository {
    pool: Pool<Sqlite>,
}

impl SqliteSyncStateRepository {
    /// Creates a repository backed by the given connection pool.
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct DeviceRow {
    device_id: String,
    device_name: String,
    folder: String,
    joined_at: String,
    paused: i64,
    portfolio_created_at: String,
    data_format_version: i64,
}

impl From<DeviceRow> for SyncDevice {
    fn from(row: DeviceRow) -> Self {
        SyncDevice::restore(StoredDevice {
            device_id: row.device_id,
            device_name: row.device_name,
            folder: row.folder,
            joined_at: row.joined_at,
            paused: row.paused != 0,
            portfolio_created_at: row.portfolio_created_at,
            data_format_version: u32::try_from(row.data_format_version).unwrap_or(0),
        })
    }
}

#[derive(sqlx::FromRow)]
struct CursorRow {
    device_id: String,
    applied_through: i64,
    last_applied_at: Option<String>,
}

impl From<CursorRow> for SyncCursor {
    fn from(row: CursorRow) -> Self {
        SyncCursor {
            device_id: row.device_id,
            applied_through: row.applied_through,
            last_applied_at: row.last_applied_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct HeldBackRow {
    id: String,
    origin_device_id: String,
    sequence: i64,
    payload: String,
    waiting_kind: String,
    waiting_identity: String,
    held_since: String,
}

impl TryFrom<HeldBackRow> for HeldBackChange {
    type Error = SyncError;

    fn try_from(row: HeldBackRow) -> Result<Self, SyncError> {
        Ok(HeldBackChange {
            id: row.id,
            origin_device_id: row.origin_device_id,
            sequence: row.sequence,
            payload: row.payload,
            waiting_kind: super::parse_stored("sync state", "waiting_kind", &row.waiting_kind)?,
            waiting_identity: row.waiting_identity,
            held_since: row.held_since,
        })
    }
}

#[derive(sqlx::FromRow)]
struct NoticeRow {
    notice_id: String,
    kind: String,
    record_kind: String,
    record_identity: String,
    record_label: String,
    other_device_id: String,
    other_device_name: String,
    raised_at: String,
}

impl TryFrom<NoticeRow> for ConflictNotice {
    type Error = SyncError;

    fn try_from(row: NoticeRow) -> Result<Self, SyncError> {
        Ok(ConflictNotice {
            notice_id: row.notice_id,
            kind: super::parse_stored("sync state", "kind", &row.kind)?,
            record_kind: super::parse_stored::<RecordKind>(
                "sync state",
                "record_kind",
                &row.record_kind,
            )?,
            record_identity: row.record_identity,
            record_label: row.record_label,
            other_device_id: row.other_device_id,
            other_device_name: row.other_device_name,
            raised_at: row.raised_at,
        })
    }
}

#[async_trait::async_trait]
impl SyncStateRepository for SqliteSyncStateRepository {
    async fn get_device(&self) -> Result<Option<SyncDevice>, SyncError> {
        let row = sqlx::query_as!(
            DeviceRow,
            r#"SELECT device_id, device_name, folder, joined_at, paused, portfolio_created_at,
                      data_format_version
               FROM sync_device WHERE id = 1"#
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| SyncError::database("get_device: query failed", error))?;
        Ok(row.map(SyncDevice::from))
    }

    async fn save_device(&self, device: &SyncDevice) -> Result<(), SyncError> {
        let paused = device.paused as i64;
        let data_format_version = i64::from(device.data_format_version);
        sqlx::query!(
            r#"INSERT INTO sync_device
               (id, device_id, device_name, folder, joined_at, paused, portfolio_created_at,
                logical_clock, derived_key, data_format_version)
               VALUES (1, ?, ?, ?, ?, ?, ?, 0, X'', ?)
               ON CONFLICT(id) DO UPDATE SET
                   device_id = excluded.device_id,
                   device_name = excluded.device_name,
                   folder = excluded.folder,
                   joined_at = excluded.joined_at,
                   paused = excluded.paused,
                   portfolio_created_at = excluded.portfolio_created_at,
                   data_format_version = excluded.data_format_version"#,
            device.device_id,
            device.device_name,
            device.folder,
            device.joined_at,
            paused,
            device.portfolio_created_at,
            data_format_version
        )
        .execute(&self.pool)
        .await
        .map_err(|error| SyncError::database("save_device: write failed", error))?;
        Ok(())
    }

    async fn discard_device_state(&self) -> Result<(), SyncError> {
        let mut transaction =
            self.pool.begin().await.map_err(|error| {
                SyncError::database("discard_device_state: begin failed", error)
            })?;
        for statement in [
            "DELETE FROM sync_device",
            "DELETE FROM sync_cursors",
            "DELETE FROM held_back_changes",
            "DELETE FROM conflict_notices",
        ] {
            sqlx::query(statement)
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    SyncError::database("discard_device_state: delete failed", error)
                })?;
        }
        transaction
            .commit()
            .await
            .map_err(|error| SyncError::database("discard_device_state: commit failed", error))
    }

    async fn get_cursor(&self, device_id: &str) -> Result<Option<SyncCursor>, SyncError> {
        let row = sqlx::query_as!(
            CursorRow,
            "SELECT device_id, applied_through, last_applied_at FROM sync_cursors WHERE device_id = ?",
            device_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| SyncError::database("get_cursor: query failed", error))?;
        Ok(row.map(SyncCursor::from))
    }

    async fn upsert_cursor(&self, cursor: &SyncCursor) -> Result<(), SyncError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|error| SyncError::database("upsert_cursor: acquire failed", error))?;
        self.upsert_cursor_on(&mut conn, cursor).await
    }

    async fn upsert_cursor_on(
        &self,
        conn: &mut SqliteConnection,
        cursor: &SyncCursor,
    ) -> Result<(), SyncError> {
        sqlx::query!(
            r#"INSERT INTO sync_cursors (device_id, applied_through, last_applied_at)
               VALUES (?, ?, ?)
               ON CONFLICT(device_id) DO UPDATE SET
                   applied_through = excluded.applied_through,
                   last_applied_at = excluded.last_applied_at"#,
            cursor.device_id,
            cursor.applied_through,
            cursor.last_applied_at
        )
        .execute(conn)
        .await
        .map_err(|error| SyncError::database("upsert_cursor: write failed", error))?;
        Ok(())
    }

    async fn insert_held_back(&self, change: &HeldBackChange) -> Result<(), SyncError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|error| SyncError::database("insert_held_back: acquire failed", error))?;
        self.insert_held_back_on(&mut conn, change).await
    }

    async fn insert_held_back_on(
        &self,
        conn: &mut SqliteConnection,
        change: &HeldBackChange,
    ) -> Result<(), SyncError> {
        let waiting_kind = change.waiting_kind.to_string();
        sqlx::query!(
            r#"INSERT INTO held_back_changes
               (id, origin_device_id, sequence, payload, waiting_kind, waiting_identity, held_since)
               VALUES (?, ?, ?, ?, ?, ?, ?)"#,
            change.id,
            change.origin_device_id,
            change.sequence,
            change.payload,
            waiting_kind,
            change.waiting_identity,
            change.held_since
        )
        .execute(conn)
        .await
        .map_err(|error| SyncError::database("insert_held_back: write failed", error))?;
        Ok(())
    }

    async fn list_held_back(&self) -> Result<Vec<HeldBackChange>, SyncError> {
        let rows = sqlx::query_as!(
            HeldBackRow,
            r#"SELECT id, origin_device_id, sequence, payload, waiting_kind, waiting_identity,
                      held_since
               FROM held_back_changes ORDER BY held_since ASC, id ASC"#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| SyncError::database("list_held_back: query failed", error))?;
        rows.into_iter().map(HeldBackChange::try_from).collect()
    }

    async fn remove_held_back(&self, id: &str) -> Result<(), SyncError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|error| SyncError::database("remove_held_back: acquire failed", error))?;
        self.remove_held_back_on(&mut conn, id).await
    }

    async fn remove_held_back_on(
        &self,
        conn: &mut SqliteConnection,
        id: &str,
    ) -> Result<(), SyncError> {
        sqlx::query!("DELETE FROM held_back_changes WHERE id = ?", id)
            .execute(conn)
            .await
            .map_err(|error| SyncError::database("remove_held_back: delete failed", error))?;
        Ok(())
    }

    async fn insert_notice(&self, notice: &ConflictNotice) -> Result<(), SyncError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|error| SyncError::database("insert_notice: acquire failed", error))?;
        self.insert_notice_on(&mut conn, notice).await
    }

    async fn insert_notice_on(
        &self,
        conn: &mut SqliteConnection,
        notice: &ConflictNotice,
    ) -> Result<(), SyncError> {
        let kind = notice.kind.to_string();
        let record_kind = notice.record_kind.to_string();
        sqlx::query!(
            r#"INSERT INTO conflict_notices
               (notice_id, kind, record_kind, record_identity, record_label, other_device_id,
                other_device_name, raised_at, dismissed)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0)"#,
            notice.notice_id,
            kind,
            record_kind,
            notice.record_identity,
            notice.record_label,
            notice.other_device_id,
            notice.other_device_name,
            notice.raised_at
        )
        .execute(conn)
        .await
        .map_err(|error| SyncError::database("insert_notice: write failed", error))?;
        Ok(())
    }

    async fn list_undismissed_notices(&self) -> Result<Vec<ConflictNotice>, SyncError> {
        let rows = sqlx::query_as!(
            NoticeRow,
            r#"SELECT notice_id, kind, record_kind, record_identity, record_label,
                      other_device_id, other_device_name, raised_at
               FROM conflict_notices WHERE dismissed = 0 ORDER BY raised_at ASC, notice_id ASC"#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| SyncError::database("list_undismissed_notices: query failed", error))?;
        rows.into_iter().map(ConflictNotice::try_from).collect()
    }

    async fn dismiss_notice(&self, notice_id: &str) -> Result<(), SyncError> {
        let updated = sqlx::query!(
            "UPDATE conflict_notices SET dismissed = 1 WHERE notice_id = ?",
            notice_id
        )
        .execute(&self.pool)
        .await
        .map_err(|error| SyncError::database("dismiss_notice: update failed", error))?;
        if updated.rows_affected() == 0 {
            return Err(SyncError::NoticeNotFound {
                notice_id: notice_id.to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::sync::domain::ConflictNoticeKind;
    use crate::shared::domain::RecordKind;
    use sqlx::sqlite::SqlitePoolOptions;

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

    fn sample_device() -> SyncDevice {
        SyncDevice::restore(StoredDevice {
            device_id: "desktop-device".into(),
            device_name: "Desktop".into(),
            folder: "/tmp/sync".into(),
            joined_at: "2026-08-22T00:00:00Z".into(),
            paused: false,
            portfolio_created_at: "2026-08-22T00:00:00Z".into(),
            data_format_version: 1,
        })
    }

    // SYN-016/052 — save then get round-trips the singleton device row.
    #[tokio::test]
    async fn save_and_get_device_round_trip() {
        let pool = make_pool().await;
        let repo = SqliteSyncStateRepository::new(pool);
        repo.save_device(&sample_device()).await.unwrap();
        let loaded = repo
            .get_device()
            .await
            .unwrap()
            .expect("must exist after save");
        assert_eq!(loaded, sample_device());
    }

    // SYN-010 — no row yet: get_device returns None.
    #[tokio::test]
    async fn get_device_returns_none_when_no_row_exists() {
        let pool = make_pool().await;
        let repo = SqliteSyncStateRepository::new(pool);
        assert!(repo.get_device().await.unwrap().is_none());
    }

    // SYN-070/072 — saving again overwrites the singleton (paused + renamed).
    #[tokio::test]
    async fn save_device_overwrites_the_singleton_row() {
        let pool = make_pool().await;
        let repo = SqliteSyncStateRepository::new(pool);
        repo.save_device(&sample_device()).await.unwrap();
        let mut renamed = sample_device();
        renamed.device_name = "Office Desktop".into();
        renamed.paused = true;
        repo.save_device(&renamed).await.unwrap();

        let loaded = repo.get_device().await.unwrap().unwrap();
        assert_eq!(loaded.device_name, "Office Desktop");
        assert!(loaded.paused);
    }

    // SYN-052/025 — saving the device again never touches the kept key or the logical clock.
    #[tokio::test]
    async fn save_device_keeps_the_derived_key_and_logical_clock() {
        let pool = make_pool().await;
        let repo = SqliteSyncStateRepository::new(pool.clone());
        repo.save_device(&sample_device()).await.unwrap();
        sqlx::query("UPDATE sync_device SET derived_key = X'AB', logical_clock = 7 WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        repo.save_device(&sample_device()).await.unwrap();

        let (key, clock): (Vec<u8>, i64) =
            sqlx::query_as("SELECT derived_key, logical_clock FROM sync_device WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(key, vec![0xAB]);
        assert_eq!(clock, 7);
    }

    // SYN-082 — discarding the device state empties every sync-owned table.
    #[tokio::test]
    async fn discard_device_state_empties_every_sync_owned_table() {
        let pool = make_pool().await;
        let repo = SqliteSyncStateRepository::new(pool.clone());
        repo.save_device(&sample_device()).await.unwrap();
        repo.upsert_cursor(&SyncCursor {
            device_id: "laptop-device".into(),
            applied_through: 1,
            last_applied_at: None,
        })
        .await
        .unwrap();
        repo.insert_held_back(&sample_held_back()).await.unwrap();
        repo.insert_notice(&sample_notice()).await.unwrap();

        repo.discard_device_state().await.unwrap();

        assert!(repo.get_device().await.unwrap().is_none());
        assert!(repo.get_cursor("laptop-device").await.unwrap().is_none());
        assert!(repo.list_held_back().await.unwrap().is_empty());
        assert!(repo.list_undismissed_notices().await.unwrap().is_empty());
    }

    // SYN-033 — upsert_cursor then get_cursor round-trips.
    #[tokio::test]
    async fn upsert_and_get_cursor_round_trip() {
        let pool = make_pool().await;
        let repo = SqliteSyncStateRepository::new(pool);
        let cursor = SyncCursor {
            device_id: "laptop-device".into(),
            applied_through: 5,
            last_applied_at: Some("2026-08-22T00:00:00Z".into()),
        };
        repo.upsert_cursor(&cursor).await.unwrap();
        let loaded = repo
            .get_cursor("laptop-device")
            .await
            .unwrap()
            .expect("must exist after upsert");
        assert_eq!(loaded, cursor);
    }

    // SYN-033 — a second upsert for the same device replaces the first value.
    #[tokio::test]
    async fn upsert_cursor_replaces_existing_value_for_the_same_device() {
        let pool = make_pool().await;
        let repo = SqliteSyncStateRepository::new(pool);
        repo.upsert_cursor(&SyncCursor {
            device_id: "laptop-device".into(),
            applied_through: 5,
            last_applied_at: None,
        })
        .await
        .unwrap();
        repo.upsert_cursor(&SyncCursor {
            device_id: "laptop-device".into(),
            applied_through: 9,
            last_applied_at: Some("2026-08-22T00:00:00Z".into()),
        })
        .await
        .unwrap();
        let loaded = repo.get_cursor("laptop-device").await.unwrap().unwrap();
        assert_eq!(loaded.applied_through, 9);
    }

    fn sample_held_back() -> HeldBackChange {
        HeldBackChange {
            id: "held-1".into(),
            origin_device_id: "laptop-device".into(),
            sequence: 7,
            payload: "{}".into(),
            waiting_kind: RecordKind::Account,
            waiting_identity: "account-1".into(),
            held_since: "2026-08-22T00:00:00Z".into(),
        }
    }

    // SYN-041 — insert then list round-trips a held-back change.
    #[tokio::test]
    async fn insert_and_list_held_back_round_trip() {
        let pool = make_pool().await;
        let repo = SqliteSyncStateRepository::new(pool);
        repo.insert_held_back(&sample_held_back()).await.unwrap();
        let listed = repo.list_held_back().await.unwrap();
        assert_eq!(listed, vec![sample_held_back()]);
    }

    // SYN-041 — remove_held_back deletes it; the list is empty afterward.
    #[tokio::test]
    async fn remove_held_back_deletes_the_entry() {
        let pool = make_pool().await;
        let repo = SqliteSyncStateRepository::new(pool);
        repo.insert_held_back(&sample_held_back()).await.unwrap();
        repo.remove_held_back("held-1").await.unwrap();
        assert!(repo.list_held_back().await.unwrap().is_empty());
    }

    fn sample_notice() -> ConflictNotice {
        ConflictNotice {
            notice_id: "notice-1".into(),
            kind: ConflictNoticeKind::DuplicateName,
            record_kind: RecordKind::Category,
            record_identity: "category-1".into(),
            record_label: "Growth".into(),
            other_device_id: "laptop-device".into(),
            other_device_name: "Laptop".into(),
            raised_at: "2026-08-22T00:00:00Z".into(),
        }
    }

    // SYN-066 — insert then list_undismissed_notices returns the notice.
    #[tokio::test]
    async fn insert_and_list_undismissed_notices_round_trip() {
        let pool = make_pool().await;
        let repo = SqliteSyncStateRepository::new(pool);
        repo.insert_notice(&sample_notice()).await.unwrap();
        let listed = repo.list_undismissed_notices().await.unwrap();
        assert_eq!(listed, vec![sample_notice()]);
    }

    // SYN-066 — dismiss_notice excludes it from the undismissed list.
    #[tokio::test]
    async fn dismiss_notice_excludes_it_from_the_undismissed_list() {
        let pool = make_pool().await;
        let repo = SqliteSyncStateRepository::new(pool);
        repo.insert_notice(&sample_notice()).await.unwrap();
        repo.dismiss_notice("notice-1").await.unwrap();
        assert!(repo.list_undismissed_notices().await.unwrap().is_empty());
    }

    // SYN-066 — dismissing an unknown notice rejects with NoticeNotFound and changes nothing.
    #[tokio::test]
    async fn dismiss_unknown_notice_returns_notice_not_found() {
        let pool = make_pool().await;
        let repo = SqliteSyncStateRepository::new(pool);
        let result = repo.dismiss_notice("does-not-exist").await;
        assert!(matches!(
            result,
            Err(SyncError::NoticeNotFound { notice_id }) if notice_id == "does-not-exist"
        ));
    }
}
