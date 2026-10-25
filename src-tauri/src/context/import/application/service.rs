use crate::context::import::domain::{ColumnMapping, ColumnMappingDraft, ColumnMappingRepository};
use crate::context::import::error::ImportError;
use crate::core::{Event, SideEffectEventBus, BACKEND};
use crate::shared::domain::{Rank, SyncedRecord};
use sqlx::SqliteConnection;
use std::result::Result as StdResult;
use std::sync::Arc;

/// The application service of the `import` bounded context: the saved column mappings
/// (IMP-020 to IMP-026, IMP-040) and their sync entry points (CFR-014, CFR-017).
pub struct ImportService {
    mapping_repo: Box<dyn ColumnMappingRepository>,
    event_bus: Option<Arc<SideEffectEventBus>>,
}

impl ImportService {
    /// Creates the service over its mapping repository.
    pub fn new(mapping_repo: Box<dyn ColumnMappingRepository>) -> Self {
        Self {
            mapping_repo,
            event_bus: None,
        }
    }

    /// Attaches the bus `ColumnMappingUpdated` is published on (IMP-026).
    pub fn with_event_bus(mut self, bus: Arc<SideEffectEventBus>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    /// IMP-020 — every saved mapping, by name.
    pub async fn list_column_mappings(&self) -> StdResult<Vec<ColumnMapping>, ImportError> {
        self.mapping_repo
            .list()
            .await
            .map_err(|e| repository_error("list_column_mappings", e))
    }

    /// IMP-020 / IMP-025 — saves `draft` as a new mapping, or, given `mapping_id`, as the
    /// mapping it names, keeping its id. The name is checked only when it is being set: a
    /// mapping a merge left sharing its name keeps it (IMP-027).
    pub async fn save_column_mapping(
        &self,
        mapping_id: Option<String>,
        draft: ColumnMappingDraft,
    ) -> StdResult<ColumnMapping, ImportError> {
        let (mapping, held) = match mapping_id {
            None => (ColumnMapping::new(draft)?, None),
            Some(mapping_id) => {
                let held = self.held(&mapping_id).await?;
                (ColumnMapping::with_id(mapping_id, draft)?, Some(held))
            }
        };
        let name_kept = held.is_some_and(|held| held.is_named(&mapping.name));
        let others = self.list_column_mappings().await?;
        if !name_kept
            && others
                .iter()
                .any(|other| other.id != mapping.id && other.is_named(&mapping.name))
        {
            return Err(ImportError::MappingNameTaken { name: mapping.name });
        }
        self.mapping_repo
            .save(&mapping)
            .await
            .map_err(|e| repository_error("save_column_mapping", e))?;
        self.notify_updated();
        Ok(mapping)
    }

    /// IMP-040 — deletes the mapping `mapping_id`.
    pub async fn delete_column_mapping(&self, mapping_id: &str) -> StdResult<(), ImportError> {
        self.held(mapping_id).await?;
        self.mapping_repo
            .delete(mapping_id)
            .await
            .map_err(|e| repository_error("delete_column_mapping", e))?;
        self.notify_updated();
        Ok(())
    }

    async fn held(&self, mapping_id: &str) -> StdResult<ColumnMapping, ImportError> {
        let held = self
            .mapping_repo
            .get_by_id(mapping_id)
            .await
            .map_err(|e| repository_error("column mapping read", e))?;
        match held {
            Some(held) => Ok(held),
            None => Err(ImportError::MappingNotFound {
                mapping_id: mapping_id.to_string(),
            }),
        }
    }

    fn notify_updated(&self) {
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::ColumnMappingUpdated);
        }
    }

    // -------------------------------------------------------------------------
    // Sync entry points (CFR-014, CFR-017) — merge executor writes; no entry guards run
    // -------------------------------------------------------------------------

    /// Stamps `rank` on every mapping never ranked (CFR-014, D6), on the first publish's
    /// enrolment transaction (SYN-013); how many were stamped.
    pub async fn stamp_sync_rank(
        &self,
        conn: &mut SqliteConnection,
        rank: &Rank,
    ) -> StdResult<u64, ImportError> {
        self.mapping_repo
            .stamp_sync_rank(conn, rank)
            .await
            .map_err(|e| repository_error("stamp_sync_rank", e))
    }

    /// The synced mapping this device holds for `identity` (CFR-014), on the apply
    /// transaction's connection; `None` when it holds none.
    pub async fn synced_record(
        &self,
        conn: &mut SqliteConnection,
        identity: &str,
    ) -> StdResult<Option<SyncedRecord>, ImportError> {
        self.mapping_repo
            .synced_record(conn, identity)
            .await
            .map_err(|e| repository_error("synced_record", e))
    }

    /// Applies an incoming mapping verbatim (CFR-017): the later change wins, and a name
    /// another mapping holds is kept (IMP-023).
    pub async fn apply_column_mapping(
        &self,
        conn: &mut SqliteConnection,
        content: &str,
        rank: Rank,
    ) -> StdResult<(), ImportError> {
        let mapping: ColumnMapping = serde_json::from_str(content).map_err(|e| {
            tracing::error!(target: BACKEND, err = %e, "synced content: malformed payload");
            ImportError::DatabaseError
        })?;
        self.mapping_repo
            .apply(conn, &mapping, &rank)
            .await
            .map_err(|e| repository_error("apply_column_mapping", e))
    }

    /// Applies an incoming removal of the mapping `identity` (CFR-017).
    pub async fn apply_removal(
        &self,
        conn: &mut SqliteConnection,
        identity: &str,
    ) -> StdResult<(), ImportError> {
        self.mapping_repo
            .remove_synced(conn, identity)
            .await
            .map_err(|e| repository_error("apply_removal", e))
    }
}

/// Translates a repository failure into `DatabaseError` after logging it.
fn repository_error(context: &'static str, e: anyhow::Error) -> ImportError {
    tracing::error!(target: BACKEND, err = ?e, "{context}: repository failure");
    ImportError::DatabaseError
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::import::domain::{
        DateFormat, DecimalMark, Encoding, MappedColumns, MockColumnMappingRepository, Separator,
    };

    fn draft(name: &str) -> ColumnMappingDraft {
        ColumnMappingDraft {
            name: name.to_string(),
            separator: Separator::Semicolon,
            decimal_mark: DecimalMark::Dot,
            date_format: DateFormat::DayMonthYear,
            encoding: Encoding::Utf8,
            columns: MappedColumns {
                date: "Date".to_string(),
                operation: "Operation".to_string(),
                isin: Some("ISIN".to_string()),
                reference: None,
                name: None,
                quantity: None,
                unit_price: None,
                gross_amount: None,
                fees: None,
                net_amount: None,
                currency: None,
            },
            operation_types: Vec::new(),
            asset_matches: Vec::new(),
        }
    }

    fn held(id: &str, name: &str) -> ColumnMapping {
        ColumnMapping::with_id(id.to_string(), draft(name)).expect("valid")
    }

    // IMP-024 — a name another mapping holds, case and accents ignored, is refused and
    // nothing is saved.
    #[tokio::test]
    async fn imp_024_a_name_another_mapping_holds_is_refused() {
        let mut repo = MockColumnMappingRepository::new();
        repo.expect_list()
            .returning(|| Ok(vec![held("m-1", "Société")]));
        repo.expect_save().never();
        let service = ImportService::new(Box::new(repo));

        let refused = service.save_column_mapping(None, draft(" SOCIETE ")).await;

        assert!(matches!(
            refused,
            Err(ImportError::MappingNameTaken { name }) if name == "SOCIETE"
        ));
    }

    // IMP-025 / IMP-026 — a saved mapping renamed keeps its id, may keep its own name, and
    // tells the change.
    #[tokio::test]
    async fn imp_025_a_saved_mapping_is_replaced_under_its_id() {
        let mut repo = MockColumnMappingRepository::new();
        repo.expect_get_by_id()
            .returning(|_| Ok(Some(held("m-1", "Broker"))));
        repo.expect_list()
            .returning(|| Ok(vec![held("m-1", "Broker")]));
        repo.expect_save()
            .withf(|mapping| mapping.id == "m-1" && mapping.name == "broker")
            .times(1)
            .returning(|_| Ok(()));
        let bus = Arc::new(SideEffectEventBus::new());
        let mut events = bus.subscribe();
        let service = ImportService::new(Box::new(repo)).with_event_bus(Arc::clone(&bus));

        let saved = service
            .save_column_mapping(Some("m-1".to_string()), draft("broker"))
            .await
            .expect("saved");

        assert_eq!(saved.id, "m-1");
        let event = events
            .drain()
            .pop()
            .expect("ColumnMappingUpdated published");
        assert_eq!(event, Event::ColumnMappingUpdated);
    }

    // IMP-027 — a mapping a merge left sharing its name is saved again under it; renaming it
    // into a name another holds is still refused.
    #[tokio::test]
    async fn imp_027_a_shared_name_is_kept_not_taken() {
        let mut repo = MockColumnMappingRepository::new();
        repo.expect_get_by_id()
            .returning(|id| Ok(Some(held(id, if id == "m-1" { "PEA" } else { "CTO" }))));
        repo.expect_list().returning(|| {
            Ok(vec![
                held("m-1", "PEA"),
                held("m-2", "PEA"),
                held("m-3", "CTO"),
            ])
        });
        repo.expect_save().times(1).returning(|_| Ok(()));
        let service = ImportService::new(Box::new(repo));

        assert!(service
            .save_column_mapping(Some("m-1".to_string()), draft("pea"))
            .await
            .is_ok());
        assert!(matches!(
            service
                .save_column_mapping(Some("m-3".to_string()), draft("PEA"))
                .await,
            Err(ImportError::MappingNameTaken { .. })
        ));
    }

    // IMP-025 / IMP-040 — saving or deleting a mapping no longer held is refused.
    #[tokio::test]
    async fn imp_040_a_mapping_no_longer_held_is_refused() {
        let mut repo = MockColumnMappingRepository::new();
        repo.expect_get_by_id().returning(|_| Ok(None));
        repo.expect_save().never();
        repo.expect_delete().never();
        let service = ImportService::new(Box::new(repo));

        assert!(matches!(
            service
                .save_column_mapping(Some("gone".to_string()), draft("A"))
                .await,
            Err(ImportError::MappingNotFound { mapping_id }) if mapping_id == "gone"
        ));
        assert!(matches!(
            service.delete_column_mapping("gone").await,
            Err(ImportError::MappingNotFound { .. })
        ));
    }
}
