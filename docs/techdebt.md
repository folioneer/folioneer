# Tech Debt

Observations of code smells, brittle patterns, or pre-existing issues surfaced
during work that don't warrant immediate action. The entry format is in
`docs/workflow.md` § 2.

Entries are observations, not commitments, and this file is the agent's: it
files here what it notices and what it did not fix. Each entry carries a
permanent `DEBT-NNN` reference (never renumbered, never reused; next free:
DEBT-102) so the human can queue it in `docs/todo.md` § Next like any todo.
Remove an entry once it has been resolved.

---

## 2026-08-23 — DEBT-001 — Local writes do not take the sync gate

- Found by: reviewer-security + reviewer-backend (PR-C, `.review/reviewer-security-2026-08-23-01.md`)
- Where: src-tauri/src/context/sync/application/run.rs (`SyncGate`), every synced repository write
- Severity: 🟡
- Observation: SYN-064 says a local write and an in-progress apply never interleave. The apply holds `SyncGate` and runs in one SQLite write transaction with the change recorder suspended; local writes do not take the gate. SQLite's single writer serialises them at the database level and the recorder reads the logical clock under that lock, so the remaining window is a local write that computed `based_on` before an apply committed — benign for the rank order, but not the guarantee the spec states. Closing it means a `begin_write()` helper on the recorder that takes the gate before opening the transaction, applied at all 30 capture sites — its own PR.
- User value: None observable — the remaining window is benign for merge order.
- Done when: A `begin_write()` recorder helper takes `SyncGate` before opening the transaction at all 30 capture sites, so SYN-064 holds as written.

## 2026-08-23 — DEBT-002 — Held-back changes and conflict notices have no bound

- Found by: reviewer-security (PR-C)
- Where: src-tauri/src/context/sync/application/run.rs (`apply_intake`), `held_back_changes`, `conflict_notices`
- Severity: 🟡
- Observation: A hostile or buggy peer could grow `held_back_changes` without bound (every run retries all of them) and `conflict_notices` never evicts. Unreachable for one user's own desktops; add caps / eviction before any multi-user or untrusted-peer scenario. Note (2026-09-12): a cap or eviction contradicts SYN-066 as written ("persist until the user dismisses them individually"), so this is a spec decision before it is a code change.
- User value: None for a user's own devices; bounds growth caused by a buggy or hostile peer.
- Done when: `held_back_changes` has a cap and `conflict_notices` evicts, both covered by tests.

## 2026-08-23 — DEBT-003 — Join replays a device's whole history in memory

- Found by: reviewer-security (PR-C)
- Where: src-tauri/src/context/sync/application/join.rs
- Severity: 🔵
- Observation: Only the per-file 64 MiB cap bounds a join; the full history of each device is held in memory inside one transaction. Acceptable under the KISS cut (a personal portfolio's history is a few KB a month); revisit with checkpoints if history or device count grows.
- User value: None at present history sizes.
- Done when: Join streams or checkpoints history instead of holding every device's full history in one in-memory transaction.

## 2026-08-22 — DEBT-004 — Account-deletion cascade is no longer a single transaction

- Found by: reviewer-backend (PR-A change capture, `.review/reviewer-backend-2026-08-22-01.md`)
- Where: src-tauri/src/context/account/service.rs (`delete` / `remove_children`)
- Severity: 🟡
- Observation: To record one change + tombstone per child (SYN-024, CFR-030), `AccountService::delete` now removes transactions, holding notes, fee schedules and catch-up positions through their own repositories — each atomic with its own change — before deleting the account. Previously a single `DELETE` with `ON DELETE CASCADE` did it all in one transaction. A crash mid-cascade leaves a half-deleted account (recoverable on retry, never silently diverging, since every child removal carries its change). Restoring single-transaction semantics needs a transaction spanning several repositories — the unit of work ADR-006 describes and the codebase never built (see the ADR-006 entry above). Fold the cascade into that unit of work when it lands.
- User value: A crash midway through deleting an account cannot leave it half-deleted.
- Done when: The cascade runs inside one unit of work spanning the child repositories.

## 2026-08-22 — DEBT-005 — ADR-006 unit of work is accepted but unimplemented

- Found by: feature-planner (multi-device sync plan, D1) — confirmed by plan-reviewer and by grep
- Where: src-tauri/src/ (no `UnitOfWork` / `TransactionManager` / `uow` anywhere; three raw `sqlx` transactions at `context/account/repository/account.rs:268`, `context/asset/repository/category.rs:109`, `context/asset/repository/asset_price.rs:128`)
- Severity: 🟡
- Observation: `docs/adr/006-unit-of-work.md` is Accepted, but nothing in the codebase implements it; cross-aggregate writes open ad-hoc transactions. Change capture (ADR-019) does not use it either: it goes through a `ChangeRecorder` port on the live connection. Decide later whether to implement ADR-006 (and route the recorder through it) or supersede it; status left Accepted meanwhile.
- User value: None directly.
- Done when: ADR-006 is implemented and the three ad-hoc `sqlx` transactions route through it, or the ADR is superseded.

## 2026-07-25 — DEBT-006 — The Linux package ships a development-only helper binary

- Found by: main-agent
- Where: src-tauri/Cargo.toml (`[[bin]] generate_bindings`), src-tauri/tauri.conf.json
- Severity: 🔵
- Observation: The `.deb` packages `generate_bindings`, the helper that regenerates the TypeScript bindings, next to the application; the Windows installer is likely affected the same way. It is never run by the application and only adds a few megabytes. The other half of this entry — the application binary shipped under the template name `tauri-app` — was fixed by the rename to Folioneer (TODO-033), which installs fresh and so crosses no updater path.
- User value: None visible — no development tool ships inside the package.
- Done when: `generate_bindings` is no longer in the `.deb` nor in the Windows installer, and `just generate-types` still works.

---

## 2026-05-10 — DEBT-008 — Migrate to FE gold layout

- Found by: manual (frontend architecture delta scan)
- Where: src/ (top-level structure + features/account_details cross-imports)
- Severity: 🟡
- Observation: Three FE layout/coupling deltas from the gold layout (F26/F27/F28 in `docs/frontend-rules.md`). The current shape works but encodes implicit conventions that diverge from it. Migration is bit-by-bit per `docs/workflow.md` § 10 — apply gold to new code; defer existing-code reshape unless it fits the 50-LOC + locality + mechanical gates.
  1. **`features/account_details/{buy,sell}_transaction/` cross-imports from `features/transactions/`.** Today the imports are `RecordPriceCheckbox` (component), `TransactionFormData` (type), and `useTransactions` (hook with state). Under F26, the first two (primitives) become fine; the third (behavior coupling via a hook) remains a code smell. Either `account_details` owns its own thin wrapper around the gateway calls it needs, or the two features consolidate. The transaction draft check (TRX-062) already takes the first road: `shared/useTransactionDraftCheck.ts` exists in both features, the same draft builder and error mapping on the generic `ui/hooks/useLatestCheck`, chosen by the owner on 2026-09-28 over a hook import; the consolidation removes the copy. Worth deciding _with_ the consolidation question (delta 2) rather than fixing the hook coupling alone.

  2. **`account_details` sub-feature bloat (8 sub-features).** Half of them — `buy_transaction`, `sell_transaction`, `deposit_transaction`, `withdrawal_transaction` — are conceptually transaction-recording flows and overlap with the `transactions/` feature. Two reasonable shapes: (a) consolidate the four into `transactions/` and let `account_details` stay focused on the holdings view, or (b) formalize the split — `account_details` owns "modals invoked from the holding row," `transactions/` owns "the transaction list page and its CRUD." Pick (b) as the lighter move; (a) is a bigger refactor.

  3. **`src/lib/*Storage.ts` adapters belong in `src/infra/settings/`.** The browser-`localStorage` UI-preference adapters (`autoFetchStorage.ts`, `autoRecordPriceStorage.ts`, `lastOperationDateStorage.ts`, `closedSectionStorage.ts`) are platform adapters per F28's Store-kinds table and should move to `src/infra/settings/`. New ones keep landing in `src/lib/` to stay consistent with their siblings (a partial move would orphan one file mid-migration). Mechanical folder move + import-path update; fold into the same `lib/ → infra/` rename PR.

  Migration is mechanical for delta 3 (folder move + import sites) and conventional for deltas 1 and 2 (depends on the consolidation decision). Cleanest as one or two dedicated pull requests.

- User value: None — internal layout.
- Done when: `src/lib/*Storage.ts` moves to `src/infra/settings/`, the `useTransactions` cross-feature coupling is removed, and the `account_details` / `transactions` split is formalised.

## 2026-05-09 — DEBT-009 — Migrate to gold DDD layout

- Found by: manual (backend layout design discussion)
- Where: src-tauri/src/ (top-level structure)
- Severity: 🟡
- Observation: Three layout deltas from the gold target (B0/B37–B43 in `docs/backend-rules.md`). The current shape works but documents the architecture imperfectly to newcomers. Migration is bit-by-bit per `docs/workflow.md` § 10 — apply gold to new code; defer existing-code reshape unless it fits the 50-LOC + locality + mechanical gates.
  1. **`service.rs` lives at the BC root, not in `application/`.** Inconsistent with `domain/` and `repository/` (which ARE folders). Migrate `service.rs` → `application/service.rs` per BC. The `account/` BC has no `application/` folder at all (`{BC}Error` lives at the BC root, per error-model gold); the folder arrives with this migration — an empty layer folder while the BC is otherwise old-layout would be speculative scaffolding (a B38 gap, accepted until then).

  2. **`repository/` should be `infrastructure/`** (DDD layer name). `repository/` is one TYPE of infrastructure; renaming protects against the day a BC adds an external API client, cache adapter, or message-queue subscriber (avoids proliferating peer folders). Today the folder only contains repository impls — stay flat (`infrastructure/{aggregate}.rs`) until non-repo infra arrives, then add siblings without nesting.

  3. **`core/` should be `shared/`**, restructured into the three DDD layer folders. `core/` overpromises ("central business logic" — but BCs ARE the business). Target shape:

     ```
     shared/
     ├── application/error.rs        ← shared InfrastructureError
     ├── domain/cash.rs              ← shared kernel (system_cash_asset_id)
     └── infrastructure/{db, event_bus, logger, specta_*, uow}
     ```

     `InfrastructureError` reclassifies as application-layer (it's the typed application translation of opaque infra failures, per the DDD doc's travel rule — the NAME describes the source, the LAYER is application).

  Migration is mechanical (folder moves + module-path updates, ~50–100 import sites total). Cleanest as a single dedicated chore pull request.

- User value: None — internal layout.
- Done when: `service.rs` moves to `application/service.rs`, `repository/` becomes `infrastructure/`, and `core/` becomes `shared/{application,domain,infrastructure}` across every bounded context.

## 2026-09-12 — DEBT-010 — A different portfolio's folder reads as a reset

- Found by: manual (closing the empty-sync-folder todo)
- Where: src-tauri/src/context/sync/application/run.rs (`header_gate`)
- Severity: 🔵
- Observation: When a removable volume's drive letter is reused by a different stick that happens to carry a `Folioneer` folder, the header decodes but its passphrase check fails — exactly what a genuine "started over elsewhere" (SYN-071) looks like from the header alone. Both write a fresh header with a new creation mark, so the two cases are indistinguishable by content; the device reports `PortfolioReset` (SYN-084) where "this is another portfolio" would be the honest message. `FolderHoldsOtherPortfolio` exists as the enable-path error but nothing in the run can justify raising it.
- User value: The reset message would not fire for a stick that merely took the same drive letter.
- Done when: A sync run can tell a reset of its own portfolio from another portfolio's folder — by something other than the header's content — and reports `FolderHoldsOtherPortfolio` for the latter.

## 2026-09-12 — DEBT-012 — Ubiquitous-language Domain Events table lags the event enum

- Found by: reviewer-arch (T2)
- Where: docs/ubiquitous-language.md § Domain Events; src/lib/store.ts `locallyHandledEvents`
- Severity: 🔵
- Observation: The table omits `AssetPriceFetchProgress`, which the enum carries; the asset contract's Events table (`docs/contracts/asset-contract.md`) likewise omits `CategoryUpdated`; and `AssetPriceUpdated` (MKT-037) is handled by its own views yet is absent from the store's locally-handled allowlist, so each publish logs an "unhandled event" debug line. The two events added today are registered in both places; the older gaps are untouched.
- User value: None — documentation and a debug-log nuisance.
- Done when: every `Event` variant has a row in the table and in its owning contract's Events table, and the allowlist names every event the global store deliberately ignores.

## 2026-09-13 — DEBT-018 — 118 interactive components in feature code carry no id

- Found by: manual (`python3 scripts/arch-check.py`, rule A6, first run)
- Where: 34 files under src/features/ listed in arch-allowlist.json `missing_ids`; mostly Cancel and secondary buttons in modals, the price-history and update-banner actions, and the design-system dev page
- Severity: 🔵
- Observation: E1–E4 ask every interactive element for a stable id, and the E2E suite selects by id, yet 118 (119 at the first run) of the 317 `Button` / `IconButton` / `TextField` / `DateField` / `CalcField` / `FAB` tags rendered by feature code have none. The architecture check freezes today's count per file and refuses any growth; the count can only go down. The 18 sibling-feature imports the same check freezes belong to the FE gold layout migration entry above; the 8 `Math.` uses are display rounding and the documented split preview (SPL-061) and need no action.
- User value: None — every control becomes addressable by tests and assistive tech.
- Done when: `missing_ids` in arch-allowlist.json is empty.

## 2026-09-14 — DEBT-025 — "Reference currency" names two different things in the vocabulary

- Found by: spec-reviewer (ACC-027–033 review on TODO-007)
- Where: `docs/ubiquitous-language.md` (Cash Holding, Dividends received, Management fees — "the account's reference currency"), `docs/spec/global-performance.md` GPF-011 and the ACC / PMV totals ("the reference currency", EUR); the same table is also "Accounts table" (ACC-021/023/026), "accounts list" (PMV-013/016, SYN-040) and "account table" (ACC-008/030–033)
- Severity: 🔵
- Observation: the vocabulary uses "the account's reference currency" for an account's own currency, while GPF-011, the price movement total and the accounts-list portfolio total use "the reference currency" for the fixed EUR every cross-account figure is reported in; the two meanings share one phrase, and neither "cross-account reference currency" nor "portfolio total" has an entry of its own.
- User value: None — one word per concept in the specs and the code.
- Done when: the vocabulary names the account's own currency and the cross-account reference currency with distinct, confirmed terms, and has a confirmed "portfolio total" entry, each validated by the human.

## 2026-09-14 — DEBT-026 — Nothing follows an account currency's rate to the reference currency

- Found by: spec-reviewer (ACC-027/028 second pass on TODO-007)
- Where: `docs/spec/fx-rate.md` FXR-013 / FXR-071 (only asset → account pairs are followed), `docs/spec/global-performance.md` GPF-020, `docs/spec/account.md` ACC-027/028, `src-tauri/src/use_cases/account_summary/orchestrator.rs` (portfolio total)
- Severity: 🟡
- Observation: rates are fetched and followed for the pairs an asset needs to reach its account's currency, but no rule follows the pair from an account's currency to the cross-account reference currency; a USD account holding only USD assets therefore never gets a USD → EUR rate unless the user declares the pair, and the accounts-list portfolio total stays marked partial for it (the global performance view has the same dependency).
- User value: the portfolio total and the global performance figures count every account without the user having to declare a pair first.
- Done when: an account whose currency differs from the reference currency has its pair followed like an asset pair (or the application asks the user to declare it), stated as an FXR rule, and a USD-only account with a fetched rate no longer leaves the total incomplete.

## 2026-09-15 — DEBT-027 — ADR-012 does not record the price history backfill's fill-only exception

- Found by: spec-reviewer (MKT-190–199 second pass on TODO-008)
- Where: `docs/adr/012-latest-write-wins-source-as-metadata.md` (decision 1), `docs/spec/market-price.md` MKT-192
- Severity: 🔵
- Observation: ADR-012 decision 1 says every price write upserts unconditionally; MKT-192 exempts the price history backfill, which writes only dates without a price. The spec names the exception and the ADR does not, so a reader of the ADR alone misses it; the ADR status vocabulary has no "amended by" form yet (DEBT-007).
- User value: None — the decision record matches the rules.
- Done when: ADR-012, or an ADR that supersedes it, names the fill-only exception, and adr-reviewer passes it.

## 2026-09-19 — DEBT-030 — A sync that only holds changes back, or raises a notice, announces nothing

- Found by: contract-reviewer (SYN-064 amendment on TODO-020)
- Where: src-tauri/src/context/sync/application/service.rs (`remember_run`), docs/contracts/sync-contract.md (Events, `SyncCompleted`), src/features/shell/sync_indicator/useSyncIndicator.ts
- Severity: 🔵
- Observation: `SyncCompleted` is raised when a sync applied at least one change or when its failures or paused state changed. A sync that applied nothing but held changes back (SYN-041), or whose only outcome is a conflict notice on a losing incoming creation (SYN-066, CFR-060), changes what the sync status reports — last sync time, held-back count, notices — without raising the event, so the header indicator's attention badge follows only at the next status read.
- User value: The attention badge appears as soon as a sync leaves something for the user to look at, not at the next launch or the next sync that applies a change.
- Done when: a sync whose held-back count or notice count differs from the previous one raises `SyncCompleted`, covered by a `remember_run` unit test, and SYN-064 and the sync contract state the full trigger set.

## 2026-09-19 — DEBT-032 — Two features carry the same two-way entry-mode toggle

- Found by: reviewer-frontend (TODO-018)
- Where: src/features/transactions/shared/EntryModeToggle.tsx, src/features/account_details/management_fee_transaction/ManagementFeeModal.tsx
- Severity: 🔵
- Observation: the buy / sell forms' price-or-total toggle and the management fee form's percentage-or-resulting-quantity toggle are the same radiogroup of two small buttons, written twice; a change to the look or to its keyboard behaviour has two places to drift.
- User value: None — one look for every two-way entry toggle.
- Done when: a generic two-option toggle lives in `src/ui/components/` and both forms use it.

## 2026-09-20 — DEBT-036 — The vocabulary has no entry for the update feature's terms

- Found by: spec-reviewer (UPD-028 review on TODO-037)
- Where: docs/ubiquitous-language.md (no Update section); docs/spec/update.md, docs/adr/020-one-extension-file-per-build.md, docs/contracts/update-contract.md
- Severity: 🔵
- Observation: "update channel", "update server", "update file", "credentials", "refused access", "distribution channel", "public build" and "private build" are used as terms by the spec, the ADR, the contract and the code, but the vocabulary defines none of them. Terms are the owner's to confirm (B5).
- User value: None.
- Done when: The vocabulary carries an Update section whose terms the owner has confirmed, and the spec, ADR, contract and code use them.

## 2026-09-23 — DEBT-038 — A build without an External provider refuses fetch commands with the runtime's own error

- Found by: reviewer-security (TODO-036)
- Where: `src-tauri/src/lib.rs` (the three fetching use cases are managed only with an External provider), `src-tauri/src/core/specta_builder.rs` (their commands stay registered in every build)
- Severity: 🔵
- Observation: in such a build `fetch_all_asset_prices`, `fetch_account_asset_prices`, `configure_scheduled_fetch`, `get_scheduled_fetch_status` and `backfill_holding_price_history` reach no managed state, so Tauri refuses the call before the command body runs. That satisfies MKT-210 and SPF-070 — nothing is fetched or written, and the message names only the command and its argument, both public in `bindings.ts` — but the refusal is a plain string, not a `{ code }` error, so the error model's typed pipeline does not see it. Nothing calls these commands today: the interface hides every control that would (MKT-212).
- User value: None today.
- Done when: a call to any of those commands in a build without an External provider answers a typed code the frontend presenter can map, through one shared guard rather than five hand-written checks — worth doing when a second optional capability (a bank feed, the advice module) makes the pattern repeat.

## 2026-09-27 — DEBT-041 — Clickable table rows have no interactive element of their own

- Found by: the main agent (DEBT-039 review — nine E2E steps click `td:first-child` to open a row)
- Where: `src/features/currency/currency_rates_view/CurrencyRatesView.tsx` (`pair-row-*`), the account rows of `src/features/accounts/`, and every row whose `<tr>` carries `onClick`; the E2E specs clicking `… td:first-child`
- Severity: 🔵
- Observation: The rows are opened by an `onClick` on the `<tr>`, made focusable with `tabIndex` and a key handler. WebDriver cannot click a `<tr>` (its centre hit-tests to a cell), so specs click the first cell and rely on the event bubbling — a click on the wrong cell, or a cell that stops propagation, breaks them. Assistive technology meets a row announced as a row, not as a control that opens something.
- User value: Screen-reader and keyboard users meet a real link or button to open each row; the E2E suite clicks that control directly.
- Done when: each clickable row holds one link or button (with an `id` and an accessible name) that opens it, the row stays clickable for the mouse, and no E2E spec clicks `td:first-child`.

## 2026-09-28 — DEBT-046 — Feature `account_details` still makes 10 business decision(s) in the interface

- Found by: architecture rule A12 (TODO-047), first freeze
- Where: `account_details/account_details_view/useAccountDetails.ts` (1), `account_details/account_details_view/useAccountDetailsView.ts` (3), `account_details/open_balance/OpenBalanceModal.tsx` (1), `account_details/shared/presenter.ts` (2), `account_details/shared/validateCashForm.ts` (1), `account_details/shared/validateFeeForm.ts` (1), `account_details/shared/validatePriceForm.ts` (1)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `account_details` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `account_details` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `account_details` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).

## 2026-09-28 — DEBT-047 — Feature `accounts` still makes 3 business decision(s) in the interface

- Found by: architecture rule A12 (TODO-047), first freeze
- Where: `accounts/account_table/useAccountTable.ts` (2), `accounts/shared/validateAccount.ts` (1)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `accounts` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `accounts` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `accounts` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).

## 2026-09-28 — DEBT-048 — Feature `assets` still makes 6 business decision(s) in the interface

- Found by: architecture rule A12 (TODO-047), first freeze
- Where: `assets/asset_table/useAssetTable.ts` (4), `assets/shared/validateAsset.ts` (1), `assets/useAssets.ts` (1)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `assets` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `assets` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `assets` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).

## 2026-09-28 — DEBT-049 — Feature `categories` still makes 2 business decision(s) in the interface

- Found by: architecture rule A12 (TODO-047), first freeze
- Where: `categories/category_table/useCategoryTable.ts` (2)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `categories` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `categories` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `categories` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).

## 2026-09-28 — DEBT-050 — Feature `performance` still makes 9 business decision(s) in the interface

- Found by: architecture rule A12 (TODO-047), first freeze
- Where: `performance/account_view/useAccountPerformance.ts` (2), `performance/global_view/useGlobalPerformance.ts` (3), `performance/shared/globalPresenter.ts` (3), `performance/shared/presenter.ts` (1)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `performance` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `performance` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `performance` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).

## 2026-09-28 — DEBT-052 — Feature `unpriced_prices` still makes 2 business decision(s) in the interface

- Found by: architecture rule A12 (TODO-047), first freeze
- Where: `unpriced_prices/useUnpricedPrices.ts` (2)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `unpriced_prices` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `unpriced_prices` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `unpriced_prices` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).

## 2026-09-28 — DEBT-053 — Feature `whats_new` still makes 3 business decision(s) in the interface

- Found by: architecture rule A12 (TODO-047), first freeze
- Where: `whats_new/parseChangelog.ts` (3)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `whats_new` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `whats_new` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `whats_new` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).

## 2026-10-03 — DEBT-066 — An asset's name, reference and category have no length or character rule

- Found by: reviewer-security on TODO-055
- Where: `Asset::validate` in `src-tauri/src/context/asset/domain/asset.rs`; `AssetService::add_named_asset` (the command line's path, with the same rules)
- Severity: 🔵
- Observation: the asset rules check that a name and a reference are not empty, and nothing else: no maximum length, and a control character is accepted from the window's form or from another device's synced change. The command line escapes what it prints.
- User value: None directly — a name pasted with a stray control character or of unreasonable length is refused where it is typed.
- Done when: `Asset::validate` refuses a control character and a name or reference above a stated length, with a typed error the form shows and the command line reports.

## 2026-10-04 — DEBT-074 — A back-dated sale passes its draft check and is refused on save

- Found by: reviewer-backend on DEBT-070
- Where: `validate_draft` in `src-tauri/src/use_cases/holding_transaction/orchestrator.rs` (the oversell check of a new sale)
- Severity: 🔵
- Observation: the draft check compares the quantity sold with what is held today. A sale dated before later purchases can sell more than was held on its date: the draft is clean, and recording refuses it (`CascadingOversell`). The draft now returns no potential gain in that case, but reports no problem.
- User value: The sell dialog says before saving that the quantity was not held at that date.
- Done when: the draft check of a new sale refuses a quantity above the position as of the sale's date, with the error recording would give, shown on the quantity field.

## 2026-10-04 — DEBT-075 — A split factor has no upper bound, and the replay's quantity can wrap

- Found by: reviewer-security and reviewer-backend on DEBT-071
- Where: `Transaction::split` in `src-tauri/src/context/account/domain/transaction.rs` (accepts any positive factor); the split arms of the replay in `src-tauri/src/context/account/domain/account.rs` (`total_quantity as i64`)
- Severity: 🔵
- Observation: the replay multiplies the quantity by the factor in 128 bits and casts the result back to 64 bits without a check. A factor large enough (far beyond any real split) makes the cast wrap: recording and the split check both then show a quantity that is wrong, or refuse a split for the wrong reason.
- User value: None in practice — no real split comes near the bound; an absurd factor typed by mistake is refused with a clear reason.
- Done when: a factor whose rescale cannot be stored is refused when the split is recorded and when it is checked, with one error code, shown by a test on each path.

## 2026-10-10 — DEBT-101 — What the prune audit found in the files 0.7.1 changed

- Found by: `/prune` after v0.7.1 (2026-10-10), on `EnableSyncModal.tsx`, `useEnableSyncModal.ts` and `useDateField.ts`, with `coverage/frontend/lcov.info` of the same day
- Where: `src/ui/components/field/useDateField.ts` — `handleDateSelect` (lines 174–177), and lines 83 and 134; `src/features/settings/sync/enable_modal/useEnableSyncModal.ts` (lines 121 and 172)
- Severity: 🔵
- Observation: `handleDateSelect` builds an ISO date in four lines where `toIsoLocal`, defined in the same file with the same formula and used by `stepDate`, does it in one; the four lines are covered. Outside the audit's categories: the same file writes the ISO-date pattern inline twice where its constant `ISO_DATE` exists, and the sync hook writes `folderState?.holds_portfolio === true` twice
- User value: None — less code saying the same thing.
- Done when: `handleDateSelect` calls `toIsoLocal`; the two patterns use `ISO_DATE`; the sync hook names the expression once; the tests of both files pass unchanged.

## 2026-10-10 — DEBT-100 — What the spec check of management fees found after the 0.8.0 batch

- Found by: `spec-checker` on `docs/spec/management-fee-deduction.md`, `main` at c0d7b05 (2026-10-10), from reading code and tests; no test was run and nothing was read again by the main agent
- Where: `src-tauri/src/context/account/domain/fee_schedule.rs`, `src-tauri/src/context/account/service.rs`, `src-tauri/src/use_cases/fee_generation/orchestrator.rs`, `src/features/account_details/`
- Severity: 🟡
- Observation: **the code does not do what the rule says.** FEE-061: reactivating a paused schedule generates the periods it was paused for (`update_from` only sets `active`, the cursor is not moved, `apply_schedule` generates everything after it), where the rule says they are not; no test reactivates a schedule. FEE-041: the annual rate is divided by the periods as an integer before the quantity is multiplied, so a removal is floored twice (0.20 % monthly on 1000 shares removes 166.660 where the formula gives 166.666) and an annual rate under 12 micro-percent on a monthly schedule generates nothing; the test uses 12 %, which divides exactly. FEE-047: a period skipped for an oversell or a zero quantity is not logged. **The rule lags the window.** FEE-010 (a header button, not an item of a "Record" menu), FEE-020 (the date defaults to the account's last operation, as CSH-020), FEE-025/021 (one alert at the bottom of the form, and no message for an out-of-range percentage), FEE-055 (the removed quantity shows unsigned), FEE-022 (c)/FEE-028 (a deduction has no `origin` field; manual and generated differ by the identity's prefix). **A clause without a test.** FEE-032 (on edit: a rate not positive, an end date not after the start; an unknown account on creation; "accepted whatever its asset"), FEE-012 (unknown asset), FEE-021 (date bounds on recording, `DateTooOld` anywhere), FEE-022 (a) (a one-off removal of zero), FEE-029 (the preview's eligibility and parameter refusals), FEE-043 (a deleted generated deduction is not generated again, cursor in place), FEE-063 (a date or note edit, an edit the replay refuses, a deletion restoring the shares, the cursor after either), FEE-064 (`FeeScheduleUpdated` published), FEE-070 (the schedule stays active and resumes), FEE-075 (the migration's value for existing accounts, the edit form sending the parameter), FEE-076 (the row's "Manage fee" hidden when the parameter is off), FEE-077 (schedules stay editable and deletable when it is off), FEE-062 (generated deductions remain after the schedule is deleted). Three tests are named after another rule than the one they prove (`fee_070_…_is_idempotent` is FEE-043, `fee_047_zero_holding_quantity_skips_period` is FEE-070, `fee_041_generates_one_deduction_per_completed_period` asserts "at least one")
- User value: A user who pauses a fee schedule and resumes it later is not charged for the pause; a small recurring rate removes what its formula says.
- Done when: FEE-061 and FEE-041 hold in the code, each with a test that fails today, or the owner amends the rule; the skipped periods are logged; the five rules of the second group say what the window does, the owner deciding where the window should change instead; each clause of the third group has a test; the three tests carry the rule they prove.

**Open questions:**

- [x] FEE-041: once a removal is floored once, what of the deductions already generated? — They stay as they are; the fix applies to deductions generated from then on (owner, 2026-10-10).
- [x] FEE-010, FEE-020, FEE-025/021, FEE-055, FEE-022 (c)/FEE-028: the rule or the window? — The window is right in all five; the rules are amended to it (owner, 2026-10-10).

## 2026-10-10 — DEBT-099 — Starting sync over: two rules to settle and three clauses without a test

- Found by: `spec-checker` on `docs/spec/multi-device-sync.md`, `main` at c0d7b05 (2026-10-10). SYN-071 and SYN-053 are implemented and tested for the case TODO-066 fixed (the hook and dialog tests, `tests/sync_first_publish.rs`, E2E Step 11, green on pull request 168)
- Where: `src/features/settings/sync/enable_modal/useEnableSyncModal.ts`, `src/features/settings/sync/SyncPage.tsx`, `src-tauri/src/use_cases/portfolio_sync/orchestrator.rs`, `src-tauri/tests/sync_first_publish.rs`
- Severity: 🟡
- Observation: **from TODO-066, known to the main agent who wrote it:** in a folder that holds no portfolio the dialog says so, then still asks the confirmation that every published file will be discarded: nothing says whether that confirmation is wanted there. The dialog alone refuses to start over in a folder of a newer data format (`UpdateRequired`, kept by a TODO-066 test). **The checker's reading, not read again:** the core clears such a folder without reading its header and the contract lists no `UpdateRequired` for `start_sync_over`: no rule says which is right. "Under the new passphrase" has no real test: `sync_first_publish.rs` asserts the header's bytes changed, which fresh derivation parameters cause with the same passphrase too. "Interrupted after clearing, the device may retry" has no test; the clearing removes the device areas, then the header, and nothing tests an interruption between the two. "Start over" is offered only where sync is enabled, where SYN-053 says any device that still holds the portfolio: one that left sync gets the join refusal. Starting over on an enrolled device keeps its cursors, held-back changes and conflict notices (`save_enrolment` rewrites the device row only), so changes held back for the discarded history would keep the status at "needs attention"; no rule, no test
- User value: A user who forgot the passphrase after leaving sync can still start over; the sync status is clean after starting over.
- Done when: the owner says whether a newer-format folder may be started over, whether the confirmation is asked in a folder that holds no portfolio, and whether a device that left sync may start over, and the rules, the core and the dialog agree; the new passphrase, the interrupted clearing and the state kept after starting over each have a test, the last with its rule.

**Open questions:**

- [x] May a folder written by a newer version be started over? — No: the core refuses it too, with the same `UpdateRequired` as the dialog, and the contract lists it (owner, 2026-10-10).
- [x] Starting over in a folder that holds no portfolio: is the confirmation still asked? — Yes, the same one (owner, 2026-10-10).
- [x] May a computer that left sync and still holds the portfolio start over? — Yes: the refusal shown when enabling sync offers it (owner, 2026-10-10).
- [x] That refusal changes a screen: its mock-up is shown in the opening block of the batch, to validate before any code. — Validated: `screenshots/design/099-*-refusal.png`, a sentence and an outlined "Start over" under the refusal's steps, opening the "Start over" dialog on the folder chosen (owner, 2026-10-10).

## 2026-10-10 — DEBT-098 — Account commands return codes their contract rows do not list

- Found by: `contract-reviewer`, one exhaustive pass on `docs/contracts/account-contract.md` closing DEBT-088 (2026-10-10), from reading the code; nothing was run
- Where: `Account::replay_holding` in `src-tauri/src/context/account/domain/account.rs` (the `ManagementFee` arm); `src-tauri/src/use_cases/account_creation/orchestrator.rs`; `src-tauri/src/use_cases/fee_generation/orchestrator.rs` (`apply_schedule`); `src-tauri/src/use_cases/holding_transaction/orchestrator.rs`; `src/features/account_details/shared/presenter.ts`
- Severity: 🟡
- Observation: **read again by the main agent:** the replay subtracts a management fee's quantity with no guard (`total_quantity -= t.quantity`), and `Holding::with_id` refuses a negative quantity with `NegativeQuantity`; creating an account seeds the Cash Asset before the account is validated and maps that failure to `DatabaseError`, so `add_account` never returns the `InvalidCurrency` its row lists (TRX-021). **The reviewer's reading, not read again:** `NegativeQuantity` reaches the wire from `cancel_transaction`, `correct_transaction`, `record_management_fee`, `sell_holding`, `record_split` and `validate_stock_split_draft` when a replay crosses a fee that takes the position below zero (buy 100, a fee removes 10, cancel the purchase), where FEE-027 words the refusal as `CascadingOversell` and the presenter's comments say the code never reaches the window; `record_management_fee` can also return `ClosedPosition` and `SplitCollapsesPosition`, `sell_holding` `SplitCollapsesPosition`; `apply_due_fee_deductions` returns every code but three from `apply_schedule` (`NegativeQuantity`, `ClosedPosition`, `SplitCollapsesPosition`, `DateTooOld` for a start date before 1900) without moving the cursor, so it would fail again at each launch and leave later schedules unprocessed, against FEE-047; `validate_stock_split_draft` lists `AccountNotFound` and answers `ClosedPosition` for an unknown account; `record_dividend` returns `TotalAmountNotPositive` when amount × rate floors to zero; the notes of `get_account_details`, `get_account_summaries` and `get_account_performance` say a failed price lookup degrades the figure, which holds for a missing latest price only — a failed read of the price history is `DatabaseError`; `HoldingDetail` is described as quantity > 0 while the cash row is always included (CSH-090); the Dividend and Free Share notes say `InsufficientCash` cannot happen, which a ledger a merge left inconsistent (CFR-042) contradicts
- User value: A user who cancels or corrects a transaction around a management fee is refused with the reason, not with a message no screen maps; fee generation at launch does not stop on one schedule.
- Done when: each case of the reviewer's reading is confirmed by a test or struck; the fee replay refuses with the code FEE-027 names; `add_account` returns `InvalidCurrency`; fee generation skips or reports a schedule it cannot apply and goes on to the next; the rows and notes of the contract say what the code then returns, and its "Known gaps" paragraph is removed.

## 2026-10-10 — DEBT-097 — A fee schedule accepts what its generation has no rule for

- Found by: `spec-reviewer` on FEE-032, amended to the code by DEBT-088 (owner's decision of 2026-10-10: the missing checks are not added)
- Where: `FeeSchedule::new` and `update_from` in `src-tauri/src/context/account/domain/fee_schedule.rs`; `src-tauri/src/use_cases/fee_generation/orchestrator.rs`; `docs/spec/management-fee-deduction.md` — FEE-011, FEE-032, FEE-040, FEE-070
- Severity: 🟡
- Observation: a schedule is accepted at exactly 100% a year (`RateAboveHundred` is raised above 100_000_000 only), where FEE-032 says below 100% and FEE-070 relies on it to keep every removal below the quantity held. A schedule is accepted on any asset and with dates of any form, and no rule says what generation does with a schedule on a Cash Asset, on an asset that does not exist, or whose start date is no date. FEE-011 puts "Manage fee" on every holding row without excluding the cash row. Not read: what generation does in each case
- User value: None known — a schedule the form cannot produce is not created by a user of the window; an agent or the command line could.
- Done when: the rate bound is the same in FEE-032, FEE-070 and the code; each of the three cases has a rule saying what generation does, and a test; FEE-011 says whether the cash row offers the action.

**Open questions:**

- [x] A schedule at exactly 100% a year: refused, or allowed with FEE-070 rewritten? — Refused, like anything above; the rules stay as written (owner, 2026-10-10).

## 2026-10-10 — DEBT-096 — Six contracts were never read against the code they describe

- Found by: `scripts/contract-check.py`, first run (FLOW-024)
- Where: `docs/contracts/{asset,scheduled-fetch,sync,currency,update,agent-connection}-contract.md`; `contract-gaps.json`
- Severity: 🟡
- Observation: the check finds four gaps outside the account contract: the asset contract names `Exchange` in three rows and defines it nowhere; the scheduled-fetch contract gives `configure_scheduled_fetch` a `ConfigureScheduledFetchArgs` struct where the command takes `enabled` and `trigger_time`. The check compares names, arguments, types and whether an error type can carry a code; which codes each command returns is not in the bindings, and on that half only the account contract was read exhaustively (22 gaps, since fixed). The six others were not
- User value: None directly — the documents the window is written against say what the core does.
- Done when: each of the six contracts is read once with `/contract`, command by command; `contract-gaps.json` holds no line; one exhaustive pass of `contract-reviewer` on each finds no critical gap.

## 2026-10-10 — DEBT-095 — WebdriverIO 10 is available; the E2E tools are on 9

- Found by: the dependency audit before the 0.7.0 release
- Where: `package.json` — `webdriverio`, `@wdio/cli`, `@wdio/globals`, `@wdio/local-runner`, `@wdio/mocha-framework`, `@wdio/spec-reporter` (9.31 installed, 10.0.2 published on 2026-10-08, three days after 10.0.0); `wdio.conf.ts`; `e2e/`
- Severity: 🟡
- Observation: the advisories left on the test tools (DEBT-092) sit on packages this chain brings, and `npm audit` clears them only with 10. The E2E suite does not run on this computer (`docs/lessons.md` L-011): CI is the only proof of the upgrade, about a quarter of an hour per attempt. 10 asks for Node 22.19 or later; the workflows ask for Node 22 without a minor, so the version they resolve is to check
- User value: None directly — the tool that drives the application in the E2E suite carries no known advisory.
- Done when: the six packages are at 10; the E2E suite is green in CI with no test skipped for the upgrade; `npm audit` no longer reports the advisories DEBT-092 names on this chain.

## 2026-10-10 — DEBT-094 — Vitest 5 is available; the tests run on 4

- Found by: the dependency audit before the 0.7.0 release
- Where: `package.json` — `vitest`, `@vitest/coverage-v8` (4.1.11 installed, 5.0.3 published on 2026-09-30); `vitest.config.ts`
- Severity: 🔵
- Observation: the two packages move together. The coverage figure the gate reads (`coverage-gates.json`, frontend) comes from `@vitest/coverage-v8`: a major can count lines differently, so the figure is to compare before and after on the same commit. Vitest 5 accepts the Vite in use (8). Vite itself has no major waiting (8.0.16 → 8.3.4, a minor)
- User value: None directly — the frontend tests and their coverage figure run on a supported version.
- Done when: both packages are at 5; every frontend test passes; the frontend coverage figure before and after is written in the pull request, and a move of the figure is explained, the floor never lowered.

## 2026-10-10 — DEBT-093 — TypeScript 7 is available; the frontend is checked with 6

- Found by: the dependency audit before the 0.7.0 release
- Where: `package.json` — `typescript` (6.0.3 installed, 7.0.2 published on 2026-07-08); `tsconfig.json`, `tsconfig.node.json`; the `build` script (`tsc && vite build`)
- Severity: 🔵
- Observation: `tsc` is the type check of `src/` in the gate and the first step of the build. The E2E specs are not in its scope. The tools that read TypeScript beside it (Vite's React plug-in, Vitest, oxlint, Biome, the generated `src/bindings.ts`) are each to check against 7 before the move
- User value: None directly — the type check that guards the frontend runs on the current compiler.
- Done when: `typescript` is at 7; `just check --frontend` and the build pass with no rule of `tsconfig.json` loosened and no error silenced; the generated bindings compile unchanged.

## 2026-10-10 — DEBT-092 — The test tools carry about twenty high advisories; six majors are available

- Found by: the dependency audit before the 0.7.0 release
- Where: `package-lock.json` — packages reached only through `devDependencies` (the WebdriverIO chain: `basic-ftp`, `extract-zip`, `braces`, `brace-expansion`, `ip-address`, `undici`, `serialize-javascript`, `diff`); `src-tauri/Cargo.toml`
- Severity: 🟡
- Observation: `npm audit --omit=dev` reports nothing: none of these packages ships. `npm audit` reports about twenty high advisories on the tools that run the E2E suite and the tests, on the developer's computer and on the CI runner. `npm audit fix` rewrites about 1,200 lines of the lock file and leaves twenty; the rest asks for WebdriverIO 10. Majors available, read from the registries on 2026-10-10: WebdriverIO 10 (DEBT-095), Vitest 5 (DEBT-094), TypeScript 7 (DEBT-093), each with its own entry; here, `@testing-library/jest-dom` 6 → 7; `sqlx` 0.8 → 0.9, `argon2` 0.5 → 0.6 with `password-hash` 0.6, `sha2` 0.10 → 0.11, `dirs` 6 → 7, `iso_currency` 0.5 → 0.7, `strum` 0.27 → 0.28. `cargo audit` reports no vulnerability; nine warnings on crates brought by others (unmaintained `paste`, `proc-macro-error`, `unic-*`; unsound `glib` 0.18; yanked `spin` 0.9.8)
- User value: None directly — the tools that prove the application carry no known advisory.
- Done when: `npm audit` reports no high or critical advisory, or each one left is named with the reason it cannot be reached; each major named here is taken or has a line saying what holds it.

## 2026-10-10 — DEBT-091 — Three commands granted to the CI reviewers can write or run a program

- Found by: `reviewer-security`, before the 0.7.0 release
- Where: `.github/workflows/review.yml` — `--allowedTools`: `Bash(sed -n *)`, `Bash(sort *)`, `Bash(git diff *)`
- Severity: 🟡
- Observation: the reviewers are granted commands by name so that none can write or run code on the runner (DEBT-082's fix). Three of the grants can: GNU `sed` runs a shell command or writes a file from its own script (`e`, `w`), `sort` runs a program (`--compress-program`) and writes (`-o`), `git diff` writes (`--output=`) and reads any path (`--no-index`). A pull request whose text steers its reviewer could use them with the job's environment. Not changed the day of a release: a reviewer that loses a command it leans on spends its turns on refusals (DEBT-087)
- User value: None directly — a pull request cannot make its own reviewer run code.
- Done when: `sed -n` is not granted (the file reader takes an offset and a limit); `sort` and `git diff` are not granted, or are granted in forms that take no output or program option; a test of the workflow holds the list; a wide pull request still gets its reports within the turn limit.

## 2026-10-10 — DEBT-090 — What the spec checks before 0.7.0 found in rules the batch did not touch

- Found by: `spec-checker` on the specs the 0.7.0 batch touched (2026-10-10)
- Where: `docs/spec/{transaction-list,command-line,cash-tracking,financial-asset-transaction,agent-connection}.md` against `src/features/{transactions,account_details,assets,categories}/` and `src-tauri/src/`
- Severity: 🔵
- Observation: **the screen does less than the rule.** TXL-010 (Account Details imports from the transactions feature), TXL-015 (Back after an account switch returns to the old account), TXL-023 (a sale row looks like a purchase), TXL-054 (the table still shows when the asset list failed), TXL-061 (the journal's empty state has no add shortcut); CSH-017 (the Cash category is listed and offered), CSH-020/030 (the default date is the account's last operation, not today), CSH-025/035 (a generic "deleted" message; `cash.deposit_deleted` and `cash.withdrawal_deleted` are unused keys), CSH-031 (no check of a withdrawal against the balance in the form), CSH-081 (wording, format, and a refused deletion shown as a generic error), CSH-090 (the as-of view drops a cash line that is not positive), CSH-091 (no currency symbol); TRX-063/067 (the full-page purchase form shows no reason when Save is disabled, and compares the asset's currency to "EUR" instead of the account's), TRX-010, TRX-023, TRX-036 and TRX-037 (the rule's text no longer matches: a modal, a remembered date, a constant default, `AccountService`); `get_account_details` refuses an as-of date that is no ISO date or lies after today (`InvalidDate`, `DateInFuture`) and no rule of `account-details.md` says so; CLI-016 (no example for `account list` and `asset add` in the overview), CLI-021 (JSON keys print alphabetically, the rule writes `status` first). **The code does less than the rule.** TRX-020 (a purchase on an unknown asset is stopped by the foreign key and surfaces as `DatabaseError`), TRX-051 (an opening balance corrected without a total uses the unit price sent). **Rules with no test or a thin one.** TXL-013, 031, 041, 044, 050–053; CLI-011 (`@` inside a name), CLI-014 (today), CLI-017, CLI-021 (a usage error with `--json`), CLI-030 (the lock error's code), CLI-031, CLI-033; CSH-021, 051, 060, 062; TRX-032, 033, 037, 041; AGT-024 (a failed start, the setting staying off), AGT-038 (8 bridges), AGT-040 (`archived`), AGT-041 (the log), AGT-047 (`SessionLimitReached`), AGT-049 (`RecordedNotMarked`). Stale comments say cash holdings are created lazily (`account.rs`, `holding_transaction/orchestrator.rs`) against CSH-012. **Found by the last checks, same day.** Sell transactions: SEL-024 (the column's default is a constant, not the creation time; no same-date test), SEL-029 (the remembered date, as CSH-020), SEL-032 (the refusal does not name the sale that would become invalid), SEL-038 (`get_realized_pnl_by_account` has no caller: dead code; the figure comes from the holding), tests for SEL-028, 031, 035, 036, 045; the spec still names `use_cases/record_transaction/` and `TransactionService`. Sync: SYN-064 (the sync page does not read the status again on `SyncCompleted`: it can differ from the header), SYN-063 (failures and the roster are kept in memory only: after a restart a paused device shows no reason and an empty roster), SYN-084 (several headers; the reset reason lost on restart), SYN-068 (a headless run pauses the device on a reset, where the rule leaves it for the next launch), SYN-054 (the note omits the derivation parameters); no test for SYN-022, 023, 030, 062, nor for one clause of SYN-016, 035, 038 (a real version 1 segment through a run), 065, 074, 081. Asset lookup: WEB-047 (the way back to the search disappears after a kind change on a pre-filled form); tests for WEB-010 (the dialog itself), WEB-041 (the crypto form), WEB-045 (`AssetUpdated` on creation). Market price: MKT-210 (no test calls a fetch in a build without a provider), MKT-213. **Error model.** `ArchivedAssetSell` (SEL-037) is raised by the use case from the asset's state and lives among the account's errors, because `sell_holding` returns them; `docs/error-model.md` asks for the use case's own error type, which changes the command's error, the bindings and the contract
- User value: None directly, except the first group: a user is told why Save is disabled on the full-page form, sees the cash line of a past date, and gets the message the spec promises.
- Done when: each rule of the first two groups is brought to the code or amended to it, the owner deciding where the screen is what he wants; each rule of the third group has a test; the stale comments say what is.

## 2026-10-10 — DEBT-089 — The agent connection against the protocol's own practices

- Found by: an audit asked by the owner on 2026-10-10, against the Model Context Protocol specification (revision 2025-06-18: lifecycle, transports, tools) and its security best practices
- Where: `src-tauri/src/agent_connection/{mcp,bridge,definitions,tools,server}.rs`
- Severity: 🟡
- Observation: followed — standard input and output carry only protocol messages, one per line; the local channel is restricted to the owner's user, never a port; consent comes before anything is served; every call is logged; session keys are random; an unknown tool is a protocol error and a refused call a tool error. Not followed: (1) **version negotiation** — `initialize` answers with whatever version the client asked for, also one this server does not know, where the protocol says to answer with a version the server supports; (2) **rate limiting**, which the protocol requires of a server — a session is capped at 500 recordings, but nothing limits calls per minute, reads included; (3) **what a tool returns is not marked as data** — names and notes the owner or an agent typed go back to the agent as they are, where a later agent may read them as instructions; (4) smaller: a line that is not JSON is skipped where JSON-RPC answers a parse error; a call before `initialize` is served under the name "An agent"; the results are text only, with no `structuredContent` or `outputSchema`; `initialize` gives no `instructions` (the place to say that amounts are in millionths)
- User value: None directly — the three points a user would notice (a notice in the window when an agent records, a cancelled call withdrawing its dialog, tools saying whether they read or record) are TODO-065.
- Done when: `initialize` answers a supported version and a test holds the list; calls are limited per minute with a refusal that says so; text typed by a person is returned in a form an agent cannot take for instructions, or the limit of that is written in the spec; the smaller points are fixed or declined in the spec, each with its reason.

## 2026-10-10 — DEBT-087 — The CI reviewers run out of turns on a wide pull request

- Found by: three reviewer lanes failing without a finding on pull requests 149 and 150 (runs 37993079482, 37995910120, 37997330569)
- Where: `.github/workflows/review.yml` — `--max-turns`, and the commands the reviewers try that their grant refuses; the reviewer prompts under `.claude/agents/`
- Severity: 🟡
- Observation: on a diff of many files a reviewer reached its 40 turns before or just after writing its report, and the job failed: once with no report (backend), twice with a clean one (infrastructure, contract). 7 to 12 turns of each run went into commands the grant refuses, which the reviewer then retried another way; one reviewer started helper sessions and ended while waiting for them. The limit is raised to 80, which hides the waste without removing it
- User value: None directly — a pull request is not held back by a review that found nothing.
- Done when: a run's refused commands are listed and each is either granted by name or removed from the prompts; a reviewer cannot start helper sessions in CI; a reviewer that writes its report within the limit never fails its job; the limit is set from the measured turns of a wide pull request, with a test of the workflow that holds it.

## 2026-10-09 — DEBT-086 — The agent connection does not exist on Windows

For 0.8.0 (owner, 2026-10-09): the next version has the agent connection working on Windows. Queued by the owner on 2026-10-10, after v0.7.1.

- Found by: the owner's decision of 2026-10-09 while building TODO-051 (Linux first)
- Where: `src-tauri/src/agent_connection/channel.rs` (the channel is a Unix socket; `AVAILABLE` is false elsewhere); `.github/workflows/` — no job compiles for Windows on a pull request, only `release.yml` does
- Severity: 🟡
- Observation: on Windows the setting "Allow agents to connect" is disabled and says the agent connection is not available on this system yet (AGT-023). The Windows channel is a named pipe restricted to the owner's user, which needs Windows-only code that no pull request compiles today: written blind, its first compilation would be the release
- User value: A Windows user lets an agent read the portfolio through the open application, as on Linux.
- Done when: a pull request that touches the Rust code is compiled for Windows before it merges; the channel exists on Windows as a named pipe only the owner's user can open, refusing remote clients; AGT-021 and AGT-023 say so, and the tests that can run on Windows run there.

**Open questions:**

- [x] How wide is the Windows job on pull requests? — On pull requests that touch Rust: compile the application for Windows and run the agent-connection tests there; not the whole Rust suite (owner, 2026-10-10).
- [x] The agent cannot run the application on Windows: does the entry wait for a trial on a real machine? — No: it closes on green CI; the owner tries it when next on Windows (owner, 2026-10-10).

## 2026-10-09 — DEBT-085 — The assets to settle are reported by the core and shown nowhere

- Found by: `spec-checker`, closing TODO-056
- Where: `get_assets_to_settle` in `src-tauri/src/context/asset/api.rs` (AST-035); no call in `src/features/assets/gateway.ts`, no command of the command line
- Severity: 🟡
- Observation: an asset that existed before kinds and that the rules would now refuse — a listed one without an ISIN, two that are the same — is kept as it is and reported by the core, but no screen and no command shows the report, so its owner is never told which assets to settle
- User value: A user sees which of their assets break a rule of their kind, and why, and settles each from its edit dialog.
- Done when: the assets to settle show where the owner decides — the assets table, a notice, the command line — each with its problem and a way to its edit dialog; a test holds it.

**Open questions:**

- [ ] Where do the assets to settle show: a line or a badge on their row of the assets table, a notice above it, or both with a command of the command line? (A screen changes: a design to validate.)

## 2026-10-09 — DEBT-084 — Three rules of the assets screen that the screen does not keep

- Found by: `spec-checker`, closing TODO-056
- Where: `src/features/assets/asset_table/AssetTable.tsx` — the empty state is tested before `fetchError` (AST-015), and the archive and unarchive confirmations close before the result is known, the failure showing above the table (AST-014); `activeCount` in `src/features/assets/useAssets.ts`, which nothing shows (AST-007)
- Severity: 🔵
- Observation: a failed first load shows "No assets" instead of the error and Retry; a refused archive is told outside the dialog that asked; the page header shows no count of active assets. All three are older than TODO-056, and none has a test
- User value: A user whose assets cannot be read is told so and can retry; a refused archive is explained where it was asked.
- Done when: each of the three is fixed with a test, or its rule is rewritten to what the screen does and the unused count removed.

## 2026-10-09 — DEBT-083 — A currency pair alone stops a device from joining; the spec does not say so

- Found by: `spec-checker`, closing DEBT-064
- Where: `installation_holds_user_data` in `src-tauri/src/use_cases/portfolio_sync/orchestrator.rs` (its last test: any currency pair); `docs/spec/multi-device-sync.md` — SYN-014 (what counts as user data: accounts, user-created assets and categories, transactions, manual prices and rates) and SYN-083 (pairs fetched before joining are discarded by the rebuild)
- Severity: 🟡
- Observation: an installation whose only record is a declared currency pair is refused as "holding user data" when it joins, while SYN-014 does not list pairs and SYN-083 treats them as observations the rebuild replaces. Either the spec is missing a line (a pair the user declared is the user's) or the code refuses too much. No test covers that branch, so that either reading can be made true without one failing.
- User value: A fresh installation that only declared a currency pair either joins, or is told why it cannot, as the rules say.
- Done when: the owner says whether a declared currency pair is user data; SYN-014 and the code agree, and a test holds the answer.

**Open questions:**

- [x] Is a currency pair the user declared user data when joining? — No: an installation holding only declared pairs may join and its pairs are replaced by the portfolio's; a manual rate still counts (owner, 2026-10-10).

## 2026-10-04 — DEBT-080 — The E2E tooling carries 22 known advisories

- Found by: `/dep-audit` before the 0.6.0 release
- Where: `package.json` — the WebdriverIO packages (`@wdio/*`, `webdriverio`, 9.31.7) and what they bring: `undici` (< 7.29.1), `basic-ftp`, `extract-zip`, `braces`
- Severity: 🔵
- Observation: `npm audit` lists 22 advisories (18 high), all in development dependencies; `npm audit --omit=dev` finds none, so nothing of it ships. `npm audit fix` moves WebdriverIO to 9.32.0 and leaves 20: the chain pins the affected versions, and the only complete fix offered is a downgrade to WebdriverIO 8. The tooling runs on CI and on this machine against the application under test, never against untrusted input.
- User value: None directly — the audit before a release reads clean again.
- Done when: `npm audit` reports no advisory, by a WebdriverIO release that drops the affected versions or by `overrides` proven on a green E2E run; or each one left is recorded with why it cannot be reached.

## 2026-10-04 — DEBT-079 — The rate edit scenario fails at random and is skipped

- Found by: CI on pull request 124 (run 37225909601), which does not touch currencies
- Where: `e2e/currency/currency_rates.test.ts` — "FXR-052: editing a rate via the UI updates the rate row", at the lookup of the old rate row just after the edit dialog closes
- Severity: 🟡
- Observation: `stale element reference: Stale element found when trying to create the node handle` on `$("#rate-row-…")` once the dialog is gone. The rate list is re-rendered when the rate changes; the scenario asks for a row of the list while it is being replaced. Whether the list re-renders once or twice after an edit, and whether the scenario should wait for the new row before looking for the old one, is not established. The test is skipped (`it.skip`): editing a rate through the interface has no end-to-end coverage until this is fixed.
- History: the same failure was filed on 2026-09-27 as DEBT-039, whose fix (views keep their rows on a re-fetch, F29) did not hold; this entry replaces it.
- User value: None directly — the scenario protects the rate edit again.
- Done when: the cause is found (in the scenario or in how the rate list refreshes), fixed, and the scenario is re-enabled and passes twenty runs in a row.
