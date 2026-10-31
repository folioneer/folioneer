# Business Rules — Import (IMP)

## Context

Every transaction is typed by hand today, which stops anyone with years of history from starting (#369). Brokers export CSV files, and none agree on columns, separators, number and date formats or encoding. An **Import** brings a file's rows into one account in two steps: the file is read through an **Import mapper** and previewed row by row, nothing written; then the user confirms, and the rows are recorded at once (IMP-050).

This spec holds the reading, the import mapper and the preview (#501), the confirmation and the **Added deposit** (#502); the screens come with #503 and #504. The import is a surface beside the window, the command line and the AI agent connection: any of them may start it, and its rows are judged and recorded through the same recorder as theirs, by the same rules — every transaction follows them, whatever records it (owner, 2026-10-25): the purchase (TRX), the sale (SEL), the dividend (DIV), the deposit and the withdrawal (CSH). This version starts it from the window (#503). Figures follow TRX-024. No mapper ships with the application: the user saves or adds the first. The reading rules were checked against a French broker's securities export: one file, ISO-8859-1 (read as Windows-1252, IMP-011), `;` with a trailing `;`, a decimal dot, no ISIN column, the trade tax on its own row (owner, 2026-10-25).

---

## Entity Definition

### ImportMapper

How one provider's export is read, saved under that provider and reused for every file of that shape. It belongs to no account: one mapper serves every account the provider's files go into.

| Field             | Business meaning                                                                                                                                        |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `id`              | The mapper's own identity, set when it is first written: a change of provider keeps it.                                                                 |
| `provider`        | Who the export comes from ("Fortuneo PEA"), in the file's info entry: not empty once trimmed, unique among mappers, case and accents ignored (IMP-024). |
| `separator`       | The character between two fields: `;`, `,` or a tab.                                                                                                    |
| `decimal_mark`    | The mark before a figure's decimals: a dot or a comma.                                                                                                  |
| `date_format`     | The order a date is written in: day/month/year, month/day/year or year-month-day (IMP-014).                                                             |
| `encoding`        | How the file's text is written: UTF-8 or Windows-1252, which reads an ISO-8859-1 file too.                                                              |
| `columns`         | Which column of the file holds each field of a row (IMP-021), each named by its header.                                                                 |
| `operation_types` | Each operation label of the file and the operation type it stands for (IMP-022).                                                                        |
| `asset_matches`   | Each name, ISIN or reference of the file the user matched to an asset, and that asset (IMP-033).                                                        |

### Preview (computed, never stored)

What reading a file through a mapper shows, nothing written (IMP-017).

| Field    | Business meaning                                                                                                                                                                                  |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `file`   | How the file was read: its encoding, separator, decimal mark and date format, detected or chosen (IMP-011 to IMP-014, IMP-018), the date format possibly not detected (IMP-014), and its columns. |
| `rows`   | Every PreviewRow, in the file's order; a label the mapper gives no type shows on its rows, rejected with it (IMP-022).                                                                            |
| `counts` | How many rows hold each status; a trade tax joined to its purchase counts with it (IMP-030, IMP-034).                                                                                             |

### PreviewRow (computed, never stored)

One line of the file that is not blank, once read through a mapper and judged.

| Field       | Business meaning                                                                                                                                                                                                       |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `line`      | Its line number in the file, the header being line 1.                                                                                                                                                                  |
| `operation` | Its operation type, date, asset as read and figures — the quantity, unit price and fees (a joined trade tax included, IMP-034) of a purchase or a sale, the amount of a dividend, a deposit or a withdrawal (IMP-031). |
| `asset`     | The asset of the portfolio it was matched to, none for a deposit and a withdrawal (IMP-032).                                                                                                                           |
| `status`    | **Ready**, **rejected** with its reason, **unknown asset**, **already recorded** or **not imported** with its label (IMP-030); a joined trade tax names its purchase's line instead.                                   |
| `funding`   | For a ready purchase the cash cannot fund, the deposit it needs (IMP-038); none for any other row.                                                                                                                     |

---

## Business Rules

### Reading the file (010–019)

**IMP-010 — A file is read for one account (backend)**: An import reads one file for one account the user picks; only the file the user last picked is read. It is rejected with a specific error when the account is unknown, when the file is not the one last picked, and when the file cannot be read, is empty, holds a header and no row — blank lines (IMP-016) are no row —, or is larger than 10 MB.

**IMP-011 — The encoding is detected (backend)**: A file whose bytes read as UTF-8 is read as UTF-8, its byte order mark dropped; any other file is read as Windows-1252, which reads every byte and reads an ISO-8859-1 file as written. A file read as UTF-8 by the user's choice or a saved mapper's (IMP-018, IMP-020E) whose bytes are not UTF-8 is rejected with a specific error.

**IMP-012 — The separator is detected (backend)**: The separator is the one of `;`, tab and `,` that splits the header into more than one field and each of the first 20 lines after it that are not blank (IMP-016) into at least as many; when several do, the first in that order. When none does, the file is rejected with a specific error, unless the user picks one (IMP-018).

**IMP-012A — How a field is read (backend)**: A field between double quotes holds the separator as text, and two double quotes inside it stand for one; a field is read trimmed. Empty fields at the end of the header, a trailing separator's among them, add no column; a line keeps every field, its empty last one included.

**IMP-013 — The decimal mark is detected (backend)**: With `,` as separator the decimal mark is the dot. Otherwise it is the comma when a field of the file outside the header, trimmed, is a figure written with a comma: a sign or none, digits — their thousands separated by dots or spaces as IMP-013A groups them, or not separated —, a comma and one or more digits ("1.234,56", "-12,5"; ",5" tells nothing); the dot in any other case.

**IMP-013A — How a figure is read (backend)**: A figure is read with its sign, which is then ignored (IMP-031), its decimals after the decimal mark — a mark with no decimals after it reads as none —, spaces between its thousands — a space, a no-break space or a narrow no-break space — and, with the decimal comma, dots between its thousands; any other character, a comma under the decimal dot among them ("1,234.56"), makes it a figure that does not read, and so does a separator that does not sit between groups of thousands — one to three digits, then groups of three, exactly one separator between two groups ("12 34" does not read, nor does a figure with two spaces in a row); a figure may have no whole part (".5"), but a sign, a mark or both alone and a figure too large for micro-units do not read; the thousands of one figure may mix those separators; it is held in micro-units, a seventh decimal rounded down (TRX-024). An empty field is no figure.

**IMP-014 — The date format is detected once the date column is known (backend)**: The date format is the first of day/month/year, year-month-day and month/day/year under which every field of the date column (IMP-021) that is not empty, on the lines that are not blank, reads as a real day; the parts of a date are separated by `/`, `-` or `.`, the year has four digits, and a time after the date is ignored. When none reads all of them, or the column holds no date, it is day/month/year, said to be not detected, and the preview rejects each row whose date does not read (IMP-030).

**IMP-015 — The header names the columns (backend)**: The first line of the file is its header; each column is named by its header field, trimmed. A file whose header holds a name twice, compared trimmed and case ignored (IMP-021), is rejected with a specific error; an empty header field names no column and is never a name held twice.

**IMP-016 — Blank and short lines (backend)**: A line whose fields are all empty once trimmed — a line of separators only among them — is blank and skipped. A line with fewer fields than the header is rejected in the preview, its reason naming the line; a line's fields beyond the header's are ignored.

**IMP-017 — Reading and previewing write nothing (backend)**: Reading a file and previewing it write nothing: no transaction, asset, holding or mapper. The portfolio is unchanged until the user confirms (IMP-050), but for an asset the user creates (IMP-052); a mapper is written only when saved or added (IMP-020, IMP-020B).

**IMP-018 — The user's choice replaces the detected one (backend)**: An encoding, separator, decimal mark or date format the user picks replaces the detected one, or the saved mapper's, and the file is read again with it; the choice is part of the mapper in use, kept when it is saved.

### Import mapper (020–029)

**IMP-020 — A mapper is a file (backend)**: An import mapper is a JSON file in the mappers folder of the data folder, on this computer: its info entry (the provider), its format, its columns, its operation types and its asset matches. Saving a new mapper writes its file, under a file name of its own: it never replaces a file of the folder. A write that fails is rejected with a specific error and leaves the folder's files as they were.

**IMP-020A — The list is what the folder holds (backend)**: The saved mappers are the `.json` files the folder holds, read each time the list is asked for, sorted by provider, case and accents ignored, then by identity; no folder, no mapper. A file that cannot be read, larger than 1 MB, not JSON, holding a field a mapper does not have, lacking one it needs or with a value outside the ones allowed, or breaking IMP-021, the empty provider of IMP-024 or IMP-024A does not read as a mapper: it is left out of the list and left in place, and the failure is logged. A folder that cannot be read is rejected with a specific error.

**IMP-020B — A mapper from elsewhere is added from its file (backend)**: The user picks a mapper's file in the system dialog; it is written into the folder as IMP-020 says, whatever the picked file was called, its provider trimmed, and joins the list. A file that cannot be read or does not read as a mapper (IMP-020A), or whose trimmed provider another mapper holds (IMP-024), is refused with a specific error saying why, and nothing is written; a write that fails is rejected as in IMP-020. Its asset matches are kept as they are (IMP-033).

**IMP-020C — The mapper chosen is remembered for its folder (backend)**: A completed confirmation made with a saved mapper remembers that mapper for the folder the file sits in, replacing what that folder remembered, on this computer and never synced — whether or not the user changed the mapper in use or any row was recorded. If remembering fails the import stands, and the failure is logged.

**IMP-020D — A folder's mapper is preselected (frontend + backend)**: A file picked from a folder has the mapper it remembers preselected; the user may pick another. A folder that remembers nothing, a mapper no longer held or that no longer reads, or a memory or folder that cannot be read, preselects none, and a failure is logged.

**IMP-020E — A saved mapper is reused (backend)**: A saved mapper picked for the next file reads it with its own separator, decimal mark, date format and encoding, none detected. The preview runs with the mapper in use, saved or not. Picking a mapper that is no longer held or no longer reads (IMP-020A) is rejected with a specific error.

**IMP-021 — The columns a row needs (backend)**: A mapper names the column of each field a row reads, by its header, compared trimmed and case ignored: the date and the operation, always; the asset by one or more of its ISIN, its reference and its name, at least one; the quantity, the unit price, the gross amount, the fees, the net amount and the currency, each optional. One column may serve two fields. A mapper without the date or the operation column, or without any asset column, is rejected with a specific error when saved or used; one naming a column the file lacks is rejected with a specific error when used on that file.

**IMP-022 — Each operation label has its operation type (backend)**: A mapper gives each operation label of the file, compared trimmed and case ignored, one operation type: purchase, sale, dividend, deposit, withdrawal, trade tax, or not imported — a corporate action this version leaves to record by hand. A row whose label the mapper in use gives no type is rejected with its label as reason, until the user gives it one; the type given is part of the mapper in use, kept when it is saved. A mapper's file giving one label two types reads with the first.

**IMP-024 — A mapper's provider (backend)**: A provider empty once trimmed is rejected with a specific error, and so is a provider another mapper holds, case and accents ignored — a provider being set, not one the mapper already holds (IMP-027). A provider is stored trimmed.

**IMP-024A — A mapper's texts are bounded (backend)**: Every text of a mapper — its provider, its column headers, its operation labels, its matched names, ISINs and references — is at most 500 characters; a longer one is rejected with a specific error.

**IMP-025 — A saved mapper is changed (backend)**: Saving a mapper that is already saved replaces its file, under its provider or a new one, keeping its identity. Saving over a mapper that is no longer held or no longer reads (IMP-020A) is rejected with a specific error.

**IMP-027 — Two mappers of one provider (backend)**: Files copied into the folder by hand may share a provider: both are listed (IMP-020A), each with its identity, and nothing is reported. Either may be saved again under that provider; the user renames one.

### Rows and the preview (030–039)

**IMP-030 — The preview judges every row (backend)**: The preview reads every row of the file through the mapper in use, in the file's order, and gives each its status: **ready**; **rejected**, with its reason; **unknown asset**, with the name, ISIN or reference read (IMP-032); **already recorded** (IMP-036); **not imported**, with its label (IMP-022); or, for a trade tax joined to its purchase, that purchase's line (IMP-034). It also counts the rows of each status. A date or a figure that does not read is a rejection naming its column.

**IMP-031 — The figures of each type (backend)**: Signs are ignored: the type gives the direction. A **purchase** and a **sale** read the quantity, the unit price — or, when the row has no unit price, the gross amount divided by the quantity, rounded down to the micro-unit, a quantity of zero being rejected first (TRX-020) — and the fees, zero when the column is absent or empty; the file's net amount is not checked. A **dividend**, a **deposit** and a **withdrawal** read the net amount when the mapper names its column and the row's field is not empty, and the gross amount otherwise. A figure the type needs and the row lacks is a rejection naming it.

**IMP-032 — Each row is matched to an asset (backend)**: A row of a purchase, a sale, a dividend or a trade tax is matched to an asset of the portfolio, archived ones included: first by the mapper's asset matches (IMP-033, IMP-033A), then by its own ISIN and then its reference, each trimmed and case ignored, then its exact name, case and accents ignored: each that the row holds is tried in that order, and the first matching exactly one asset gives it. A row that none matches to exactly one asset is an **unknown asset**. A deposit and a withdrawal are the account's cash and take no asset.

**IMP-033 — An unknown asset is matched once (backend)**: The user matches an unknown asset's name, ISIN or reference to an asset of the portfolio; the match is part of the mapper in use, kept when it is saved, so the next file of that provider matches it. A match is compared as the field it was read from is (IMP-032). Every row of that name, ISIN or reference then takes that asset, and the preview judges them again. A match to an asset unknown to the portfolio is rejected with a specific error when it is made. Saving a mapper keeps its matches as they are. The user may create the asset from the row instead (IMP-052).

**IMP-033A — A match whose asset is not held is passed over (backend)**: A saved match whose asset the portfolio does not hold, as in a mapper written elsewhere, is passed over: the next matches of IMP-032 apply, and the row is unknown if none does. Of two matches for one text, the first whose asset is held applies.

**IMP-034 — A trade tax joins its purchase (backend)**: A trade-tax row is added to the fees of the purchase in the file with the same asset — the same ISIN, reference and name as read —, on the same date, of the same quantity when the tax row has one; its amount is its net amount, or its gross amount. The tax row keeps its own line in the preview, its status naming the purchase's line, and is counted and recorded with that purchase. When several purchases fit, the first in the file's order not yet holding one takes it: a purchase takes one trade tax at most, and a second tax row of the same asset, date and quantity takes the next such purchase. A trade-tax row with no such purchase is rejected — "no purchase of this asset, date and quantity in the file".

**IMP-035 — Rows follow the rules of manual entry (backend)**: The rows are judged in the order IMP-035A gives them, each as it would be typed by hand into the account in that order, after the ready rows before it: every rule of manual entry for its type applies — among them the date (TRX-020), the asset's currency being the account's (IMP-037), a sale of nothing held on its date (SEL-012), above what is held then (SEL-021) or leaving a later transaction short (SEL-032), a sale of an archived asset (SEL-037), a dividend of an asset not held as DIV-011 states it — held today, the ready rows included, until #688 makes it held on the dividend's date —, a withdrawal above the cash (CSH-031). A row these rules refuse is rejected with the reason manual entry gives. A row already recorded (IMP-036) is not judged again: it stands among the rows before the next ones as the transaction it matches. The cash a purchase needs is the one exception (IMP-038).

**IMP-035A — The order of a day's rows (backend)**: The rows are judged in date order and, within one day, the inflows first — the deposits and the dividends —, then the other rows, each in the file's order. A day's own deposit therefore funds that day's purchases, and a deposit is added only for what it leaves uncovered (IMP-038). A sale is no inflow here: a purchase written before a same-day sale that would fund it still needs its deposit. A dividend on the day of a first purchase comes before it, and is rejected as DIV-011 states.

**IMP-036 — A row already recorded is shown so (backend)**: A row is **already recorded** when the account holds a transaction of the same type, date and asset, with the same quantity and the same total amount as manual entry computes it (TRX-026 for a purchase, its trade tax included; SEL-023 for a sale) — or, for a deposit, a withdrawal and a dividend, the same amount. Two such rows in the file match two such transactions, not one. Importing the same file twice therefore shows already recorded, the second time, every row the first import recorded. Already recorded is judged after the currency (IMP-037), which tells two amounts apart, and before the other rules of manual entry (IMP-035).

**IMP-036A — A later file's deposit row and an added deposit (backend)**: An added deposit (IMP-051) is a deposit like any other for IMP-036: a later file's deposit row of its date and amount is already recorded, so the cash is not counted twice, and the deposit stays marked as added.

**IMP-037 — One currency in this version (backend)**: A row whose asset's currency, or whose own currency when the file has one, is not the account's is rejected — "another currency: record it by hand". The exchange rate is therefore always 1.

**IMP-038 — A purchase the cash cannot fund (backend)**: A ready purchase the account's cash cannot fund, as CSH-041 judges it after the rows before it, is not rejected: it stays ready, the preview showing the smallest deposit on its date that funds it, and the confirmation adds the deposit it needs or leaves it out, by the user's choice (IMP-051). The rows after it are judged as if that deposit were added.

### Deletion (040–049)

**IMP-040 — A mapper is deleted (backend)**: The user deletes a saved mapper; nothing already recorded changes. Deleting a mapper that is no longer held, or a file that cannot be removed, is rejected with a specific error.

### Confirmation (050–059)

**IMP-050 — The confirmed import records every ready row at once (backend)**: On confirmation the file is read and its rows judged again, as the preview judges them (IMP-030 to IMP-038), against the account as it is then. Every ready row is recorded, in the order IMP-035A gives them, in one write with the added deposits (IMP-051) and the Cash Asset (IMP-050A): all of them or, when the write fails, none. The rows rejected, of an unknown asset, already recorded or not imported are left out. The confirmation reports the rows as judged then; the ready ones are those recorded.

**IMP-050A — What a confirmation announces (backend)**: The import raises one `TransactionUpdated`, none when nothing is recorded, and `AssetUpdated` when it creates the Cash Asset of the account's currency, ensured whenever a row is recorded (CSH-010). A write that fails announces nothing.

**IMP-051 — An added deposit funds a purchase (backend)**: Unless they are left out (IMP-051B), each ready purchase the cash cannot fund (IMP-038) is recorded after the deposit IMP-038 gives it as judged at the confirmation, on the purchase's date: an **Added deposit**, marked as added.

**IMP-051A — The added-deposit mark stays with the deposit (backend)**: The added-deposit mark is part of the deposit: it is kept when the deposit is corrected and synced with it.

**IMP-051B — The added deposits can be left out (frontend + backend)**: The added deposits are on by default; the user may leave them out for this import: such a purchase is then judged without one and rejected as manual entry rejects it (CSH-041).

**IMP-052 — An unknown asset is created from its row (frontend)**: From an unknown-asset row the user opens the usual asset form, prefilled with what the row read — its name, ISIN and reference — and the account's currency; its kind, class and category are chosen there, every field stays editable, and the form's refusals hold (AST-001, AST-031, AST-032).

**IMP-052A — The asset created takes its row (frontend + backend)**: The window asks for the asset created from a row to be matched (IMP-033) to the first the row read of its ISIN, its reference and its name, whatever was edited in the form, and the preview judges the rows again (IMP-032); created in another currency, they are rejected (IMP-037).

**IMP-053 — The same file imported twice records nothing new (backend)**: Importing a file again shows already recorded every row the first import recorded (IMP-036), so nothing new is recorded and nothing is announced. An added deposit is no row of the file: its purchase being already recorded, none is added again.

**IMP-054 — The journal shows an added deposit (frontend)**: On every device, an added deposit's row in the account journal shows an "Added deposit" chip in the Recorded by column, styled as the AI agent's chip, with the hover hint "Added by an import to fund a purchase" (TXL-061).

---

## Workflow

```
pick an account and a file
        │
        ▼
read: encoding, separator, decimal mark detected (IMP-011 to IMP-013)
        │  — or a saved mapper's (IMP-020E, IMP-020D), or the user's choice (IMP-018)
        ▼
map the columns, the operation labels (IMP-021, IMP-022); the date format follows (IMP-014)
        │                                                   ── save the mapper (IMP-020)
        ▼
preview: each row ready / rejected / unknown asset / already recorded / not imported (IMP-030)
        │  match an unknown asset (IMP-033) or create it (IMP-052) → judged again
        ▼
confirm: the ready rows recorded at once (IMP-050), the added deposits by default (IMP-051)
```

---

## UX Draft

The screens are #503 (picking the file, mapping the columns) and #504 (the preview); their mocks come with those issues. This spec's rules are the core's, but for leaving the added deposits out (IMP-051B), the preselected mapper (IMP-020D), the prefilled asset form (IMP-052, IMP-052A) and the journal's chip (IMP-054).

---

## Cross-amendments

- SYN-038 adds version 12: the column mappings of version 10 are no longer a kind of synced record; no device ever held one, since no window offered it. A mapper's file is not synced for now (#715).
- SYN-038 adds version 11: a transaction carries the added-deposit mark (IMP-051).
- CSH: a deposit may be an added deposit, marked as such (IMP-051).
- TXL-060: a journal row carries the added-deposit mark (IMP-051). TXL-061: its Recorded by column shows the chip (IMP-054). AGT-045: that chip is not an AI agent's mark; an added deposit has no session.

---

## Open Questions

- [x] Where does an asset come from when the file has no ISIN? — Its exact name, case and accents ignored; one matching none or two is an unknown asset, matched once and remembered by the mapper (owner, 2026-10-25).
- [x] When is the mapper chosen remembered? — At the confirmation, not at the choice, so a file only previewed changes nothing (IMP-017) (agent's reading of the owner's 2026-10-31 decision).
- [x] Where does a saved mapper live? — In a JSON file in the mappers folder of the data folder, never the database, so there is one source of truth and the community can add its own; its sync is #715 (owner, 2026-10-31).
- [x] Where does a trade tax on its own row go? — Into the fees of the purchase of the same asset, date and quantity; one with no such purchase is rejected (owner, 2026-10-25).
- [x] Within one day, are inflows judged before purchases? — Yes: deposits and dividends first, so a day's own deposit funds its purchases (owner, 2026-10-26).
- [x] Can a later file's deposit row match an added deposit? — Yes, by date and amount, and the mark stays (owner, 2026-10-26).
- [x] How does the journal show an added deposit? — A chip with a hover hint in Recorded by, styled as the AI agent's chip (owner, validated with #504's mock).
- [x] What is the asset created from a row matched to? — The strongest the row read, so a row of the same name and another ISIN does not take it; whatever was edited in the form, since the match is what the file wrote (agent, 2026-10-31; #504 builds it).
- [x] Which rules does an imported row follow? — Those of an operation typed by hand; a dividend needs the asset held on its date, which manual entry does not check yet (#688) (owner, 2026-10-25).
