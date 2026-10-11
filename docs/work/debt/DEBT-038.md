# 2026-09-23 — DEBT-038 — A build without an External provider refuses fetch commands with the runtime's own error

- Found by: reviewer-security (TODO-036)
- Where: `src-tauri/src/lib.rs` (the three fetching use cases are managed only with an External provider), `src-tauri/src/core/specta_builder.rs` (their commands stay registered in every build)
- Severity: 🔵
- Observation: in such a build `fetch_all_asset_prices`, `fetch_account_asset_prices`, `configure_scheduled_fetch`, `get_scheduled_fetch_status` and `backfill_holding_price_history` reach no managed state, so Tauri refuses the call before the command body runs. That satisfies MKT-210 and SPF-070 — nothing is fetched or written, and the message names only the command and its argument, both public in `bindings.ts` — but the refusal is a plain string, not a `{ code }` error, so the error model's typed pipeline does not see it. Nothing calls these commands today: the interface hides every control that would (MKT-212).
- User value: None today.
- Done when: a call to any of those commands in a build without an External provider answers a typed code the frontend presenter can map, through one shared guard rather than five hand-written checks — worth doing when a second optional capability (a bank feed, the advice module) makes the pattern repeat.
