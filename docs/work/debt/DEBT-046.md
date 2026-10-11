# 2026-09-28 — DEBT-046 — Feature `account_details` still makes 10 business decision(s) in the interface

- Found by: architecture rule A12 (TODO-047), first freeze
- Where: `account_details/account_details_view/useAccountDetails.ts` (1), `account_details/account_details_view/useAccountDetailsView.ts` (3), `account_details/open_balance/OpenBalanceModal.tsx` (1), `account_details/shared/presenter.ts` (2), `account_details/shared/validateCashForm.ts` (1), `account_details/shared/validateFeeForm.ts` (1), `account_details/shared/validatePriceForm.ts` (1)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `account_details` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `account_details` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `account_details` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).
