use crate::context::account_import::domain::{
    ImportMapper, ImportMapperDraft, ImportMapperStore, MapperChoiceRepository, PickedMapperFile,
    LARGEST_MAPPER_FILE,
};
use crate::context::account_import::error::AccountImportError;
use crate::core::BACKEND;
use std::path::Path;
use std::result::Result as StdResult;

/// The application service of the `account_import` bounded context: the import mappers, files of the
/// mappers folder (IMP-020 to IMP-027, IMP-040).
pub struct AccountImportService {
    store: Box<dyn ImportMapperStore>,
    choices: Box<dyn MapperChoiceRepository>,
    /// Held across a check of the folder and the write it allows (IMP-020, IMP-024).
    writing: tokio::sync::Mutex<()>,
}

impl AccountImportService {
    /// Creates the service over the mappers folder and what each folder remembers.
    pub fn new(
        store: Box<dyn ImportMapperStore>,
        choices: Box<dyn MapperChoiceRepository>,
    ) -> Self {
        Self {
            store,
            choices,
            writing: tokio::sync::Mutex::new(()),
        }
    }

    /// IMP-020A — the mappers the folder holds, by provider; a file that does not read as a
    /// mapper is left out, its failure logged.
    pub async fn list_import_mappers(&self) -> StdResult<Vec<ImportMapper>, AccountImportError> {
        let ids = self.store.ids().await.map_err(store_error("list"))?;
        let mut mappers = Vec::with_capacity(ids.len());
        for id in ids {
            let bytes = match self.store.read(&id).await {
                Ok(Some(bytes)) => bytes,
                Ok(None) => {
                    tracing::warn!(target: BACKEND, mapper = %id, "mapper file left out: not found");
                    continue;
                }
                Err(e) => {
                    tracing::warn!(target: BACKEND, mapper = %id, err = ?e, "mapper file left out");
                    continue;
                }
            };
            match read_mapper(&id, &bytes) {
                Ok(mapper) => mappers.push(mapper),
                Err(e) => {
                    tracing::warn!(target: BACKEND, mapper = %id, err = %e, "mapper file left out")
                }
            }
        }
        mappers.sort_by_cached_key(|mapper| {
            (
                crate::context::account_import::folded(&mapper.info.provider),
                mapper.id.clone(),
            )
        });
        Ok(mappers)
    }

    /// IMP-020 / IMP-025 — writes `draft` as a new mapper, or, given `mapper_id`, as the
    /// mapper it names, keeping its file. The provider is checked only when it is being set:
    /// a mapper sharing its provider keeps it (IMP-027).
    pub async fn save_import_mapper(
        &self,
        mapper_id: Option<String>,
        draft: ImportMapperDraft,
    ) -> StdResult<ImportMapper, AccountImportError> {
        let _writing = self.writing.lock().await;
        let held = match &mapper_id {
            Some(id) => Some(self.get_import_mapper(id).await?),
            None => None,
        };
        let id = mapper_id.unwrap_or_default();
        let checked = ImportMapper::with_id(id, draft)?;
        let provider_kept = held.is_some_and(|held| held.has_provider(&checked.info.provider));
        if !provider_kept {
            self.check_provider_free(&checked).await?;
        }
        self.write_mapper(checked).await
    }

    /// IMP-020B — the mapper's file at `path` copied into the folder; refused with why, nothing
    /// written.
    pub async fn add_import_mapper_from(
        &self,
        path: &Path,
    ) -> StdResult<ImportMapper, AccountImportError> {
        let picked = self
            .store
            .read_picked(path, LARGEST_MAPPER_FILE)
            .await
            .map_err(|e| {
                tracing::warn!(target: BACKEND, err = ?e, "mapper file cannot be read");
                AccountImportError::MapperFileUnreadable
            })?;
        let PickedMapperFile::Read(bytes) = picked else {
            return Err(AccountImportError::MapperFileTooLarge);
        };
        let _writing = self.writing.lock().await;
        let checked = ImportMapper::with_id(String::new(), ImportMapperDraft::from_file(&bytes)?)?;
        self.check_provider_free(&checked).await?;
        self.write_mapper(checked).await
    }

    /// IMP-020 — the mapper `mapper_id`, refused when it is no longer held or no longer reads.
    pub async fn get_import_mapper(
        &self,
        mapper_id: &str,
    ) -> StdResult<ImportMapper, AccountImportError> {
        let bytes = self
            .store
            .read(mapper_id)
            .await
            .map_err(store_error("read"))?
            .ok_or_else(|| AccountImportError::MapperNotFound {
                mapper_id: mapper_id.to_string(),
            })?;
        read_mapper(mapper_id, &bytes)
    }

    /// IMP-040 — removes the file of the mapper `mapper_id`.
    pub async fn delete_import_mapper(&self, mapper_id: &str) -> StdResult<(), AccountImportError> {
        let _writing = self.writing.lock().await;
        let held = self
            .store
            .read(mapper_id)
            .await
            .map_err(store_error("read"))?;
        if held.is_none() {
            return Err(AccountImportError::MapperNotFound {
                mapper_id: mapper_id.to_string(),
            });
        }
        self.store
            .delete(mapper_id)
            .await
            .map_err(store_error("delete"))
    }

    /// IMP-020D — the mapper `folder` remembers, when it is still held and reads.
    pub async fn remembered_import_mapper(&self, folder: &str) -> Option<String> {
        let id = match self.choices.remembered(folder).await {
            Ok(id) => id?,
            Err(e) => {
                tracing::warn!(target: BACKEND, err = ?e, "mapper choice not read");
                return None;
            }
        };
        match self.get_import_mapper(&id).await {
            Ok(_) => Some(id),
            Err(e) => {
                tracing::warn!(target: BACKEND, mapper = %id, err = %e, "remembered mapper not preselected");
                None
            }
        }
    }

    /// IMP-020C — `folder` now remembers `mapper_id`. A failure is logged and goes no further:
    /// the import it follows stands.
    pub async fn remember_import_mapper(&self, folder: &str, mapper_id: &str) {
        if let Err(e) = self.choices.remember(folder, mapper_id).await {
            tracing::warn!(target: BACKEND, err = ?e, "mapper choice not remembered");
        }
    }

    /// IMP-024 — refused when another mapper holds its provider.
    async fn check_provider_free(
        &self,
        mapper: &ImportMapper,
    ) -> StdResult<(), AccountImportError> {
        let others = self.list_import_mappers().await?;
        if others
            .iter()
            .any(|other| other.id != mapper.id && other.has_provider(&mapper.info.provider))
        {
            return Err(AccountImportError::MapperProviderTaken {
                provider: mapper.info.provider.clone(),
            });
        }
        Ok(())
    }

    /// IMP-020 — writes the mapper's file; one with no id yet takes its file name here.
    async fn write_mapper(
        &self,
        mapper: ImportMapper,
    ) -> StdResult<ImportMapper, AccountImportError> {
        let held = self.store.ids().await.map_err(store_error("list"))?;
        let mapper = mapper.named_among(&held);
        let bytes = mapper.to_file().map_err(store_error("serialize"))?;
        self.store
            .write(&mapper.id, &bytes)
            .await
            .map_err(store_error("write"))?;
        Ok(mapper)
    }
}

/// IMP-020A — the file `id` read as a mapper, and valid.
fn read_mapper(id: &str, bytes: &[u8]) -> StdResult<ImportMapper, AccountImportError> {
    if bytes.len() as u64 > LARGEST_MAPPER_FILE {
        return Err(AccountImportError::MapperFileTooLarge);
    }
    ImportMapper::with_id(id.to_string(), ImportMapperDraft::from_file(bytes)?)
}

/// A failure of the mappers folder: its cause logged, never sent.
fn store_error(step: &'static str) -> impl Fn(anyhow::Error) -> AccountImportError {
    move |e| {
        tracing::error!(target: BACKEND, step = %step, err = ?e, "mappers folder failure");
        AccountImportError::MapperFolderFailed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account_import::domain::{
        DateFormat, DecimalMark, Encoding, MappedColumns, MapperInfo, MockImportMapperStore,
        MockMapperChoiceRepository, Separator,
    };

    fn draft() -> ImportMapperDraft {
        ImportMapperDraft {
            info: MapperInfo {
                provider: "Broker".to_string(),
            },
            separator: Separator::Comma,
            decimal_mark: DecimalMark::Dot,
            date_format: DateFormat::YearMonthDay,
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

    // IMP-020 — a write that fails is rejected with a specific error.
    #[tokio::test]
    async fn imp_020_a_write_that_fails_is_rejected() {
        let mut store = MockImportMapperStore::new();
        store.expect_ids().returning(|| Ok(Vec::new()));
        store
            .expect_write()
            .returning(|_, _| Err(anyhow::anyhow!("disk full")));
        let service =
            AccountImportService::new(Box::new(store), Box::new(MockMapperChoiceRepository::new()));
        assert!(matches!(
            service.save_import_mapper(None, draft()).await,
            Err(AccountImportError::MapperFolderFailed)
        ));
    }

    // IMP-020A — a file that cannot be read is left out; the others stay listed.
    #[tokio::test]
    async fn imp_020a_a_file_that_cannot_be_read_is_left_out() {
        let file = draft();
        let held = ImportMapper::with_id("held".to_string(), file)
            .expect("valid")
            .to_file()
            .expect("written");
        let mut store = MockImportMapperStore::new();
        store
            .expect_ids()
            .returning(|| Ok(vec!["locked".to_string(), "held".to_string()]));
        store.expect_read().returning(move |id| {
            if id == "locked" {
                Err(anyhow::anyhow!("permission denied"))
            } else {
                Ok(Some(held.clone()))
            }
        });
        let service =
            AccountImportService::new(Box::new(store), Box::new(MockMapperChoiceRepository::new()));
        let listed = service.list_import_mappers().await.expect("listed");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "held");
    }

    // IMP-020C — a failure to remember goes no further.
    #[tokio::test]
    async fn imp_020c_a_failure_to_remember_goes_no_further() {
        let mut choices = MockMapperChoiceRepository::new();
        choices
            .expect_remember()
            .times(1)
            .returning(|_, _| Err(anyhow::anyhow!("database locked")));
        let service =
            AccountImportService::new(Box::new(MockImportMapperStore::new()), Box::new(choices));
        service.remember_import_mapper("/folder", "broker").await;
    }
}
