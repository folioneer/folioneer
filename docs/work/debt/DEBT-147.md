# 2026-10-18 — DEBT-147 — Account details shows no total unrealized P&L, and a price deletion does not name the price

- Found by: `spec-checker` at the hand-over of the 2026-10-18 batch, on main at 7938e09
- Where: `src/features/account_details/` (presenter, `PriceHistoryModal.tsx:230`), `src/lib/store.ts:211`
- Severity: 🟡
- Size: S.
- Observation: MKT-041 (the account's total unrealized P&L in the details summary) is not implemented: only the accounts list shows it (ACC-023). MKT-089: the delete confirmation names the date, not the price. MKT-037: `AssetPriceUpdated` is missing from `locallyHandledEvents`, so each one logs "unhandled event".
- User value: The account page shows what its unrealized gain is; deleting a price says which one.
- Done when: MKT-041 is shown in the summary (or the rule removed in favour of ACC-023, an open question if a design is needed); the confirmation reads "Delete the price {{price}} recorded on {{date}}?" in English and French; the event is handled; each with a test.
