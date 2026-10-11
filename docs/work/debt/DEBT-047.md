# 2026-09-28 — DEBT-047 — Feature `accounts` still makes 3 business decision(s) in the interface

- Found by: architecture rule A12 (TODO-047), first freeze
- Where: `accounts/account_table/useAccountTable.ts` (2), `accounts/shared/validateAccount.ts` (1)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `accounts` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `accounts` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `accounts` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).
