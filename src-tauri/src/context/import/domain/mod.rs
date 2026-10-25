/// IMP-020 — the column mapping aggregate and its repository port.
pub mod column_mapping;

pub use column_mapping::{
    AssetMatch, ColumnMapping, ColumnMappingDraft, ColumnMappingRepository, DateFormat,
    DecimalMark, Encoding, MappedColumns, OperationLabel, OperationType, Separator,
    MAPPING_TEXT_LENGTH,
};

#[cfg(test)]
pub use column_mapping::MockColumnMappingRepository;
