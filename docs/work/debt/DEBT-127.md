# 2026-10-12 — DEBT-127 — What the spec check of prices found after the 2026-10-11 batch

- Found by: `spec-checker` on MKT-100 to MKT-104 of `docs/spec/market-price.md`, at the hand-over of the batch (2026-10-12); it only read. The second point was read again in the code by the main agent, the others were not
- Where: `price_as_of` in `src-tauri/src/context/asset/service.rs`; `PricedAsset::price_as_of` in `src-tauri/src/use_cases/shared/valuation.rs`; `get_account_details_as_of` in `src-tauri/src/use_cases/account_details/orchestrator.rs`; `price_named` in `src-tauri/src/use_cases/holding_transaction/named_listing.rs`
- Severity: 🔵
- Size: S — one pull request: backend.
- Observation: MKT-104 is implemented and every clause is exercised, by one test of the agent connection (`agt_071`): no test of the asset context names it. **Words ahead of the code.** MKT-104 says it is the resolution the as-of views value a holding with (PRF-022): the two views each keep their own — one parses dates and skips one it cannot read, one relies on the order the prices come in, `price_as_of` compares text — and no test holds the three equal. For an asset that does not exist `price_as_of` answers `AssetNotFound`, which the rule does not say and `price_named` turns into `DatabaseError`. The date is compared as text: "never a later one" holds for a date written `YYYY-MM-DD`, which the one caller ensures and the rule does not state
- User value: None directly.
- Done when: the as-of views and `get_price` resolve a price through one function, or MKT-104 stops saying they do; `price_as_of` has a test of its own; the rule says what it answers for an unknown asset and what form the date takes.
