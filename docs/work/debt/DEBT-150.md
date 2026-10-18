# 2026-10-18 — DEBT-150 — Older rules with no test of their own

- Found by: `spec-checker` at the hand-over of the 2026-10-18 batch, on main at 7938e09
- Where: account details, account performance, agent connection, market price, scheduled fetch
- Severity: 🔵
- Size: M — tests only.
- Observation: ACD-039/040 (a second fetch on TransactionUpdated/AssetUpdated), ACD-043 (a Sell removed recomputes the last sale date), ACD-047 (no closed section when empty); PRF-060 (AccountUpdated re-fetch), PRF-085 (a dividend after the close stays out); AGT-041 (the log of a call), AGT-043 (`AccountAmbiguous` in `list_holdings`, `DatabaseError` from the read tools), AGT-086 (a write started finishes when the agent leaves); MKT-027, MKT-112, MKT-199, no component test for `PriceHistoryModal` / `EditPriceForm`; SPF-016/020 (the `--scheduled-fetch` branch).
- User value: Nothing visible.
- Done when: a test per rule above; the frontend coverage floor rises if it moves a point.
