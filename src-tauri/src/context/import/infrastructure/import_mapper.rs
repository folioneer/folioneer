use crate::context::import::domain::{
    names_a_file, ImportMapperStore, MapperChoiceRepository, PickedMapperFile, LARGEST_MAPPER_FILE,
};
use crate::core::BACKEND;
use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::{Pool, Sqlite};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;

/// The mappers folder of the data folder (IMP-020).
pub struct FsImportMapperStore {
    folder: PathBuf,
}

impl FsImportMapperStore {
    /// The store over `folder`, created at the first write.
    pub fn new(folder: PathBuf) -> Self {
        Self { folder }
    }

    /// The file `id` of the folder; `None` when `id` would name anything else.
    fn file(&self, id: &str) -> Option<PathBuf> {
        names_a_file(id).then(|| self.folder.join(format!("{id}.json")))
    }
}

#[async_trait]
impl ImportMapperStore for FsImportMapperStore {
    async fn ids(&self) -> Result<Vec<String>> {
        let mut entries = match tokio::fs::read_dir(&self.folder).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e).context("mappers folder not listed"),
        };
        let mut ids = Vec::new();
        while let Some(entry) = entries.next_entry().await.context("mappers folder entry")? {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                if let Some(id) = path.file_stem().and_then(|stem| stem.to_str()) {
                    if self.file(id).is_some() {
                        ids.push(id.to_string());
                    } else {
                        tracing::warn!(target: BACKEND, file = %id, "mapper file name left out");
                    }
                }
            }
        }
        Ok(ids)
    }

    async fn read(&self, id: &str) -> Result<Option<Vec<u8>>> {
        let Some(file) = self.file(id) else {
            return Ok(None);
        };
        let opened = match tokio::fs::File::open(&file).await {
            Ok(opened) => opened,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e).context("mapper file not opened"),
        };
        // Read one byte past the limit: enough to tell a file too large (IMP-020A).
        let mut bytes = Vec::new();
        opened
            .take(LARGEST_MAPPER_FILE + 1)
            .read_to_end(&mut bytes)
            .await
            .context("mapper file not read")?;
        Ok(Some(bytes))
    }

    async fn write(&self, id: &str, bytes: &[u8]) -> Result<()> {
        let file = self
            .file(id)
            .with_context(|| "a mapper's id names no file of the folder")?;
        tokio::fs::create_dir_all(&self.folder)
            .await
            .context("mappers folder not created")?;
        // Written aside then moved, so a failure never leaves half a file in the list.
        let aside = self.folder.join(format!(".{id}.tmp"));
        tokio::fs::write(&aside, bytes)
            .await
            .context("mapper file not written")?;
        tokio::fs::rename(&aside, &file)
            .await
            .context("mapper file not moved in place")
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let Some(file) = self.file(id) else {
            return Ok(());
        };
        match tokio::fs::remove_file(&file).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e).context("mapper file not removed"),
        }
    }

    async fn read_picked(&self, path: &Path, largest: u64) -> Result<PickedMapperFile> {
        // Checked before it is opened: opening a pipe would wait for a writer.
        let metadata = tokio::fs::metadata(path)
            .await
            .context("picked mapper file not found")?;
        anyhow::ensure!(metadata.is_file(), "a picked mapper file is a plain file");
        let file = tokio::fs::File::open(path)
            .await
            .context("picked mapper file not opened")?;
        let mut bytes = Vec::new();
        file.take(largest + 1)
            .read_to_end(&mut bytes)
            .await
            .context("picked mapper file not read")?;
        Ok(if bytes.len() as u64 > largest {
            PickedMapperFile::TooLarge
        } else {
            PickedMapperFile::Read(bytes)
        })
    }
}

/// The mapper each folder remembers, in a table no sync reads (IMP-020C).
pub struct SqliteMapperChoiceRepository {
    pool: Pool<Sqlite>,
}

impl SqliteMapperChoiceRepository {
    /// The repository over `pool`.
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MapperChoiceRepository for SqliteMapperChoiceRepository {
    async fn remembered(&self, folder: &str) -> Result<Option<String>> {
        let id = sqlx::query_scalar!(
            "SELECT mapper_id FROM import_mapper_choices WHERE folder = ?",
            folder
        )
        .fetch_optional(&self.pool)
        .await
        .context("mapper choice not read")?;
        Ok(id)
    }

    async fn remember(&self, folder: &str, mapper_id: &str) -> Result<()> {
        sqlx::query!(
            "INSERT INTO import_mapper_choices (folder, mapper_id) VALUES (?, ?)
             ON CONFLICT(folder) DO UPDATE SET mapper_id = excluded.mapper_id",
            folder,
            mapper_id
        )
        .execute(&self.pool)
        .await
        .context("mapper choice not written")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // IMP-020 — an id naming anything but a file of the folder is held by none and not written.
    #[tokio::test]
    async fn imp_020_an_id_never_leaves_the_folder() {
        let dir = tempfile::tempdir().expect("dir");
        let store = FsImportMapperStore::new(dir.path().join("mappers"));
        assert!(store.ids().await.expect("no folder").is_empty());
        for id in ["../escape", "a/b", "a\\b", ".hidden", "", "C:x", "con"] {
            assert!(store.write(id, b"{}").await.is_err(), "{id}");
            assert_eq!(store.read(id).await.expect("read"), None, "{id}");
        }
        assert!(!dir.path().join("escape.json").exists());
    }

    // IMP-020 — a file written is listed and read; only `.json` files count; deleted, gone.
    #[tokio::test]
    async fn imp_020_the_folder_holds_the_files() {
        let dir = tempfile::tempdir().expect("dir");
        let store = FsImportMapperStore::new(dir.path().to_path_buf());
        store.write("broker", b"{}").await.expect("written");
        std::fs::write(dir.path().join("notes.txt"), "x").expect("other file");
        assert_eq!(
            store.ids().await.expect("listed"),
            vec!["broker".to_string()]
        );
        assert_eq!(
            store.read("broker").await.expect("read"),
            Some(b"{}".to_vec())
        );
        store.delete("broker").await.expect("deleted");
        store.delete("broker").await.expect("deleted twice");
        assert_eq!(store.read("broker").await.expect("read"), None);
    }

    // IMP-020B — a picked file is a plain file, read no further than allowed.
    #[tokio::test]
    async fn imp_020b_a_picked_file_is_a_plain_file_read_within_its_limit() {
        let dir = tempfile::tempdir().expect("dir");
        let store = FsImportMapperStore::new(dir.path().join("mappers"));
        let file = dir.path().join("picked.json");
        std::fs::write(&file, "12345").expect("written");
        assert_eq!(
            store.read_picked(&file, 5).await.expect("read"),
            PickedMapperFile::Read(b"12345".to_vec())
        );
        assert_eq!(
            store.read_picked(&file, 4).await.expect("read"),
            PickedMapperFile::TooLarge
        );
        assert!(store.read_picked(dir.path(), 5).await.is_err());
    }
}
