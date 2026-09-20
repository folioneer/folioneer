//! Enrolling this device as the origin of a portfolio (SYN-013/026/081) — as the first device
//! of an empty folder, as the new origin after a start-over (SYN-071), or into an empty
//! folder it moves to (SYN-074): the folder holds no portfolio yet, so enrolling writes the
//! header, publishes this device's whole current portfolio as one segment — one creation
//! change per existing record, including everything recorded before sync existed (SYN-026) —
//! stamps every existing record's rank columns with that segment's logical timestamp
//! (CFR-014, D6), then rewrites the manifest. A failure at any point rolls everything back
//! (SYN-013). Moving to a folder that already holds the same portfolio (SYN-074) lives here
//! too, since it is decided with the same kept key.

use std::sync::Arc;

use zeroize::Zeroizing;

use crate::context::sync::application::run::kept_key;
use crate::context::sync::domain::{
    ensure_device_name, ChangeLogRepository, DerivationParameters, FolderHeader, FolderProblem,
    FolderStore, Manifest, PortfolioRecord, PortfolioSnapshot, RankStamper, Segment, SegmentChange,
    SyncDevice, SyncStateRepository, SyncStatus, WriteHeaderOutcome, APP_VERSION,
};
use crate::context::sync::error::SyncError;
use crate::context::sync::infrastructure::codec::{
    decode_header, encode_header, encode_manifest, encode_segment, header_data_format_version,
    DATA_FORMAT_VERSION,
};
use crate::context::sync::infrastructure::crypto::{
    derive_key_blocking, ensure_derivation_parameters, ensure_passphrase_length,
    generate_derivation_parameters, make_check, verify_check, Key,
};
use crate::core::logger::BACKEND;
use crate::shared::domain::{LogicalTimestamp, Operation, Origin, Rank};

/// Enrolls this installation as the origin of a shared portfolio.
pub struct FirstPublish {
    change_log: Arc<dyn ChangeLogRepository>,
    state_repo: Arc<dyn SyncStateRepository>,
    folder_store: Arc<dyn FolderStore>,
    rank_stamper: Arc<dyn RankStamper>,
    snapshot: Arc<dyn PortfolioSnapshot>,
}

impl FirstPublish {
    /// Creates the enrolment orchestration bound to the given change log, sync state, folder,
    /// rank stamper, and portfolio snapshot.
    pub fn new(
        change_log: Arc<dyn ChangeLogRepository>,
        state_repo: Arc<dyn SyncStateRepository>,
        folder_store: Arc<dyn FolderStore>,
        rank_stamper: Arc<dyn RankStamper>,
        snapshot: Arc<dyn PortfolioSnapshot>,
    ) -> Self {
        Self {
            change_log,
            state_repo,
            folder_store,
            rank_stamper,
            snapshot,
        }
    }

    /// Enables sync as the first device (SYN-013): re-checks for a header immediately before
    /// writing it (`PortfolioCreatedElsewhere`, SYN-081), writes the header, publishes one
    /// segment carrying a `Created` change per existing synced record — all stamped with the
    /// same logical timestamp, `Origin::User`, and this device's identity (CFR-014) — then
    /// the manifest. Rolls back everything written on any failure (`PublishFailed`).
    pub async fn enable_as_first_device(
        &self,
        folder: String,
        passphrase: String,
        device_name: String,
    ) -> Result<SyncStatus, SyncError> {
        ensure_passphrase_length(&passphrase)?;
        ensure_device_name(&device_name)?;
        let passphrase = Zeroizing::new(passphrase);
        let derivation_parameters = generate_derivation_parameters();
        let key = derive_key_blocking(passphrase, derivation_parameters.clone()).await?;
        self.publish_as_origin(folder, device_name, key, derivation_parameters)
            .await
    }

    /// Designates a different folder for an enrolled device (SYN-074): one holding the same
    /// portfolio (its passphrase check matches the kept key) is adopted as is; an empty one
    /// receives this device's current portfolio as a first device, under the kept key and
    /// the current folder's derivation parameters. Any other folder is
    /// `FolderHoldsOtherPortfolio`.
    pub async fn change_folder(
        &self,
        device: SyncDevice,
        folder: String,
    ) -> Result<SyncStatus, SyncError> {
        let Some(key) = kept_key(self.change_log.as_ref()).await? else {
            tracing::error!(target: BACKEND, "change_folder: the kept key is unusable");
            return Err(SyncError::DatabaseError);
        };
        self.folder_store.retarget(&folder);
        self.folder_store
            .check_available()
            .await
            .map_err(|problem| SyncError::FolderUnavailable { problem })?;
        match self.folder_store.read_header_bytes().await? {
            Some(bytes) => {
                if let Some(data_format_version) = header_data_format_version(&bytes)
                    .filter(|version| *version > DATA_FORMAT_VERSION)
                {
                    return Err(SyncError::UpdateRequired {
                        data_format_version,
                    });
                }
                let header = decode_header(&bytes)?;
                if !verify_check(&key, &header.passphrase_check) {
                    return Err(SyncError::FolderHoldsOtherPortfolio);
                }
                let moved = device.designate_folder(folder);
                self.state_repo.save_device(&moved).await?;
                Ok(SyncStatus::for_device(&moved, None, vec![]))
            }
            None => {
                let derivation_parameters = self.derivation_parameters_of(&device.folder).await?;
                self.publish_as_origin(
                    folder,
                    device.device_name.clone(),
                    key,
                    derivation_parameters,
                )
                .await
            }
        }
    }

    /// The derivation parameters of the portfolio published in `folder` — what every other
    /// device combines with the passphrase to reach the kept key (SYN-051). A header carrying
    /// parameters outside this build's bounds is `HeaderRejected`, never propagated.
    async fn derivation_parameters_of(
        &self,
        folder: &str,
    ) -> Result<DerivationParameters, SyncError> {
        self.folder_store.retarget(folder);
        let bytes =
            self.folder_store
                .read_header_bytes()
                .await?
                .ok_or(SyncError::FolderUnavailable {
                    problem: FolderProblem::Missing,
                })?;
        let derivation_parameters = decode_header(&bytes)?.derivation_parameters;
        ensure_derivation_parameters(&derivation_parameters)?;
        Ok(derivation_parameters)
    }

    /// Publishes this device's whole portfolio into `folder` as its origin (SYN-013): header,
    /// first segment, manifest, device row — or nothing at all.
    async fn publish_as_origin(
        &self,
        folder: String,
        device_name: String,
        key: Key,
        derivation_parameters: DerivationParameters,
    ) -> Result<SyncStatus, SyncError> {
        self.folder_store.retarget(&folder);
        let records = self.snapshot.records().await?;
        let logical_clock = self.change_log.logical_clock().await? + 1;
        let timestamp = LogicalTimestamp::new(logical_clock as u64);
        let device = match self.state_repo.get_device().await? {
            Some(enrolled) => enrolled.re_enroll(
                device_name,
                folder,
                timestamp.as_str().to_string(),
                DATA_FORMAT_VERSION,
            )?,
            None => SyncDevice::new(
                device_name,
                folder,
                timestamp.as_str().to_string(),
                DATA_FORMAT_VERSION,
            )?,
        };
        let header = FolderHeader {
            derivation_parameters,
            passphrase_check: make_check(&key),
            data_format_version: DATA_FORMAT_VERSION,
            created_at: timestamp.as_str().to_string(),
            created_by_device_id: device.device_id.clone(),
        };
        match self
            .folder_store
            .write_header_if_absent(encode_header(&header)?)
            .await?
        {
            WriteHeaderOutcome::AlreadyExists => return Err(SyncError::PortfolioCreatedElsewhere),
            WriteHeaderOutcome::Written => {}
        }
        match self
            .enroll(&device, &key, logical_clock, &timestamp, records)
            .await
        {
            Ok(()) => Ok(SyncStatus::for_device(&device, None, vec![])),
            Err(error) => {
                self.roll_back_folder(&device.device_id).await;
                Err(error)
            }
        }
    }

    /// The device row, the first segment's changes, and every rank stamp in one transaction;
    /// the segment and the manifest written before it commits (SYN-013).
    async fn enroll(
        &self,
        device: &SyncDevice,
        key: &Key,
        logical_clock: i64,
        timestamp: &LogicalTimestamp,
        records: Vec<PortfolioRecord>,
    ) -> Result<(), SyncError> {
        let mut transaction = self.change_log.begin().await?;
        self.change_log
            .save_enrolment(&mut transaction, device, key.as_bytes(), logical_clock)
            .await?;
        self.change_log
            .retire_earlier_changes(&mut transaction, &device.device_id)
            .await?;
        let first_sequence = self
            .change_log
            .next_sequence(&mut transaction, &device.device_id)
            .await?;

        let rank = Rank {
            origin: Origin::User,
            logical_timestamp: timestamp.clone(),
            device_id: device.device_id.clone(),
        };
        let mut changes = Vec::with_capacity(records.len());
        for (offset, record) in records.into_iter().enumerate() {
            let change = SegmentChange {
                sequence: first_sequence + offset as i64,
                logical_timestamp: rank.logical_timestamp.as_str().to_string(),
                based_on: None,
                record_kind: record.record_kind,
                record_identity: record.record_identity.as_str().to_string(),
                operation: Operation::Created,
                origin: Origin::User,
                content: Some(record.content),
            };
            self.change_log
                .append_published_change(&mut transaction, &device.device_id, &change)
                .await?;
            changes.push(change);
        }
        let stamped_rows = self
            .rank_stamper
            .stamp_unranked_rows(&mut transaction, &rank)
            .await?;
        tracing::info!(target: BACKEND, stamped_rows, "first publish: existing rows ranked");

        let latest_sequence = changes
            .last()
            .map_or(first_sequence - 1, |change| change.sequence);
        if let (Some(first), Some(last)) = (changes.first(), changes.last()) {
            let segment = Segment {
                device_id: device.device_id.clone(),
                first_sequence: first.sequence,
                last_sequence: last.sequence,
                data_format_version: DATA_FORMAT_VERSION,
                changes,
            };
            self.folder_store
                .write_segment(
                    &device.device_id,
                    segment.first_sequence,
                    segment.last_sequence,
                    encode_segment(key, &segment)?,
                )
                .await?;
        }
        let manifest = Manifest {
            device_id: device.device_id.clone(),
            device_name: device.device_name.clone(),
            data_format_version: DATA_FORMAT_VERSION,
            app_version: Some(APP_VERSION.to_string()),
            latest_sequence,
        };
        self.folder_store
            .write_manifest(&device.device_id, encode_manifest(key, &manifest)?)
            .await?;
        transaction
            .commit()
            .await
            .map_err(|error| SyncError::database("enroll: commit failed", error))
    }

    /// Removes what this device wrote into the folder before the failure: its area and the
    /// header it created (SYN-013).
    async fn roll_back_folder(&self, device_id: &str) {
        if let Err(error) = self.folder_store.remove_device_area(device_id).await {
            tracing::warn!(target: BACKEND, err = %error, "roll_back_folder: device area not removed");
        }
        if let Err(error) = self.folder_store.remove_header().await {
            tracing::warn!(target: BACKEND, err = %error, "roll_back_folder: header not removed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::{
        AccountService, SqliteAccountRepository, SqliteHoldingRepository,
        SqliteTransactionRepository, UpdateFrequency,
    };
    use crate::context::asset::{
        AssetClass, AssetService, CreateAssetDTO, SqliteAssetCategoryRepository,
        SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
    };
    use crate::context::sync::domain::{MockFolderStore, MockPortfolioSnapshot, MockRankStamper};
    use crate::context::sync::infrastructure::device::SqliteSyncStateRepository;
    use crate::context::sync::infrastructure::SqliteChangeLogRepository;
    use crate::shared::domain::{RecordIdentity, RecordKind};
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::{Pool, Sqlite, SqliteConnection};

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

    /// A stamper that ranks the `accounts` table through the enrolment connection — enough to
    /// prove the stamps ride the same transaction as the device row (SYN-013).
    struct AccountTableStamper;

    #[async_trait::async_trait]
    impl RankStamper for AccountTableStamper {
        async fn stamp_unranked_rows(
            &self,
            conn: &mut SqliteConnection,
            rank: &Rank,
        ) -> Result<u64, SyncError> {
            sqlx::query(
                "UPDATE accounts SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
                 WHERE sync_logical_timestamp IS NULL",
            )
            .bind(rank.logical_timestamp.as_str())
            .bind(rank.origin.to_string())
            .bind(&rank.device_id)
            .execute(conn)
            .await
            .map(|done| done.rows_affected())
            .map_err(|error| SyncError::database("test stamper: update failed", error))
        }
    }

    fn first_publish_over(
        pool: &Pool<Sqlite>,
        folder_store: MockFolderStore,
        rank_stamper: Arc<dyn RankStamper>,
        snapshot: Arc<dyn PortfolioSnapshot>,
    ) -> FirstPublish {
        FirstPublish::new(
            Arc::new(SqliteChangeLogRepository::new(pool.clone())),
            Arc::new(SqliteSyncStateRepository::new(pool.clone())),
            Arc::new(folder_store),
            rank_stamper,
            snapshot,
        )
    }

    /// Seeds one account, one asset, and one transaction through the real BC services — the
    /// portfolio the first publish must fully replay into the first segment (SYN-013).
    async fn seed_small_portfolio(pool: &Pool<Sqlite>) {
        let account_service = AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        );
        let asset_service = AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        );
        asset_service
            .create_asset(CreateAssetDTO {
                name: "AAPL".into(),
                reference: "AAPL".into(),
                isin: None,
                class: AssetClass::Stocks,
                currency: "USD".into(),
                risk_level: 2,
                category_id: SYSTEM_CATEGORY_ID.into(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .unwrap();
        asset_service.seed_cash_asset("USD").await.unwrap();
        let account = account_service
            .create(
                "Portfolio".into(),
                String::new(),
                "USD".into(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        account_service
            .seed_cash_holding(&account.id)
            .await
            .unwrap();
        account_service
            .record_deposit(&account.id, "2026-01-01".into(), 1_000_000_000, None)
            .await
            .unwrap();
    }

    /// A snapshot of one account record — what the first segment must carry.
    fn one_account_snapshot() -> Arc<MockPortfolioSnapshot> {
        let mut snapshot = MockPortfolioSnapshot::new();
        snapshot.expect_records().returning(|| {
            Ok(vec![PortfolioRecord {
                record_kind: RecordKind::Account,
                record_identity: RecordIdentity::canonical(RecordKind::Account, &["account-1"]),
                content: "{\"id\":\"account-1\"}".into(),
            }])
        });
        Arc::new(snapshot)
    }

    // SYN-013/026 — the first publish writes the header, one segment carrying one Created
    // change per existing record, and the manifest; a sync_device row is created with a
    // derived key.
    #[tokio::test]
    async fn first_publish_writes_header_segment_manifest_and_sync_device_row() {
        let pool = make_pool().await;
        seed_small_portfolio(&pool).await;
        let mut folder_store = MockFolderStore::new();
        folder_store.expect_retarget().return_const(());
        folder_store
            .expect_write_header_if_absent()
            .times(1)
            .returning(|_| Ok(WriteHeaderOutcome::Written));
        folder_store
            .expect_write_segment()
            .withf(|_, first, last, _| *first == 1 && *last == 1)
            .times(1)
            .returning(|_, _, _, _| Ok(()));
        folder_store
            .expect_write_manifest()
            .times(1)
            .returning(|_, _| Ok(()));
        let first_publish = first_publish_over(
            &pool,
            folder_store,
            Arc::new(AccountTableStamper),
            one_account_snapshot(),
        );

        let status = first_publish
            .enable_as_first_device(
                "/tmp/sync".into(),
                "correct horse battery staple".into(),
                "Desktop".into(),
            )
            .await
            .expect("first publish on an empty folder must succeed");

        assert!(status.enabled);
        assert!(!status.paused);

        let device_row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sync_device")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            device_row_count, 1,
            "SYN-052: a sync_device row must exist afterward"
        );

        let key_length: i64 = sqlx::query_scalar("SELECT LENGTH(derived_key) FROM sync_device")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            key_length, 32,
            "SYN-052: the derived key is kept on the device"
        );

        let stamped_accounts: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM accounts WHERE sync_logical_timestamp IS NOT NULL",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            stamped_accounts, 1,
            "CFR-014/D6: every existing account row must be stamped with a rank"
        );

        let published_changes: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM changes WHERE published = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            published_changes, 1,
            "SYN-013: one Created change per record, published"
        );
    }

    // SYN-081 — a header appearing between the pre-check and the write rejects with
    // PortfolioCreatedElsewhere and nothing is written.
    #[tokio::test]
    async fn first_publish_rejects_when_header_appears_between_precheck_and_write() {
        let pool = make_pool().await;
        seed_small_portfolio(&pool).await;
        let mut folder_store = MockFolderStore::new();
        folder_store.expect_retarget().return_const(());
        folder_store
            .expect_write_header_if_absent()
            .returning(|_| Ok(WriteHeaderOutcome::AlreadyExists));
        let first_publish = first_publish_over(
            &pool,
            folder_store,
            Arc::new(MockRankStamper::new()),
            one_account_snapshot(),
        );

        let result = first_publish
            .enable_as_first_device(
                "/tmp/sync".into(),
                "correct horse battery staple".into(),
                "Desktop".into(),
            )
            .await;
        assert!(matches!(result, Err(SyncError::PortfolioCreatedElsewhere)));

        let device_row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sync_device")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            device_row_count, 0,
            "SYN-081: nothing must be written when pre-empted"
        );
    }

    // SYN-013 — a publish failure after the header was written rolls everything back: no
    // sync_device row, no rank stamps, and the header and area this device wrote are removed.
    #[tokio::test]
    async fn first_publish_rolls_back_everything_on_publish_failure() {
        let pool = make_pool().await;
        seed_small_portfolio(&pool).await;
        let mut folder_store = MockFolderStore::new();
        folder_store.expect_retarget().return_const(());
        folder_store
            .expect_write_header_if_absent()
            .returning(|_| Ok(WriteHeaderOutcome::Written));
        folder_store.expect_write_segment().returning(|_, _, _, _| {
            Err(SyncError::PublishFailed {
                problem: FolderProblem::OutOfSpace,
            })
        });
        folder_store
            .expect_remove_device_area()
            .times(1)
            .returning(|_| Ok(()));
        folder_store
            .expect_remove_header()
            .times(1)
            .returning(|| Ok(()));
        let first_publish = first_publish_over(
            &pool,
            folder_store,
            Arc::new(AccountTableStamper),
            one_account_snapshot(),
        );

        let result = first_publish
            .enable_as_first_device(
                "/tmp/sync".into(),
                "correct horse battery staple".into(),
                "Desktop".into(),
            )
            .await;
        assert!(matches!(result, Err(SyncError::PublishFailed { .. })));

        let device_row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sync_device")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            device_row_count, 0,
            "SYN-013: a failed publish leaves no sync_device row"
        );

        let stamped_accounts: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM accounts WHERE sync_logical_timestamp IS NOT NULL",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            stamped_accounts, 0,
            "SYN-013: a failed publish leaves no rank stamps"
        );
    }

    // SYN-012 — a short passphrase is rejected before anything is read or written.
    #[tokio::test]
    async fn first_publish_rejects_a_short_passphrase_before_touching_anything() {
        let pool = make_pool().await;
        let first_publish = first_publish_over(
            &pool,
            MockFolderStore::new(),
            Arc::new(MockRankStamper::new()),
            Arc::new(MockPortfolioSnapshot::new()),
        );
        let result = first_publish
            .enable_as_first_device("/tmp/sync".into(), "short".into(), "Desktop".into())
            .await;
        assert!(matches!(
            result,
            Err(SyncError::PassphraseTooShort { minimum: 12 })
        ));
    }

    // SYN-018 — a blank device name is rejected before anything is read or written.
    #[tokio::test]
    async fn first_publish_rejects_a_blank_device_name_before_touching_anything() {
        let pool = make_pool().await;
        let first_publish = first_publish_over(
            &pool,
            MockFolderStore::new(),
            Arc::new(MockRankStamper::new()),
            Arc::new(MockPortfolioSnapshot::new()),
        );
        let result = first_publish
            .enable_as_first_device(
                "/tmp/sync".into(),
                "correct horse battery staple".into(),
                "  ".into(),
            )
            .await;
        assert!(matches!(result, Err(SyncError::DeviceNameBlank)));
    }
}
