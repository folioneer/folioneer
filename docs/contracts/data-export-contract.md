# Contract — Data Export

> Domain: `data_export` (use case — reads `account` accounts and transactions, `asset` assets and prices)
> Last updated by: `data-export` spec

> **Error model on the wire**: each command's error serializes as a flat `{ code: "VariantName" }` object. The FE matches on `code`. Infrastructure failures surface as `{ code: "DatabaseError" }` (no payload; diagnostic chain preserved server-side via `tracing::error!`).

---

## Commands

| Command            | Args                                   | Return         | Errors                                                                  |
| ------------------ | -------------------------------------- | -------------- | ----------------------------------------------------------------------- |
| `export_portfolio` | `format: ExportFormat, folder: String` | `ExportReport` | `FolderUnavailable` (EXP-015), `WriteFailed` (EXP-015), `DatabaseError` |

Traceability: `export_portfolio` ← EXP-010 (the action, after the system's folder picker), EXP-011 to EXP-014 (what is written, under which name), EXP-015 (complete or nothing: `FolderUnavailable` for a folder that is not there or not a folder, `WriteFailed` for one that cannot be written, nothing left behind), EXP-016 (the report), EXP-020 to EXP-025 and EXP-030 to EXP-033 (the format), EXP-051 (the mapper written beside the CSV tables).

The folder is chosen in the system's picker by the frontend; closing it calls nothing (EXP-010).

---

## Shared Types

```rust
enum ExportFormat {
    Csv,    // a folder of CSV files (EXP-011)
    Json,   // one JSON file (EXP-012)
}

struct ExportReport {
    written_to: String,   // the full path of the folder or file written; the window shows its last part (EXP-016)
    accounts: u32,        // how many accounts it holds
    assets: u32,          // how many assets it holds
    transactions: u32,    // how many transactions it holds
    prices: u32,          // how many prices it holds
}
```

---

## Changelog

- 2026-11-07 — #509 (EXP): `export_portfolio` with `ExportFormat` and `ExportReport`; errors `FolderUnavailable`, `WriteFailed`, `DatabaseError`.
