use crate::context::account_import::error::AccountImportError;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::Path;

/// IMP-024A — the longest text a mapper holds, in characters.
pub const MAPPER_TEXT_LENGTH: usize = 500;

/// IMP-020B — the largest mapper file taken in, in bytes.
pub const LARGEST_MAPPER_FILE: u64 = 1_000_000;

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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct OperationLabel {
    /// The label as the file writes it.
    pub label: String,
    /// The operation type it stands for.
    pub transaction_type: OperationType,
}

/// A name, ISIN or reference of the file the user matched to an asset (IMP-033).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(deny_unknown_fields)]
pub struct AssetMatch {
    /// The name, ISIN or reference as the file writes it.
    pub read: String,
    /// The asset it stands for.
    pub asset_id: String,
}

/// The longest reason told about a file: it may quote the file's own text.
const REASON_LENGTH: usize = 200;

/// The info entry of a mapper's file (IMP-020).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(deny_unknown_fields)]
pub struct MapperInfo {
    /// Who the export comes from, trimmed (IMP-024).
    pub provider: String,
}

/// IMP-020 — how one provider's export is read: a file in the mappers folder, reused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ImportMapper {
    /// Its file's name without `.json`: a change of provider keeps it (IMP-025).
    pub id: String,
    /// Who the export comes from.
    pub info: MapperInfo,
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

/// What a mapper holds besides its identity: exactly what its file holds (IMP-020). A field
/// it does not have is refused, never ignored (IMP-020B).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(deny_unknown_fields)]
pub struct ImportMapperDraft {
    /// Who the export comes from.
    pub info: MapperInfo,
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

impl ImportMapperDraft {
    /// IMP-020B — a draft read from a mapper's file, a byte order mark dropped; `reason` says
    /// why it does not read.
    pub fn from_file(bytes: &[u8]) -> Result<Self, AccountImportError> {
        let text = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
        serde_json::from_slice(text).map_err(|e| AccountImportError::MapperFileInvalid {
            reason: e.to_string().chars().take(REASON_LENGTH).collect(),
        })
    }
}

impl MappedColumns {
    /// IMP-021 — the date and the operation column are named, and at least one asset column.
    pub fn check(&self) -> Result<(), AccountImportError> {
        if self.date.trim().is_empty() || self.operation.trim().is_empty() {
            return Err(AccountImportError::MapperColumnMissing);
        }
        if [&self.isin, &self.reference, &self.name]
            .iter()
            .all(|column| {
                column
                    .as_deref()
                    .is_none_or(|header| header.trim().is_empty())
            })
        {
            return Err(AccountImportError::MapperColumnMissing);
        }
        Ok(())
    }
}

impl ImportMapper {
    /// IMP-021 / IMP-024 / IMP-024A — a mapper under the id of its file, validated.
    pub fn with_id(id: String, draft: ImportMapperDraft) -> Result<Self, AccountImportError> {
        let provider = draft.info.provider.trim().to_string();
        if provider.is_empty() {
            return Err(AccountImportError::MapperProviderEmpty);
        }
        let columns = draft.columns;
        columns.check()?;
        let texts = [&provider, &columns.date, &columns.operation]
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
            .any(|text| text.chars().count() > MAPPER_TEXT_LENGTH)
        {
            return Err(AccountImportError::MapperTextTooLong);
        }
        Ok(Self {
            id,
            info: MapperInfo { provider },
            separator: draft.separator,
            decimal_mark: draft.decimal_mark,
            date_format: draft.date_format,
            encoding: draft.encoding,
            columns,
            operation_types: draft.operation_types,
            asset_matches: draft.asset_matches,
        })
    }

    /// IMP-020 — the mapper under the file name it takes among `held`, when it has none yet.
    pub fn named_among(mut self, held: &[String]) -> Self {
        if self.id.is_empty() {
            self.id = new_mapper_id(&self.info.provider, held);
        }
        self
    }

    /// IMP-020 — what its file holds.
    pub fn to_file(&self) -> Result<Vec<u8>> {
        let draft = ImportMapperDraft {
            info: self.info.clone(),
            separator: self.separator,
            decimal_mark: self.decimal_mark,
            date_format: self.date_format,
            encoding: self.encoding,
            columns: self.columns.clone(),
            operation_types: self.operation_types.clone(),
            asset_matches: self.asset_matches.clone(),
        };
        Ok(serde_json::to_vec_pretty(&draft)?)
    }

    /// IMP-024 — whether this mapper's provider is `provider`, case and accents ignored.
    pub fn has_provider(&self, provider: &str) -> bool {
        folded(&self.info.provider) == folded(provider)
    }
}

/// Whether `id` can name a file of the mappers folder on every platform shipped: never a
/// path, a hidden file or a name Windows reserves.
pub fn names_a_file(id: &str) -> bool {
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let stem = id.split('.').next().unwrap_or(id);
    !id.is_empty()
        && !id.starts_with('.')
        && !id.ends_with(['.', ' '])
        && !id
            .chars()
            .any(|c| c.is_control() || r#"/\\:<>"|?*"#.contains(c))
        && !RESERVED.iter().any(|name| stem.eq_ignore_ascii_case(name))
}

/// IMP-020 — the file name a new mapper of `provider` takes: readable, and none of `held`.
pub fn new_mapper_id(provider: &str, held: &[String]) -> String {
    let slug = folded(provider)
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let base: String = if slug.is_empty() {
        "mapper".to_string()
    } else {
        slug.chars().take(64).collect()
    };
    let base = if names_a_file(&base) {
        base
    } else {
        format!("{base}-mapper")
    };
    let taken = |id: &str| held.iter().any(|other| other.eq_ignore_ascii_case(id));
    if !taken(&base) {
        return base;
    }
    let mut number = 2;
    while taken(&format!("{base}-{number}")) {
        number += 1;
    }
    format!("{base}-{number}")
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

/// What a picked mapper's file gave (IMP-020B).
#[derive(Debug, PartialEq, Eq)]
pub enum PickedMapperFile {
    /// Its bytes, whole.
    Read(Vec<u8>),
    /// It holds more than the bytes allowed.
    TooLarge,
}

/// The mappers folder (IMP-020): each file by its name without `.json`. An id that names no
/// file of the folder — a path among them — is held by none.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait ImportMapperStore: Send + Sync {
    /// The id of every `.json` file of the folder; none when there is no folder.
    async fn ids(&self) -> Result<Vec<String>>;

    /// The bytes of the file `id`, when held.
    async fn read(&self, id: &str) -> Result<Option<Vec<u8>>>;

    /// Writes the file `id` whole, created or replaced.
    async fn write(&self, id: &str, bytes: &[u8]) -> Result<()>;

    /// Removes the file `id`.
    async fn delete(&self, id: &str) -> Result<()>;

    /// IMP-020B — the file at `path`, anywhere, read no further than `largest` bytes.
    async fn read_picked(&self, path: &Path, largest: u64) -> Result<PickedMapperFile>;
}

/// The mapper each folder remembers, on this computer only (IMP-020C).
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait MapperChoiceRepository: Send + Sync {
    /// The id `folder` remembers, when it remembers one.
    async fn remembered(&self, folder: &str) -> Result<Option<String>>;

    /// `folder` now remembers `mapper_id`.
    async fn remember(&self, folder: &str, mapper_id: &str) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapper(draft: ImportMapperDraft) -> Result<ImportMapper, AccountImportError> {
        ImportMapper::with_id("broker".to_string(), draft)
    }

    fn draft(name: &str) -> ImportMapperDraft {
        ImportMapperDraft {
            info: MapperInfo {
                provider: name.to_string(),
            },
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

    // IMP-024 — a provider is stored trimmed; one empty once trimmed is refused.
    #[test]
    fn imp_024_a_provider_is_trimmed_and_never_empty() {
        let held = mapper(draft("  Broker PEA ")).expect("named");
        assert_eq!(held.info.provider, "Broker PEA");
        assert!(matches!(
            mapper(draft("   ")),
            Err(AccountImportError::MapperProviderEmpty)
        ));
    }

    // IMP-024 — providers are compared case and accents ignored.
    #[test]
    fn imp_024_providers_compare_case_and_accents_ignored() {
        let held = mapper(draft("Épargne Société")).expect("named");
        assert!(held.has_provider(" epargne SOCIETE"));
        assert!(!held.has_provider("Epargne"));
    }

    // IMP-021 — the date, the operation and one asset column are required.
    #[test]
    fn imp_021_the_date_the_operation_and_an_asset_column_are_required() {
        let mut no_date = draft("A");
        no_date.columns.date = " ".to_string();
        assert!(matches!(
            mapper(no_date),
            Err(AccountImportError::MapperColumnMissing)
        ));
        let mut no_asset = draft("B");
        no_asset.columns.name = Some(String::new());
        assert!(matches!(
            mapper(no_asset),
            Err(AccountImportError::MapperColumnMissing)
        ));
        let mut by_isin = draft("C");
        by_isin.columns.name = None;
        by_isin.columns.isin = Some("ISIN".to_string());
        assert!(mapper(by_isin).is_ok());
    }

    // IMP-024A — every text is at most MAPPER_TEXT_LENGTH characters, counted as characters.
    #[test]
    fn imp_024a_a_text_longer_than_the_bound_is_refused() {
        let mut long_label = draft("A");
        long_label.operation_types.push(OperationLabel {
            label: "x".repeat(MAPPER_TEXT_LENGTH + 1),
            transaction_type: OperationType::Purchase,
        });
        assert!(matches!(
            mapper(long_label),
            Err(AccountImportError::MapperTextTooLong)
        ));
        let mut long_header = draft("B");
        long_header.columns.fees = Some("x".repeat(MAPPER_TEXT_LENGTH + 1));
        assert!(matches!(
            mapper(long_header),
            Err(AccountImportError::MapperTextTooLong)
        ));
        let mut exact = draft(&"é".repeat(MAPPER_TEXT_LENGTH));
        exact.asset_matches.push(AssetMatch {
            read: "é".repeat(MAPPER_TEXT_LENGTH),
            asset_id: "asset-1".to_string(),
        });
        assert!(mapper(exact).is_ok());
    }

    // A draft carrying a field it does not have is refused when read.
    #[test]
    fn a_draft_with_an_unknown_field_is_refused() {
        let mut sent = serde_json::to_value(draft("A")).expect("serializes");
        sent["extra"] = serde_json::json!(1);
        assert!(serde_json::from_value::<ImportMapperDraft>(sent).is_err());
    }

    // IMP-020 / IMP-020B — a mapper's file reads back as what was written, a byte order mark
    // aside; what does not read says why.
    #[test]
    fn imp_020b_a_file_reads_back_and_a_bad_one_says_why() {
        let held = mapper(draft("Broker")).expect("valid");
        let file = held.to_file().expect("written");
        assert_eq!(
            ImportMapperDraft::from_file(&file).expect("reads"),
            draft("Broker")
        );
        let with_mark = [&[0xEF, 0xBB, 0xBF], file.as_slice()].concat();
        assert!(ImportMapperDraft::from_file(&with_mark).is_ok());
        for bad in [&b"not json"[..], br#"{"info":{"provider":"A"}}"#] {
            assert!(matches!(
                ImportMapperDraft::from_file(bad),
                Err(AccountImportError::MapperFileInvalid { reason }) if !reason.is_empty()
            ));
        }
    }

    // IMP-020 — a new mapper's file name reads as its provider, and takes no held one.
    #[test]
    fn imp_020_a_new_mappers_file_name() {
        assert_eq!(
            new_mapper_id("Épargne: Société/PEA", &[]),
            "epargne-societe-pea"
        );
        assert_eq!(new_mapper_id("../..", &[]), "mapper");
        assert_eq!(
            new_mapper_id("Broker", &["BROKER".to_string(), "broker-2".to_string()]),
            "broker-3"
        );
    }

    // IMP-020B — a reason quoting a file's text is bounded.
    #[test]
    fn imp_020b_a_reason_is_bounded() {
        let field = "x".repeat(5_000);
        let file = format!(r#"{{"{field}":1}}"#);
        let Err(AccountImportError::MapperFileInvalid { reason }) =
            ImportMapperDraft::from_file(file.as_bytes())
        else {
            panic!("refused");
        };
        assert!(reason.chars().count() <= 200);
    }

    // IMP-020 — a file name works on every platform shipped, a reserved one taking a suffix.
    #[test]
    fn imp_020_a_name_windows_reserves_is_not_taken() {
        assert_eq!(new_mapper_id("CON", &[]), "con-mapper");
        assert_eq!(new_mapper_id("com1", &[]), "com1-mapper");
        for id in ["nul", "Aux.x", "a.", "a ", "a?b", "a|b", "a*b", "a\nb"] {
            assert!(!names_a_file(id), "{id}");
        }
        assert!(names_a_file("console"));
    }

    // IMP-020B — a field a mapper does not have is refused at any depth, never dropped.
    #[test]
    fn imp_020b_a_nested_unknown_field_is_refused() {
        let mut with_lists = draft("Broker");
        with_lists.operation_types = vec![OperationLabel {
            label: "Buy".to_string(),
            transaction_type: OperationType::Purchase,
        }];
        with_lists.asset_matches = vec![AssetMatch {
            read: "Name".to_string(),
            asset_id: "a-1".to_string(),
        }];
        let written = serde_json::to_string(&with_lists).expect("JSON");
        assert!(ImportMapperDraft::from_file(written.as_bytes()).is_ok());
        let value: serde_json::Value = serde_json::from_str(&written).expect("JSON");
        for pointer in [
            "/info",
            "/columns",
            "/operation_types/0",
            "/asset_matches/0",
        ] {
            let mut changed = value.clone();
            changed
                .pointer_mut(pointer)
                .and_then(serde_json::Value::as_object_mut)
                .expect("an object")
                .insert("extra".to_string(), serde_json::json!(1));
            assert!(
                ImportMapperDraft::from_file(changed.to_string().as_bytes()).is_err(),
                "{pointer}"
            );
        }
    }
}
