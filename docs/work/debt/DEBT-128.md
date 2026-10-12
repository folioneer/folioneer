# 2026-10-12 — DEBT-128 — What the prune of the 0.10.0 release recommends

- Found by: `/prune` on the 23 production files v0.10.0 changed, after the release (2026-10-12), against the coverage report of the same day; it read the lines the release changed and followed the symbols they add, not the unchanged parts of the larger files
- Where: `src-tauri/src/agent_connection/tools.rs` and `src-tauri/src/use_cases/holding_transaction/named_recording.rs` (`Refusal`); `src-tauri/src/use_cases/holding_transaction/mod.rs` (`TRANSACTIONS_PER_PAGE`)
- Severity: 🔵
- Size: S — one pull request: backend.
- Observation: two recommendations, both for a person to judge. `Refusal` is defined twice with the same two fields, in the use case and in the agent connection, and `tools.rs` rebuilds one from the other field by field in six places, three added by this release (`tools.rs` 620 of 659 lines covered). `TRANSACTIONS_PER_PAGE` is re-exported from its module and read by nothing outside its file (covered). Not recommended, as no test reaches them: three helpers with the same body — `DatabaseError`, "the portfolio could not be read" — in `named_listing.rs`, `command_line/orchestrator.rs` and `tools.rs`
- User value: None directly.
- Done when: one `Refusal`, or one conversion between the two; the re-export gone or read; the three helpers covered by a test of a failed read, then one.
