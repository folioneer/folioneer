use std::sync::Arc;

use super::super::domain::{AssetCategory, AssetCategoryRepository};
use super::asset::{asset_identity, current_asset_timestamp, fetch_asset, stamp_asset_rank};
use crate::shared::domain::{
    ChangeDraft, LogicalTimestamp, Operation, Origin, RecordIdentity, RecordKind,
};
use crate::shared::infrastructure::change_recorder::{
    ChangeRecorder, NoopChangeRecorder, RankColumns,
};
use anyhow::{Context, Result};
use sqlx::{Pool, Sqlite, SqliteConnection, Transaction};

#[derive(sqlx::FromRow)]
struct CategoryRow {
    id: String,
    name: String,
}

impl From<CategoryRow> for AssetCategory {
    fn from(row: CategoryRow) -> Self {
        AssetCategory::from_storage(row.id, row.name)
    }
}

/// SQLite implementation of the AssetCategoryRepository.
#[derive(Clone)]
pub struct SqliteAssetCategoryRepository {
    pool: Pool<Sqlite>,
    change_recorder: Arc<dyn ChangeRecorder>,
}

impl SqliteAssetCategoryRepository {
    /// Creates a new SqliteAssetCategoryRepository.
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

    async fn record_category_state(
        &self,
        conn: &mut SqliteConnection,
        category: &AssetCategory,
        operation: Operation,
        based_on: Option<LogicalTimestamp>,
    ) -> Result<()> {
        let draft = ChangeDraft::new(
            RecordKind::Category,
            category_identity(&category.id),
            operation,
            Origin::User,
            based_on,
            Some(serde_json::to_string(category)?),
        );
        let rank = self.change_recorder.record(conn, draft).await?;
        if let Some(rank) = rank {
            let columns = RankColumns::from(rank);
            sqlx::query!(
                r#"UPDATE categories SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ? WHERE id = ?"#,
                columns.logical_timestamp,
                columns.origin,
                columns.device_id,
                category.id
            )
            .execute(conn)
            .await
            .with_context(|| format!("Failed to stamp rank on category: {}", category.id))?;
        }
        Ok(())
    }
}

fn category_identity(id: &str) -> RecordIdentity {
    RecordIdentity::canonical(RecordKind::Category, &[id])
}

/// CFR-011 — the logical timestamp of the category's current state, the `based_on` of the
/// next local change to it; `None` while absent or never ranked.
async fn current_category_timestamp(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<Option<LogicalTimestamp>> {
    let stored = sqlx::query_scalar!(
        r#"SELECT sync_logical_timestamp AS "sync_logical_timestamp?: String" FROM categories WHERE id = ?"#,
        id
    )
    .fetch_optional(conn)
    .await
    .with_context(|| format!("Failed to read the rank of category {}", id))?;
    Ok(stored
        .flatten()
        .and_then(|timestamp| LogicalTimestamp::from_wire(&timestamp)))
}

#[async_trait::async_trait]
impl AssetCategoryRepository for SqliteAssetCategoryRepository {
    async fn get_all(&self) -> Result<Vec<AssetCategory>> {
        let categories = sqlx::query_as!(
            CategoryRow,
            r#"
            SELECT id, name
            FROM categories 
            WHERE is_deleted = 0
            "#
        )
        .fetch_all(&self.pool)
        .await
        .with_context(|| "Failed to fetch all categories")?;

        Ok(categories.into_iter().map(AssetCategory::from).collect())
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<AssetCategory>> {
        let row = sqlx::query_as!(
            CategoryRow,
            r#"
            SELECT id, name
            FROM categories
            WHERE LOWER(name) = LOWER(?) AND is_deleted = 0
            "#,
            name
        )
        .fetch_optional(&self.pool)
        .await
        .with_context(|| format!("Failed to find category by name: {}", name))?;

        Ok(row.map(AssetCategory::from))
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<AssetCategory>> {
        let row = sqlx::query_as!(
            CategoryRow,
            r#"
            SELECT id, name
            FROM categories 
            WHERE id = ? AND is_deleted = 0
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .with_context(|| format!("Failed to fetch category with id {}", id))?;

        Ok(row.map(AssetCategory::from))
    }

    async fn create(&self, category: AssetCategory) -> Result<AssetCategory> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin category create")?;
        sqlx::query!(
            r#"INSERT INTO categories (id, name, is_deleted) VALUES (?, ?, 0)"#,
            category.id,
            category.name,
        )
        .execute(&mut *tx)
        .await
        .with_context(|| format!("Failed to create category: {}", category.name))?;
        self.record_category_state(&mut tx, &category, Operation::Created, None)
            .await?;
        tx.commit()
            .await
            .context("Failed to commit category create")?;

        Ok(category)
    }

    async fn update(&self, category: AssetCategory) -> Result<AssetCategory> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin category update")?;
        let based_on = current_category_timestamp(&mut tx, &category.id).await?;
        let written = sqlx::query!(
            r#"UPDATE categories SET name = ? WHERE id = ?"#,
            category.name,
            category.id
        )
        .execute(&mut *tx)
        .await
        .with_context(|| format!("Failed to update category: {}", category.id))?;
        if written.rows_affected() > 0 {
            self.record_category_state(&mut tx, &category, Operation::Updated, based_on)
                .await?;
        }
        tx.commit()
            .await
            .context("Failed to commit category update")?;

        Ok(category)
    }

    async fn reassign_assets_and_delete(&self, category_id: &str, fallback_id: &str) -> Result<()> {
        let mut tx: Transaction<Sqlite> = self.pool.begin().await?;

        let reassigned_asset_ids: Vec<String> = sqlx::query_scalar!(
            r#"SELECT id FROM assets WHERE category_id = ? AND is_deleted = 0"#,
            category_id
        )
        .fetch_all(&mut *tx)
        .await
        .with_context(|| format!("Failed to list assets of category: {}", category_id))?;

        sqlx::query!(
            r#"UPDATE assets SET category_id = ? WHERE category_id = ? AND is_deleted = 0"#,
            fallback_id,
            category_id
        )
        .execute(&mut *tx)
        .await
        .with_context(|| format!("Failed to reassign assets from category: {}", category_id))?;

        // CFR-030 — every reassigned asset is its own Updated change.
        for asset_id in &reassigned_asset_ids {
            let based_on = current_asset_timestamp(&mut tx, asset_id).await?;
            if let Some(asset) = fetch_asset(&mut tx, asset_id).await? {
                let draft = ChangeDraft::new(
                    RecordKind::Asset,
                    asset_identity(&asset.id),
                    Operation::Updated,
                    Origin::User,
                    based_on,
                    Some(serde_json::to_string(&asset)?),
                );
                let rank = self.change_recorder.record(&mut tx, draft).await?;
                if let Some(rank) = rank {
                    stamp_asset_rank(&mut tx, &asset.id, rank).await?;
                }
            }
        }

        let based_on = current_category_timestamp(&mut tx, category_id).await?;
        let deleted = sqlx::query!(
            r#"UPDATE categories SET is_deleted = 1 WHERE id = ?"#,
            category_id
        )
        .execute(&mut *tx)
        .await
        .with_context(|| format!("Failed to delete category: {}", category_id))?;
        if deleted.rows_affected() > 0 {
            let draft = ChangeDraft::new(
                RecordKind::Category,
                category_identity(category_id),
                Operation::Removed,
                Origin::User,
                based_on,
                None,
            );
            self.change_recorder.record(&mut tx, draft).await?;
        }

        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::sync::SqliteChangeRecorder;
    use sqlx::sqlite::SqlitePoolOptions;
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

    fn category(id: &str, name: &str) -> AssetCategory {
        AssetCategory::from_storage(id.to_string(), name.to_string())
    }

    // SYN-020/021 — create records exactly one Created change, rank-stamped.
    #[tokio::test]
    async fn create_records_one_created_change_with_rank_stamped() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAssetCategoryRepository::new(pool.clone()).with_change_recorder(recorder);

        repo.create(category("cat-1", "Tech")).await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Created").await, 1);
        let row = sqlx::query!("SELECT sync_logical_timestamp FROM categories WHERE id = 'cat-1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(row.sync_logical_timestamp.is_some());
    }

    // SYN-020 — update records exactly one Updated change.
    #[tokio::test]
    async fn update_records_one_updated_change() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let setup_repo = SqliteAssetCategoryRepository::new(pool.clone());
        setup_repo.create(category("cat-1", "Tech")).await.unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAssetCategoryRepository::new(pool.clone()).with_change_recorder(recorder);
        repo.update(category("cat-1", "Technology")).await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Updated").await, 1);
    }

    // CFR-030 — reassign_assets_and_delete records one Removed change for the category and
    // one Updated change per reassigned asset (SYN-024's "N asset updates").
    #[tokio::test]
    async fn reassign_assets_and_delete_records_one_removed_and_n_updated_changes() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let setup_repo = SqliteAssetCategoryRepository::new(pool.clone());
        setup_repo.create(category("cat-1", "Tech")).await.unwrap();

        // Two assets in the category being deleted, reassigned to the fallback.
        for id in ["a1", "a2"] {
            sqlx::query!(
                "INSERT INTO assets (id, name, reference, asset_class, currency, risk_level, category_id, is_archived)
                 VALUES (?, 'Asset', 'REF', 'Stocks', 'EUR', 3, 'cat-1', 0)",
                id,
            )
            .execute(&pool)
            .await
            .unwrap();
        }

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAssetCategoryRepository::new(pool.clone()).with_change_recorder(recorder);
        repo.reassign_assets_and_delete("cat-1", "default-uncategorized")
            .await
            .unwrap();

        assert_eq!(
            changes_with_operation(&pool, "Removed").await,
            1,
            "CFR-030: one Removed change for the deleted category"
        );
        assert_eq!(
            changes_with_operation(&pool, "Updated").await,
            2,
            "CFR-030: one Updated change per reassigned asset"
        );
        let tombstone = sqlx::query!(
            "SELECT record_identity FROM tombstones WHERE record_kind = 'Category' AND record_identity = 'cat-1'"
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert!(tombstone.is_some());
    }

    // SYN-020 — reassigning a category with no assets in it still records exactly one
    // Removed change for the category and zero asset updates (the N=0 edge of CFR-030).
    #[tokio::test]
    async fn reassign_assets_and_delete_with_no_assets_records_only_the_category_removal() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let setup_repo = SqliteAssetCategoryRepository::new(pool.clone());
        setup_repo.create(category("cat-1", "Tech")).await.unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAssetCategoryRepository::new(pool.clone()).with_change_recorder(recorder);
        repo.reassign_assets_and_delete("cat-1", "default-uncategorized")
            .await
            .unwrap();

        assert_eq!(changes_with_operation(&pool, "Removed").await, 1);
        assert_eq!(
            changes_with_operation(&pool, "Updated").await,
            0,
            "no assets were in the category — no asset-update changes"
        );
    }
}
