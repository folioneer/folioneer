# Business Rules — Return of Capital (ROC)

## Context

A company sometimes pays its shareholders back part of what they invested: a return of capital. It is no income: cash comes in, the shares held stay the same, and what the holding cost falls by what was paid back (owner, 2026-10-18, #387). Recorded as a dividend it would inflate income and leave the cost basis above the broker's.

The application computes no tax (owner, 2026-11-14): it applies one convention to its own figures and keeps the raw values from which each country's treatment can be computed (owner, 2026-11-15, #553; #845). The core is #848; the window, then the agent connection, then the command line follow (owner, 2026-10-18).

Figures follow TRX-024. Every amount is in the account's currency unless said otherwise.

---

## Entity Definition

### Return of capital (new `TransactionType` variant)

| Field               | Business meaning                                                                                                                 |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `transaction_type`  | The return of capital variant of `TransactionType`.                                                                              |
| `account_id`        | The account holding the asset.                                                                                                   |
| `asset_id`          | The asset that paid it back. **Not** a Cash Asset.                                                                               |
| `date`              | The day it was paid (TRX-020).                                                                                                   |
| `received_amount`   | The total received, as entered: in the asset's currency, or in the account's (ROC-010).                                          |
| `received_currency` | The currency `received_amount` is in: the asset's or the account's.                                                              |
| `exchange_rate`     | The rate from `received_currency` to the account's (TRX-021); one when they are the same.                                        |
| `total_amount`      | The cash credited: the total received times the rate, rounded down (ROC-020).                                                    |
| `realized_pnl`      | The return of capital beyond cost: what the holding's cost could not absorb (ROC-021, ROC-022); none when it absorbed all of it. |
| `note`              | Optional free text.                                                                                                              |

> A return of capital carries no quantity, no unit price and no fees, as a dividend (DIV-023).

### The raw values of a return of capital

One list, each value stored or computed. Stored, and synced (ROC-051): `received_amount`, `received_currency`, `exchange_rate`, the cash credited (`total_amount`) and the return of capital beyond cost (`realized_pnl`). Computed by the replay at its place, and given by every read (ROC-050): the quantity held at its place, the holding's cost just before it, the cost it lowered and, under FIFO, how much each lot's cost was lowered. With the holding's transactions, they let each country's treatment be computed — the cost of shares already sold reduced, for one (owner, 2026-11-15, #553; #845).

---

## Business Rules

### Eligibility and Initiation (010–019)

**ROC-010 — What it takes (backend)**: A return of capital names an account, an asset that is not a Cash Asset, a date within the TRX bounds (TRX-020), the total received — strictly positive and within TRX-020's limit on money — and its currency: the asset's, with the rate to the account's when they differ, or the account's, with a rate of one, as a dividend may be entered (DIV-022, DIV-029). A rate other than one between the same currencies is rejected (TRF-022), and so is a total that converts to nothing (DIV-022). An unknown account or asset, and each argument missing or out of bounds, is rejected with a specific error.

**ROC-011 — Held on its date or before (backend)**: The asset was held in the account on the return of capital's date or before it — a strictly positive quantity after some transaction dated on or before it —, as a dividend (DIV-011); otherwise it is rejected with a specific error. A return of capital paid after the position was sold is accepted (owner, 2026-11-15, #553).

### Creation (020–029)

**ROC-020 — Cash in, quantity unchanged (backend)**: Recording a return of capital, atomically: it credits the account's cash with the total received times the rate, rounded down; the holding's quantity does not change; no price is recorded.

**ROC-021 — The cost falls, never below 0, under the weighted average (backend)**: A return of capital takes its place in the replay by its date, then the order entered (TRX-036). Under the weighted average the holding's cost at that place, as the replay holds it (TRX-030), falls by the cash credited; a cost that would fall below 0 stops at 0, and the return of capital beyond cost is the cash credited minus the cost just before it, rounded down. The average price is then the cost over the quantity, rounded down (TRX-030). With nothing held at its place — a position already sold (ROC-011) — the cost is 0 and all the cash credited is beyond cost.

**ROC-022 — Lot by lot under FIFO (backend)**: Under FIFO the cash credited is spread over the lots held at its place by their units (owner, 2026-11-15, #553): each lot's cost falls by `floor(cash credited × its units ÷ quantity held)`, what the roundings leave over going to the most recent lot. A lot's cost stops at 0; what a lot could not absorb is added to the return of capital beyond cost. Lots taken whole before its place are not touched; with no lot held, all the cash credited is beyond cost.

**ROC-023 — No income (backend)**: (a) A return of capital is no dividend and no income: it counts in no dividend figure (DIV-024, PRF-072) and in no figure of income. (b) Its return beyond cost counts in the holding's realized result, as a sale's does (SEL-024, ACD-043).

**ROC-024 — In performance (backend)**: (a) A return of capital is no flow of the account: the cash it credits moves value from the holding to the cash and lands in the period's P&L (PRF-073), never in its dividends. (b) In a position's performance it is an outflow of the position at the cash credited (ACD-056, PRF-083, PRF-084), and so in the portfolio's asset scope (GPF-040).

**ROC-025 — Later transactions (backend)**: A later sale, write-off or transfer out takes the lowered cost; an earlier one is unchanged.

### Updates and Status Changes (030–039)

**ROC-030 — Corrected (backend)**: Its date, total received, currency, rate and note are corrected and checked as a recording is (ROC-010, ROC-011); the holding and the realized results after it are recomputed.

**ROC-031 — Another transaction's change (backend)**: A transaction of the asset recorded, corrected or cancelled that would leave a later return of capital with its asset never held on its date or before is rejected with ROC-011's error, as DIV-045 does for a dividend. One that leaves it with less, or nothing, held at its place is accepted: the replay recomputes it (ROC-021, ROC-022). A history a merge left inconsistent is replayed as it stands (CFR-042).

### Deletion (040–049)

**ROC-040 — Deleted (backend)**: A return of capital is deleted like any transaction; a deletion that would leave a later withdrawal or purchase short of cash is rejected (CSH-080).

### Extensions (050–059)

**ROC-050 — Its raw values are given (backend)**: Wherever a transaction is read — the account journal, an AI agent's reads, the data export — a return of capital gives every raw value of the list above, stored and computed (#845).

**ROC-051 — Synced (backend)**: A return of capital is synced as its transaction, the new type, `received_amount` and `received_currency` in the data format version that ships them, which SYN-038 names when the core ships (#848).

---

## Cross-amendments

The rules below are amended by the core's pull request (#848), each in its own spec, when the behaviour ships; until then they describe the application as it is.

- **TRX entity, TRX-020, TRX-024, TRX-030** — `TransactionType` gains the return of capital, with no quantity; its total received meets the limit on money; the holding's cost falls by it (ROC-010, ROC-021).
- **CBM-022** — under FIFO a return of capital lowers each lot's cost by its units (ROC-022).
- **SEL-024, ACD-043** — a holding's realized result counts its returns of capital beyond cost (ROC-023).
- **DIV-024, PRF-072, PRF-073, PRF-083, PRF-084, ACD-056, GPF-040** — a return of capital is no dividend; its cash lands in the P&L and is an outflow of its position (ROC-023, ROC-024).
- **EXP-031** — the export's types and columns gain the return of capital and its raw values (ROC-050).
- **SYN-038** — the version that ships the return of capital (ROC-051).
- **AGT-077** — the rules an AI agent is told gain the return of capital, once the core ships (#848).

---

## Open Questions

- [x] More than the holding still costs: a realized gain, or refused? — The cost stops at 0 and the excess is its own figure, "return of capital beyond cost", counted as realized (owner, 2026-10-18, #387).
- [x] Typed per unit or in all? — As a dividend is: the total received, in the asset's currency with a rate when it differs (owner, 2026-10-18, #387); in the account's currency as a dividend may be (agent's reading of that answer, DIV-029).
- [x] Under FIFO, how is the fall spread over the lots? — By units, lot by lot, each lot never below 0; what a lot cannot absorb is beyond cost (owner, 2026-11-15, #553 — the practice of German and US statements).
- [x] Paid after the position was sold? — Accepted when the asset was held on its date or before; all of it beyond cost; the raw values stay given so a country's rule can be computed (owner, 2026-11-15, #553).
