# 2026-09-28 — DEBT-050 — Feature `performance` still makes 9 business decision(s) in the interface

- Found by: architecture rule A12 (TODO-047), first freeze
- Where: `performance/account_view/useAccountPerformance.ts` (2), `performance/global_view/useGlobalPerformance.ts` (3), `performance/shared/globalPresenter.ts` (3), `performance/shared/presenter.ts` (1)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `performance` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `performance` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `performance` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).
