use crate::context::import::error::ImportError;
use crate::shared::domain::{Rank, SyncedRecord};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqliteConnection;
use uuid::Uuid;

/// IMP-024A — the longest text a mapping holds, in characters: the longest a device takes in
/// from another (the holding note's cap, HNO-011), so a saved mapping is never refused there.
pub const MAPPING_TEXT_LENGTH: usize = 500;

/// The character between two fields of a file (IMP-012).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum Separator {
    /// `;`
    Semicolon,
    /// `,`
    Comma,
    /// A tab.
    Tab,
}

/// The mark before a figure's decimals (IMP-013).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum DecimalMark {
    /// `.`
    Dot,
    /// `,`
    Comma,
}

/// The order a date is written in (IMP-014).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum DateFormat {
    /// Day, month, year.
    DayMonthYear,
    /// Month, day, year.
    MonthDayYear,
    /// Year, month, day.
    YearMonthDay,
}

/// How a file's text is written (IMP-011).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum Encoding {
    /// UTF-8.
    Utf8,
    /// Windows-1252, which reads an ISO-8859-1 file too.
    Windows1252,
}

/// The operation type an operation label stands for (IMP-022).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum OperationType {
    /// A purchase.
    Purchase,
    /// A sale.
    Sale,
    /// A dividend.
    Dividend,
    /// A deposit.
    Deposit,
    /// A withdrawal.
    Withdrawal,
    /// A trade tax, added to its purchase's fees (IMP-034).
    TradeTax,
    /// A corporate action this version leaves to record by hand.
    NotImported,
}

/// Which column of the file holds each field of a row, by its header (IMP-021). An absent
/// optional column is one the file does not have.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct MappedColumns {
    /// The date's column.
    pub date: String,
    /// The operation label's column.
    pub operation: String,
    /// The asset's ISIN column, when the file has one.
    pub isin: Option<String>,
    /// The asset's reference column, when the file has one.
    pub reference: Option<String>,
    /// The asset's name column, when the file has one.
    pub name: Option<String>,
    /// The quantity's column, when the file has one.
    pub quantity: Option<String>,
    /// The unit price's column, when the file has one.
    pub unit_price: Option<String>,
    /// The gross amount's column, when the file has one.
    pub gross_amount: Option<String>,
    /// The fees' column, when the file has one.
    pub fees: Option<String>,
    /// The net amount's column, when the file has one.
    pub net_amount: Option<String>,
    /// The currency's column, when the file has one.
    pub currency: Option<String>,
}

/// One operation label of the file and the operation type it stands for (IMP-022).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct OperationLabel {
    /// The label as the file writes it.
    pub label: String,
    /// The operation type it stands for.
    pub transaction_type: OperationType,
}

/// A name, ISIN or reference of the file the user matched to an asset (IMP-033).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AssetMatch {
    /// The name, ISIN or reference as the file writes it.
    pub read: String,
    /// The asset it stands for.
    pub asset_id: String,
}

/// IMP-020 — how one broker's export is read, saved under a name and reused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ColumnMapping {
    /// Its own identity: a rename keeps it (IMP-025).
    pub id: String,
    /// What the user calls it, trimmed (IMP-024).
    pub name: String,
    /// The character between two fields.
    pub separator: Separator,
    /// The mark before a figure's decimals.
    pub decimal_mark: DecimalMark,
    /// The order a date is written in.
    pub date_format: DateFormat,
    /// How the file's text is written.
    pub encoding: Encoding,
    /// Which column holds each field of a row.
    pub columns: MappedColumns,
    /// Each operation label and its operation type.
    pub operation_types: Vec<OperationLabel>,
    /// Each name, ISIN or reference matched to an asset.
    pub asset_matches: Vec<AssetMatch>,
}

/// What a mapping holds besides its identity: what the user saves (IMP-020).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ColumnMappingDraft {
    /// What the user calls it.
    pub name: String,
    /// The character between two fields.
    pub separator: Separator,
    /// The mark before a figure's decimals.
    pub decimal_mark: DecimalMark,
    /// The order a date is written in.
    pub date_format: DateFormat,
    /// How the file's text is written.
    pub encoding: Encoding,
    /// Which column holds each field of a row.
    pub columns: MappedColumns,
    /// Each operation label and its operation type.
    pub operation_types: Vec<OperationLabel>,
    /// Each name, ISIN or reference matched to an asset.
    pub asset_matches: Vec<AssetMatch>,
}

impl ColumnMapping {
    /// IMP-020 / IMP-021 / IMP-024 / IMP-024A — a new mapping, its id generated.
    pub fn new(draft: ColumnMappingDraft) -> Result<Self, ImportError> {
        Self::with_id(Uuid::new_v4().to_string(), draft)
    }

    /// IMP-025 — a mapping under the id it already holds, validated as `new` validates.
    pub fn with_id(id: String, draft: ColumnMappingDraft) -> Result<Self, ImportError> {
        let name = draft.name.trim().to_string();
        if name.is_empty() {
            return Err(ImportError::MappingNameEmpty);
        }
        let columns = draft.columns;
        if columns.date.trim().is_empty() || columns.operation.trim().is_empty() {
            return Err(ImportError::MappingColumnMissing);
        }
        if [&columns.isin, &columns.reference, &columns.name]
            .iter()
            .all(|column| {
                column
                    .as_deref()
                    .is_none_or(|header| header.trim().is_empty())
            })
        {
            return Err(ImportError::MappingColumnMissing);
        }
        let texts = [&name, &columns.date, &columns.operation]
            .into_iter()
            .chain(
                [
                    &columns.isin,
                    &columns.reference,
                    &columns.name,
                    &columns.quantity,
                    &columns.unit_price,
                    &columns.gross_amount,
                    &columns.fees,
                    &columns.net_amount,
                    &columns.currency,
                ]
                .into_iter()
                .flatten(),
            )
            .chain(
                draft
                    .operation_types
                    .iter()
                    .map(|operation| &operation.label),
            )
            .chain(
                draft
                    .asset_matches
                    .iter()
                    .flat_map(|matched| [&matched.read, &matched.asset_id]),
            );
        if texts
            .into_iter()
            .any(|text| text.chars().count() > MAPPING_TEXT_LENGTH)
        {
            return Err(ImportError::MappingTextTooLong);
        }
        Ok(Self {
            id,
            name,
            separator: draft.separator,
            decimal_mark: draft.decimal_mark,
            date_format: draft.date_format,
            encoding: draft.encoding,
            columns,
            operation_types: draft.operation_types,
            asset_matches: draft.asset_matches,
        })
    }

    /// Rebuilds a stored mapping under its id, no validation (repository only).
    pub fn restore(id: String, stored: ColumnMappingDraft) -> Self {
        Self {
            id,
            name: stored.name,
            separator: stored.separator,
            decimal_mark: stored.decimal_mark,
            date_format: stored.date_format,
            encoding: stored.encoding,
            columns: stored.columns,
            operation_types: stored.operation_types,
            asset_matches: stored.asset_matches,
        }
    }

    /// IMP-024 — whether this mapping's name is `name`, case and accents ignored.
    pub fn is_named(&self, name: &str) -> bool {
        folded(&self.name) == folded(name)
    }
}

/// A text compared case and accents ignored (IMP-024, IMP-032): trimmed, lower case, every
/// accent of a Latin letter dropped.
pub fn folded(text: &str) -> String {
    use unicode_normalization::char::is_combining_mark;
    use unicode_normalization::UnicodeNormalization;

    text.trim()
        .nfd()
        .filter(|character| !is_combining_mark(*character))
        .flat_map(char::to_lowercase)
        .collect()
}

/// Persistence of the column mappings, with their sync entry points (CFR-014, CFR-017).
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait ColumnMappingRepository: Send + Sync {
    /// Every mapping, by name.
    async fn list(&self) -> Result<Vec<ColumnMapping>>;

    /// The mapping `id`, when held.
    async fn get_by_id(&self, id: &str) -> Result<Option<ColumnMapping>>;

    /// Stores `mapping`, created or replaced, recording the change the user made (SYN-020).
    async fn save(&self, mapping: &ColumnMapping) -> Result<()>;

    /// Deletes the mapping `id`, recording its removal (SYN-024).
    async fn delete(&self, id: &str) -> Result<()>;

    /// Stamps `rank` on every mapping never ranked (CFR-014, D6); how many were stamped.
    async fn stamp_sync_rank(&self, conn: &mut SqliteConnection, rank: &Rank) -> Result<u64>;

    /// The synced mapping `identity` with its rank, when held (CFR-014).
    async fn synced_record(
        &self,
        conn: &mut SqliteConnection,
        identity: &str,
    ) -> Result<Option<SyncedRecord>>;

    /// Writes an incoming mapping verbatim with its rank (CFR-017).
    async fn apply(
        &self,
        conn: &mut SqliteConnection,
        mapping: &ColumnMapping,
        rank: &Rank,
    ) -> Result<()>;

    /// Removes the mapping `identity` on an incoming removal (CFR-017).
    async fn remove_synced(&self, conn: &mut SqliteConnection, identity: &str) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(name: &str) -> ColumnMappingDraft {
        ColumnMappingDraft {
            name: name.to_string(),
            separator: Separator::Semicolon,
            decimal_mark: DecimalMark::Dot,
            date_format: DateFormat::DayMonthYear,
            encoding: Encoding::Windows1252,
            columns: MappedColumns {
                date: "Date".to_string(),
                operation: "Opération".to_string(),
                isin: None,
                reference: None,
                name: Some("libellé".to_string()),
                quantity: Some("Qté".to_string()),
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

    // IMP-024 — a name is stored trimmed; one empty once trimmed is refused.
    #[test]
    fn imp_024_a_name_is_trimmed_and_never_empty() {
        let mapping = ColumnMapping::new(draft("  Broker PEA ")).expect("named");
        assert_eq!(mapping.name, "Broker PEA");
        assert!(matches!(
            ColumnMapping::new(draft("   ")),
            Err(ImportError::MappingNameEmpty)
        ));
    }

    // IMP-024 — names are compared case and accents ignored.
    #[test]
    fn imp_024_names_compare_case_and_accents_ignored() {
        let mapping = ColumnMapping::new(draft("Épargne Société")).expect("named");
        assert!(mapping.is_named(" epargne SOCIETE"));
        assert!(!mapping.is_named("Epargne"));
    }

    // IMP-021 — the date, the operation and one asset column are required.
    #[test]
    fn imp_021_the_date_the_operation_and_an_asset_column_are_required() {
        let mut no_date = draft("A");
        no_date.columns.date = " ".to_string();
        assert!(matches!(
            ColumnMapping::new(no_date),
            Err(ImportError::MappingColumnMissing)
        ));
        let mut no_asset = draft("B");
        no_asset.columns.name = Some(String::new());
        assert!(matches!(
            ColumnMapping::new(no_asset),
            Err(ImportError::MappingColumnMissing)
        ));
        let mut by_isin = draft("C");
        by_isin.columns.name = None;
        by_isin.columns.isin = Some("ISIN".to_string());
        assert!(ColumnMapping::new(by_isin).is_ok());
    }

    // IMP-024A — every text is at most MAPPING_TEXT_LENGTH characters, counted as characters.
    #[test]
    fn imp_024a_a_text_longer_than_the_bound_is_refused() {
        let mut long_label = draft("A");
        long_label.operation_types.push(OperationLabel {
            label: "x".repeat(MAPPING_TEXT_LENGTH + 1),
            transaction_type: OperationType::Purchase,
        });
        assert!(matches!(
            ColumnMapping::new(long_label),
            Err(ImportError::MappingTextTooLong)
        ));
        let mut long_header = draft("B");
        long_header.columns.fees = Some("x".repeat(MAPPING_TEXT_LENGTH + 1));
        assert!(matches!(
            ColumnMapping::new(long_header),
            Err(ImportError::MappingTextTooLong)
        ));
        let mut exact = draft(&"é".repeat(MAPPING_TEXT_LENGTH));
        exact.asset_matches.push(AssetMatch {
            read: "é".repeat(MAPPING_TEXT_LENGTH),
            asset_id: "asset-1".to_string(),
        });
        assert!(ColumnMapping::new(exact).is_ok());
    }
}
