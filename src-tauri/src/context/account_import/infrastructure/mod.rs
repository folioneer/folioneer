/// The built-in presets, compiled into the program (IMP-020F).
pub mod built_in;
/// The mappers folder and the mapper each folder remembers.
pub mod import_mapper;

pub use built_in::built_in_presets;
pub use import_mapper::{FsImportMapperStore, SqliteMapperChoiceRepository};
