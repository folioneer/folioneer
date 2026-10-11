# 2026-09-28 — DEBT-053 — Feature `whats_new` still makes 3 business decision(s) in the interface

- Found by: architecture rule A12 (TODO-047), first freeze
- Where: `whats_new/parseChangelog.ts` (3)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `whats_new` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `whats_new` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `whats_new` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).
