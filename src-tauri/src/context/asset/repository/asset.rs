use std::str::FromStr;
use std::sync::Arc;

use super::super::domain::{
    exchange, Asset, AssetCategory, AssetClass, AssetPrice, AssetRepository,
};
use super::asset_price::source_from_storage;
use crate::shared::domain::{
    ChangeDraft, LogicalTimestamp, Operation, Origin, Rank, RecordIdentity, RecordKind,
    SyncedRecord,
};
use crate::shared::infrastructure::change_recorder::{
    rank_from_columns, ChangeRecorder, NoopChangeRecorder, RankColumns,
};
use anyhow::{Context, Result};
use sqlx::{Pool, Sqlite, SqliteConnection};

#[derive(sqlx::FromRow)]
struct AssetRow {
    id: String,
    name: String,
    reference: String,
    isin: Option<String>,
    asset_class: String,
    currency: String,
    risk_level: i64,
    category_id: Option<String>,
    category_name: Option<String>,
    is_archived: bool,
    exchange_code: Option<String>,
    price_refresh_blocked: bool,
    interest_bearing: bool,
}

impl From<AssetRow> for Asset {
    fn from(row: AssetRow) -> Self {
        let asset_class = AssetClass::from_str(&row.asset_class).unwrap_or_default();
        let exchange = row.exchange_code.as_deref().and_then(exchange::lookup);
        // CFR-030 — an asset whose category stands removed is shown in the default category,
        // derived on read; the stored category id is left as it is.
        let category = match (row.category_id, row.category_name) {
            (Some(id), Some(name)) => AssetCategory::from_storage(id, name),
            _ => AssetCategory::default(),
        };
        Asset::restore(
            row.id,
            row.name,
            asset_class,
            category,
            row.currency,
            row.risk_level.try_into().unwrap_or(0),
            row.reference,
            row.isin,
            row.is_archived,
            exchange,
            row.price_refresh_blocked,
            row.interest_bearing,
        )
    }
}

/// Loads one asset by id on the given connection, whatever its deleted or archived state —
/// the full record state a change's content carries (SYN-020).
pub(super) async fn fetch_asset(conn: &mut SqliteConnection, id: &str) -> Result<Option<Asset>> {
    let row = sqlx::query_as!(
        AssetRow,
        r#"
        SELECT
            a.id, a.name, a.reference, a.isin, a.asset_class, a.currency, a.risk_level,
            c.id as "category_id?: String",
            c.name as "category_name?: String",
            a.is_archived as "is_archived: bool",
            a.exchange_code,
            a.price_refresh_blocked as "price_refresh_blocked: bool",
            a.interest_bearing as "interest_bearing: bool"
        FROM assets a
        LEFT JOIN categories c ON a.category_id = c.id AND c.is_deleted = 0
        WHERE a.id = ?
        "#,
        id
    )
    .fetch_optional(conn)
    .await
    .with_context(|| format!("Failed to fetch asset with id: {}", id))?;
    Ok(row.map(Asset::from))
}

/// Stamps the CFR-014 rank columns on one asset row.
pub(super) async fn stamp_asset_rank(
    conn: &mut SqliteConnection,
    id: &str,
    rank: Rank,
) -> Result<()> {
    let columns = RankColumns::from(rank);
    sqlx::query!(
        r#"UPDATE assets SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ? WHERE id = ?"#,
        columns.logical_timestamp,
        columns.origin,
        columns.device_id,
        id
    )
    .execute(conn)
    .await
    .with_context(|| format!("Failed to stamp rank on asset with id: {}", id))?;
    Ok(())
}

/// CFR-011 — the logical timestamp of the asset's current state, the `based_on` of the next
/// local change to it; `None` while absent or never ranked.
pub(super) async fn current_asset_timestamp(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<Option<LogicalTimestamp>> {
    let stored = sqlx::query_scalar!(
        r#"SELECT sync_logical_timestamp AS "sync_logical_timestamp?: String" FROM assets WHERE id = ?"#,
        id
    )
    .fetch_optional(conn)
    .await
    .with_context(|| format!("Failed to read the rank of asset with id: {}", id))?;
    Ok(stored
        .flatten()
        .and_then(|timestamp| LogicalTimestamp::from_wire(&timestamp)))
}

pub(super) fn asset_identity(id: &str) -> RecordIdentity {
    RecordIdentity::canonical(RecordKind::Asset, &[id])
}

/// Splits an `asset:date` price identity (CFR-012) into its two keys.
fn split_price_identity(identity: &str) -> Result<(&str, &str)> {
    identity
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("malformed asset price identity: '{identity}'"))
}

/// The three rank columns of one synced row (CFR-014).
#[derive(sqlx::FromRow)]
struct RankRow {
    sync_logical_timestamp: Option<String>,
    sync_origin: Option<String>,
    sync_device_id: Option<String>,
}

impl RankRow {
    fn rank(self) -> Option<Rank> {
        rank_from_columns(
            self.sync_logical_timestamp,
            self.sync_origin,
            self.sync_device_id,
        )
    }
}

/// SQLite implementation of the AssetRepository.
#[derive(Clone)]
pub struct SqliteAssetRepository {
    pool: Pool<Sqlite>,
    change_recorder: Arc<dyn ChangeRecorder>,
}

impl SqliteAssetRepository {
    /// Creates a new SqliteAssetRepository.
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

    async fn record_asset_state(
        &self,
        conn: &mut SqliteConnection,
        asset: &Asset,
        operation: Operation,
        based_on: Option<LogicalTimestamp>,
    ) -> Result<()> {
        let draft = ChangeDraft::new(
            RecordKind::Asset,
            asset_identity(&asset.id),
            operation,
            Origin::User,
            based_on,
            Some(serde_json::to_string(asset)?),
        );
        let rank = self.change_recorder.record(conn, draft).await?;
        if let Some(rank) = rank {
            stamp_asset_rank(conn, &asset.id, rank).await?;
        }
        Ok(())
    }

    /// Re-reads the asset an id-only update touched and records it as Updated, based on the
    /// state the update found (CFR-011); a statement that matched no row records nothing
    /// (SYN-020).
    async fn record_updated_by_id(
        &self,
        conn: &mut SqliteConnection,
        id: &str,
        based_on: Option<LogicalTimestamp>,
        rows_affected: u64,
    ) -> Result<()> {
        if rows_affected == 0 {
            return Ok(());
        }
        if let Some(asset) = fetch_asset(conn, id).await? {
            self.record_asset_state(conn, &asset, Operation::Updated, based_on)
                .await?;
        }
        Ok(())
    }

    /// One id-only column update (archive, unarchive, block, unblock), recorded as an
    /// Updated change of the asset's full state.
    async fn update_flag(&self, id: &str, statement: &str, what: &str) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .with_context(|| format!("Failed to begin asset {what}"))?;
        let based_on = current_asset_timestamp(&mut tx, id).await?;
        let written = sqlx::query(statement)
            .bind(id)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("Failed to {what} asset with id: {}", id))?;
        self.record_updated_by_id(&mut tx, id, based_on, written.rows_affected())
            .await?;
        tx.commit()
            .await
            .with_context(|| format!("Failed to commit asset {what}"))?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl AssetRepository for SqliteAssetRepository {
    async fn stamp_sync_rank(&self, conn: &mut SqliteConnection, rank: &Rank) -> Result<u64> {
        let columns = RankColumns::from(rank.clone());
        let (timestamp, origin, device_id) = (
            &columns.logical_timestamp,
            &columns.origin,
            &columns.device_id,
        );
        let mut stamped = 0;
        stamped += sqlx::query!(
            "UPDATE assets SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
             WHERE sync_logical_timestamp IS NULL",
            timestamp,
            origin,
            device_id
        )
        .execute(&mut *conn)
        .await
        .context("Failed to stamp unranked assets")?
        .rows_affected();
        stamped += sqlx::query!(
            "UPDATE categories SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
             WHERE sync_logical_timestamp IS NULL",
            timestamp,
            origin,
            device_id
        )
        .execute(&mut *conn)
        .await
        .context("Failed to stamp unranked categories")?
        .rows_affected();
        stamped += sqlx::query!(
            "UPDATE asset_prices SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
             WHERE sync_logical_timestamp IS NULL",
            timestamp,
            origin,
            device_id
        )
        .execute(&mut *conn)
        .await
        .context("Failed to stamp unranked asset prices")?
        .rows_affected();
        Ok(stamped)
    }

    async fn get_all(&self) -> Result<Vec<Asset>> {
        let rows = sqlx::query_as!(
            AssetRow,
            r#"
            SELECT
                a.id, a.name, a.reference, a.isin, a.asset_class, a.currency, a.risk_level,
                c.id as "category_id?: String",
                c.name as "category_name?: String",
                a.is_archived as "is_archived: bool",
                a.exchange_code,
                a.price_refresh_blocked as "price_refresh_blocked: bool",
                a.interest_bearing as "interest_bearing: bool"
            FROM assets a
            LEFT JOIN categories c ON a.category_id = c.id AND c.is_deleted = 0
            WHERE a.is_deleted = 0 AND a.is_archived = 0
            "#
        )
        .fetch_all(&self.pool)
        .await
        .with_context(|| "Failed to fetch assets from database")?;

        Ok(rows.into_iter().map(Asset::from).collect())
    }

    async fn get_all_including_archived(&self) -> Result<Vec<Asset>> {
        let rows = sqlx::query_as!(
            AssetRow,
            r#"
            SELECT
                a.id, a.name, a.reference, a.isin, a.asset_class, a.currency, a.risk_level,
                c.id as "category_id?: String",
                c.name as "category_name?: String",
                a.is_archived as "is_archived: bool",
                a.exchange_code,
                a.price_refresh_blocked as "price_refresh_blocked: bool",
                a.interest_bearing as "interest_bearing: bool"
            FROM assets a
            LEFT JOIN categories c ON a.category_id = c.id AND c.is_deleted = 0
            WHERE a.is_deleted = 0
            "#
        )
        .fetch_all(&self.pool)
        .await
        .with_context(|| "Failed to fetch assets including archived from database")?;

        Ok(rows.into_iter().map(Asset::from).collect())
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Asset>> {
        let row = sqlx::query_as!(
            AssetRow,
            r#"
            SELECT
                a.id, a.name, a.reference, a.isin, a.asset_class, a.currency, a.risk_level,
                c.id as "category_id?: String",
                c.name as "category_name?: String",
                a.is_archived as "is_archived: bool",
                a.exchange_code,
                a.price_refresh_blocked as "price_refresh_blocked: bool",
                a.interest_bearing as "interest_bearing: bool"
            FROM assets a
            LEFT JOIN categories c ON a.category_id = c.id AND c.is_deleted = 0
            WHERE a.id = ? AND a.is_deleted = 0
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .with_context(|| format!("Failed to fetch asset with id: {}", id))?;

        Ok(row.map(Asset::from))
    }

    async fn create(&self, asset: Asset) -> Result<Asset> {
        let asset_class_str = asset.class.to_string();
        let exchange_code = asset.exchange.as_ref().map(|e| e.code.clone());
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin asset create")?;
        sqlx::query!(
            r#"INSERT INTO assets (id, name, reference, isin, asset_class, currency, risk_level, is_deleted, is_archived, category_id, exchange_code, interest_bearing) VALUES (?, ?, ?, ?, ?, ?, ?, 0, 0, ?, ?, ?)"#,
            asset.id,
            asset.name,
            asset.reference,
            asset.isin,
            asset_class_str,
            asset.currency,
            asset.risk_level,
            asset.category.id,
            exchange_code,
            asset.interest_bearing
        )
        .execute(&mut *tx)
        .await
        .with_context(|| format!("Failed to create asset: {}", asset.name))?;
        self.record_asset_state(&mut tx, &asset, Operation::Created, None)
            .await?;
        tx.commit().await.context("Failed to commit asset create")?;
        Ok(asset)
    }

    async fn update(&self, asset: Asset) -> Result<Asset> {
        let asset_class_str = asset.class.to_string();
        let exchange_code = asset.exchange.as_ref().map(|e| e.code.clone());
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin asset update")?;
        let based_on = current_asset_timestamp(&mut tx, &asset.id).await?;
        let written = sqlx::query!(
            r#"UPDATE assets SET name = ?, reference = ?, isin = ?, asset_class = ?, currency = ?, risk_level = ?, category_id = ?, exchange_code = ?, interest_bearing = ? WHERE id = ? AND is_archived = 0"#,
            asset.name,
            asset.reference,
            asset.isin,
            asset_class_str,
            asset.currency,
            asset.risk_level,
            asset.category.id,
            exchange_code,
            asset.interest_bearing,
            asset.id
        )
        .execute(&mut *tx)
        .await
        .with_context(|| format!("Failed to update asset with id: {}", asset.id))?;
        if written.rows_affected() > 0 {
            self.record_asset_state(&mut tx, &asset, Operation::Updated, based_on)
                .await?;
        }
        tx.commit().await.context("Failed to commit asset update")?;
        Ok(asset)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin asset delete")?;
        let based_on = current_asset_timestamp(&mut tx, id).await?;
        let deleted = sqlx::query!(r#"UPDATE assets SET is_deleted = 1 WHERE id = ?"#, id)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("Failed to soft delete asset with id: {}", id))?;
        if deleted.rows_affected() > 0 {
            let draft = ChangeDraft::new(
                RecordKind::Asset,
                asset_identity(id),
                Operation::Removed,
                Origin::User,
                based_on,
                None,
            );
            self.change_recorder.record(&mut tx, draft).await?;
        }
        tx.commit().await.context("Failed to commit asset delete")?;
        Ok(())
    }

    async fn archive(&self, id: &str) -> Result<()> {
        self.update_flag(
            id,
            "UPDATE assets SET is_archived = 1 WHERE id = ? AND is_deleted = 0",
            "archive",
        )
        .await
    }

    async fn unarchive(&self, id: &str) -> Result<()> {
        self.update_flag(
            id,
            "UPDATE assets SET is_archived = 0 WHERE id = ? AND is_deleted = 0",
            "unarchive",
        )
        .await
    }

    async fn block_price_refresh(&self, id: &str) -> Result<()> {
        self.update_flag(
            id,
            "UPDATE assets SET price_refresh_blocked = 1 WHERE id = ? AND is_deleted = 0",
            "price-refresh block",
        )
        .await
    }

    async fn unblock_price_refresh(&self, id: &str) -> Result<()> {
        self.update_flag(
            id,
            "UPDATE assets SET price_refresh_blocked = 0 WHERE id = ? AND is_deleted = 0",
            "price-refresh unblock",
        )
        .await
    }

    async fn synced_record(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> Result<Option<SyncedRecord>> {
        match kind {
            RecordKind::Asset => {
                let Some(asset) = fetch_asset(conn, identity).await? else {
                    return Ok(None);
                };
                let rank = sqlx::query_as!(
                    RankRow,
                    "SELECT sync_logical_timestamp, sync_origin, sync_device_id FROM assets WHERE id = ?",
                    identity
                )
                .fetch_optional(conn)
                .await
                .with_context(|| format!("Failed to read the rank of asset {}", identity))?
                .and_then(RankRow::rank);
                Ok(Some(SyncedRecord {
                    rank,
                    content: serde_json::to_string(&asset)?,
                }))
            }
            RecordKind::Category => {
                let row = sqlx::query!(
                    "SELECT id, name, sync_logical_timestamp, sync_origin, sync_device_id FROM categories WHERE id = ?",
                    identity
                )
                .fetch_optional(conn)
                .await
                .with_context(|| format!("Failed to read synced category {}", identity))?;
                row.map(|row| {
                    let category = AssetCategory::from_storage(row.id, row.name);
                    Ok(SyncedRecord {
                        rank: rank_from_columns(
                            row.sync_logical_timestamp,
                            row.sync_origin,
                            row.sync_device_id,
                        ),
                        content: serde_json::to_string(&category)?,
                    })
                })
                .transpose()
            }
            RecordKind::AssetPrice => {
                let (asset_id, date) = split_price_identity(identity)?;
                let row = sqlx::query!(
                    r#"SELECT asset_id, date, price, source, sync_logical_timestamp, sync_origin, sync_device_id
                       FROM asset_prices WHERE asset_id = ? AND date = ?"#,
                    asset_id,
                    date
                )
                .fetch_optional(conn)
                .await
                .with_context(|| format!("Failed to read synced asset price {}", identity))?;
                row.map(|row| {
                    let source = source_from_storage(&row.source);
                    let price = AssetPrice::restore(row.asset_id, row.date, row.price, source);
                    Ok(SyncedRecord {
                        rank: rank_from_columns(
                            row.sync_logical_timestamp,
                            row.sync_origin,
                            row.sync_device_id,
                        ),
                        content: serde_json::to_string(&price)?,
                    })
                })
                .transpose()
            }
            RecordKind::Account
            | RecordKind::Transaction
            | RecordKind::FeeSchedule
            | RecordKind::FeeCatchUpPosition
            | RecordKind::CurrencyPair
            | RecordKind::CurrencyRate
            | RecordKind::HoldingNote => Ok(None),
        }
    }

    async fn clashing_category_name_rank(
        &self,
        conn: &mut SqliteConnection,
        category_id: &str,
        name: &str,
    ) -> Result<Option<Rank>> {
        let row = sqlx::query_as!(
            RankRow,
            r#"SELECT sync_logical_timestamp, sync_origin, sync_device_id
               FROM categories WHERE LOWER(name) = LOWER(?) AND id <> ? AND is_deleted = 0
               ORDER BY sync_origin, sync_logical_timestamp, sync_device_id, id
               LIMIT 1"#,
            name,
            category_id
        )
        .fetch_optional(conn)
        .await
        .with_context(|| format!("Failed to look up categories named {}", name))?;
        Ok(row.and_then(RankRow::rank))
    }

    async fn apply_asset(
        &self,
        conn: &mut SqliteConnection,
        asset: &Asset,
        rank: &Rank,
    ) -> Result<()> {
        let asset_class_str = asset.class.to_string();
        let exchange_code = asset.exchange.as_ref().map(|e| e.code.clone());
        let columns = RankColumns::from(rank.clone());
        sqlx::query!(
            r#"INSERT INTO assets (id, name, reference, isin, asset_class, currency, risk_level, is_deleted,
                                   is_archived, category_id, exchange_code, price_refresh_blocked, interest_bearing,
                                   sync_logical_timestamp, sync_origin, sync_device_id)
               VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(id) DO UPDATE SET
                   name = excluded.name,
                   reference = excluded.reference,
                   isin = excluded.isin,
                   asset_class = excluded.asset_class,
                   currency = excluded.currency,
                   risk_level = excluded.risk_level,
                   is_deleted = 0,
                   is_archived = excluded.is_archived,
                   category_id = excluded.category_id,
                   exchange_code = excluded.exchange_code,
                   price_refresh_blocked = excluded.price_refresh_blocked,
                   interest_bearing = excluded.interest_bearing,
                   sync_logical_timestamp = excluded.sync_logical_timestamp,
                   sync_origin = excluded.sync_origin,
                   sync_device_id = excluded.sync_device_id"#,
            asset.id,
            asset.name,
            asset.reference,
            asset.isin,
            asset_class_str,
            asset.currency,
            asset.risk_level,
            asset.is_archived,
            asset.category.id,
            exchange_code,
            asset.price_refresh_blocked,
            asset.interest_bearing,
            columns.logical_timestamp,
            columns.origin,
            columns.device_id
        )
        .execute(conn)
        .await
        .with_context(|| format!("Failed to apply asset {}", asset.id))?;
        Ok(())
    }

    async fn apply_category(
        &self,
        conn: &mut SqliteConnection,
        category: &AssetCategory,
        rank: &Rank,
    ) -> Result<()> {
        let columns = RankColumns::from(rank.clone());
        sqlx::query!(
            r#"INSERT INTO categories (id, name, is_deleted, sync_logical_timestamp, sync_origin, sync_device_id)
               VALUES (?, ?, 0, ?, ?, ?)
               ON CONFLICT(id) DO UPDATE SET
                   name = excluded.name,
                   is_deleted = 0,
                   sync_logical_timestamp = excluded.sync_logical_timestamp,
                   sync_origin = excluded.sync_origin,
                   sync_device_id = excluded.sync_device_id"#,
            category.id,
            category.name,
            columns.logical_timestamp,
            columns.origin,
            columns.device_id
        )
        .execute(conn)
        .await
        .with_context(|| format!("Failed to apply category {}", category.id))?;
        Ok(())
    }

    async fn apply_asset_price(
        &self,
        conn: &mut SqliteConnection,
        price: &AssetPrice,
        rank: &Rank,
    ) -> Result<()> {
        let source = price.source.to_string();
        let columns = RankColumns::from(rank.clone());
        sqlx::query!(
            r#"INSERT INTO asset_prices (asset_id, date, price, source, sync_logical_timestamp, sync_origin, sync_device_id)
               VALUES (?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(asset_id, date) DO UPDATE SET
                   price = excluded.price,
                   source = excluded.source,
                   sync_logical_timestamp = excluded.sync_logical_timestamp,
                   sync_origin = excluded.sync_origin,
                   sync_device_id = excluded.sync_device_id"#,
            price.asset_id,
            price.date,
            price.price,
            source,
            columns.logical_timestamp,
            columns.origin,
            columns.device_id
        )
        .execute(conn)
        .await
        .context("Failed to apply asset price")?;
        Ok(())
    }

    async fn remove_synced(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> Result<()> {
        match kind {
            RecordKind::Asset => {
                sqlx::query!("UPDATE assets SET is_deleted = 1 WHERE id = ?", identity)
                    .execute(conn)
                    .await
                    .with_context(|| format!("Failed to remove asset {}", identity))?;
            }
            RecordKind::Category => {
                sqlx::query!(
                    "UPDATE categories SET is_deleted = 1 WHERE id = ?",
                    identity
                )
                .execute(conn)
                .await
                .with_context(|| format!("Failed to remove category {}", identity))?;
            }
            RecordKind::AssetPrice => {
                let (asset_id, date) = split_price_identity(identity)?;
                sqlx::query!(
                    "DELETE FROM asset_prices WHERE asset_id = ? AND date = ?",
                    asset_id,
                    date
                )
                .execute(conn)
                .await
                .with_context(|| format!("Failed to remove asset price {}", identity))?;
            }
            RecordKind::Account
            | RecordKind::Transaction
            | RecordKind::FeeSchedule
            | RecordKind::FeeCatchUpPosition
            | RecordKind::CurrencyPair
            | RecordKind::CurrencyRate
            | RecordKind::HoldingNote => {}
        }
        Ok(())
    }

    async fn discard_asset_prices(&self, conn: &mut SqliteConnection) -> Result<()> {
        sqlx::query!("DELETE FROM asset_prices")
            .execute(conn)
            .await
            .context("Failed to discard asset prices")?;
        Ok(())
    }

    async fn ensure_seeded(
        &self,
        conn: &mut SqliteConnection,
        category: &AssetCategory,
        asset: &Asset,
    ) -> Result<()> {
        sqlx::query!(
            "INSERT OR IGNORE INTO categories (id, name, is_deleted) VALUES (?, ?, 0)",
            category.id,
            category.name
        )
        .execute(&mut *conn)
        .await
        .with_context(|| format!("Failed to seed category {}", category.id))?;
        let asset_class_str = asset.class.to_string();
        sqlx::query!(
            r#"INSERT OR IGNORE INTO assets (id, name, reference, isin, asset_class, currency, risk_level, is_deleted, is_archived, category_id, exchange_code, interest_bearing)
               VALUES (?, ?, ?, ?, ?, ?, ?, 0, 0, ?, NULL, ?)"#,
            asset.id,
            asset.name,
            asset.reference,
            asset.isin,
            asset_class_str,
            asset.currency,
            asset.risk_level,
            asset.category.id,
            asset.interest_bearing
        )
        .execute(conn)
        .await
        .with_context(|| format!("Failed to seed asset {}", asset.id))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_pool() -> Pool<Sqlite> {
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

    /// Inserts a minimal active asset (FK-satisfied by the migration-seeded
    /// `default-uncategorized` category) so the column-update methods have a row.
    async fn seed_asset(pool: &Pool<Sqlite>, id: &str) {
        sqlx::query!(
            "INSERT INTO assets (id, name, reference, asset_class, currency, risk_level, category_id, is_archived)
             VALUES (?, 'Test Asset', 'REF', 'Stocks', 'USD', 3, 'default-uncategorized', 0)",
            id,
        )
        .execute(pool)
        .await
        .expect("seed asset");
    }

    // MKT-150 — block_price_refresh sets the flag; get_by_id reflects it.
    #[tokio::test]
    async fn block_price_refresh_sets_flag_and_round_trips() {
        let pool = setup_pool().await;
        seed_asset(&pool, "a1").await;
        let repo = SqliteAssetRepository::new(pool);

        let before = repo.get_by_id("a1").await.unwrap().unwrap();
        assert!(
            !before.price_refresh_blocked,
            "seeded asset starts unlocked"
        );

        repo.block_price_refresh("a1").await.unwrap();

        let after = repo.get_by_id("a1").await.unwrap().unwrap();
        assert!(after.price_refresh_blocked);
    }

    /// Builds an active asset for the round-trip tests, FK-satisfied by the
    /// migration-seeded `default-uncategorized` category.
    fn asset_with_interest_bearing(id: &str, interest_bearing: bool) -> Asset {
        Asset::restore(
            id.to_string(),
            "Euro Fund".to_string(),
            AssetClass::MutualFunds,
            AssetCategory::from_storage(
                "default-uncategorized".to_string(),
                "Uncategorized".to_string(),
            ),
            "EUR".to_string(),
            2,
            "EUROFUND".to_string(),
            None,
            false,
            None,
            false,
            interest_bearing,
        )
    }

    // AST-024 — interest_bearing round-trips through create → get_by_id → update.
    #[tokio::test]
    async fn interest_bearing_round_trips_through_create_and_update() {
        let pool = setup_pool().await;
        let repo = SqliteAssetRepository::new(pool);

        repo.create(asset_with_interest_bearing("a1", true))
            .await
            .unwrap();
        let created = repo.get_by_id("a1").await.unwrap().unwrap();
        assert!(created.interest_bearing);

        repo.update(asset_with_interest_bearing("a1", false))
            .await
            .unwrap();
        let updated = repo.get_by_id("a1").await.unwrap().unwrap();
        assert!(!updated.interest_bearing);
    }

    // AST-024 — a raw seeded row (no interest_bearing column value) loads with
    // the migration default of false.
    #[tokio::test]
    async fn seeded_asset_defaults_to_not_interest_bearing() {
        let pool = setup_pool().await;
        seed_asset(&pool, "a1").await;
        let repo = SqliteAssetRepository::new(pool);

        let loaded = repo.get_by_id("a1").await.unwrap().unwrap();
        assert!(!loaded.interest_bearing);
    }

    // MKT-156 — unblock_price_refresh clears the flag set by block.
    #[tokio::test]
    async fn unblock_price_refresh_clears_flag() {
        let pool = setup_pool().await;
        seed_asset(&pool, "a1").await;
        let repo = SqliteAssetRepository::new(pool);

        repo.block_price_refresh("a1").await.unwrap();
        repo.unblock_price_refresh("a1").await.unwrap();

        let after = repo.get_by_id("a1").await.unwrap().unwrap();
        assert!(!after.price_refresh_blocked);
    }

    use crate::context::sync::SqliteChangeRecorder;
    use std::sync::Arc;

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

    async fn seed_sync_device(pool: &Pool<Sqlite>) {
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

    async fn changes_with_operation(pool: &Pool<Sqlite>, operation: &str) -> i64 {
        sqlx::query_scalar!(
            "SELECT COUNT(*) FROM changes WHERE operation = ?",
            operation
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    fn test_asset(id: &str) -> Asset {
        Asset::restore(
            id.to_string(),
            "Euro Fund".to_string(),
            AssetClass::MutualFunds,
            AssetCategory::from_storage(
                "default-uncategorized".to_string(),
                "Uncategorized".to_string(),
            ),
            "EUR".to_string(),
            2,
            "EUROFUND".to_string(),
            None,
            false,
            None,
            false,
            false,
        )
    }

    // SYN-020/021 — create records exactly one Created change, rank-stamped.
    #[tokio::test]
    async fn create_records_one_created_change_with_rank_stamped() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAssetRepository::new(pool.clone()).with_change_recorder(recorder);

        repo.create(test_asset("a1")).await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Created").await, 1);
        let row = sqlx::query!("SELECT sync_logical_timestamp FROM assets WHERE id = 'a1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(row.sync_logical_timestamp.is_some());
    }

    // CFR-014/D6 — stamp_sync_rank ranks only the rows that were never ranked; a row the
    // recorder already ranked keeps its own rank.
    #[tokio::test]
    async fn stamp_sync_rank_stamps_only_unranked_rows() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        SqliteAssetRepository::new(pool.clone())
            .create(test_asset("a-unranked"))
            .await
            .unwrap();
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAssetRepository::new(pool.clone()).with_change_recorder(recorder);
        repo.create(test_asset("a-ranked")).await.unwrap();

        let rank = Rank {
            origin: Origin::User,
            logical_timestamp: crate::shared::domain::LogicalTimestamp::new(99),
            device_id: "desktop-device".to_string(),
        };
        let mut conn = pool.acquire().await.unwrap();
        let stamped = repo.stamp_sync_rank(&mut conn, &rank).await.unwrap();
        drop(conn);
        assert!(stamped >= 1, "the unranked asset is stamped: {stamped}");

        let rows = sqlx::query!("SELECT id, sync_logical_timestamp FROM assets")
            .fetch_all(&pool)
            .await
            .unwrap();
        let stamp_of = |id: &str| {
            rows.iter()
                .find(|row| row.id == id)
                .and_then(|row| row.sync_logical_timestamp.clone())
        };
        assert_eq!(
            stamp_of("a-unranked").as_deref(),
            Some("00000000000000000099")
        );
        assert_ne!(
            stamp_of("a-ranked").as_deref(),
            Some("00000000000000000099"),
            "a row the recorder ranked keeps its rank"
        );
        let unranked_categories: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM categories WHERE sync_logical_timestamp IS NULL"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(unranked_categories, 0, "categories are stamped too");
    }

    // SYN-020 — update records exactly one Updated change.
    #[tokio::test]
    async fn update_records_one_updated_change() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let setup_repo = SqliteAssetRepository::new(pool.clone());
        setup_repo.create(test_asset("a1")).await.unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAssetRepository::new(pool.clone()).with_change_recorder(recorder);
        let mut renamed = test_asset("a1");
        renamed.name = "Euro Fund Renamed".to_string();
        repo.update(renamed).await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Updated").await, 1);
    }

    // SYN-020/024 — delete (soft-delete) records exactly one Removed change + tombstone.
    #[tokio::test]
    async fn delete_records_one_removed_change_and_tombstone() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let setup_repo = SqliteAssetRepository::new(pool.clone());
        setup_repo.create(test_asset("a1")).await.unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAssetRepository::new(pool.clone()).with_change_recorder(recorder);
        repo.delete("a1").await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Removed").await, 1);
        let tombstone = sqlx::query!(
            "SELECT record_identity FROM tombstones WHERE record_kind = 'Asset' AND record_identity = 'a1'"
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert!(tombstone.is_some());
    }

    // SYN-020 — archive records exactly one Updated change (a state field, not a removal).
    #[tokio::test]
    async fn archive_records_one_updated_change() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let setup_repo = SqliteAssetRepository::new(pool.clone());
        setup_repo.create(test_asset("a1")).await.unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAssetRepository::new(pool.clone()).with_change_recorder(recorder);
        repo.archive("a1").await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Updated").await, 1);
    }

    // SYN-020 — unarchive records exactly one Updated change.
    #[tokio::test]
    async fn unarchive_records_one_updated_change() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let setup_repo = SqliteAssetRepository::new(pool.clone());
        setup_repo.create(test_asset("a1")).await.unwrap();
        setup_repo.archive("a1").await.unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAssetRepository::new(pool.clone()).with_change_recorder(recorder);
        repo.unarchive("a1").await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Updated").await, 1);
    }

    // SYN-020 — block_price_refresh records exactly one Updated change.
    #[tokio::test]
    async fn block_price_refresh_records_one_updated_change() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let setup_repo = SqliteAssetRepository::new(pool.clone());
        setup_repo.create(test_asset("a1")).await.unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAssetRepository::new(pool.clone()).with_change_recorder(recorder);
        repo.block_price_refresh("a1").await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Updated").await, 1);
    }

    // SYN-020 — unblock_price_refresh records exactly one Updated change.
    #[tokio::test]
    async fn unblock_price_refresh_records_one_updated_change() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let setup_repo = SqliteAssetRepository::new(pool.clone());
        setup_repo.create(test_asset("a1")).await.unwrap();
        setup_repo.block_price_refresh("a1").await.unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAssetRepository::new(pool.clone()).with_change_recorder(recorder);
        repo.unblock_price_refresh("a1").await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Updated").await, 1);
    }

    // SYN-020 — a failed write records no change (rollback).
    #[tokio::test]
    async fn create_rolls_back_change_when_the_write_fails() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAssetRepository::new(pool.clone()).with_change_recorder(recorder);
        repo.create(test_asset("a1")).await.unwrap();

        // Same id — PRIMARY KEY violation, the whole write must fail atomically.
        let result = repo.create(test_asset("a1")).await;
        assert!(result.is_err());

        assert_eq!(
            changes_with_operation(&pool, "Created").await,
            1,
            "only the first (successful) create recorded a change"
        );
    }

    // -------------------------------------------------------------------------
    // CFR-030 — an asset whose category stands removed (tombstoned by another device's
    // merge) resolves to the default category on read, instead of disappearing under an
    // INNER JOIN. `get_all`, `get_all_including_archived`, and `get_by_id` all read
    // `categories` through `JOIN … WHERE c.is_deleted = 0` today, which drops the asset
    // entirely once its category is tombstoned — the fix is a `LEFT JOIN` resolving to
    // `SYSTEM_CATEGORY_ID`, derived on read, nothing rewritten.
    // -------------------------------------------------------------------------

    use crate::context::asset::SYSTEM_CATEGORY_ID;

    async fn seed_asset_in_a_removed_category(pool: &Pool<Sqlite>, asset_id: &str) {
        sqlx::query(
            "INSERT INTO categories (id, name, is_deleted) VALUES ('cat-removed', 'Tech', 0)",
        )
        .execute(pool)
        .await
        .expect("seed category");
        sqlx::query(
            "INSERT INTO assets (id, name, reference, asset_class, currency, risk_level, category_id, is_archived)
             VALUES (?, 'Orphaned Asset', 'REF', 'Stocks', 'USD', 3, 'cat-removed', 0)",
        )
        .bind(asset_id)
        .execute(pool)
        .await
        .expect("seed asset");
        // Simulates the category's tombstone having applied on this device (CFR-030)
        // before the asset row itself was ever visited — the state a joining/syncing
        // device can genuinely be in between applying the two changes.
        sqlx::query("UPDATE categories SET is_deleted = 1 WHERE id = 'cat-removed'")
            .execute(pool)
            .await
            .expect("mark category removed");
    }

    // CFR-030 — get_all must still return the asset, with the default category resolved.
    #[tokio::test]
    async fn get_all_resolves_a_removed_category_to_the_default_instead_of_dropping_the_asset() {
        let pool = setup_pool().await;
        seed_asset_in_a_removed_category(&pool, "asset-orphaned").await;
        let repo = SqliteAssetRepository::new(pool);

        let assets = repo.get_all().await.unwrap();
        let orphaned = assets
            .iter()
            .find(|asset| asset.id == "asset-orphaned")
            .expect("CFR-030: the asset must not disappear when its category is tombstoned");
        assert_eq!(orphaned.category.id, SYSTEM_CATEGORY_ID);
    }

    // CFR-030 — get_all_including_archived must likewise resolve, not drop.
    #[tokio::test]
    async fn get_all_including_archived_resolves_a_removed_category() {
        let pool = setup_pool().await;
        seed_asset_in_a_removed_category(&pool, "asset-orphaned").await;
        let repo = SqliteAssetRepository::new(pool);

        let assets = repo.get_all_including_archived().await.unwrap();
        let orphaned = assets
            .iter()
            .find(|asset| asset.id == "asset-orphaned")
            .expect("CFR-030: the asset must not disappear when its category is tombstoned");
        assert_eq!(orphaned.category.id, SYSTEM_CATEGORY_ID);
    }

    // CFR-030 — get_by_id must likewise resolve, not return None.
    #[tokio::test]
    async fn get_by_id_resolves_a_removed_category() {
        let pool = setup_pool().await;
        seed_asset_in_a_removed_category(&pool, "asset-orphaned").await;
        let repo = SqliteAssetRepository::new(pool);

        let asset = repo
            .get_by_id("asset-orphaned")
            .await
            .unwrap()
            .expect("CFR-030: the asset must still be found by id");
        assert_eq!(asset.category.id, SYSTEM_CATEGORY_ID);
    }
}
