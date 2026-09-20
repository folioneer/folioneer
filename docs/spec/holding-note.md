# Business Rules — Holding Note (HNO)

## Context

The user can pin a free-text note to a line they hold — a position `(account, asset)` — as a reminder or intention, e.g. on Air Liquide: "acheter 7 actions si moins de 150€". A note optionally carries a **price alarm**: a share-price threshold and direction; when the asset's current price crosses it, the line shows an active bell. This note is per-position and independent of the per-transaction `note` field. In-app only — no OS notification.

## Entity Definition

`HoldingNote` — a new persisted entity **owned by the `account` bounded context** (its own repository + service methods; the `account_details` use case joins it into the read).

| Field                 | Type                         | Notes                                      |
| --------------------- | ---------------------------- | ------------------------------------------ |
| `account_id`          | string (FK, cascade)         | PK part — the owning account               |
| `asset_id`            | string (FK, cascade)         | PK part — the held asset                   |
| `text`                | string                       | required, trimmed, 1–500 chars             |
| `threshold_price`     | i64 micro, nullable          | asset-currency share price; alarm part 1   |
| `threshold_direction` | `Below` \| `Above`, nullable | alarm part 2; both alarm fields or neither |

---

## Business Rules

### Model & persistence

**HNO-010 — One note per holding (backend)**: A holding note belongs to a `(account_id, asset_id)` pair — at most one per pair, persisted in its own `holding_note` table (holdings themselves are replay-derived and carry no editable state). Both foreign keys cascade on delete, so removing the account or the asset removes its notes.

**HNO-011 — Validation (backend)**: `text` must be non-empty after trimming (`NoteTextEmpty`) and at most 500 characters (`NoteTextTooLong`). When an alarm is present, `threshold_price` must be strictly positive (`ThresholdNotPositive`); a direction without a threshold, or a threshold without a direction, is rejected (`ThresholdIncomplete`). The account must exist; the asset must not be the cash line (`NoteOnCashAsset`); the pair must have holding history — at least one transaction entry — (`NoteOnUnheldAsset`). Whether the asset is archived is not checked (the FE affordance only appears on rendered holding rows).

### Commands

**HNO-020 — Upsert (backend)**: `upsert_holding_note` creates or replaces the note for the pair (validated per HNO-011). Editing is a full replace — the modal is pre-filled from the stored note.

**HNO-021 — Delete (backend)**: `delete_holding_note` removes the note for the pair; deleting a non-existent note is a no-op success.

**HNO-022 — Refresh after mutation (frontend)**: On upsert/delete success the note modal's success path re-fetches the account-details read (the same `data.retry()` pattern as the transaction modals), so the row's note text and bell reflect the change immediately. No new backend event is required.

### Alarm semantics

**HNO-030 — Stateless live trigger (backend)**: The alarm has no persisted state. On every account-details read, `alarm_triggered` is computed from the holding's current price (the same resolved price the read already carries): `Below` triggers when `current_price < threshold_price`, `Above` when `current_price > threshold_price` — strict comparisons; equality with the threshold triggers neither. No price available → not triggered. The alarm re-arms by itself when the price moves back across the threshold; there is no acknowledgement.

**HNO-031 — Threshold is nominal (backend)**: `threshold_price` is a nominal amount in the asset's current currency. Editing the asset's currency does not re-base stored thresholds — the number is simply reinterpreted; the user edits the note if the nominal no longer makes sense.

### Display

**HNO-040 — Read surface (backend)**: The account-details read returns the note (text, threshold, direction, `alarm_triggered`) on each `HoldingDetail`; holdings without a note return null. The as-of (historical) view omits notes — they are a live-view affordance.

**HNO-041 — Row rendering (frontend)**: On the account-details holding row, the note text renders under the asset name (single line, truncated with ellipsis, full text as tooltip). When an alarm exists, a bell icon precedes the text: outline in the on-surface-variant tone while armed, filled in the error tone while triggered (HNO-030). No note → nothing renders.

**HNO-042 — Note affordance (frontend)**: The holding row offers a "Note" action (icon button, non-cash rows, hidden in as-of view) opening the note modal: textarea, an "Alert me when the price crosses" toggle revealing direction (below/above) + amount (asset currency) fields, and — when a note already exists — a delete action. Save calls upsert (HNO-020); inline validation mirrors HNO-011; submit is disabled while saving; a backend rejection is shown inline (F27); success closes the modal, refreshes per HNO-022, and shows a snackbar.

**HNO-043 — HoldingNoteUpdated event registration (frontend + backend)**: `HoldingNoteUpdated` is added to the event-bus enum, published by the `account` bounded context whenever a note is written or deleted on this device (HNO-020/021). The Account Details view re-fetches on receipt; a note applied from another device reaches it through `SyncCompleted` (SYN-064). The global store treats it as a locally-handled event (no global re-fetch). `ARCHITECTURE.md` registers it in the event-bus table.

---

## UX Draft

- **Entry point**: holding-row icon button ("Note", sticky-note icon) in the row action cluster; non-cash rows, hidden in as-of view. Rows with an existing note render the text under the asset name + optional bell (HNO-041).
- **Modal** (`FormModal`, `holding-note-*` stable ids): textarea (autofocused), alarm toggle → direction select (below/above) + price field with the asset currency suffix, footer Cancel / Delete (existing note only, destructive style) / Save.
- **States**: save disabled on empty text / incomplete alarm / while saving; inline error on rejection; snackbar + refresh on success.

## Open Questions

None — alarm equality semantics, currency staleness, eligibility, and refresh are resolved above.
