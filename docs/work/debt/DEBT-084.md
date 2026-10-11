# 2026-10-09 — DEBT-084 — Three rules of the assets screen that the screen does not keep

- Found by: `spec-checker`, closing TODO-056
- Where: `src/features/assets/asset_table/AssetTable.tsx` — the empty state is tested before `fetchError` (AST-015), and the archive and unarchive confirmations close before the result is known, the failure showing above the table (AST-014); `activeCount` in `src/features/assets/useAssets.ts`, which nothing shows (AST-007)
- Severity: 🔵
- Observation: a failed first load shows "No assets" instead of the error and Retry; a refused archive is told outside the dialog that asked; the page header shows no count of active assets. All three are older than TODO-056, and none has a test
- User value: A user whose assets cannot be read is told so and can retry; a refused archive is explained where it was asked.
- Done when: each of the three is fixed with a test, or its rule is rewritten to what the screen does and the unused count removed.
