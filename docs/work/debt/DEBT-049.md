# 2026-09-28 — DEBT-049 — Feature `categories` still makes 2 business decision(s) in the interface

- Found by: architecture rule A12 (TODO-047), first freeze
- Where: `categories/category_table/useCategoryTable.ts` (2)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `categories` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `categories` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `categories` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).
