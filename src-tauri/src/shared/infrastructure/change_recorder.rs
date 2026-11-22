//! `ChangeRecorder` port (D1, ADR-019): every synced repository write appends a change
//! through this port on the same database connection/transaction as the write, so the
//! record and its change exist together or not at all (SYN-020). The port lives in
//! `infrastructure/` rather than `domain/` because it takes the live connection handle;
//! the sync bounded context implements it (`context::sync::infrastructure::change_log`).

use crate::shared::domain::name::same_name;
use crate::shared::domain::{ChangeDraft, LogicalTimestamp, Rank};

tokio::task_local! {
    /// Set for the task running an apply or a join's rebuild (SYN-020).
    static APPLYING: ();
}

/// Runs `work` as an apply (SYN-020): the writes it makes, in its own task, record no change,
/// and `is_recording` reports `false` there. A local write in another task meanwhile records
/// its change as always (SYN-064, #920) — the suspension follows the applying task, never the
/// whole process.
pub async fn while_applying<F: std::future::Future>(work: F) -> F::Output {
    APPLYING.scope((), work).await
}

/// `true` inside `while_applying`'s work, in its own task.
pub fn applying() -> bool {
    APPLYING.try_with(|_| ()).is_ok()
}

/// The application's own local write was refused before it was recorded (CFR-016): the
/// record's current state — a removal the user made — outranks it. The repository's
/// transaction rolls back with it; the write is not made and produces no change.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("local write of {record_kind} {record_identity} is outranked by the user's state")]
pub struct LocalWriteOutranked {
    /// The kind of the refused record.
    pub record_kind: crate::shared::domain::RecordKind,
    /// Its canonical identity (CFR-012).
    pub record_identity: String,
}

/// Appends one change on the same connection/transaction as the write it describes
/// (SYN-020), allocating the device's next sequence number (SYN-025) and advancing the
/// Lamport logical clock (CFR-010). Returns the `Rank` the record's row must be stamped
/// with (CFR-014), or `None` — the D6 NULL sentinel, `Rank::NEVER` — when nothing was
/// recorded (no `sync_device` row exists yet, or the device is paused, SYN-010/070).
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait ChangeRecorder: Send + Sync {
    /// Records `draft`, or reports dormancy via `Ok(None)`. `Err` carrying
    /// `LocalWriteOutranked` when the draft is the application's own write over a state
    /// the user made (CFR-016) — the caller's transaction must roll back.
    async fn record(
        &self,
        conn: &mut sqlx::SqliteConnection,
        draft: ChangeDraft,
    ) -> anyhow::Result<Option<Rank>>;

    /// `false` inside an apply's own task — SYN-020's "applying another device's change
    /// never records a change" (`while_applying`) — or while sync has never been enabled or
    /// is paused; `true` otherwise.
    async fn is_recording(&self) -> bool;
}

/// The CFR-014 rank in the TEXT form the three `sync_*` columns store: a repository stamps
/// a row with these right after `ChangeRecorder::record` returned a rank.
pub struct RankColumns {
    /// `sync_logical_timestamp` — the zero-padded 20-character wire form.
    pub logical_timestamp: String,
    /// `sync_origin` — `User` or `Application`.
    pub origin: String,
    /// `sync_device_id` — the device that made the change.
    pub device_id: String,
}

impl From<Rank> for RankColumns {
    fn from(rank: Rank) -> Self {
        Self {
            logical_timestamp: rank.logical_timestamp.as_str().to_string(),
            origin: rank.origin.to_string(),
            device_id: rank.device_id,
        }
    }
}

/// Reads the CFR-014 rank back from the three nullable `sync_*` columns of a row: `None` —
/// the D6 NULL sentinel — when the row has never been ranked or a column is unreadable.
pub fn rank_from_columns(
    logical_timestamp: Option<String>,
    origin: Option<String>,
    device_id: Option<String>,
) -> Option<Rank> {
    Some(Rank {
        logical_timestamp: LogicalTimestamp::from_wire(&logical_timestamp?)?,
        origin: origin?.parse().ok()?,
        device_id: device_id?,
    })
}

/// The recorder every repository is wired with while sync has never been enabled
/// (SYN-010): records nothing, and is never "recording".
pub struct NoopChangeRecorder;

#[async_trait::async_trait]
impl ChangeRecorder for NoopChangeRecorder {
    async fn record(
        &self,
        _conn: &mut sqlx::SqliteConnection,
        _draft: ChangeDraft,
    ) -> anyhow::Result<Option<Rank>> {
        Ok(Rank::NEVER)
    }

    async fn is_recording(&self) -> bool {
        false
    }
}

/// A row's name beside its rank columns, to find the one sharing a name (`same_name`).
#[derive(sqlx::FromRow)]
pub struct NamedRankRow {
    /// The name the row carries.
    pub name: String,
    /// The rank's logical timestamp, as stored.
    pub sync_logical_timestamp: Option<String>,
    /// The rank's origin, as stored.
    pub sync_origin: Option<String>,
    /// The rank's device, as stored.
    pub sync_device_id: Option<String>,
}

/// The rank of the first of `rows`, in their order, that shares `name`.
pub fn rank_of_same_name(rows: Vec<NamedRankRow>, name: &str) -> Option<Rank> {
    rows.into_iter()
        .find(|row| same_name(&row.name, name))
        .and_then(|row| {
            rank_from_columns(
                row.sync_logical_timestamp,
                row.sync_origin,
                row.sync_device_id,
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    // #810 / CFR-035 — the clash a merge reports is found whatever the case of an accented
    // letter, and it is the first row in rank order.
    #[test]
    fn rank_of_same_name_ignores_the_case_of_accented_letters() {
        let row = |name: &str, device: &str| NamedRankRow {
            name: name.to_string(),
            sync_logical_timestamp: Some("0000000000000001".to_string()),
            sync_origin: Some("User".to_string()),
            sync_device_id: Some(device.to_string()),
        };
        let rows = || {
            vec![
                row("Epargne", "a"),
                row("éPARGNE", "b"),
                row("Épargne", "c"),
            ]
        };
        let found = rank_of_same_name(rows(), "Épargne").expect("a clash");
        let expected = rank_from_columns(
            Some("0000000000000001".to_string()),
            Some("User".to_string()),
            Some("b".to_string()),
        )
        .expect("a rank");
        assert_eq!(found, expected);
        assert!(rank_of_same_name(rows(), "Autre").is_none());
    }
    use crate::shared::domain::{Operation, Origin, RecordIdentity, RecordKind};
    use sqlx::sqlite::SqlitePoolOptions;

    // The trait compiles with mockall's automock — a capture-site test expresses "record
    // was called once with this draft" as a hard, checkable expectation.
    #[tokio::test]
    async fn mock_change_recorder_compiles_and_reports_expectations() {
        let mut mock = MockChangeRecorder::new();
        mock.expect_is_recording().returning(|| true);
        assert!(mock.is_recording().await);
    }

    // SYN-010/D6 — a NoopChangeRecorder records nothing and reports the NULL sentinel.
    #[tokio::test]
    async fn noop_change_recorder_records_nothing_and_reports_never_ranked() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("test pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations");
        let mut conn = pool.acquire().await.expect("conn");

        let recorder = NoopChangeRecorder;
        assert!(!recorder.is_recording().await);

        let draft = ChangeDraft::new(
            RecordKind::Account,
            RecordIdentity::canonical(RecordKind::Account, &["account-1"]),
            Operation::Created,
            Origin::User,
            None,
            None,
        );
        let rank = recorder
            .record(&mut conn, draft)
            .await
            .expect("noop record never fails");
        assert_eq!(
            rank,
            Rank::NEVER,
            "D6: the noop recorder reports the NULL sentinel"
        );
    }
}
