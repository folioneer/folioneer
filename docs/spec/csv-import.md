# Business Rules — Import (IMP)

## Context

Every transaction is typed by hand today, which stops anyone with years of history from starting (#369). Brokers export CSV files, and none agree on columns, separators, number and date formats or encoding. An **Import** brings a file's rows into one account in two steps: the file is read through a **Column mapping** and previewed row by row, nothing written; then the user confirms, and the rows are recorded at once (#502).

This spec holds the reading, the column mapping and the preview (#501); the confirmation and the **Added deposit** come with #502, the screens with #503 and #504. The import is a surface beside the window, the command line and the AI agent connection: any of them may start it, and its rows are judged and recorded through the same recorder as theirs, by the same rules — every transaction follows them, whatever records it (owner, 2026-10-25): the purchase (TRX), the sale (SEL), the dividend (DIV), the deposit and the withdrawal (CSH). This version starts it from the window (#503). Figures follow TRX-024. The first saved mapping is modelled on a French broker's securities export: one file, ISO-8859-1 (read as Windows-1252, IMP-011), `;` with a trailing `;`, a decimal dot, no ISIN column, the trade tax on its own row (owner, 2026-10-25).

---

## Entity Definition

### ColumnMapping

How one broker's export is read, saved under a name and reused for every file of that shape. It belongs to no account: one mapping serves every account the broker's files go into.

| Field             | Business meaning                                                                                                            |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `id`              | The mapping's own identity, independent of its name: a rename keeps it.                                                     |
| `name`            | What the user calls it ("Fortuneo PEA"): not empty once trimmed, unique among mappings, case and accents ignored (IMP-024). |
| `separator`       | The character between two fields: `;`, `,` or a tab.                                                                        |
| `decimal_mark`    | The mark before a figure's decimals: a dot or a comma.                                                                      |
| `date_format`     | The order a date is written in: day/month/year, month/day/year or year-month-day (IMP-014).                                 |
| `encoding`        | How the file's text is written: UTF-8 or Windows-1252, which reads an ISO-8859-1 file too.                                  |
| `columns`         | Which column of the file holds each field of a row (IMP-021), each named by its header.                                     |
| `operation_types` | Each operation label of the file and the operation type it stands for (IMP-022).                                            |
| `asset_matches`   | Each name, ISIN or reference of the file the user matched to an asset, and that asset (IMP-033).                            |

### Preview (computed, never stored)

What reading a file through a mapping shows: the encoding, separator, decimal mark and date format it was read with, detected or chosen (IMP-011 to IMP-014, IMP-018); the file's columns, named by its header; its operation labels, each with the operation type the mapping gives it or none (IMP-022); every import row; and the count of rows of each status (IMP-030).

### Import row (computed, never stored)

One line of the file once read through a mapping: its line number, its operation type, its date, its asset, its figures — the quantity, the unit price and the fees (the trade tax joined, IMP-034) of a purchase or a sale, the amount of a dividend, a deposit or a withdrawal, each that read — and its **status** — ready, rejected with its reason, unknown asset, already recorded, or not imported with its reason (IMP-030).

---

## Business Rules

### Reading the file (010–019)

**IMP-010 — A file is read for one account (backend)**: An import reads one file for one account the user picks. It is rejected with a specific error when the account is unknown, and when the file cannot be read, is empty, holds a header and no row, or is larger than 10 MB.

**IMP-011 — The encoding is detected (backend)**: A file whose bytes read as UTF-8 is read as UTF-8, its byte order mark dropped; any other file is read as Windows-1252, which reads every byte and reads an ISO-8859-1 file as written.

**IMP-012 — The separator is detected (backend)**: The separator is the one of `;`, tab and `,` that splits the header into more than one field and each of the first 20 lines after it that are not blank (IMP-016) into at least as many; when several do, the first in that order. When none does, the file is rejected with a specific error, unless the user picks one (IMP-018).

**IMP-012A — How a field is read (backend)**: A field between double quotes holds the separator as text, and two double quotes inside it stand for one; a field is read trimmed. A trailing separator at the end of a line adds no column.

**IMP-013 — The decimal mark is detected (backend)**: With `,` as separator the decimal mark is the dot. Otherwise it is the comma when a field of the file outside the header, trimmed, is a figure written with a comma: a sign or none, digits — their thousands separated by dots or spaces, or not separated —, a comma and one or more digits ("1.234,56", "-12,5"); the dot in any other case.

**IMP-013A — How a figure is read (backend)**: A figure is read with its sign, which is then ignored (IMP-031), its decimals after the decimal mark, spaces between its thousands and, with the decimal comma, dots between its thousands; it is held in micro-units, a seventh decimal rounded down (TRX-024). An empty field is no figure.

**IMP-014 — The date format is detected once the date column is known (backend)**: The date format is the first of day/month/year, year-month-day and month/day/year under which every field of the date column (IMP-021) reads as a real day; the parts of a date are separated by `/`, `-` or `.`, the year has four digits, and a time after the date is ignored. When none reads all of them, it is day/month/year, and the preview rejects each row whose date does not read (IMP-030).

**IMP-015 — The header names the columns (backend)**: The first line of the file is its header; each column is named by its header field, trimmed. A file whose header holds a name twice is rejected with a specific error.

**IMP-016 — Blank and short lines (backend)**: A line that holds no field once trimmed is skipped. A line with fewer fields than the header is rejected in the preview, its reason naming the line; a line's fields beyond the header's are ignored.

**IMP-017 — Reading and previewing write nothing (backend)**: Reading a file and previewing it write nothing: no transaction, asset, holding or mapping. The portfolio is unchanged until the user confirms (#502); a mapping is written only when saved (IMP-020).

**IMP-018 — The user's choice replaces the detected one (backend)**: An encoding, separator, decimal mark or date format the user picks replaces the detected one, and the file is read again with it.

### Column mapping (020–029)

**IMP-020 — A mapping is saved and reused (backend)**: The user saves the mapping in use under a name (ColumnMapping): its format, its columns, its operation types and its asset matches. A saved mapping picked for the next file reads it with its own separator, decimal mark, date format and encoding, none detected. The preview runs with the mapping in use, saved or not.

**IMP-021 — The columns a row needs (backend)**: A mapping names the column of each field a row reads, by its header, compared trimmed and case ignored: the date and the operation, always; the asset by one or more of its ISIN, its reference and its name, at least one; the quantity, the unit price, the gross amount, the fees, the net amount and the currency, each optional. A mapping without the date or the operation column, or without any asset column, is rejected with a specific error when saved or used; one naming a column the file lacks is rejected with a specific error when used on that file.

**IMP-022 — Each operation label has its operation type (backend)**: A mapping gives each operation label of the file, compared trimmed and case ignored, one operation type: purchase, sale, dividend, deposit, withdrawal, trade tax, or not imported — a corporate action this version leaves to record by hand. A row whose label the mapping in use gives no type is rejected with its label as reason, until the user gives it one; the type given is part of the mapping in use, kept when it is saved.

**IMP-023 — A mapping is synced (backend)**: A column mapping is a synced record (SYN-021), identified by its id (CFR-012): saved, renamed or deleted on one of the owner's computers, it is the same on the others, the later change winning (CFR-020), a deletion leaving a tombstone (SYN-024).

**IMP-024 — A mapping's name (backend)**: A name empty once trimmed is rejected with a specific error, and so is a name another mapping holds, case and accents ignored — a name being set, not one the mapping already holds (IMP-027). A name is stored trimmed.

**IMP-024A — A mapping's texts are bounded (backend)**: Every text of a mapping — its name, its column headers, its operation labels, its matched names, ISINs and references — is at most 500 characters, the longest text a device takes in from another (the holding note's cap, HNO-011); a longer one is rejected with a specific error.

**IMP-025 — A saved mapping is changed or renamed (backend)**: Saving a mapping that is already saved replaces what it holds, under its name or a new one, keeping its id. Saving a mapping that is no longer held is rejected with a specific error.

**IMP-026 — A change to the mappings is told (backend)**: A mapping saved, changed, renamed or deleted on this computer raises the `ColumnMappingUpdated` event; one changed by a sync is announced with the sync (SYN-064).

**IMP-027 — Two mappings of one name (backend)**: Two mappings that a merge leaves with the same name both survive, each with its id, and nothing is reported: a duplicate name is reported for accounts and categories (CFR-035), not for mappings (CFR-060). Either may be saved again under that name; the user renames one.

### Rows and the preview (030–039)

**IMP-030 — The preview judges every row (backend)**: The preview reads every row of the file through the mapping in use, in the file's order, and gives each its status: **ready**; **rejected**, with its reason; **unknown asset**, with the name, ISIN or reference read (IMP-032); **already recorded** (IMP-036); or **not imported**, with its reason (IMP-022). It also counts the rows of each status. A date or a figure that does not read is a rejection naming its column.

**IMP-031 — The figures of each type (backend)**: Signs are ignored: the type gives the direction. A **purchase** and a **sale** read the quantity, the unit price — or, when the row has no unit price, the gross amount divided by the quantity, rounded down to the micro-unit, a quantity of zero being rejected first (TRX-020) — and the fees, zero when the column is absent or empty; the file's net amount is not checked. A **dividend**, a **deposit** and a **withdrawal** read the net amount when the mapping names its column and the row's field is not empty, and the gross amount otherwise. A figure the type needs and the row lacks is a rejection naming it.

**IMP-032 — Each row is matched to an asset (backend)**: A row of a purchase, a sale, a dividend or a trade tax is matched to an asset of the portfolio, archived ones included: first by the mapping's asset matches (IMP-033), then by its own ISIN and then its reference, each trimmed and case ignored, then its exact name, case and accents ignored: each that the row holds is tried in that order, and the first matching exactly one asset gives it. A row that none matches to exactly one asset is an **unknown asset**. A deposit and a withdrawal are the account's cash and take no asset.

**IMP-033 — An unknown asset is matched once (backend)**: The user matches an unknown asset's name, ISIN or reference to an asset of the portfolio; the match is part of the mapping in use, kept when it is saved, so the next file of that broker matches it. A match is compared as the field it was read from is (IMP-032). Every row of that name, ISIN or reference then takes that asset, and the preview judges them again. A match to an asset unknown to the portfolio is rejected with a specific error when it is made. Saving a mapping keeps its matches as they are. A mapping refers to no other record, so a synced one is applied at once (SYN-041 does not hold it back): a match whose asset this device has not received yet leaves its rows unknown until the asset arrives. Creating the asset from the row instead comes with the confirmation (#502).

**IMP-034 — A trade tax joins its purchase (backend)**: A trade-tax row is added to the fees of the purchase in the file with the same asset — the same ISIN, reference and name as read —, on the same date, of the same quantity when the tax row has one; its amount is its net amount, or its gross amount. The tax row then shows as part of that purchase, not as a row of its own, and takes that purchase's status. When several purchases fit, the first in the file's order takes it; a purchase takes one trade tax at most. A trade-tax row with no such purchase is rejected — "no purchase of this asset, date and quantity in the file".

**IMP-035 — Rows follow the rules of manual entry (backend)**: The ready rows are judged in date order, the file's order within one day, as each would be judged typed by hand into the account in that order, after the ready rows before it: every rule of manual entry for its type applies — among them the date (TRX-020), the asset's currency being the account's (IMP-037), a sale of nothing held on its date (SEL-012), above what is held then (SEL-021) or leaving a later transaction short (SEL-032), a sale of an archived asset (SEL-037), a dividend of an asset not held as DIV-011 states it — held today, the ready rows included, until #688 makes it held on the dividend's date —, a withdrawal above the cash (CSH-031). A row these rules refuse is rejected with the reason manual entry gives. A row already recorded (IMP-036) is not judged again: it stands among the rows before the next ones as the transaction it matches. The cash a purchase needs is the one exception (IMP-038).

**IMP-036 — A row already recorded is shown so (backend)**: A row is **already recorded** when the account holds a transaction of the same type, date and asset, with the same quantity and the same total amount as manual entry computes it (TRX-026 for a purchase, its trade tax included; SEL-023 for a sale) — or, for a deposit, a withdrawal and a dividend, the same amount. Two such rows in the file match two such transactions, not one. Importing the same file twice therefore shows already recorded, the second time, every row the first import recorded. Already recorded is judged before the rules of manual entry (IMP-035).

**IMP-037 — One currency in this version (backend)**: A row whose asset's currency, or whose own currency when the file has one, is not the account's is rejected — "another currency: record it by hand". The exchange rate is therefore always 1.

**IMP-038 — A purchase the cash cannot fund (backend)**: A ready purchase the account's cash cannot fund, as CSH-041 judges it after the rows before it, is not rejected: it stays ready, and the confirmation adds the deposit it needs or leaves it out, by the user's choice (#502). The rows after it are judged as if that deposit were added.

### Deletion (040–049)

**IMP-040 — A mapping is deleted (backend)**: The user deletes a saved mapping; nothing already recorded changes. Deleting a mapping that is no longer held is rejected with a specific error.

---

## Workflow

```
pick an account and a file
        │
        ▼
read: encoding, separator, decimal mark detected (IMP-011 to IMP-013)
        │  — or a saved mapping's (IMP-020), or the user's choice (IMP-018)
        ▼
map the columns, the operation labels (IMP-021, IMP-022); the date format follows (IMP-014)
        │                                                   ── save the mapping (IMP-020)
        ▼
preview: each row ready / rejected / unknown asset / already recorded / not imported (IMP-030)
        │  match an unknown asset (IMP-033) → judged again
        ▼
confirm (#502)
```

---

## UX Draft

The screens are #503 (picking the file, mapping the columns) and #504 (the preview); their mocks come with those issues. This spec's rules are the core's.

---

## Cross-amendments

- SYN-021 lists column mappings among the synced records; CFR-012 gives their identity; SYN-038 adds version 10.
- SYN-014 counts a saved column mapping as user data: the user made it, and a rebuild would not replace it.
- SYN-064 lists `ColumnMappingUpdated` among the events raised by this device's own writes (IMP-026).
- CFR-060 does not report two column mappings left with the same name (IMP-027).

---

## Open Questions

- [x] Where does an asset come from when the file has no ISIN? — Its exact name, case and accents ignored; one matching none or two is an unknown asset, matched once and remembered by the mapping (owner, 2026-10-25).
- [x] Is a saved mapping synced? — Yes, as accounts and assets are, the later change winning (owner, 2026-10-25).
- [x] Where does a trade tax on its own row go? — Into the fees of the purchase of the same asset, date and quantity; one with no such purchase is rejected (owner, 2026-10-25).
- [x] Which rules does an imported row follow? — Those of a operation typed by hand; a dividend needs the asset held on its date, which manual entry does not check yet (#688) (owner, 2026-10-25).
