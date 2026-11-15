# Business Rules — Share Exchange (SHX)

## Context

A merger or an acquisition exchanges the shares of one asset for another's. Recorded as a sale and a purchase, it shows a gain or a loss that was never realized and breaks the cost basis. A share exchange moves the cost basis of the shares given up to the shares received and realizes nothing, but on a cash part paid with the exchange, which takes its share of the cost (owner, 2026-10-18, #382).

The application computes no tax (owner, 2026-11-14): it stores the exchange's raw values — what was given, what was received, the cash paid and the value of the shares received — from which each country's treatment can be computed (#845), and applies one convention to its own figures (owner, 2026-11-15, #520). The core is #847; the window, then the agent connection, then the command line follow (owner, 2026-10-11).

Figures follow TRX-024. Every amount is in the account's currency unless said otherwise. "The cash part" is the third part of an exchange; "the cash paid" is its amount.

---

## Entity Definition

### Share exchange given, share exchange received, share exchange cash (new `TransactionType` variants)

An exchange is stored as its parts within one account, linked as a transfer's are (TRF): a given side on the asset given up, a received side on the asset received, and, when cash is paid with the exchange, a cash part on the asset given up. No part carries fees.

| Field               | Business meaning                                                                                                                                                                  |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `transaction_type`  | The given side, the received side or the cash part.                                                                                                                               |
| `account_id`        | The account holding both assets; the same on every part.                                                                                                                          |
| `asset_id`          | The asset given up on the given side and the cash part; the asset received on the received side. **Not** a Cash Asset.                                                            |
| `date`              | The day of the exchange, the same on every part.                                                                                                                                  |
| `quantity`          | The quantity given up on the given side, the quantity received on the received side; none on the cash part.                                                                       |
| `total_amount`      | The cost given up on the given side (SHX-020); the cost the shares received keep on the received side (SHX-022); the cash credited on the cash part (SHX-021).                    |
| `unit_price`        | On the two sides, their cost per unit, rounded down, as a transfer's sides (TRF-022); on the cash part, the cash paid, in the given asset's currency.                             |
| `exchange_rate`     | On the cash part, the rate from the given asset's currency to the account's; on the received side, the rate of `value_received` to the account's currency; one on the given side. |
| `value_received`    | On the received side only: the value of one share received on the exchange's date, in the received asset's currency (SHX-011), or none.                                           |
| `realized_pnl`      | On the cash part only: the cash credited minus its share of the cost (SHX-021). None on the two sides.                                                                            |
| `share_exchange_id` | The exchange the part belongs to, shared by its parts; no other transaction carries one.                                                                                          |
| `carried_lots`      | Under FIFO, on the given side the lots it took, and on the received side those lots as the shares received keep them (SHX-023).                                                   |
| `note`              | Optional free text, the same on every part.                                                                                                                                       |

### The raw values of an exchange

What the exchange stores and gives, whatever convention computes from them (#845): `quantity_given`, `cost_given` (the given side's `total_amount`), `quantity_received`, `cash_paid` (the cash part's `unit_price`, in the given asset's currency, with its rate), `value_received` (per share, in the received asset's currency, with its rate). The application's own figures — the cash part's share of the cost, its realized result, the cost the shares received keep — are computed from them (SHX-021, SHX-022).

---

## Business Rules

### Eligibility and Initiation (010–019)

**SHX-010 — What an exchange takes (backend)**: An exchange names an account, an asset given up and an asset received — two different assets, neither a Cash Asset, neither archived (SEL-037) —, a quantity given and a quantity received, both strictly positive and within TRX-020's bounds, a date within the TRX bounds (TRX-020), and optionally the cash paid, strictly positive, in the given asset's currency, with the rate to the account's (DIV-022: one between the same currencies, any other rate refused), and a note. An unknown account or asset, and each argument missing or out of bounds, is rejected with a specific error.

**SHX-011 — The value of the shares received (backend)**: An exchange carries the value of one share received on its date, strictly positive and within TRX-020's bounds, in the received asset's currency, with the rate to the account's — the one given, or else the stored rate of the date (FXR-035), one between the same currencies. When no value is given it is the received asset's price recorded on the exchange's date, if any (owner, 2026-11-15). An exchange with a cash part and no value, given or recorded, or no rate for it, is rejected with a specific error. Without a cash part a missing value stays none; the value computes no cost and no realized result then (SHX-024), and values the parts' flows (SHX-027).

**SHX-012 — Held on its date (backend)**: The quantity given is held in the account on the exchange's date, the exchange itself left out; otherwise the exchange is rejected with a specific error naming the quantity held (SEL-012). An exchange that would leave a later sale, write-off, management fee, transfer out or exchange of the given asset above what is held at its own date is rejected; likewise a sale, write-off, management fee, transfer out, exchange or opening balance recorded, corrected or cancelled is rejected when the replay would then leave a later given side above what is held at its date (TRF-012).

### Creation (020–029)

**SHX-020 — The given side (backend)**: Recording an exchange, atomically — nothing persists on a refusal: the given side takes the quantity given out of the given holding at its cost under the account's method — the average price at its place in the replay times the quantity, rounded down, under the weighted average; the oldest lots first under FIFO (CBM-022). That cost is the cost given up. It moves no cash and realizes nothing; leaving nothing held, it closes the given position as a sale does (TRF-024, ACD-043).

**SHX-021 — The cash part (backend)**: With cash paid, a third part is recorded on the asset given up. It credits the account's cash with `cash = floor(cash paid × rate)`; with `value = floor(floor(value per share × quantity received) × its rate)`, as a purchase's total is rounded (TRX-026), it takes `floor(cost given up × cash ÷ (cash + value))` as its share of the cost; its realized result is `cash − that share`, a figure of its own, counted in the given holding's realized result as a sale's is (SEL-024, ACD-043). It moves neither the given holding's quantity nor its cost: the given side took the whole cost given up.

**SHX-022 — The received side (backend)**: The received side adds the quantity received to the received holding at the cost given up minus the cash part's share of it (SHX-021) — zero when the shares given up cost nothing. It moves no cash and realizes nothing. From its date the received asset is held (DIV-011).

**SHX-023 — The lots under FIFO (backend)**: Under FIFO, the received holding gets the lots the given side took (CBM-022), each keeping the date it was acquired on and whether it came at no cost (CBM-020) (owner, 2026-11-15): each lot's units are `floor(its units × quantity received ÷ quantity given)` and its cost `floor(its cost × cost received ÷ cost given up)` — 0 when the cost given up is 0 —, what the roundings leave over going to the most recent lot, so the lots add up to the received side's quantity and cost. They take their place among the received holding's lots by their dates (CBM-021), from the exchange's date on: what left the received holding before the exchange is unchanged (as CBM-024 (d)).

**SHX-024 — Without a cash part (backend)**: An exchange without cash paid has no third part: the shares received keep the whole cost given up, and nothing is realized.

**SHX-025 — Its figures follow the given holding (backend)**: The cost given up is the given holding's at the exchange's place at every moment: a recording, a correction or a removal in the account, a change of method (CBM-030) and a re-costed transfer in of the given asset (TRF-025) that change it rewrite the given side, the cash part's share and realized result, and the received side and its lots, in the same write; a later transfer out or exchange of the received asset follows in turn, as a transfer's cost follows its sending account (TRF-025). An exchange applied from another computer is taken as it arrives, as published (SYN-021).

**SHX-026 — In the account's performance (backend)**: In an account's performance an exchange is no flow: value moves between two of its holdings, and the cash part into its cash as a sale's proceeds do (PRF-030, PRF-070).

**SHX-027 — In the position's performance (backend)**: In a position's performance (ACD-056, PRF-083) the given side is an outflow of the given position and the received side an inflow of the received position, each at the value of the shares received on the date converted at its rate — the received side's cost when there is none —, and the cash part an outflow of the given position at the cash credited.

**SHX-028 — In the portfolio's performance (backend)**: In the portfolio's performance (GPF-030, GPF-040) an exchange is no flow; its cash part's realized result counts as a sale's.

**SHX-029 — Recorded as one operation (backend)**: Recording an exchange gives back its parts and publishes `TransactionUpdated` once; so do its correction, its cancellation and a write that re-costs it (SHX-025), as a transfer's (TRF-029).

### Updates and Status Changes (030–039)

**SHX-030 — Corrected as one (backend)**: An exchange's date, quantities, cash paid and its rate, value and its rate, and note are corrected as one operation, checked again as SHX-010 to SHX-012 say on the history without it, with the same errors; a date moved with no value given reads the price of the new date again (SHX-011). A correction keeps each part's identity, adds the cash part a correction gives and removes the one it takes away. It is refused when it would leave a later transaction of the received asset above what is held at its date (TRF-030), a dividend of the received asset before any holding (DIV-045), or a later withdrawal or purchase short of cash (CSH-080). An unknown exchange is rejected with a specific error.

**SHX-031 — Not corrected by its parts (backend)**: A part of an exchange is not corrected through the transactions' modification (TRX-031): it is rejected with a specific error naming the exchange as what to correct (TRF-031).

### Deletion (040–049)

**SHX-040 — Cancelled as one (backend)**: Cancelling any part cancels the exchange, every part; it is refused when it would leave a later transaction of the received asset above what is held at its date (TRF-012), a dividend of the received asset before any holding (DIV-045), or a later withdrawal or purchase short of cash (CSH-080). An unknown exchange is rejected with a specific error.

### Extensions (050–059)

**SHX-050 — Read as one, and its raw values (backend)**: An exchange is read as one from its identity: its account, its two assets, its quantities, the cost given up, the cash paid with its rate, the value of the shares received with its rate, and its date and note (as TRF-060). Each row of an account journal that is a part of an exchange carries the asset on its other side (as TRF-062). Wherever a transaction is read — the account journal, an AI agent's reads, the data export — an exchange's parts give its raw values (#845).

**SHX-051 — Synced as its parts (backend)**: An exchange is synced as its parts, the new types, `value_received` and `share_exchange_id` in a new data format version (SYN-038). Two computers correcting one exchange at once are resolved as a transfer's parts are (CFR-012, TRF-032).

---

## Cross-amendments

- **TRX-020, TRX-021** — the cash part has no quantity; the received side's cost can be zero; the sides' unit price is a cost per unit in the account's currency and the cash part's the cash paid in the given asset's currency (SHX-010, SHX-021, SHX-022).
- **TRX-031** — a part of an exchange is corrected as the exchange (SHX-031).
- **CBM-020, CBM-021, CBM-022, CBM-030** — the given side takes lots as a transfer out does; the received side brings the lots SHX-023 says; a change of method re-costs an exchange (SHX-025).
- **SEL-012, SEL-032, WOF-012, FEE-027, FSD-041** — a removal that would leave a later given side short is refused (SHX-012).
- **SEL-024, ACD-043** — a holding's realized result counts its exchanges' cash parts; a given side that leaves nothing held closes the position (SHX-020, SHX-021).
- **DIV-011, DIV-045** — the received side is a way an asset comes to be held (SHX-022).
- **TXL-060, CSH-050, CSH-080** — the cash part credits cash (SHX-021).
- **PRF-030, PRF-070, PRF-071, PRF-083, PRF-084, ACD-056, GPF-030, GPF-040** — an exchange is no flow of the account or the portfolio; its parts are flows of their positions (SHX-026 to SHX-028).
- **EXP-020, EXP-026, EXP-031, EXP-032** — the export holds the parts, their raw values and the given side's lots (SHX-050).
- **SYN-038** — a new data format version carries the exchange (SHX-051).
- **AGT-077** — the rules an AI agent is told gain the share exchange's convention, once the core ships (#847).

---

## Open Questions

- [x] The cash part: a gain realized on that part alone, or a reduction of the cost basis? — It takes its share of the cost; the difference is a realized result on its own line (owner, 2026-10-18, #382).
- [x] How is that share computed? — `cost × cash ÷ (cash + value of the shares received)`, the value stored with the exchange — by default the received asset's price recorded that day; the application applies only this convention, and each country's treatment is computed from the raw values (owner, 2026-11-15, #520).
- [x] The cash paid's currency? — The given asset's, with its rate to the account's, as a dividend; cash paid in another currency is entered converted, at a rate of one (owner, 2026-11-15, #520).
- [x] One transaction on two assets, or linked transactions? — Linked lines, one in each asset's journal, corrected and cancelled together, as a transfer (owner, 2026-10-18, #382).
- [x] The lots under FIFO? — The shares received keep the lots of the shares given up, each with its acquisition date (owner, 2026-11-15, #520 — the US and German practice); each lot's cost falls by the cash part's share in proportion to its cost (agent's choice, #520: the share is of the whole cost).
- [ ] A fraction of a share the exchange leaves, paid in cash ("cash for a fraction", owner 2026-10-18): a kind of its own, recorded with the exchange — its spec comes with its slice of #382.
- [ ] A distribution of another company's shares (a spin-off, owner 2026-10-18): recorded with the exchange's form, the original kept — its spec comes with its slice of #382.
