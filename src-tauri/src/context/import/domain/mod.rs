/// IMP-010 to IMP-016 — reading a file: its encoding, separator, decimal mark, dates, columns.
pub mod file;
/// IMP-020 — the import mapper aggregate and its ports.
pub mod import_mapper;
/// IMP-030 to IMP-034 — a file's rows read through a mapper, before they are judged.
pub mod row;

pub use file::LARGEST_FILE;
pub use file::{read_date, read_file, DateReading, FileLine, FileRead, ImportFile, ReadingChoices};
pub use import_mapper::{
    folded, names_a_file, new_mapper_id, AssetMatch, DateFormat, DecimalMark, Encoding,
    ImportMapper, ImportMapperDraft, ImportMapperStore, MappedColumns, MapperChoiceRepository,
    MapperInfo, OperationLabel, OperationType, PickedMapperFile, Separator, LARGEST_MAPPER_FILE,
    MAPPER_TEXT_LENGTH,
};
pub use row::{
    read_figure, read_rows, AssetRead, OperationRead, Row, RowFigures, RowFormat, RowRead,
    RowRejection,
};

#[cfg(test)]
pub use import_mapper::{MockImportMapperStore, MockMapperChoiceRepository};
