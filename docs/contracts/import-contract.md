# Contract — Import

> Domain: `import`
> Last updated by: `csv-import` spec

> **Error model on the wire**: each command's error serializes as a flat `{ code: "VariantName", ...payload }` object. The FE matches on `code`. Infrastructure failures surface as `{ code: "DatabaseError" }` (no payload; diagnostic logged server-side), consistent with the other contracts.

---

## Commands

### Column mappings

> A column mapping says how one broker's export is read (IMP-020): saved under a name, reused for
> the next file, synced (IMP-023).

| Command                 | Args                                                    | Return               | Errors                                                                                                                                                                                              |
| ----------------------- | ------------------------------------------------------- | -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `list_column_mappings`  | —                                                       | `Vec<ColumnMapping>` | `DatabaseError` _(IMP-020 — by name; empty when none is saved)_                                                                                                                                     |
| `save_column_mapping`   | `mapping_id: Option<String>, draft: ColumnMappingDraft` | `ColumnMapping`      | `MappingNameEmpty` (IMP-024), `MappingNameTaken { name }` (IMP-024), `MappingColumnMissing` (IMP-021), `MappingTextTooLong` (IMP-024A), `MappingNotFound { mapping_id }` (IMP-025), `DatabaseError` |
| `delete_column_mapping` | `mapping_id: String`                                    | `()`                 | `MappingNotFound { mapping_id }` (IMP-040), `DatabaseError`                                                                                                                                         |

### Reading and previewing a file

> A file is read for one account (IMP-010), its format detected or chosen (IMP-011 to IMP-018),
> then previewed through the mapping in use, every row judged and nothing written (IMP-017,
> IMP-030). `path` is what `pick_import_file` returned: a file the user did not pick there is
> refused (IMP-010). The preview reads the file
> with the mapping's own format, none detected (IMP-020); a choice the user makes is an edit of
> that mapping (IMP-018). `mapping_id` names the saved mapping it was picked from: its asset
> matches stand, any other was made now and must name an asset of the portfolio (IMP-033).

The three commands return `CsvImportError`: the codes of `ImportError`, of `AccountError` (`AccountNotFound`, `DatabaseError`) and of the use case's own `CsvImportTask` (`FileUnreadable`, `FileNotPicked`, `MatchedAssetNotFound`, `UnknownError` — a broken invariant, logged), each on the wire as `{ "code": … }`.

**File errors** (`read_import_file`, `preview_import`): `AccountNotFound { account_id }`, `FileUnreadable`, `FileNotPicked`, `FileTooLarge`, `FileEmpty`, `FileHasNoRow` (IMP-010), `FileNotUtf8` (IMP-011), `SeparatorNotFound` (IMP-012), `HeaderNameRepeated { name }` (IMP-015), `DatabaseError`.

| Command            | Args                                                                                        | Return           | Errors                                                                                                                                                                                     |
| ------------------ | ------------------------------------------------------------------------------------------- | ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `pick_import_file` | —                                                                                           | `Option<String>` | `FileUnreadable` _(IMP-010 — the system dialog; `None` when the user closes it)_                                                                                                           |
| `read_import_file` | `account_id: String, path: String, choices: ReadingChoices`                                 | `FileRead`       | the file errors                                                                                                                                                                            |
| `preview_import`   | `account_id: String, path: String, mapping_id: Option<String>, mapping: ColumnMappingDraft` | `ImportPreview`  | the file errors, `MappingColumnMissing` (IMP-021), `MappedColumnNotInFile { column }` (IMP-021), `MappingNotFound { mapping_id }` (IMP-020), `MatchedAssetNotFound { asset_id }` (IMP-033) |

---

## Shared Types

```rust
// IMP-020 — how one broker's export is read.
struct ColumnMapping {
    id: String,            // kept by a rename (IMP-025)
    name: String,          // trimmed, unique case and accents ignored; every text ≤ 500 characters (IMP-024A)
    separator: Separator,
    decimal_mark: DecimalMark,
    date_format: DateFormat,
    encoding: Encoding,
    columns: MappedColumns,
    operation_types: Vec<OperationLabel>,
    asset_matches: Vec<AssetMatch>,
}

// What the user saves: a ColumnMapping without its id. `save_column_mapping` without a
// `mapping_id` creates one; with it, replaces the mapping it names (IMP-025).
struct ColumnMappingDraft {
    name: String,
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
// when the match is made (`preview_import`) and when it is used: a saved match whose asset is
// gone leaves its rows unknown. Saving does not check it.
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
}

// IMP-010 to IMP-016 — how a file reads.
struct FileRead {
    encoding: Encoding,
    separator: Separator,
    decimal_mark: DecimalMark,
    date_reading: Option<DateReading>, // None until the date column is known
    columns: Vec<String>,              // the header's names, in the file's order, empty ones left out
    rows: u32,                         // lines after the header that are not blank
}

// IMP-014 — `detected: false`: no format read every date, day/month/year taken.
struct DateReading { format: DateFormat, detected: bool }

// IMP-030 — the file read through the mapping, every row judged.
struct ImportPreview {
    file: FileRead,
    rows: Vec<PreviewRow>, // every line that is not blank, in the file's order
    counts: PreviewCounts, // a joined trade tax counts with its purchase
}

struct PreviewRow { line: u32 /* from 1 for the header */, status: RowStatus }

struct PreviewCounts { ready: u32, rejected: u32, unknown_asset: u32, already_recorded: u32, not_imported: u32 }

// IMP-030 — on the wire `{ "status": "Ready", ... }`.
enum RowStatus {
    Ready { operation: OperationRead, asset_id: Option<String>, funding: Option<i64> }, // funding: IMP-038
    Rejected { reason: RowReason },
    UnknownAsset { asset: AssetRead },                                                  // IMP-032
    AlreadyRecorded { operation: OperationRead, asset_id: Option<String> },             // IMP-036
    NotImported { label: String },                                                      // IMP-022
    JoinedTradeTax { purchase_line: u32 },                                              // IMP-034
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

| Event                  | Payload | Rule                                                                                                                |
| ---------------------- | ------- | ------------------------------------------------------------------------------------------------------------------- |
| `ColumnMappingUpdated` | —       | IMP-026 — a mapping saved, renamed or deleted on this computer; bare signal; a synced change raises `SyncCompleted` |

---

## Changelog

- 2026-10-25 — Added by `csv-import` (#501): `list_column_mappings`, `save_column_mapping`, `delete_column_mapping`, the mapping types, event `ColumnMappingUpdated`.
- 2026-10-25 — Amended by `csv-import` (#501): the reading errors of `ImportError` (IMP-010 to IMP-021), ahead of the preview command.
- 2026-10-25 — Amended by `csv-import` (#501): `read_import_file`, `preview_import`, their types and errors (IMP-010 to IMP-038).
