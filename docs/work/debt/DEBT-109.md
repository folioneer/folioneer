# 2026-10-10 — DEBT-109 — What the reading of the asset contract found in the asset specs and the code

- Found by: `contract-reviewer` on `docs/contracts/asset-contract.md`, closing DEBT-096 (2026-10-10)
- Where: `docs/spec/asset.md`; `docs/spec/market-price.md` (MKT-083, MKT-090); `docs/spec/cash-tracking.md` (CSH-016); `src-tauri/src/use_cases/archive_asset/orchestrator.rs`; `src-tauri/src/use_cases/delete_asset/orchestrator.rs`
- Severity: 🟡
- Observation: **no rule.** No rule says an asset an account still holds cannot be archived (`ActiveHoldings`), nor that an asset a transaction names cannot be deleted (`ExistingTransactions`); the asset spec lists hard delete under its future features while the window calls `delete_asset`. **Order.** Both use cases ask the account context before they look at the asset, so a Cash Asset that is held answers `ActiveHoldings` / `ExistingTransactions` where CSH-016 promises `CashAssetNotEditable`. **Stale names.** MKT-083 and MKT-090 name `NotFound` / `Unknown` where the code answers `PriceNotFound` / `DatabaseError`; CSH-016 names four `…CommandError` types that no longer exist.
- User value: None directly — archiving and deleting an asset refuse what their rules say, in the order they say.
- Done when: the asset spec has the two rules and the contract rows name them; the Cash Asset refusal comes first, with a test, or CSH-016 says otherwise; the three rules name the codes the code returns.
