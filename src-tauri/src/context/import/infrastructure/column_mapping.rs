use crate::context::import::domain::{ColumnMapping, ColumnMappingDraft, ColumnMappingRepository};
use crate::shared::domain::{
    ChangeDraft, LogicalTimestamp, Operation, Origin, Rank, RecordIdentity, RecordKind,
    SyncedRecord,
};
use crate::shared::infrastructure::change_recorder::{
    rank_from_columns, ChangeRecorder, NoopChangeRecorder, RankColumns,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use sqlx::{SqliteConnection, SqlitePool};
use std::sync::Arc;

/// SQLite-backed implementation of `ColumnMappingRepository`.
pub struct SqliteColumnMappingRepository {
    pool: SqlitePool,
    change_recorder: Arc<dyn ChangeRecorder>,
}

impl SqliteColumnMappingRepository {
    /// Creates a new repository backed by the given connection pool.
    pub fn new(pool: SqlitePool) -> Self {
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

/// A stored mapping's row: its scalars as text, its lists and columns as JSON.
struct MappingRow {
    id: String,
    name: String,
    separator: String,
    decimal_mark: String,
    date_format: String,
    encoding: String,
    columns: String,
    operation_types: String,
    asset_matches: String,
}

/// The text a unit enum is stored as: its serialized name.
fn stored<T: serde::Serialize>(value: &T) -> Result<String> {
    match serde_json::to_value(value)? {
        serde_json::Value::String(text) => Ok(text),
        other => Ok(other.to_string()),
    }
}

fn read<T: DeserializeOwned>(text: &str, what: &str) -> Result<T> {
    serde_json::from_str(text).with_context(|| format!("Failed to read a column mapping's {what}"))
}

fn read_name<T: DeserializeOwned>(text: &str, what: &str) -> Result<T> {
    serde_json::from_value(serde_json::Value::String(text.to_string()))
        .with_context(|| format!("Failed to read a column mapping's {what}"))
}

impl TryFrom<MappingRow> for ColumnMapping {
    type Error = anyhow::Error;

    fn try_from(row: MappingRow) -> Result<Self> {
        Ok(ColumnMapping::restore(
            row.id,
            ColumnMappingDraft {
                name: row.name,
                separator: read_name(&row.separator, "separator")?,
                decimal_mark: read_name(&row.decimal_mark, "decimal mark")?,
                date_format: read_name(&row.date_format, "date format")?,
                encoding: read_name(&row.encoding, "encoding")?,
                columns: read(&row.columns, "columns")?,
                operation_types: read(&row.operation_types, "operation types")?,
                asset_matches: read(&row.asset_matches, "asset matches")?,
            },
        ))
    }
}

/// The identity a mapping is synced under (CFR-012).
fn mapping_identity(id: &str) -> RecordIdentity {
    RecordIdentity::canonical(RecordKind::ColumnMapping, &[id])
}

/// Inserts or replaces `mapping` on `conn`, leaving its rank columns as they are.
async fn upsert_on(conn: &mut SqliteConnection, mapping: &ColumnMapping) -> Result<()> {
    let separator = stored(&mapping.separator)?;
    let decimal_mark = stored(&mapping.decimal_mark)?;
    let date_format = stored(&mapping.date_format)?;
    let encoding = stored(&mapping.encoding)?;
    let columns = serde_json::to_string(&mapping.columns)?;
    let operation_types = serde_json::to_string(&mapping.operation_types)?;
    let asset_matches = serde_json::to_string(&mapping.asset_matches)?;
    sqlx::query!(
        r#"INSERT INTO column_mappings
               (id, name, separator, decimal_mark, date_format, encoding, columns,
                operation_types, asset_matches)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(id) DO UPDATE SET
               name = excluded.name,
               separator = excluded.separator,
               decimal_mark = excluded.decimal_mark,
               date_format = excluded.date_format,
               encoding = excluded.encoding,
               columns = excluded.columns,
               operation_types = excluded.operation_types,
               asset_matches = excluded.asset_matches"#,
        mapping.id,
        mapping.name,
        separator,
        decimal_mark,
        date_format,
        encoding,
        columns,
        operation_types,
        asset_matches
    )
    .execute(conn)
    .await
    .with_context(|| format!("Failed to write column mapping {}", mapping.id))?;
    Ok(())
}

/// The logical timestamp the mapping `id` was last changed at, when it is held and ranked.
async fn current_timestamp(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<Option<Option<LogicalTimestamp>>> {
    let row = sqlx::query_scalar!(
        "SELECT sync_logical_timestamp FROM column_mappings WHERE id = ?",
        id
    )
    .fetch_optional(conn)
    .await
    .with_context(|| format!("Failed to read column mapping {id} before writing it"))?;
    Ok(row.map(|stamp| stamp.as_deref().and_then(LogicalTimestamp::from_wire)))
}

#[async_trait]
impl ColumnMappingRepository for SqliteColumnMappingRepository {
    async fn list(&self) -> Result<Vec<ColumnMapping>> {
        let rows = sqlx::query_as!(
            MappingRow,
            r#"SELECT id, name, separator, decimal_mark, date_format, encoding, columns,
                      operation_types, asset_matches
               FROM column_mappings ORDER BY name COLLATE NOCASE, id"#
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list column mappings")?;
        rows.into_iter().map(ColumnMapping::try_from).collect()
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<ColumnMapping>> {
        let row = sqlx::query_as!(
            MappingRow,
            r#"SELECT id, name, separator, decimal_mark, date_format, encoding, columns,
                      operation_types, asset_matches
               FROM column_mappings WHERE id = ?"#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .with_context(|| format!("Failed to read column mapping {id}"))?;
        row.map(ColumnMapping::try_from).transpose()
    }

    async fn save(&self, mapping: &ColumnMapping) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin column mapping write")?;
        let (operation, based_on) = match current_timestamp(&mut tx, &mapping.id).await? {
            Some(based_on) => (Operation::Updated, based_on),
            None => (Operation::Created, None),
        };
        upsert_on(&mut tx, mapping).await?;
        let draft = ChangeDraft::new(
            RecordKind::ColumnMapping,
            mapping_identity(&mapping.id),
            operation,
            Origin::User,
            based_on,
            Some(serde_json::to_string(mapping)?),
        );
        if let Some(rank) = self.change_recorder.record(&mut tx, draft).await? {
            let columns = RankColumns::from(rank);
            sqlx::query!(
                "UPDATE column_mappings SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
                 WHERE id = ?",
                columns.logical_timestamp,
                columns.origin,
                columns.device_id,
                mapping.id
            )
            .execute(&mut *tx)
            .await
            .context("Failed to stamp rank on column mapping")?;
        }
        tx.commit()
            .await
            .context("Failed to commit column mapping write")
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin column mapping removal")?;
        let Some(based_on) = current_timestamp(&mut tx, id).await? else {
            return Ok(());
        };
        sqlx::query!("DELETE FROM column_mappings WHERE id = ?", id)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("Failed to delete column mapping {id}"))?;
        let draft = ChangeDraft::new(
            RecordKind::ColumnMapping,
            mapping_identity(id),
            Operation::Removed,
            Origin::User,
            based_on,
            None,
        );
        self.change_recorder.record(&mut tx, draft).await?;
        tx.commit()
            .await
            .context("Failed to commit column mapping removal")
    }

    async fn stamp_sync_rank(&self, conn: &mut SqliteConnection, rank: &Rank) -> Result<u64> {
        let columns = RankColumns::from(rank.clone());
        let stamped = sqlx::query!(
            "UPDATE column_mappings SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
             WHERE sync_logical_timestamp IS NULL",
            columns.logical_timestamp,
            columns.origin,
            columns.device_id
        )
        .execute(conn)
        .await
        .context("Failed to stamp unranked column mappings")?;
        Ok(stamped.rows_affected())
    }

    async fn synced_record(
        &self,
        conn: &mut SqliteConnection,
        identity: &str,
    ) -> Result<Option<SyncedRecord>> {
        let row = sqlx::query!(
            r#"SELECT id, name, separator, decimal_mark, date_format, encoding, columns,
                      operation_types, asset_matches,
                      sync_logical_timestamp, sync_origin, sync_device_id
               FROM column_mappings WHERE id = ?"#,
            identity
        )
        .fetch_optional(conn)
        .await
        .with_context(|| format!("Failed to read synced column mapping {identity}"))?;
        row.map(|row| {
            let mapping = ColumnMapping::try_from(MappingRow {
                id: row.id,
                name: row.name,
                separator: row.separator,
                decimal_mark: row.decimal_mark,
                date_format: row.date_format,
                encoding: row.encoding,
                columns: row.columns,
                operation_types: row.operation_types,
                asset_matches: row.asset_matches,
            })?;
            Ok(SyncedRecord {
                rank: rank_from_columns(
                    row.sync_logical_timestamp,
                    row.sync_origin,
                    row.sync_device_id,
                ),
                content: serde_json::to_string(&mapping)?,
            })
        })
        .transpose()
    }

    async fn apply(
        &self,
        conn: &mut SqliteConnection,
        mapping: &ColumnMapping,
        rank: &Rank,
    ) -> Result<()> {
        upsert_on(&mut *conn, mapping).await?;
        let columns = RankColumns::from(rank.clone());
        sqlx::query!(
            "UPDATE column_mappings SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
             WHERE id = ?",
            columns.logical_timestamp,
            columns.origin,
            columns.device_id,
            mapping.id
        )
        .execute(conn)
        .await
        .context("Failed to stamp rank on an applied column mapping")?;
        Ok(())
    }

    async fn remove_synced(&self, conn: &mut SqliteConnection, identity: &str) -> Result<()> {
        sqlx::query!("DELETE FROM column_mappings WHERE id = ?", identity)
            .execute(conn)
            .await
            .with_context(|| format!("Failed to remove column mapping {identity}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::import::domain::{
        AssetMatch, ColumnMappingDraft, DateFormat, DecimalMark, Encoding, MappedColumns,
        OperationLabel, OperationType, Separator,
    };
    use crate::context::sync::SqliteChangeRecorder;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn make_pool() -> SqlitePool {
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

    // IMP-024A — a mapping's text bound is the one every device takes in.
    #[test]
    fn imp_024a_the_text_bound_is_the_sync_bound() {
        assert_eq!(
            crate::context::import::domain::MAPPING_TEXT_LENGTH,
            crate::context::sync::domain::received_change::TEXT_BOUND
        );
    }

    async fn seed_sync_device(pool: &SqlitePool) {
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

    async fn operations(pool: &SqlitePool) -> Vec<String> {
        sqlx::query_scalar!(
            "SELECT operation FROM changes WHERE record_kind = 'ColumnMapping' ORDER BY rowid"
        )
        .fetch_all(pool)
        .await
        .expect("changes")
    }

    fn mapping(name: &str) -> ColumnMapping {
        ColumnMapping::with_id(
            "m-1".to_string(),
            ColumnMappingDraft {
                name: name.to_string(),
                separator: Separator::Semicolon,
                decimal_mark: DecimalMark::Dot,
                date_format: DateFormat::DayMonthYear,
                encoding: Encoding::Windows1252,
                columns: MappedColumns {
                    date: "Date".to_string(),
                    operation: "Opération".to_string(),
                    isin: None,
                    reference: None,
                    name: Some("libellé".to_string()),
                    quantity: Some("Qté".to_string()),
                    unit_price: Some("Prix".to_string()),
                    gross_amount: None,
                    fees: Some("Courtage".to_string()),
                    net_amount: Some("Montant net".to_string()),
                    currency: Some("Devise".to_string()),
                },
                operation_types: vec![OperationLabel {
                    label: "Achat Comptant".to_string(),
                    transaction_type: OperationType::Purchase,
                }],
                asset_matches: vec![AssetMatch {
                    read: "ACME SA".to_string(),
                    asset_id: "asset-1".to_string(),
                }],
            },
        )
        .expect("valid mapping")
    }

    // IMP-020 / IMP-023 — a mapping saved reads back whole; saving it again records an
    // update, deleting it a removal (SYN-020, SYN-024).
    #[tokio::test]
    async fn imp_023_a_mapping_records_its_changes_and_reads_back() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteColumnMappingRepository::new(pool.clone()).with_change_recorder(recorder);

        repo.save(&mapping("Broker")).await.expect("created");
        repo.save(&mapping("Broker PEA")).await.expect("renamed");
        assert_eq!(
            repo.get_by_id("m-1").await.expect("read"),
            Some(mapping("Broker PEA"))
        );
        assert_eq!(
            repo.list().await.expect("list"),
            vec![mapping("Broker PEA")]
        );

        repo.delete("m-1").await.expect("deleted");
        repo.delete("m-1")
            .await
            .expect("deleting nothing records nothing");
        assert_eq!(repo.get_by_id("m-1").await.expect("read"), None);
        assert_eq!(operations(&pool).await, ["Created", "Updated", "Removed"]);
    }

    // CFR-014 / CFR-017 — an applied mapping is held with its rank and read back as synced;
    // a removal drops it.
    #[tokio::test]
    async fn cfr_017_an_applied_mapping_is_read_back_then_removed() {
        let pool = make_pool().await;
        let repo = SqliteColumnMappingRepository::new(pool.clone());
        let mut conn = pool.acquire().await.expect("connection");
        let rank = Rank {
            origin: Origin::User,
            logical_timestamp: LogicalTimestamp::new(7),
            device_id: "laptop".to_string(),
        };

        repo.apply(&mut conn, &mapping("Broker"), &rank)
            .await
            .expect("applied");
        let held = repo
            .synced_record(&mut conn, "m-1")
            .await
            .expect("read")
            .expect("held");
        assert_eq!(held.rank, Some(rank));
        let content: ColumnMapping = serde_json::from_str(&held.content).expect("content");
        assert_eq!(content, mapping("Broker"));

        repo.remove_synced(&mut conn, "m-1").await.expect("removed");
        assert!(repo
            .synced_record(&mut conn, "m-1")
            .await
            .expect("read")
            .is_none());
    }
}
