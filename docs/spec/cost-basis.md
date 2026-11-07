# Business Rules — Cost Basis Method (CBM)

## Context

Each account has a cost basis method: the weighted average (TRX-030), the rule in France and Italy, or first in, first out, the rule in Germany, Spain and Poland, so a user sees the realized gains their broker and tax office see (#395). The method is preselected from the account's country (ADR-025) and changeable, a change recomputing the account's history. Under FIFO a holding is a queue of lots, and what leaves it takes the oldest first. A transfer in is one lot (CBM-024); carrying the sender's lots across is #788.

---

## Entity Definition

### Account (amended)

| Field               | Business meaning                                                                         |
| ------------------- | ---------------------------------------------------------------------------------------- |
| `cost_basis_method` | How the account sets the cost of the units that leave it: the weighted average, or FIFO. |

### Lot

Under FIFO, the units one transaction brought into a holding, with their acquisition date and cost. A lot belongs to its holding and is derived from the account's transactions on every replay: none is stored, and none is shown or sent anywhere.

| Field         | Business meaning                                                 |
| ------------- | ---------------------------------------------------------------- |
| `acquired_on` | The date of the transaction that brought the units in.           |
| `quantity`    | The units of the lot still held.                                 |
| `cost`        | What those units cost, in the account's currency, fees included. |

---

## Business Rules

### Eligibility and Initiation (010–019)

**CBM-010 — One method per account (backend)**: Every account has exactly one cost basis method, the weighted average or FIFO; it applies to every holding of the account. Its cash line is not affected: cash is its own unit, at an average price of one (CSH, Holding entity).

**CBM-011 — The method a new account starts with (backend)**: A new account takes the method its country implies (ADR-025): the country of its wrapper when the wrapper belongs to a country's list (ACC-040), otherwise the account's country, its own or the portfolio's (ACC-041). France and Italy imply the weighted average; Germany, Spain and Poland FIFO; every other country the weighted average. An account a synced change creates is the exception (CBM-012B).

**CBM-012 — Accounts that existed before (backend)**: An account that existed before the method did is on the weighted average, and none of its figures moves.

**CBM-012A — A change from an older device (backend)**: An account change synced from a device that predates the method leaves the account's method as it is (SYN-035).

**CBM-012B — An account an older device created (backend)**: An account a synced change creates takes the method the change carries, and the weighted average when the change predates the method.

**CBM-013 — The method stays put (backend)**: Once an account is created, only a change of method (CBM-030) or a synced change of the account (CBM-012A, CBM-033) changes it: editing its name, bank, currency, country or wrapper leaves it, and so would a change of the portfolio's country (ACC-041).

### Creation (020–029)

**CBM-020 — What brings a lot in (backend)**: Under FIFO, each purchase and opening balance brings one lot in, dated the transaction's date, at its `total_amount`; free shares (FSD-022) and interest on an asset line (INT-024) bring one lot in at no cost; a transfer in brings one lot as CBM-024 says.

**CBM-021 — The order of the lots (backend)**: Lots are ordered by acquisition date, then in the order the replay meets them (TRX-036).

**CBM-022 — What leaves takes the oldest lots first (backend)**: Under FIFO: (a) a sale, a write-off (WOF-020) and a transfer out (TRF-020) take their quantity from the oldest lots first; (b) a lot taken whole leaves with its whole cost, a lot taken in part with its cost in proportion to the units taken, rounded down, the rest staying with its units still held; (c) a sale's realized P&L is its `total_amount` minus the cost of the units it took; (d) a write-off's is minus that cost (WOF-021); (e) a transfer out moves that cost and realizes nothing (TRF-023).

**CBM-023 — A fee in units shrinks every lot (backend)**: Under FIFO, a management fee taken in units (FEE-023) takes from every lot the same share of its units, each lot keeping its cost; nothing is realized and the holding's cost is unchanged, as under the weighted average (owner, 2026-11-07, on #514). Each lot's units kept are rounded down, and the units that rounding leaves over go to the most recent lot, so the lots add up to the quantity FEE-028 computes.

**CBM-024 — A transfer in is one lot (backend)**: Under FIFO, a transfer in brings one lot dated the transfer's date, at the cost it moved (TRF-020, TRF-025).

**CBM-025 — A split rescales every lot (backend)**: A split (SPL-020) rescales every lot's quantity by its factor, each keeping its cost. Each lot's rescaled units are rounded down, and the units that rounding leaves over go to the most recent lot, so the lots add up to the quantity SPL-020 computes.

**CBM-026 — The quantity never depends on the method (backend)**: A holding's quantity, and every check on it (SEL-030, FEE-027, SPL-012, SPL-021, WOF-010), is the same under either method.

**CBM-027 — The average price under FIFO (backend)**: Under FIFO, a holding's average price is the cost of its lots divided by its quantity, rounded down. A holding whose quantity reaches zero keeps the average price it had just before (TRX-040, SEL-026).

**CBM-028 — Cost left with no units (backend)**: A cost a holding keeps with no units — a fee that removed every unit (FEE-028), or under FIFO a lot that rounding left with none (CBM-023, CBM-025) — stays in the holding's cost and in its average price (CBM-027). Under the weighted average it is averaged into the units acquired next; under FIFO it leaves in full with the units taken when its lot's turn comes (CBM-022), whether by a sale, a write-off or a transfer out; a fee leaves it in place.

### Updates and Status Changes (030–039)

**CBM-030 — Changing the method (backend)**: The owner can change an account's method. (a) The change replays every holding of the account under the new method: every sale's and write-off's realized P&L, every holding's average price and every transfer out's cost are recomputed, and a transfer in that a recomputed transfer out feeds is re-costed as TRF-025 says. (b) The change and what it recomputes are saved together, or nothing is. (c) An unknown account is rejected with a specific error. (d) The change is signaled by `AccountUpdated` and `TransactionUpdated`.

**CBM-030A — Who changes it (backend)**: An AI agent and the command line cannot change an account's method; no tool or command of theirs offers it.

**CBM-031 — Choosing the method in place (backend)**: Setting an account's method to the one it has changes nothing and is accepted: nothing is recomputed, signaled or published.

**CBM-032 — Figures as of a date (backend)**: Every figure reconstructed as of a date — a holding's average price and realized P&L as of a day (TDI-010), in the account's details as of a past date and in an AI agent's preview (AGT-072) — follows the account's method.

**CBM-033 — The method is synced (backend)**: The method is part of the account, carried to every device of the sync with it (SYN-038). The device that changes it publishes what the change recomputed, its transfers in elsewhere included (TRF-025). A device that receives the change replays the account's holdings under the new method and takes the recomputed transactions as they were published. A sale or a write-off recorded under the old method on another device and crossing the change is replayed under the new one wherever it lands; a transfer out crossing it keeps the cost it was published with, as any transfer out a device receives (TRF-025), until a change of that transfer re-costs it. An account change that outranks the change of method (CFR-020) carries the method it holds, and every device replays under the method that wins, so every device ends with the same figures (CFR-013).

---

## Open Questions

- [x] Per account or per portfolio? — Per account, preselected from the wrapper's country, changeable (owner, 2026-10-17, #395).
- [x] Other methods? — Weighted average and FIFO only (owner, 2026-10-17, #395).
- [x] The United Kingdom and the United States? — Their own methods (share pooling and 30-day matching; lot identification) are left until planned; until then they take the weighted average (owner, 2026-11-07, on #764).
- [x] A fee in units under FIFO? — Every lot shrinks, keeping its cost (owner, 2026-11-07, on #514).
- [x] A transfer between two accounts? — It carries its lots (owner, 2026-11-07, on #514): #788; until it ships a transfer in is one lot (CBM-024).
