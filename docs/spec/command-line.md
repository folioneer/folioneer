# Business Rules — Command Line (CLI)

## Context

The installed program records holdings from a terminal or a script without opening a window (#044): an opening balance of an existing asset in an existing account, then a purchase or a sale. It starts the way the scheduled price download does (SPF-020), calls the same core as the window, and is checked by the same rules. All its text is English.

## Entity Definition

No new entity and no Tauri command, so no contract: a command records a `Transaction` as the TRX Entity Definition describes, through the use case the window calls.

---

## Business Rules

### Commands (010–019)

**CLI-010 — Commands (backend)**: `folioneer holding open` records an opening balance, `folioneer holding buy` a purchase, `folioneer holding sell` a sale. Only a first argument of `holding` starts a command; the program's other starts (no argument, `--scheduled-fetch`) are unchanged, and any other first argument opens the window as before.

**CLI-013 — Options (backend)**: Every command takes `--account` and `--asset` (CLI-011), `--quantity`, and optionally `--date` and `--json` (CLI-021). An opening balance takes `--total-cost`, in the account's currency (TRX-047). A purchase or a sale takes either `--price` — the unit price in the asset's currency (TRX-021) — or `--total` — the broker's all-in amount in the account's currency (TRX-060, SEL-050), never both; and optionally `--fees` in the account's currency, `--rate` — the exchange rate from the asset's currency to the account's (TRX-021) — and `--note`.

**CLI-014 — Defaults (backend)**: `--date` defaults to today on the computer running the command; `--fees` defaults to 0 and `--rate` to 1.

**CLI-015 — Numbers and dates (backend)**: Quantities and amounts are decimals with a dot and at most six decimals; a date is `YYYY-MM-DD`. Anything else is a usage error (CLI-022). A leading minus is read as written, so the recording rules refuse a negative figure with their own reason (CLI-012).

**CLI-016 — Help (backend)**: `--help` or `-h` anywhere after `holding` (`folioneer holding --help`, `folioneer holding buy -h`) prints how to use the commands on standard output and exits 0.

**CLI-011 — Account and asset by what the user typed (backend)**: `--account` names an account by its name; `--asset` names an asset by its name or its reference; case is ignored for every letter. A Cash Asset is never matched: cash is moved by deposits and withdrawals (CSH-018, TRX-064). A name that matches no account is refused with `AccountNotFound`, one that two accounts share (possible after a merge, CFR-035) with `AccountAmbiguous`; a name that matches no asset with `AssetNotFound`, one that matches more than one with `AssetAmbiguous`. A refusal names only what was typed: it never lists the existing accounts or assets.

**CLI-012 — The window's rules (backend)**: A command records through the same checks as the window, and is refused with the rejection recording makes: an opening balance as TRX-044 to TRX-048, TRX-050 (archived asset) and CSH-061 (a second line behind CLI-011, which never matches a Cash Asset); a purchase as TRX-020, TRX-060 and CSH-041 (cash short); a sale as SEL-020, SEL-021 (oversell), SEL-012 (closed position), SEL-037 (archived asset) and SEL-050.

**CLI-017 — A purchase of an archived asset (backend)**: A purchase of an archived asset is recorded and brings the asset back from the archive (TRX-028), without the confirmation the window asks for (TRX-029).

### Output (020–029)

**CLI-020 — Text output (backend)**: By default a recorded transaction prints one line starting `Recorded:` — what was recorded, the account, the date, and its total in the account's currency — on standard output; a refusal prints one line starting `Refused:` with the reason on standard error.

**CLI-021 — JSON output (backend)**: With `--json`, a command that ran prints one JSON object on standard output: `{"status":"recorded","transaction":{…}}` with the recorded transaction — its figures in micro-units, as the TRX Entity Definition stores them (TRX-024) — or `{"status":"refused","code":…,"message":…}` with the code of the refusal: the code of the rejection recording makes, or `AccountNotFound`, `AccountAmbiguous`, `AssetNotFound`, `AssetAmbiguous`, `NoPortfolio`, `WindowOpen`, `DatabaseError`. A usage error is never JSON (CLI-022).

**CLI-022 — Exit codes (backend)**: A command exits with 0 when it recorded, 1 when it was refused (a rule, something not found or ambiguous, no portfolio, the window open, the data unreachable), and 2 when the command line itself is wrong (unknown command or option, a missing, repeated or unreadable value), printing the reason and how to use the commands on standard error.

### Running beside the window (030–039)

**CLI-030 — The window owns the portfolio (backend)**: While the application window runs it holds a lock on a file in its data folder; the operating system releases it when the window closes or stops. A command takes the same lock and holds it until it has written: when a window holds it, the command refuses with `WindowOpen` and writes nothing; while a command holds it, a second command refuses the same way, and a window starting meanwhile takes the lock as soon as the command releases it. A lock file replaced by a symbolic link is not followed; a lock that cannot be taken for such a reason is refused as the data unreachable (`DatabaseError`).

**CLI-031 — No window, changes shared (backend)**: A command opens no window. What it records is recorded for multi-device sync like any change (SYN-020) and published by the next sync of the application. The scheduled price download (SPF-020) takes no lock; a command and a download writing at once are serialised by the database.

**CLI-032 — An existing portfolio only (backend)**: A command works on the portfolio of the user running it; where none has been created yet — the application never opened on this computer, or the Linux program run inside WSL — it refuses with `NoPortfolio` and creates nothing. A user without a data folder at all is refused as the data unreachable (`DatabaseError`).

**CLI-033 — No network, nothing listening (backend)**: A command makes no network request and listens on nothing: it reads its arguments, writes the portfolio, prints, and exits.

---

## Open Questions

None.
