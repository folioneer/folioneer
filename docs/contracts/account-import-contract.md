# Contract — Account import

> Domain: `account_import`
> Last updated by: `csv-import` spec

> **Error model on the wire**: each command's error serializes as a flat `{ code: "VariantName", ...payload }` object. The FE matches on `code`. Infrastructure failures surface as `{ code: "DatabaseError" }`, or `{ code: "MapperFolderFailed" }` for the mappers folder (no payload; diagnostic logged server-side), consistent with the other contracts.

---

## Commands

### Import mappers

> An import mapper says how one provider's export is read (IMP-020): a JSON file in the mappers
> folder of the data folder, reused for the next file, not synced for now (#715).

| Command                       | Args                                                  | Return                 | Errors                                                                                                                                                                                                                                                                                                                         |
| ----------------------------- | ----------------------------------------------------- | ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `list_import_mappers`         | —                                                     | `Vec<ImportMapper>`    | `MapperFolderFailed` _(IMP-020A — by provider; empty when none; a file that does not read is left out)_                                                                                                                                                                                                                        |
| `save_import_mapper`          | `mapper_id: Option<String>, draft: ImportMapperDraft` | `ImportMapper`         | `MapperProviderEmpty` (IMP-024), `MapperProviderTaken { provider }` (IMP-024), `MapperColumnMissing` (IMP-021), `MapperTextTooLong` (IMP-024A), `MapperNotFound { mapper_id }`, `MapperFileInvalid { reason }`, `MapperFileTooLarge` (all IMP-025), `MapperFolderFailed` (IMP-020)                                             |
| `add_import_mapper_from_file` | —                                                     | `Option<ImportMapper>` | `MapperFileUnreadable`, `MapperFileTooLarge`, `MapperFileInvalid { reason }` (IMP-020B), `MapperProviderEmpty` (IMP-024), `MapperProviderTaken { provider }` (IMP-024), `MapperColumnMissing` (IMP-021), `MapperTextTooLong` (IMP-024A), `MapperFolderFailed` _(IMP-020B — the system dialog; `None` when the user closed it)_ |
| `delete_import_mapper`        | `mapper_id: String`                                   | `()`                   | `MapperNotFound { mapper_id }` (IMP-040), `MapperFolderFailed`                                                                                                                                                                                                                                                                 |

### Reading and previewing a file

> A file is read for one account (IMP-010), its format detected or chosen (IMP-011 to IMP-018),
> then previewed through the mapper in use, every row judged and nothing written (IMP-017,
> IMP-030). `path` is what the last `pick_import_file` returned: any other file is
> refused (IMP-010). The preview reads the file
> with the mapper's own format, none detected (IMP-020E); a choice the user makes is an edit of
> that mapper (IMP-018). `mapper_id` names the saved mapper it was picked from: its asset
> matches stand, any other was made now and must name an asset of the portfolio (IMP-033);
> saving a mapper keeps its matches as they are, unchecked.

The five commands return `CsvImportError`: the codes of `AccountImportError`, of `AccountError` (`AccountNotFound`, `DatabaseError`) and of the use case's own `CsvImportTask` (`FileUnreadable`, `FileNotPicked`, `MatchedAssetNotFound`, `UnknownError` — a broken invariant, logged), each on the wire as `{ "code": … }`.

**File errors** (`read_import_file`, `preview_import`): `AccountNotFound { account_id }`, `FileUnreadable`, `FileNotPicked`, `FileTooLarge`, `FileEmpty`, `FileHasNoRow` (IMP-010), `FileNotUtf8` (IMP-011), `SeparatorNotFound` (IMP-012), `HeaderNameRepeated { name }` (IMP-015), `DatabaseError`.

| Command                    | Args                                                                                                         | Return           | Errors                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| -------------------------- | ------------------------------------------------------------------------------------------------------------ | ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pick_import_file`         | —                                                                                                            | `Option<String>` | `FileUnreadable` _(IMP-010 — the system dialog; `None` when the user closes it)_                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `remembered_import_mapper` | `path: String`                                                                                               | `Option<String>` | `FileUnreadable`, `FileNotPicked` (IMP-010) _(IMP-020D — the mapper the file's folder remembers, `None` when it can't be told; `confirm_import` with a `mapper_id` remembers it, IMP-020C)_                                                                                                                                                                                                                                                                                                                     |
| `read_import_file`         | `account_id: String, path: String, choices: ReadingChoices`                                                  | `FileRead`       | the file errors                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `confirm_import`           | `account_id: String, path: String, mapper_id: Option<String>, mapper: ImportMapperDraft, add_deposits: bool` | `ImportPreview`  | the file errors, `MapperColumnMissing` (IMP-021), `MappedColumnsNotInFile { columns }` (IMP-021), with a `mapper_id`, its saved file read through `get_import_mapper` and `ImportMapper::with_id` (IMP-020E): `MapperNotFound { mapper_id }`, `MapperFileInvalid { reason }`, `MapperFileTooLarge`, `MapperProviderEmpty`, `MapperTextTooLong`, `MapperFolderFailed`; `MatchedAssetNotFound { asset_id }` (IMP-033), `UnknownError` — the rows as judged at the confirmation, the ready ones recorded (IMP-050) |
| `preview_import`           | `account_id: String, path: String, mapper_id: Option<String>, mapper: ImportMapperDraft`                     | `ImportPreview`  | the file errors, `MapperColumnMissing` (IMP-021), `MappedColumnsNotInFile { columns }` (IMP-021), with a `mapper_id`, its saved file read through `get_import_mapper` and `ImportMapper::with_id` (IMP-020E): `MapperNotFound { mapper_id }`, `MapperFileInvalid { reason }`, `MapperFileTooLarge`, `MapperProviderEmpty`, `MapperTextTooLong`, `MapperFolderFailed`; `MatchedAssetNotFound { asset_id }` (IMP-033), `UnknownError`                                                                             |

---

## Shared Types

```rust
// IMP-020 — how one provider's export is read: a file of the mappers folder.
struct ImportMapper {
    id: String,            // its file's name; kept by a change of provider (IMP-025)
    info: MapperInfo,
    separator: Separator,
    decimal_mark: DecimalMark,
    date_format: DateFormat,
    encoding: Encoding,
    columns: MappedColumns,
    operation_types: Vec<OperationLabel>,
    asset_matches: Vec<AssetMatch>,
}

struct MapperInfo {
    provider: String,      // trimmed, unique case and accents ignored; every text ≤ 500 characters (IMP-024A)
}

// What a mapper's file holds: an ImportMapper without its id. `save_import_mapper` without a
// `mapper_id` creates one; with it, replaces the file it names (IMP-025).
struct ImportMapperDraft {
    info: MapperInfo,
    separator: Separator,
    decimal_mark: DecimalMark,
    date_format: DateFormat,
    encoding: Encoding,
    columns: MappedColumns,
    operation_types: Vec<OperationLabel>,
    asset_matches: Vec<AssetMatch>,
}

// IMP-021 — the header of the column holding each field. `None`: the file has no such
// column. At least one of isin, reference and name.
struct MappedColumns {
    date: String,
    operation: String,
    isin: Option<String>,
    reference: Option<String>,
    name: Option<String>,
    quantity: Option<String>,
    unit_price: Option<String>,
    gross_amount: Option<String>,
    fees: Option<String>,
    net_amount: Option<String>,
    currency: Option<String>,
}

// IMP-022 — one operation label of the file and its operation type.
struct OperationLabel {
    label: String,
    transaction_type: OperationType,
}

// IMP-033 — a name, ISIN or reference of the file matched to an asset. The asset is checked
// when the match is made (`preview_import`, `confirm_import`) and when it is used: a saved
// match whose asset is gone is passed over (IMP-033A). Saving does not check it.
struct AssetMatch {
    read: String,
    asset_id: String,
}

enum Separator { Semicolon, Comma, Tab }                    // IMP-012
enum DecimalMark { Dot, Comma }                             // IMP-013
enum DateFormat { DayMonthYear, MonthDayYear, YearMonthDay } // IMP-014
enum Encoding { Utf8, Windows1252 }                         // IMP-011
enum OperationType { Purchase, Sale, Dividend, Deposit, Withdrawal, TradeTax, NotImported } // IMP-022

// IMP-018 — what the user picked; each `None` is detected. `date_column`: the header the date
// format is detected on (IMP-014), once mapped.
struct ReadingChoices {
    encoding: Option<Encoding>,
    separator: Option<Separator>,
    decimal_mark: Option<DecimalMark>,
    date_format: Option<DateFormat>,
    date_column: Option<String>,
    operation_column: Option<String>, // IMP-022A — never detected; None, or a header the file lacks: no labels
}

// IMP-010 to IMP-016 — how a file reads.
struct FileRead {
    encoding: Encoding,
    separator: Separator,
    decimal_mark: DecimalMark,
    date_reading: Option<DateReading>, // None until the date column is known
    columns: Vec<String>,              // the header's names, in the file's order, empty ones left out
    rows: u32,                         // lines after the header that are not blank
    operation_labels: Vec<FileOperationLabel>, // empty until the operation column is known (IMP-022A)
}

// IMP-022A — a label of the operation column, as first met, and how many rows hold it.
struct FileOperationLabel { label: String, rows: u32 }

// IMP-014 — `detected: false`: no format read every date, day/month/year taken.
struct DateReading { format: DateFormat, detected: bool }

// IMP-030 — the file read through the mapper, every row judged.
struct ImportPreview {
    file: FileRead,
    rows: Vec<PreviewRow>, // every line that is not blank, in the file's order
    counts: PreviewCounts, // a joined trade tax counts with its purchase
    all_recorded: bool,    // IMP-053, IMP-066 — rows, and every one already recorded
    deposits: DepositsNeeded,
    unknown_assets: Vec<UnknownAssetRows>, // IMP-067 — as first met
    matchable_assets: Vec<MatchableAsset>, // IMP-032, IMP-067 — by name, archived included, no Cash Asset
}

// IMP-068 — `ready_without`: the ready rows left when the deposits are left out (IMP-051B).
struct DepositsNeeded { count: u32, total: i64, ready_without: u32 }

// IMP-067 — `read`: the first the rows read of ISIN, reference and name, what a match stands for.
struct UnknownAssetRows { read: String, asset: AssetRead, lines: Vec<u32> }

struct MatchableAsset { id: String, name: String }

struct PreviewRow {
    line: u32,           // from 1 for the header
    status: RowStatus,
    amount: Option<i64>, // IMP-030 — its total as manual entry computes it (TRX-026, SEL-023), or its amount
}

struct PreviewCounts { ready: u32, rejected: u32, unknown_asset: u32, already_recorded: u32, not_imported: u32 }

// IMP-030 — on the wire `{ "status": "Ready", ... }`.
enum RowStatus {
    Ready { operation: OperationRead, asset_id: Option<String>, funding: Option<i64> }, // funding: IMP-038
    Rejected { reason: RowReason, operation: Option<OperationRead> },                   // none: the row read as no operation
    UnknownAsset { asset: AssetRead, operation: OperationRead },                        // IMP-032
    AlreadyRecorded { operation: OperationRead, asset_id: Option<String> },             // IMP-036
    NotImported { label: String, date: Option<String>, asset: AssetRead },              // IMP-022
    JoinedTradeTax { purchase_line: u32, operation: OperationRead },                    // IMP-034
}

// On the wire `{ "code": "Unread", ... }`.
enum RowReason {
    Unread { rejection: RowRejection },   // IMP-016, IMP-022, IMP-030, IMP-031, IMP-034
    OtherCurrency,                        // IMP-037
    AssetNotHeld,                         // DIV-011
    ManualEntry { refusal: AccountError }, // IMP-035: the refusal manual entry gives (account contract)
}

// On the wire `{ "code": "LineTooShort" }`.
enum RowRejection {
    LineTooShort,                         // IMP-016
    OperationTypeMissing { label: String }, // IMP-022
    DateUnreadable { column: String },    // IMP-030
    FigureUnreadable { column: String },  // IMP-030
    FigureMissing { field: String },      // IMP-031: `quantity`, `unit_price` or `net_amount`
    QuantityZero,                         // TRX-020
    TradeTaxUnpaired,                     // IMP-034
}

// A row read as an operation, figures in micro-units, signs ignored (IMP-031).
struct OperationRead {
    date: String,             // YYYY-MM-DD
    asset: AssetRead,         // all None for a deposit and a withdrawal
    currency: Option<String>, // the row's own, when mapped
    figures: RowFigures,
}

struct AssetRead { isin: Option<String>, reference: Option<String>, name: Option<String> } // IMP-032

// On the wire `{ "type": "Purchase", ... }`.
enum RowFigures {
    Purchase { quantity: i64, unit_price: i64, fees: i64 }, // fees include a joined trade tax
    Sale { quantity: i64, unit_price: i64, fees: i64 },
    Dividend { amount: i64 },
    Deposit { amount: i64 },
    Withdrawal { amount: i64 },
    TradeTax { amount: i64, quantity: Option<i64> },
}
```

---

## Events

### Published

| Event                | Payload | Rule                                                                                               |
| -------------------- | ------- | -------------------------------------------------------------------------------------------------- |
| `TransactionUpdated` | —       | IMP-050A — `confirm_import` recorded at least one row; bare signal, once per import                |
| `AssetUpdated`       | —       | IMP-050A, CSH-010 — `confirm_import` created the Cash Asset of the account's currency; bare signal |

---

## Changelog

- 2026-10-25 — Added by `csv-import` (#501): `list_column_mappings`, `save_column_mapping`, `delete_column_mapping`, the mapping types, event `ColumnMappingUpdated`.
- 2026-10-25 — Amended by `csv-import` (#501): the reading errors of `ImportError` (IMP-010 to IMP-021), ahead of the preview command.
- 2026-10-25 — Amended by `csv-import` (#501): `read_import_file`, `preview_import`, their types and errors (IMP-010 to IMP-038).
- 2026-10-25 — Amended by `csv-import` (#502): `confirm_import` (IMP-050 to IMP-053).
- 2026-10-31 — Amended by `csv-import` (#696): the events `confirm_import` publishes; saving keeps matches unchecked.
- 2026-10-31 — Amended by `csv-import` (#714): a column mapping becomes an import mapper, a JSON file: `list_import_mappers`, `save_import_mapper`, `delete_import_mapper` replace the column mapping commands; `add_import_mapper_from_file`, `remembered_import_mapper` added; event `ColumnMappingUpdated` removed.
- 2026-10-31 — Amended by `csv-import` (#714): the domain is `account_import`; `ImportError` is `AccountImportError`, its codes unchanged.
- 2026-10-31 — Amended by `csv-import` (#503): `MappedColumnNotInFile { column }` becomes `MappedColumnsNotInFile { columns }`, naming every column the file lacks (IMP-021); `ReadingChoices` gains `operation_column` and `FileRead` gains `operation_labels` (IMP-022A).
- 2026-10-31 — Amended by `csv-import` (#504): each previewed row carries what it read and its amount (IMP-030); the preview adds `all_recorded`, `deposits`, `unknown_assets`, `matchable_assets` (IMP-066 to IMP-068).
