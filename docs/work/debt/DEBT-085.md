# 2026-10-09 — DEBT-085 — The assets to settle are reported by the core and shown nowhere

- Found by: `spec-checker`, closing TODO-056
- Where: `get_assets_to_settle` in `src-tauri/src/context/asset/api.rs` (AST-035); no call in `src/features/assets/gateway.ts`, no command of the command line
- Severity: 🟡
- Observation: an asset that existed before kinds and that the rules would now refuse — a listed one without an ISIN, two that are the same — is kept as it is and reported by the core, but no screen and no command shows the report, so its owner is never told which assets to settle
- User value: A user sees which of their assets break a rule of their kind, and why, and settles each from its edit dialog.
- Done when: the assets to settle show where the owner decides — the assets table, a notice, the command line — each with its problem and a way to its edit dialog; a test holds it.

**Open questions:**

- [x] Where do the assets to settle show: a line or a badge on their row of the assets table, a notice above it, or both with a command of the command line? (A screen changes: a design to validate.) — As TODO-086 now says: a line above the table and a mark on each row, with a sentence and a fix per anomaly; this entry closes with TODO-086. (owner, 2026-10-18)
