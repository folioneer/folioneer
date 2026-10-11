# 2026-10-10 — DEBT-101 — What the prune audit found in the files 0.7.1 changed

- Found by: `/prune` after v0.7.1 (2026-10-10), on `EnableSyncModal.tsx`, `useEnableSyncModal.ts` and `useDateField.ts`, with `coverage/frontend/lcov.info` of the same day
- Where: `src/ui/components/field/useDateField.ts` — `handleDateSelect` (lines 174–177), and lines 83 and 134; `src/features/settings/sync/enable_modal/useEnableSyncModal.ts` (lines 121 and 172)
- Severity: 🔵
- Observation: `handleDateSelect` builds an ISO date in four lines where `toIsoLocal`, defined in the same file with the same formula and used by `stepDate`, does it in one; the four lines are covered. Outside the audit's categories: the same file writes the ISO-date pattern inline twice where its constant `ISO_DATE` exists, and the sync hook writes `folderState?.holds_portfolio === true` twice
- User value: None — less code saying the same thing.
- Done when: `handleDateSelect` calls `toIsoLocal`; the two patterns use `ISO_DATE`; the sync hook names the expression once; the tests of both files pass unchanged.
