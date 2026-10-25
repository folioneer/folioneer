/// IMP-020 — the column mapping aggregate and its repository port.
pub mod column_mapping;
/// IMP-010 to IMP-016 — reading a file: its encoding, separator, decimal mark, dates, columns.
pub mod file;
/// IMP-030 to IMP-034 — a file's rows read through a mapping, before they are judged.
pub mod row;

pub use column_mapping::{
    AssetMatch, ColumnMapping, ColumnMappingDraft, ColumnMappingRepository, DateFormat,
    DecimalMark, Encoding, MappedColumns, OperationLabel, OperationType, Separator,
    MAPPING_TEXT_LENGTH,
};
pub use file::{read_date, read_file, DateReading, FileLine, FileRead, ImportFile, ReadingChoices};
pub use row::{
    read_figure, read_rows, AssetRead, OperationRead, Row, RowFigures, RowFormat, RowRead,
    RowRejection,
};

#[cfg(test)]
pub use column_mapping::MockColumnMappingRepository;
