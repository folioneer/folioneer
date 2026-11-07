# Business Rules — Data export (EXP)

## Context

A user can always take their data elsewhere (#392, #509): every account, every asset, every transaction of every type and every recorded asset price leave Folioneer in a documented format, as CSV — a folder of files — or as JSON — one file. The format is written for someone reading it without Folioneer, and for the account import (IMP) to read back: an account's transactions of the five types the import reads come back to the same figures (EXP-050). Teaching the import the other types is its own issue.

The export is local: it writes where the owner chooses on this computer and sends nothing anywhere. It is where the backup of #508 will sit later.

---

## Computed Concepts

The export persists nothing: what it writes is computed from the portfolio when asked.

### Export

| Concept      | Business meaning                                                                                                            |
| ------------ | --------------------------------------------------------------------------------------------------------------------------- |
| `format`     | CSV — a folder of files (EXP-011) — or JSON — one file (EXP-012).                                                           |
| `written_to` | The full path of the folder or the file written, in the folder the owner chose; the owner is shown its last part (EXP-016). |
| `counts`     | How many accounts, assets, transactions and prices it holds (EXP-016).                                                      |

---

## Business Rules

### The action (010–019)

**EXP-010 — Where the export is asked (frontend)**: Settings holds a section "Your data", the last in its list. It offers a format — CSV or JSON, CSV chosen when the section opens, the choice not remembered — and an action "Export…" that opens the system's folder picker. Choosing a folder exports there; closing the picker exports nothing.

**EXP-011 — The CSV folder (backend)**: An export in CSV writes a folder named `folioneer-export-{date}` holding `accounts.csv`, `assets.csv`, `transactions.csv` and `prices.csv`; `format.json`, holding `format`, `version` and `exported_at` as the JSON file does (EXP-032), without the tables; one `transactions-{account}.csv` per account holding a transaction, with exactly that account's rows of `transactions.csv`, in their order (EXP-033); and `import-mapper.json`, an import mapper that reads those per-account files (EXP-051) (owner, 2026-11-07). `{account}` is the account's name, its first 64 characters, with each of `< > : " / \ | ? *` and every control character replaced by `_`, so the folder stays valid on any system; when two names give one file name, case ignored, each of them becomes `transactions-{account}-{id}.csv`.

**EXP-012 — The JSON file (backend)**: An export in JSON writes one file `folioneer-export-{date}.json` holding the four tables (EXP-032).

**EXP-013 — Its name (backend)**: `{date}` is today on this computer, `YYYY-MM-DD`.

**EXP-014 — Never over what is there (backend)**: When the name is taken in the chosen folder, ` (2)`, ` (3)`… is added before a file's extension or at the end of a folder's name, the first free one: an export never overwrites anything.

**EXP-015 — Complete or nothing (backend)**: What an export writes appears in the chosen folder complete or not at all. A folder that is not there or is not a folder is refused with a specific error; a write that fails — a folder that cannot be written, a full disk — leaves nothing of the export behind and is refused with another.

**EXP-016 — What the owner is told (frontend)**: While an export is written, the action shows it is working and cannot be started again. After an export, one line under the action says where it went and what it holds: the folder or file name, then the number of accounts, assets, transactions and prices. A failed export says that nothing was written, with its reason.

### What is exported (020–029)

**EXP-020 — What an export holds (backend)**: An export holds every account; every asset, the archived ones included — the Cash Assets excepted, each being the cash of its currency, not an asset the owner holds; every transaction of every account and every type; and every recorded price of every asset. Nothing is filtered by date, account or archive state.

**EXP-021 — What it leaves out (backend)**: An export holds no fee schedule (FEE), no holding note (HNO), no exchange rate (FXR), no setting — the portfolio's country and reference currency among them —, no import mapper of the owner's, no AI agent's mark on a recording (AGT-045), no reason this computer's last fetch gave for an unpriced asset (MKT-220) and nothing of sync. An asset's category is written by its name, not as a table.

**EXP-022 — Every link holds (backend)**: Every transaction an export holds names an account it holds and, but for the Cash Holding's, an asset it holds; a transfer's two sides and its fee in kind are in it, or none of them (TRF-020, TRF-029); each per-account file holds exactly its account's rows of `transactions.csv`; and the counts told are those of what was written. A transaction recorded or removed while the export is written — by hand, by the scheduled fetch, by an AI agent or by sync — is in it whole or not at all.

**EXP-023 — Figures as text (backend)**: Every figure is written as decimal text with a dot and no grouping mark, with every decimal it was recorded with and no trailing zero: a quantity of a non-cash asset to eight decimals at most (TRX-024), any other figure to six. A negative figure starts with `-`. A missing figure is an empty field in CSV, `null` in JSON. Dates are `YYYY-MM-DD`. The moment a transaction was entered is an ISO 8601 date and time in UTC, empty for one entered before that moment was kept (its unknown moment, TRX-036).

**EXP-024 — A transaction names what it touches (backend)**: Each transaction row carries its account's name and currency, and its asset's name, reference and ISIN beside the asset's identifier, so that a row reads on its own. The Cash Holding's transactions — a deposit, a withdrawal, interest on the cash — leave the asset's identifier, name, reference and ISIN empty.

**EXP-025 — Text as it is (backend)**: A text is written exactly as it was recorded, in CSV as in JSON: a line break stays, and a text starting with `=`, `+`, `-` or `@` is not guarded against a spreadsheet reading it as a formula (owner, 2026-11-07).

### The format (030–039)

**EXP-030 — CSV (backend)**: A CSV file follows the CSV standard (RFC 4180; owner, 2026-11-07): UTF-8 without a byte-order mark, fields separated by a comma, a field holding a comma, a quote or a line break quoted with `"` and its quotes doubled, every line — the last included — ended by CRLF, a header naming each column on the first line, in the order EXP-031 gives. A table with no row is its header alone.

**EXP-031 — The columns (backend)**: The tables, their columns in this order and what each holds:

| Table          | Columns                                                                                                                                                                                                                                                                                                                                                                        |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `accounts`     | `id`, `name`, `bank`, `currency` (ISO 4217), `country` (ISO 3166-1 alpha-2, empty when none), `wrapper` (its identifier, ACC-040, empty when none), `management_fees_enabled`                                                                                                                                                                                                  |
| `assets`       | `id`, `name`, `reference`, `isin` (empty when none), `kind` (`Listed`, `Crypto`, `Custom`), `class` (`Bonds`, `RealEstate`, `MutualFunds`, `ETF`, `ETP`, `Stocks`, `DigitalAsset`, `Derivatives`), `currency`, `category` (its name), `risk_level` (1 to 5), `exchange` (its ISO 10383 code, empty when none), `interest_bearing`, `price_typed_by_hand` (MKT-150), `archived` |
| `transactions` | `id`, `account_id`, `account_name`, `account_currency`, `date`, `type`, `asset_id`, `asset_name`, `asset_reference`, `asset_isin`, `quantity`, `unit_price`, `exchange_rate`, `fees`, `total_amount`, `realized_pnl`, `split_factor`, `dividend_amount`, `dividend_currency`, `write_off_reason`, `transfer_id`, `added_deposit`, `note`, `entered_at`                         |
| `prices`       | `asset_id`, `asset_reference`, `date`, `price`, `currency` (the asset's), `source` (`Manual`, `YahooFinance`, `Agent`)                                                                                                                                                                                                                                                         |

A transaction's `type` is one of `Purchase`, `Sell`, `OpeningBalance`, `Deposit`, `Withdrawal`, `Dividend`, `FreeShares`, `ManagementFee`, `Split`, `Interest`, `WriteOff`, `TransferOut`, `TransferIn`. `unit_price` is in the asset's currency and `exchange_rate` converts the asset's currency to the account's, but for an opening balance and a transfer's sides, whose unit price is in the account's currency (TRX-047, TRF-022), a transfer in's rate converting the sending account's currency to its own; `fees`, `total_amount` and `realized_pnl` are in the account's currency. On a split and on a dividend `quantity` is 0, their figures being `split_factor` — a split's new shares per old one (SPL-010) — and `dividend_amount` — a dividend's net amount (DIV-023) in `dividend_currency`, the currency it was entered in (DIV-029, DIV-043). `write_off_reason` is one of `NetworkFee`, `Dust`, `Lost`, `Other`. A flag — `management_fees_enabled`, `interest_bearing`, `archived`, `added_deposit` — is `true` or `false`. Each type fills its figures so: a purchase and a sale `quantity`, `unit_price`, `exchange_rate`, `fees` and `total_amount`, a sale `realized_pnl` too; an opening balance `quantity`, `unit_price` and `total_amount`, its cost; a deposit and a withdrawal `quantity` and `total_amount`, its amount; a dividend `quantity` (0), `dividend_amount`, `dividend_currency`, `exchange_rate` and `total_amount`, what the account was credited; free shares and interest on a holding `quantity`, at no cost; interest on the cash `quantity`, its amount; a management fee `quantity`, the units removed; a split `quantity` (0) and `split_factor`; a write-off `quantity`, `total_amount`, its cost, `realized_pnl` and `write_off_reason`, and `transfer_id` when it is a transfer's fee in kind (TRF-021); a transfer's sides `quantity`, `unit_price`, `exchange_rate`, `total_amount` and `transfer_id`. A figure a type does not fill is empty.

**EXP-032 — JSON (backend)**: The JSON file is one object: `format` (`"folioneer-export"`), `version` (`1`), `exported_at` (ISO 8601, UTC), then `accounts`, `assets`, `transactions` and `prices`, each an array of objects whose keys are EXP-031's columns, in the same order: a figure is a string (EXP-023), a flag a boolean, `risk_level` a number, an empty field `null`. A change to the format raises `version`.

**EXP-033 — The order of the rows (backend)**: Accounts and assets by name, then identifier; transactions by account name, then account identifier, then date, then the moment entered — an unknown moment first —, then identifier, the order the replay follows (TRX-036, CFR-041); prices by asset reference, then asset identifier, then date. Names and references compare by their characters, as written.

### Back in through the import (050–059)

**EXP-050 — The round trip (backend)**: An account's `transactions-{account}.csv`, read through `import-mapper.json` into a new account of the same currency, in a portfolio where the exported assets exist, gives that account the same holdings, the same cash and the same realized P&L as the original, when no holding of it is inconsistent (CFR-042), every one of its transactions is a purchase, a sale, a dividend, a deposit or a withdrawal; every asset it touches is in its currency (IMP-037), told apart by its ISIN or its reference (IMP-032) and, if it sells it, not archived (SEL-037); every quantity has six decimals at most (IMP-013A); every purchase's and sale's `total_amount` is what its quantity, unit price, a rate of 1 and its fees give (TRX-026, SEL-023, IMP-037); every purchase is funded by the cash the rows before it leave, in IMP-035A's order (IMP-038); no dividend is dated on a day its asset was not held the day before (IMP-035A); and no field of its rows holds a line break, which the import does not read inside a quoted field yet (#782). An account's file over 10 MB is beyond the import (IMP-010).

**EXP-051 — The mapper written beside it (backend)**: `import-mapper.json` is an import mapper (IMP-020) whose provider is "Folioneer export {date}": comma separator, dot decimal mark, year-month-day dates, UTF-8; its columns `date` for the date, `type` for the operation, `asset_isin`, `asset_reference` and `asset_name` for the asset, `quantity`, `unit_price`, `fees`, `total_amount` for the net amount and `account_currency` for the currency; and its operation types `Purchase` → purchase, `Sell` → sale, `Dividend` → dividend, `Deposit` → deposit, `Withdrawal` → withdrawal, every other type → not imported. It holds no asset match. The owner adds it to the import from its file (IMP-020B); a second export of the same day writes the same mapper, which the import then holds already — the round trip reads through that held mapper as written, not as the owner may have changed it since.

---

## Workflow

```
Settings → Your data
  → format: CSV | JSON                                               (EXP-010)
  → Export… → system folder picker
      → closed: nothing                                              (EXP-010)
      → folder chosen → the action works, cannot be started again    (EXP-016)
          → CSV: folioneer-export-2026-11-07/ — the four tables, format.json, one file per account, import-mapper.json (EXP-011)
          → JSON: folioneer-export-2026-11-07.json                   (EXP-012)
          → done: "folioneer-export-2026-11-07 — 3 accounts, 12 assets, 418 transactions, 2 960 prices" (EXP-016)
          → failed: nothing written, the reason                      (EXP-015, EXP-016)
```

---

## UX Draft

### Entry Point

Settings → "Your data", the last section (EXP-010), validated on #509 (folioneer/folioneer-design, `509/`).

### Main Component

A Format choice (CSV, JSON) and an "Export…" button.

### States

- **Idle**: the format and the button.
- **Working**: the button shows it is working and cannot be pressed (EXP-016).
- **Done**: one line naming where the export went and what it holds.
- **Failed**: one line saying nothing was written, with its reason.

### User Flow

1. The owner opens Settings → Your data, keeps CSV or picks JSON, presses Export….
2. The system's folder picker opens; the owner picks a folder.
3. The line under the button names the export and its counts.

---

## Open Questions

None — the scope, the round trip, the per-account files and the CSV standard were settled on #509 (owner, 2026-11-07).
