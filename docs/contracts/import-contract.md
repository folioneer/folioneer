# Contract — Import

> Domain: `import`
> Last updated by: `csv-import` spec

> **Error model on the wire**: each command's error serializes as a flat `{ code: "VariantName", ...payload }` object. The FE matches on `code`. Infrastructure failures surface as `{ code: "DatabaseError" }` (no payload; diagnostic logged server-side), consistent with the other contracts.

---

## Commands

### Column mappings

> A column mapping says how one broker's export is read (IMP-020): saved under a name, reused for
> the next file, synced (IMP-023). Reading and previewing a file come with the next amendment.

| Command                 | Args                                                    | Return               | Errors                                                                                                                                                                                              |
| ----------------------- | ------------------------------------------------------- | -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `list_column_mappings`  | —                                                       | `Vec<ColumnMapping>` | `DatabaseError` _(IMP-020 — by name; empty when none is saved)_                                                                                                                                     |
| `save_column_mapping`   | `mapping_id: Option<String>, draft: ColumnMappingDraft` | `ColumnMapping`      | `MappingNameEmpty` (IMP-024), `MappingNameTaken { name }` (IMP-024), `MappingColumnMissing` (IMP-021), `MappingTextTooLong` (IMP-024A), `MappingNotFound { mapping_id }` (IMP-025), `DatabaseError` |
| `delete_column_mapping` | `mapping_id: String`                                    | `()`                 | `MappingNotFound { mapping_id }` (IMP-040), `DatabaseError`                                                                                                                                         |

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
// when the match is made (the preview's command, next amendment) and when it is used: a
// saved match whose asset is gone leaves its rows unknown. Saving does not check it.
struct AssetMatch {
    read: String,
    asset_id: String,
}

enum Separator { Semicolon, Comma, Tab }                    // IMP-012
enum DecimalMark { Dot, Comma }                             // IMP-013
enum DateFormat { DayMonthYear, MonthDayYear, YearMonthDay } // IMP-014
enum Encoding { Utf8, Windows1252 }                         // IMP-011
enum OperationType { Purchase, Sale, Dividend, Deposit, Withdrawal, TradeTax, NotImported } // IMP-022
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
