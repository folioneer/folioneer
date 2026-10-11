# 2026-10-11 — DEBT-116 — What the spec check of prices found after TODO-059

- Found by: `spec-checker` on the rules the batch changed in `docs/spec/market-price.md`, at the hand-over of the batch (2026-10-11), on the tree of TODO-059's second pull request before an agent's price became a plain upsert; it only read. Split from DEBT-113 on 2026-10-11. Nothing below was read again by the main agent
- Where: `src-tauri/src/context/asset/service.rs` (tests of `record_asset_price`, `update_asset_price`, `record_asset_price_by_agent`); `src-tauri/src/context/asset/repository/asset_price.rs`; `src/features/account_details/price_history/PriceHistoryModal.tsx`; `src/features/account_details/shared/presenter.test.ts`
- Severity: 🔵
- Size: M — one pull request: backend tests and frontend tests.
- Observation: every rule read is implemented. **A clause without a test.** MKT-103: that a price an agent recorded publishes `AssetPriceUpdated`; a price recorded on an asset under the refresh lock; the origin of the change as the repository writes it (only `origin_of` is tested). MKT-101: that `record_asset_price` and an edit stamp `Manual` — no test edits a fetched or an agent's price and reads `Manual` back. MKT-152: no test records, edits or deletes a price on a locked asset. MKT-141: no test of the price history's source badge. MKT-142: `toHoldingRow` with the source `Agent`.
- User value: None directly — what a price's source promises is each held by a test.
- Done when: each clause above has a test that fails when the clause is broken.
