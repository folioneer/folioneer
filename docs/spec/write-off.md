# Business Rules — Write-off (WOF)

## Context

A write-off records a quantity that leaves a holding **without proceeds**: a network fee paid in the asset itself, dust left behind by a conversion, coins sent to a wallet the portfolio does not track or lost for good. Without it the owner records a sale at a symbolic price and a withdrawal, which invents cash and pads the realized gains of a history that matters for tax (gh273, #394).

The feature adds a write-off variant to `TransactionType`. A write-off is attributed to the asset it takes out of; it moves no cash and touches no price. The window (#551) and the agent connection (#552) record it; this spec holds the core's rules and the window's (050–059), the agent connection its own (AGT-065).

Figures follow TRX-024. A fund's ongoing charges taken in units are a management fee deduction (FEE), which keeps the cost basis and realizes nothing; a fee paid in the asset for one operation — a network fee for a transfer — is a write-off with the reason `NetworkFee`. In French a write-off is a « Sortie sans contrepartie » (owner, 2026-10-19).

---

## Entity Definition

### Write-off (new `TransactionType` variant)

| Field              | Business meaning                                                                                                                          |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `transaction_type` | The write-off variant of `TransactionType`.                                                                                               |
| `account_id`       | The account whose holding the quantity leaves.                                                                                            |
| `asset_id`         | The asset written off. **Not** a Cash Asset.                                                                                              |
| `date`             | The day the quantity left (not in the future, not older than the TRX lower bound, TRX-020).                                               |
| `quantity`         | The quantity written off: strictly positive, micro-units (TRX-024).                                                                       |
| `write_off_reason` | Why it left: `NetworkFee` (a fee paid in the asset), `Dust` (a remainder too small to use), `Lost` (sent away or lost for good), `Other`. |
| `note`             | Optional free text.                                                                                                                       |
| `realized_pnl`     | The realized result: the cost taken out, negated (WOF-021).                                                                               |

> A write-off carries **no proceeds, no unit price, no exchange rate and no fees**: the contract gives the stored convention. Every other transaction type carries no `write_off_reason`.

### HoldingDetail (extended)

| Field           | Business meaning                                                                                               |
| --------------- | -------------------------------------------------------------------------------------------------------------- |
| `realized_lost` | The realized P&L of the holding's write-offs whose reason is `Lost`, account currency, zero or less (WOF-022). |

---

## Business Rules

### Eligibility (010–019)

**WOF-010 — What can be written off (backend)**: A write-off is recorded only for an account and an asset that both exist, the asset not a Cash Asset and not archived (as a sale, SEL-037), and only up to what the account holds of the asset on the write-off's date — its transactions dated on or before that day replayed in order (TRX-036). Each case is refused with an error of its own: an unknown account or asset, a Cash Asset, an archived asset, an asset not held on that date, a quantity above what is held then.

**WOF-011 — Its inputs (backend)**: The quantity must be strictly positive; the date a well-formed ISO date, neither in the future nor before the TRX lower bound (TRX-020); the reason one of the four of the entity. These replace TRX-020's figure checks for a write-off, which carries no proceeds (its total is 0). The note is free text, absent when empty. Each failure is refused with an error of its own.

**WOF-012 — A backdated write-off (backend)**: A write-off dated before later transactions of the asset is refused when the replay would then leave a later sale, write-off or management fee above what is held at its own date, as an edit that does so is (SEL-032).

### Effect (020–029)

**WOF-020 — At its cost, without cash (backend)**: Recording a write-off, atomically — nothing persists on a refusal: (a) decreases the holding's quantity by the quantity written off; (b) decreases its cost basis by that quantity at the holding's average price (VWAP) on that date (TRX-030), so the average price is unchanged, as a sale leaves it (SEL-027); (c) moves no cash — the account's Cash Holding is untouched; (d) creates or modifies no price. A write-off that takes out the whole quantity closes the position, as a sale does.

**WOF-021 — Its realized P&L (backend)**: A write-off's realized P&L is the cost it takes out, negated — the quantity times the average price on its date, in account currency, rounded down — and nothing for a holding at zero cost. It counts in the holding's realized P&L as a sale's does (SEL-024), and the holding's `last_sold_date` (ACD-043) takes its date as a sale's would. A sale or write-off dated after it computes against the unchanged average price.

**WOF-022 — Lost, kept apart (backend)**: The realized P&L of the holding's write-offs whose reason is `Lost` is also given apart, as the holding's **lost** amount (account currency, zero or less), so the owner sees what disappeared apart from what fees and dust cost. It is included in the holding's realized P&L, not added to it. In a read with a date, both count the write-offs dated on or before it. There is no account-level total of it.

**WOF-023 — Not a cash flow (backend)**: A write-off is no external flow of the performance figures, of the account (PRF-030) or of the position (ACD-056, PRF-083, GPF-030): the value it takes out is a loss of the position, as a fall in price is.

### Correction (030–039)

**WOF-030 — Corrected as a sale is (backend)**: A write-off's date, quantity, reason and note are corrected through the transactions' modification (TRX-031), checked again as WOF-010 to WOF-012 say, with their codes, on the corrected history replayed from the log (TRX-036), counting what was held just before the write-off, itself left out. The archived-asset check is not repeated, as for a sale.

**WOF-031 — Synced (backend)**: A write-off is a transaction like any other for sync (SYN-021); the write-off type and its reason are values of data format version 7 (SYN-038).

**WOF-032 — An edit that oversells a later write-off (backend)**: The correction or deletion of another transaction of the asset in the account is refused when the replay would then leave a later write-off above what is held at its date, as one that leaves a later sale so is (SEL-032, FSD-041).

### Removal (040–049)

**WOF-040 — Removed as a sale is (backend)**: A write-off is removed through the transactions' deletion (TRX-034); the holding is replayed from the log, so removing it restores the holding exactly.

### Window (050–059)

**WOF-050 — The row's action (frontend)**: Each active, non-cash holding row of an asset that is not archived carries a "Write off" action, after Sell; the Cash Holding row and the as-of view (read-only) have none. It opens the write-off form for that asset.

**WOF-051 — The form (frontend)**: The form, titled "Write off" (FR « Sortie sans contrepartie »), holds a line naming the asset, the quantity held and its average price; the date, the account's last operation date by default, as the other forms; the reason — "Network fee paid in the asset", "Dust left by a conversion", "Lost or sent to an untracked wallet", "Other" (FR « Frais de réseau payés dans l'actif », « Reste d'une conversion », « Perdu ou envoyé vers un portefeuille non suivi », « Autre »), `NetworkFee` by default; the quantity written off; the note; and the action "Record the write-off" (FR « Enregistrer la sortie »).

**WOF-052 — Its estimate (frontend)**: While the quantity is a positive figure within what is held, a line gives what it takes out at the average price shown and says that no cash moves and, for `Lost`, that the result is shown apart as lost. It is the form's estimate; the core takes the cost at the date (WOF-021).

**WOF-053 — Its quantity (frontend)**: While the quantity is empty, zero, below one micro-unit or not a positive figure, the record action is inactive. As it is typed, a quantity above the quantity held shown in the form is refused under the field, naming what is held, and the record action stays inactive; what was held on the chosen date is the core's check, its refusal worded as WOF-055 says.

**WOF-054 — Saving (frontend)**: While it saves, the record action is inactive; on success the form closes, a confirmation shows and the holding row shows the new figures.

**WOF-055 — Its refusals (frontend)**: A refusal for lack of units — a quantity above what was held on the chosen date, a date on which nothing was held, a later sale, write-off or management fee left short — shows under the quantity with the wording of SEL-046, for a recording and a correction alike. Any other refusal shows below the form as the message for that refusal. The record action stays inactive while the form shows a refusal, until the date, the quantity or the reason changes.

**WOF-056 — What the window records (backend)**: The window records a write-off under WOF-010 to WOF-012 and WOF-020, with their refusals, as the agent connection does (AGT-065), and corrects one under WOF-030, taking its date, quantity, reason and note.

**WOF-057 — In the journals (frontend)**: A write-off's Type reads with its short reason — "Write-off · Network fee", "· Dust", "· Lost", "· Other" (FR « Sortie sans contrepartie · Frais de réseau », « · Reste », « · Perdu », « · Autre »). Its Quantity shows the quantity written off; Unit Price and Total Amount the neutral placeholder `—`; Exchange Rate and Fees their stored values, not a placeholder, as for free shares; Realized P&L its realized P&L (WOF-021), as for a sale (TXL-022).

**WOF-058 — Corrected and deleted from the journals (frontend)**: A write-off's edit opens the write-off form, with the same title and action, the asset fixed and the fields filled, once the account's holdings are read; if that read fails, the message for its refusal shows and no form opens, as for the other correction forms (DIV-042); there the quantity held counts the write-off being edited back in, and the average price shown is the write-off's own, its cost over its quantity. Its deletion is the journals' (WOF-040).

**WOF-059 — Lost shown apart (frontend)**: Under an active holding's realized P&L, a line "of which lost:" (FR « dont perdu : ») gives the lost amount (WOF-022) in the account's currency when it is not zero; in the as-of view, the lost amount as of its date. A closed position shows none for now (#593).

---

## UX Draft

The validated mock is on #551 (folioneer/folioneer-design, `551/`).

1. On an active holding row, "Write off" (after Sell) opens the form for that asset (WOF-050, WOF-051).
2. The owner picks the date and the reason, types the quantity; the estimate line follows the quantity (WOF-052, WOF-053).
3. "Record the write-off": the form closes and the row shows the new quantity, its realized P&L and, for `Lost`, "of which lost:" (WOF-054, WOF-059); or a refusal shows and the action waits for a change (WOF-055).
4. From a journal, the row's edit reopens the form filled; its deletion is the journal's (WOF-058).

## Cross-amendments

- **TXL-022, TXL-023** — a write-off row's Type, its placeholder columns and its Realized P&L (WOF-057).
- **TXL-030** — a write-off's edit opens the write-off form (WOF-058).
- **ACD UX Draft** — the active positions' actions gain "Write off" after Sell (WOF-050), and the Realized P&L its lost line (WOF-059).
- **ACD-048** — the closed positions column reads "Closed on", a write-off closing a position as a sale does (WOF-021).

---

## Open Questions

- A closed position's lost amount (WOF-059) — deferred, #593.

Otherwise none — the owner's answers on #394 and #550 (2026-10-17, 2026-10-19) settle the kind, its reasons, at cost, no cash and lost apart; the rest follows the sale's rules.
