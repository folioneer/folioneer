# Business Rules — Stock Split (SPL)

## Context

A stock split (or reverse split) is a corporate action that rescales a held position without changing its value: the issuer multiplies the share count and divides the per-share price by the same factor. Reference case: Alphabet's 20-for-1 split effective 2022-07-15 — 10 shares at ~2,235 USD became 200 shares at ~112 USD, same position value, same ticker. The position stays the **same asset** — a split never forks the holding, its cost basis, or its history. (A corporate rename such as Google → Alphabet in 2015 is likewise an edit of the existing asset's name/reference, never a new asset.)

## Entity (reused)

SPL reuses the TRX `Transaction` entity with a new `TransactionType::Split` variant. The wire encoding is load-bearing: the **split factor** rides micro-scaled in the `quantity` field; `unit_price`, `fees`, `total_amount` are `0`; `exchange_rate` is `1_000_000`.

---

## Business Rules

### Recording

**SPL-010 — Split transaction (backend)**: A new `TransactionType::Split` variant is introduced. `record_split` records a `Split` transaction on a held position `(account_id, asset_id)` at a date, carrying the split factor micro-scaled in `quantity` (20-for-1 → `20_000_000`; 1-for-10 reverse → `100_000`; 3-for-2 → `1_500_000`); the money fields are zero per the entity note (a split moves no money and has no FX leg), so the `Split` factory bypasses the generic money validation exactly as `FreeShares` / `Interest` / `ManagementFee` do. Date bounds are the shared transaction date validation (not in the future, TRX-046-style). An optional note is allowed.

**SPL-011 — Factor validation (backend)**: The factor must be strictly positive (`SplitFactorNotPositive`) and different from ×1 (`SplitFactorIsOne` — a no-op split is a data-entry error). Both bounds are enforced in the domain factory.

**SPL-012 — Eligible position (backend)**: A split applies to shares actually held. The use case requires the asset to exist (`AssetNotFound`) and to be **currently held** — quantity > 0 today (`AssetNotHeld`) — matching the FreeShares/Interest cross-BC guard family (FSD-011); a backfilled split on a since-sold position is therefore rejected at the boundary. The domain replay additionally rejects a split dated where the replayed position quantity is zero (`ClosedPosition`). The cash line cannot be split (`SplitOnCashAsset`).

### Position rescale

**SPL-020 — Value-neutral rescale (backend)**: During holding replay, a `Split` transaction rescales the running position at its date: `quantity ← floor(quantity × factor / MICRO)`, then `average_price ← round(previous_average_price × previous_quantity / new_quantity)` — deriving the new average from the preserved cost basis rather than dividing by the factor, so `quantity × average_price` (the cost basis) is preserved up to last-micro rounding across the rescale. Realized P&L already booked is untouched; sells after the split consume the rescaled average price.

**SPL-021 — Position-collapse guard (backend)**: A rescale whose `new_quantity` floors to `0` on an open position is rejected (`SplitCollapsesPosition`) — on the record path and on every replay-revalidation path (corrections that move transactions across the split, cancellation of a purchase the split depended on). The guard runs before the average-price derivation, so the division in SPL-020 is never reached with a zero quantity.

**SPL-022 — Chronological interplay (backend)**: The rescale participates in the normal chronological replay: corrections that move a purchase/sell across the split date, cascading oversell checks, and deletions all evaluate against the timeline with the split applied at its date. Within a single date, replay order is insertion order (the TRX-036 `created_at` tiebreak) — recording the split after the day's trades applies it after them.

### Corrections

**SPL-030 — Correction and deletion (backend)**: `correct_transaction` on a `Split` row may change the date, the factor (validated per SPL-011), and the note — the money fields are ignored. A split is deleted via the existing `cancel_transaction` command, replaying the holding without it. Both paths re-run the SPL-012 eligibility, the SPL-021 collapse guard, and the cascading oversell validation.

### Valuation continuity

**SPL-040 — Post-split price record (frontend)**: Recorded market prices are point-in-time observations and are never retro-adjusted. Without a post-split observation, carrying a pre-split price forward across the split date would misvalue the rescaled position by the factor. The split modal therefore offers a **"Record post-split price"** checkbox, checked by default, pre-filled with `round(latest price strictly before the split date ÷ factor)` in the asset currency and editable; on submit it records that price at the split date through the existing price-record flow (best-effort, like MKT-055). The prefill is empty and the checkbox unchecked when no prior price exists.

### Performance

**SPL-050 — Performance neutrality (backend)**: A split is not a flow and not performance: it contributes `0` to `cash_flow`, `asset_flow` and `dividends` in every bridge scope (PRF-070/071, PRF-084, GPF-040), adds no Dietz flow in windowed or lifetime metrics, and adds no market-valued rate date. With the post-split price recorded (SPL-040), the position's end value is continuous across the split and the split itself produces no pnl.

### Display

**SPL-060 — Journal rendering (frontend)**: A split row in the journal/transaction tables shows the factor in the quantity column as "×N" with the micro factor formatted as a trimmed decimal ("×20", "×1.5", "×0.1"); the money columns render "—". Split rows are editable per SPL-030 through the edit affordance and deletable like any transaction.

**SPL-061 — Split affordance (frontend)**: The account-details holding row offers a "Split" action (icon button, non-cash active holdings only, hidden in as-of view) opening the split modal: ratio input as **new : old** positive-integer pair (factor = `round(new × MICRO / old)`, so non-terminating ratios like 1:3 round at the micro), date, the SPL-040 price checkbox, optional note, and a preview of the resulting quantity and average price. Submit is disabled while the ratio is invalid (SPL-011) or the preview quantity would floor to zero (SPL-021).

---

## UX Draft

- **Entry point**: holding-row icon button ("Split", scissors-style icon) next to the existing Buy/Sell actions; hidden for cash rows, archived assets, and in as-of view.
- **Modal** (`FormModal`, `split-trx-*` stable ids): date field (default last-operation date), ratio pair `new : old` (two integer inputs side by side, default 2 : 1), read-only preview line "10 shares @ 150.00 → 20 shares @ 75.00", "Record post-split price" checkbox + editable derived price field (SPL-040), note textarea, Cancel/Save footer.
- **States**: submit disabled on invalid ratio / collapsing preview / while saving; backend rejection shown inline (F27); success closes the modal and refreshes the view (snackbar).

## Open Questions

None — same-date ordering, factor rounding, and the collapse guard are resolved above.
