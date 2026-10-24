# Business Rules — Transfer (TRF)

## Context

Moving an asset between two accounts of the portfolio — an exchange to a hardware wallet, one broker to another — has no transaction kind today: the owner records a sale at cost, a withdrawal, a deposit and a purchase, which show trades that never happened, cash that never existed and a distorted net cash input in both accounts (gh275, #393).

A transfer is one operation of two sides: a **transfer out** of the sending account and a **transfer in** to the receiving one, of the same asset on the same day. It moves quantity and cost, never cash, and realizes nothing. A quantity received below the quantity sent is a **fee in kind**: a write-off with the reason `NetworkFee` on the sending side (WOF), part of the transfer. This spec holds the core's rules; the window's form (#511), the agent connection (#512) and the replacement of four past recordings (#513) add theirs.

Figures follow TRX-024. Assets only: moving cash between accounts is its own feature (#619). A transfer whose two sides land on different days is not covered: the owner records it on the day it arrives.

---

## Entity Definition

### Transfer out, transfer in (new `TransactionType` variants)

| Field              | Business meaning                                                                                                                     |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------ |
| `transaction_type` | The transfer out variant (the sending side) or the transfer in variant (the receiving side).                                         |
| `account_id`       | The sending account for the transfer out, the receiving account for the transfer in.                                                 |
| `asset_id`         | The asset moved, the same on both sides. **Not** a Cash Asset.                                                                       |
| `date`             | The day of the transfer, the same on both sides.                                                                                     |
| `quantity`         | The quantity received, the same on both sides.                                                                                       |
| `total_amount`     | The cost moved, in the side's account currency (TRF-020, TRF-022); zero for a holding at zero cost.                                  |
| `unit_price`       | The cost moved per unit, in the side's account currency, rounded down (TRF-022).                                                     |
| `exchange_rate`    | On the transfer in, the rate used from the sending account's currency to the receiving account's (TRF-022); one on the transfer out. |
| `transfer_id`      | The transfer the part belongs to, shared by its two sides and its fee in kind; no other transaction carries one.                     |
| `note`             | Optional free text, the same on every part of the transfer.                                                                          |
| `realized_pnl`     | None: a transfer realizes nothing (TRF-023).                                                                                         |

> A transfer carries **no fees and no proceeds**. Its fee in kind is a write-off (WOF) carrying the same `transfer_id`. As an opening balance (TRX-047), a transfer side's unit price is in its account's currency, not the asset's (TRX-021).

### Transfer (a view of its parts)

The transfer is not stored on its own: it is read from its parts, which carry its identity, and has no lifecycle apart from theirs.

| Field               | Business meaning                                                                    |
| ------------------- | ----------------------------------------------------------------------------------- |
| `transfer_id`       | Identifies the operation.                                                           |
| `from_account_id`   | The sending account: the transfer out's.                                            |
| `to_account_id`     | The receiving account: the transfer in's, another account of the portfolio.         |
| `asset_id`          | The asset moved.                                                                    |
| `date`              | The day of the transfer.                                                            |
| `quantity_sent`     | What left the sending account: the transfer out's quantity plus its fee in kind's.  |
| `quantity_received` | What reached the receiving account: the transfer in's quantity.                     |
| `exchange_rate`     | The rate used between the two accounts' currencies; one when they share a currency. |
| `note`              | Optional free text.                                                                 |

---

## Business Rules

### Eligibility (010–019)

**TRF-010 — What can be transferred (backend)**: A transfer is recorded only between two different accounts that both exist, of an asset that exists, is not a Cash Asset and is not archived (as a sale, SEL-037), and only up to what the sending account holds of it on the transfer's date. Otherwise it is rejected with a specific error each: the account unknown, the two accounts the same, the asset unknown, the Cash Asset, the asset archived, nothing held on that date (SEL-012), the quantity sent above what is held — naming what was held on that date, as a sale does (SEL-021).

**TRF-011 — Its inputs (backend)**: The quantities sent and received must be strictly positive, the quantity received not above the quantity sent; the date a well-formed ISO date, neither in the future nor before the TRX lower bound; the rate, when given, strictly positive. Each is rejected with a specific error. These replace TRX-020's checks of the figures: a holding at zero cost (free shares only) is transferred at a cost of zero.

**TRF-012 — A backdated transfer (backend)**: A transfer dated before later transactions of the asset in the sending account is refused when the replay would then leave a later sale, write-off, management fee or transfer out above what is held at its own date, as a write-off is (WOF-012).

### Recording (020–029)

**TRF-020 — At its cost, without cash (backend)**: Recording a transfer, atomically across both accounts — nothing persists on a refusal: (a) the transfer out decreases the sending holding's quantity by the quantity received and its cost basis by that quantity times the average price at the transfer's place in the replay (date, then recording order, TRX-036), the cost it moves, rounded down as a write-off's (WOF-021); (b) the transfer in increases the receiving holding's quantity by the quantity received and its cost basis by the cost moved, converted (TRF-022); (c) neither account's cash moves, the sending holding's average price does not change and no price is recorded.

**TRF-021 — A fee in kind (backend)**: When the quantity received is below the quantity sent, the difference leaves the sending account as a write-off with the reason `NetworkFee`, on the transfer's date, with its note — the order of the two on that date leaves the figures the same, both leaving at the average price (WOF-020, WOF-021): it leaves at its cost and is counted as any write-off. It is part of the transfer: recorded, corrected and cancelled with it (TRF-030, TRF-040).

**TRF-022 — Between currencies (backend)**: When the two accounts' currencies differ, the cost moved converts into the receiving account's currency at the rate given or, when none is given, at the stored rate of the transfer's date from the sending account's currency to the receiving account's — the most recent on or before it, as every valuation reads a rate (FXR-035); with neither, the transfer is rejected with a specific error. The converted cost is the cost moved times the rate, rounded down; each side's unit price is its cost over the quantity, rounded down (as TRX-047). The rate used is recorded on the transfer in. Between accounts of one currency the rate is one, and a rate given other than one is rejected with a specific error.

**TRF-023 — Absent from realized gains (backend)**: Neither side of a transfer has a realized result: the sending holding's realized P&L, its total and the account's realized figures are unchanged by it. Only its fee in kind realizes the loss of a write-off (WOF-021).

**TRF-024 — A closed position (backend)**: A transfer out that leaves nothing held closes the sending position as a sale or a write-off does: its date is the position's "Closed on" date (ACD-043, ACD-048).

**TRF-025 — The receiving cost follows the sending cost (backend)**: The cost a transfer moves is the sending holding's average price at the transfer's place at every moment, not only when it is recorded: a recording, a correction or a removal in the sending account that changes that average rewrites the cost and unit price of both sides — the transfer out's, and the transfer in's at its recorded rate — in the same write, and a transfer onward from the receiving account follows in turn. The result is the one the transfer would have given had it been recorded after them. A change applied from another computer is taken as it arrives: the computer that made it re-costed and published both sides (SYN-021). When two computers' changes cross — a purchase corrected on one, a transfer from that account recorded on the other — the transfer keeps the cost it arrived with until the next write in the sending account re-costs it.

**TRF-026 — In the account's performance (backend)**: In an account's performance the transfer in is an in-kind contribution and the transfer out an in-kind withdrawal, counted in the account's net external flow and its asset in/out as an opening balance is (PRF-030, PRF-071, PRF-086): the side's quantity valued at its carry-forward price on the transfer's date (PRF-022) and the rate of that date (FXR-042) in windowed metrics, the side's quantity (the quantity received) valued so; its cost moved when no usable price or rate exists then, and its cost moved in lifetime metrics. Neither moves the account's cash.

**TRF-027 — In the position's performance (backend)**: In a position's performance (ACD-056, PRF-083) the transfer in is an inflow of the receiving position and the transfer out an outflow of the sending position, each valued as TRF-026 says.

**TRF-028 — In the portfolio's performance (backend)**: In the portfolio's performance (GPF-030, GPF-040) a transfer is no flow at all, neither in the flows nor in the bridge's asset in/out: what leaves one account reaches the other. Its fee in kind is a loss as any write-off (WOF-023).

**TRF-029 — Recorded as one operation (backend)**: Recording a transfer gives back its two sides and its fee in kind, if any, and publishes `TransactionUpdated` once; so do its correction and its cancellation, and a write that re-costs a transfer publishes it once, re-costing included (TRF-025).

### Correction (030–039)

**TRF-030 — Corrected together (backend)**: A transfer's date, quantities, rate and note are corrected as one operation, checked again as TRF-010 to TRF-012 and TRF-022 say on the history without it, with the same errors; and refused when the receiving account's replay would then leave a later sale, write-off, management fee or transfer out above what is held at its own date (WOF-032). A correction that names no rate takes the stored rate of its date again (TRF-022). A correction that leaves the quantity received equal to the quantity sent removes the fee in kind, and one that brings it below adds it. Its accounts and its asset are not corrected: a transfer between other accounts, or of another asset, is a new one. A transfer that is unknown, or that has lost a side (TRF-041), is rejected with a specific error. A refusal leaves the transfer as it was.

**TRF-031 — Not corrected by its parts (backend)**: A part of a transfer is not corrected through the transactions' modification (TRX-031): it is rejected with a specific error naming the transfer as what to correct.

**TRF-032 — Synced (backend)**: Each part of a transfer is a transaction like any other for sync (SYN-021); the transfer kinds and a transaction's transfer arrive with data format version 9 (SYN-038). Two computers correcting one transfer at once each publish all its parts; the parts keep the later change of each (CFR-012), so they may mix the two corrections until the next one. A mixed transfer reads its quantity sent from its transfer out and fee in kind, its quantity received, rate and note from its transfer in; the next correction or removal rewrites all its parts as any other.

### Removal (040–049)

**TRF-040 — Cancelled together (backend)**: Removing any part of a transfer — through the transactions' deletion (TRX-034) or the transfer's cancellation — removes its two sides and its fee in kind together, atomically — an unknown transfer is rejected with a specific error; both holdings are replayed from their logs, so removing it restores both exactly. Without the transfer the sending account only holds more; the removal is refused when the receiving account's replay would then leave a later sale, write-off, management fee or transfer out above what is held at its own date (WOF-032), with the same error.

**TRF-041 — An account removed (backend)**: Deleting an account removes the parts of a transfer it holds, as all its transactions (ACC-005, ACC-006). The parts left in the other account stay, with their figures: a transfer in left alone is an entry at its cost, a transfer out an exit at its cost, a fee in kind a write-off. A part left alone is no longer corrected (TRF-031 still holds); its removal removes what is left of its transfer.

---

## Workflow

```
record (sending, receiving, asset, date, sent, received, rate?)
  ├─ checks TRF-010 … TRF-012, TRF-022
  ├─ sending account:   [write-off NetworkFee (sent − received)] + transfer out (received)
  └─ receiving account: transfer in (received, cost × rate)
           one write; one TransactionUpdated
```

---

## Cross-amendments

- **TRX-020, TRX-021** — a transfer's figures are checked by TRF-011, and its unit price is in its account's currency (TRF-022), as an opening balance's (TRX-047).
- **TRX-031, TRX-034** — a part of a transfer is corrected as the transfer (TRF-031) and removed with it (TRF-040).
- **SEL-032, FSD-041, WOF-012, WOF-032** — a later transfer out counts as what must stay held, as a sale does (TRF-012).
- **ACC-006** — a transfer's parts in another account stay (TRF-041).
- **FEE-027, FEE-047** — a later transfer out counts as a sale does (TRF-012).
- **WOF-030, WOF-040, WOF-056** — a fee in kind is corrected and removed with its transfer (TRF-031, TRF-040).
- **PRF-084, GPF-040** — the bridges' asset in/out (TRF-026 to TRF-028).
- **ACD-043, ACD-048** — a transfer out closes a position as a sale does (TRF-024).
- **PRF-030, PRF-071, PRF-083, PRF-086, ACD-056, GPF-030** — the transfer's flows (TRF-026 to TRF-028).
- **CFR-012** — the parts of a transfer corrected on two computers (TRF-032).
- **SYN-038** — version 9 carries the transfer kinds and a transaction's transfer (TRF-032).

---

## Open Questions

None — the owner's answers on #393 (2026-10-17) and #510 (2026-10-23) settle the cost, the rate, the fee in kind, realized gains and cash. The vocabulary — transfer, transfer in, transfer out, fee in kind — and its French awaits the owner's validation with the window's form (#511).
