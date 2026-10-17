# 2026-10-17 — DEBT-134 — Recording by name and previewing it are two dispatches written alike

- Found by: `reviewer-arch` on TODO-076 (2026-10-17)
- Where: `src-tauri/src/use_cases/holding_transaction/named_recording.rs` (`record_named`, `record_on_holding`, `record_split_named`, `preview_named`); `orchestrator.rs` (`buy_holding`, `sell_holding`, `open_holding`, `record_split` and their `preview_*`)
- Severity: 🔵
- Size: S — one pull request: one resolver of the account, the asset, the date and the operation, used by both.
- Observation: `preview_named` repeats `record_named`'s lookup by name, its default date and its refusal mapping, and builds its operation through `trade_op` while recording passes its figures one by one. The checks before the account's and the account's own recording are shared (`ensure_*`, `apply`), so the two give the same refusals today; they stay aligned only because the dispatch is written the same way twice
- User value: None directly.
- Done when: recording and previewing a purchase, a sale, an opening balance or a split resolve their target and their operation through one function, and the `agt_072` tests stay as they are.
