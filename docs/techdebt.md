# Tech Debt

Observations of code smells, brittle patterns, or pre-existing issues surfaced
during work that don't warrant immediate action. The entry format is in
`docs/workflow.md` § 2.

Entries are observations, not commitments, and this file is the agent's: it
files here what it notices and what it did not fix. Each entry carries a
permanent `TD-NNN` reference (never renumbered, never reused; next free:
TD-075) so the human can queue it in `docs/todo.md` § Next like any todo.
Remove an entry once it has been resolved.

---

## 2026-08-23 — TD-001 — Local writes do not take the sync gate

- Found by: reviewer-security + reviewer-backend (PR-C, `.review/reviewer-security-2026-08-23-01.md`)
- Where: src-tauri/src/context/sync/application/run.rs (`SyncGate`), every synced repository write
- Severity: 🟡
- Observation: SYN-064 says a local write and an in-progress apply never interleave. The apply holds `SyncGate` and runs in one SQLite write transaction with the change recorder suspended; local writes do not take the gate. SQLite's single writer serialises them at the database level and the recorder reads the logical clock under that lock, so the remaining window is a local write that computed `based_on` before an apply committed — benign for the rank order, but not the guarantee the spec states. Closing it means a `begin_write()` helper on the recorder that takes the gate before opening the transaction, applied at all 30 capture sites — its own PR.
- User value: None observable — the remaining window is benign for merge order.
- Done when: A `begin_write()` recorder helper takes `SyncGate` before opening the transaction at all 30 capture sites, so SYN-064 holds as written.

## 2026-08-23 — TD-002 — Held-back changes and conflict notices have no bound

- Found by: reviewer-security (PR-C)
- Where: src-tauri/src/context/sync/application/run.rs (`apply_intake`), `held_back_changes`, `conflict_notices`
- Severity: 🟡
- Observation: A hostile or buggy peer could grow `held_back_changes` without bound (every run retries all of them) and `conflict_notices` never evicts. Unreachable for one user's own desktops; add caps / eviction before any multi-user or untrusted-peer scenario. Note (2026-09-12): a cap or eviction contradicts SYN-066 as written ("persist until the user dismisses them individually"), so this is a spec decision before it is a code change.
- User value: None for a user's own devices; bounds growth caused by a buggy or hostile peer.
- Done when: `held_back_changes` has a cap and `conflict_notices` evicts, both covered by tests.

## 2026-08-23 — TD-003 — Join replays a device's whole history in memory

- Found by: reviewer-security (PR-C)
- Where: src-tauri/src/context/sync/application/join.rs
- Severity: 🔵
- Observation: Only the per-file 64 MiB cap bounds a join; the full history of each device is held in memory inside one transaction. Acceptable under the KISS cut (a personal portfolio's history is a few KB a month); revisit with checkpoints if history or device count grows.
- User value: None at present history sizes.
- Done when: Join streams or checkpoints history instead of holding every device's full history in one in-memory transaction.

## 2026-08-22 — TD-004 — Account-deletion cascade is no longer a single transaction

- Found by: reviewer-backend (PR-A change capture, `.review/reviewer-backend-2026-08-22-01.md`)
- Where: src-tauri/src/context/account/service.rs (`delete` / `remove_children`)
- Severity: 🟡
- Observation: To record one change + tombstone per child (SYN-024, CFR-030), `AccountService::delete` now removes transactions, holding notes, fee schedules and catch-up positions through their own repositories — each atomic with its own change — before deleting the account. Previously a single `DELETE` with `ON DELETE CASCADE` did it all in one transaction. A crash mid-cascade leaves a half-deleted account (recoverable on retry, never silently diverging, since every child removal carries its change). Restoring single-transaction semantics needs a transaction spanning several repositories — the unit of work ADR-006 describes and the codebase never built (see the ADR-006 entry above). Fold the cascade into that unit of work when it lands.
- User value: A crash midway through deleting an account cannot leave it half-deleted.
- Done when: The cascade runs inside one unit of work spanning the child repositories.

## 2026-08-22 — TD-005 — ADR-006 unit of work is accepted but unimplemented

- Found by: feature-planner (multi-device sync plan, D1) — confirmed by plan-reviewer and by grep
- Where: src-tauri/src/ (no `UnitOfWork` / `TransactionManager` / `uow` anywhere; three raw `sqlx` transactions at `context/account/repository/account.rs:268`, `context/asset/repository/category.rs:109`, `context/asset/repository/asset_price.rs:128`)
- Severity: 🟡
- Observation: `docs/adr/006-unit-of-work.md` is Accepted, but nothing in the codebase implements it; cross-aggregate writes open ad-hoc transactions. Change capture (ADR-019) does not use it either: it goes through a `ChangeRecorder` port on the live connection. Decide later whether to implement ADR-006 (and route the recorder through it) or supersede it; status left Accepted meanwhile.
- User value: None directly.
- Done when: ADR-006 is implemented and the three ad-hoc `sqlx` transactions route through it, or the ADR is superseded.

## 2026-07-25 — TD-006 — The Linux package ships a development-only helper binary

- Found by: main-agent
- Where: src-tauri/Cargo.toml (`[[bin]] generate_bindings`), src-tauri/tauri.conf.json
- Severity: 🔵
- Observation: The `.deb` packages `generate_bindings`, the helper that regenerates the TypeScript bindings, next to the application; the Windows installer is likely affected the same way. It is never run by the application and only adds a few megabytes. The other half of this entry — the application binary shipped under the template name `tauri-app` — was fixed by the rename to Folioneer (#033), which installs fresh and so crosses no updater path.
- User value: None visible — no development tool ships inside the package.
- Done when: `generate_bindings` is no longer in the `.deb` nor in the Windows installer, and `just generate-types` still works.

---

## 2026-05-10 — TD-008 — Migrate to FE gold layout

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

## 2026-05-09 — TD-009 — Migrate to gold DDD layout

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

## 2026-09-12 — TD-010 — A different portfolio's folder reads as a reset

- Found by: manual (closing the empty-sync-folder todo)
- Where: src-tauri/src/context/sync/application/run.rs (`header_gate`)
- Severity: 🔵
- Observation: When a removable volume's drive letter is reused by a different stick that happens to carry a `Folioneer` folder, the header decodes but its passphrase check fails — exactly what a genuine "started over elsewhere" (SYN-071) looks like from the header alone. Both write a fresh header with a new creation mark, so the two cases are indistinguishable by content; the device reports `PortfolioReset` (SYN-084) where "this is another portfolio" would be the honest message. `FolderHoldsOtherPortfolio` exists as the enable-path error but nothing in the run can justify raising it.
- User value: The reset message would not fire for a stick that merely took the same drive letter.
- Done when: A sync run can tell a reset of its own portfolio from another portfolio's folder — by something other than the header's content — and reports `FolderHoldsOtherPortfolio` for the latter.

## 2026-09-12 — TD-012 — Ubiquitous-language Domain Events table lags the event enum

- Found by: reviewer-arch (T2)
- Where: docs/ubiquitous-language.md § Domain Events; src/lib/store.ts `locallyHandledEvents`
- Severity: 🔵
- Observation: The table omits `AssetPriceFetchProgress`, which the enum carries; the asset contract's Events table (`docs/contracts/asset-contract.md`) likewise omits `CategoryUpdated`; and `AssetPriceUpdated` (MKT-037) is handled by its own views yet is absent from the store's locally-handled allowlist, so each publish logs an "unhandled event" debug line. The two events added today are registered in both places; the older gaps are untouched.
- User value: None — documentation and a debug-log nuisance.
- Done when: every `Event` variant has a row in the table and in its owning contract's Events table, and the allowlist names every event the global store deliberately ignores.

## 2026-09-13 — TD-018 — 118 interactive components in feature code carry no id

- Found by: manual (`python3 scripts/arch-check.py`, rule A6, first run)
- Where: 34 files under src/features/ listed in arch-allowlist.json `missing_ids`; mostly Cancel and secondary buttons in modals, the price-history and update-banner actions, and the design-system dev page
- Severity: 🔵
- Observation: E1–E4 ask every interactive element for a stable id, and the E2E suite selects by id, yet 118 (119 at the first run) of the 317 `Button` / `IconButton` / `TextField` / `DateField` / `CalcField` / `FAB` tags rendered by feature code have none. The architecture check freezes today's count per file and refuses any growth; the count can only go down. The 18 sibling-feature imports the same check freezes belong to the FE gold layout migration entry above; the 8 `Math.` uses are display rounding and the documented split preview (SPL-061) and need no action.
- User value: None — every control becomes addressable by tests and assistive tech.
- Done when: `missing_ids` in arch-allowlist.json is empty.

## 2026-09-14 — TD-025 — "Reference currency" names two different things in the vocabulary

- Found by: spec-reviewer (ACC-027–033 review on #007)
- Where: `docs/ubiquitous-language.md` (Cash Holding, Dividends received, Management fees — "the account's reference currency"), `docs/spec/global-performance.md` GPF-011 and the ACC / PMV totals ("the reference currency", EUR); the same table is also "Accounts table" (ACC-021/023/026), "accounts list" (PMV-013/016, SYN-040) and "account table" (ACC-008/030–033)
- Severity: 🔵
- Observation: the vocabulary uses "the account's reference currency" for an account's own currency, while GPF-011, the price movement total and the accounts-list portfolio total use "the reference currency" for the fixed EUR every cross-account figure is reported in; the two meanings share one phrase, and neither "cross-account reference currency" nor "portfolio total" has an entry of its own.
- User value: None — one word per concept in the specs and the code.
- Done when: the vocabulary names the account's own currency and the cross-account reference currency with distinct, confirmed terms, and has a confirmed "portfolio total" entry, each validated by the human.

## 2026-09-14 — TD-026 — Nothing follows an account currency's rate to the reference currency

- Found by: spec-reviewer (ACC-027/028 second pass on #007)
- Where: `docs/spec/fx-rate.md` FXR-013 / FXR-071 (only asset → account pairs are followed), `docs/spec/global-performance.md` GPF-020, `docs/spec/account.md` ACC-027/028, `src-tauri/src/use_cases/account_summary/orchestrator.rs` (portfolio total)
- Severity: 🟡
- Observation: rates are fetched and followed for the pairs an asset needs to reach its account's currency, but no rule follows the pair from an account's currency to the cross-account reference currency; a USD account holding only USD assets therefore never gets a USD → EUR rate unless the user declares the pair, and the accounts-list portfolio total stays marked partial for it (the global performance view has the same dependency).
- User value: the portfolio total and the global performance figures count every account without the user having to declare a pair first.
- Done when: an account whose currency differs from the reference currency has its pair followed like an asset pair (or the application asks the user to declare it), stated as an FXR rule, and a USD-only account with a fetched rate no longer leaves the total incomplete.

## 2026-09-15 — TD-027 — ADR-012 does not record the price history backfill's fill-only exception

- Found by: spec-reviewer (MKT-190–199 second pass on #008)
- Where: `docs/adr/012-latest-write-wins-source-as-metadata.md` (decision 1), `docs/spec/market-price.md` MKT-192
- Severity: 🔵
- Observation: ADR-012 decision 1 says every price write upserts unconditionally; MKT-192 exempts the price history backfill, which writes only dates without a price. The spec names the exception and the ADR does not, so a reader of the ADR alone misses it; the ADR status vocabulary has no "amended by" form yet (TD-007).
- User value: None — the decision record matches the rules.
- Done when: ADR-012, or an ADR that supersedes it, names the fill-only exception, and adr-reviewer passes it.

## 2026-09-19 — TD-030 — A sync that only holds changes back, or raises a notice, announces nothing

- Found by: contract-reviewer (SYN-064 amendment on #020)
- Where: src-tauri/src/context/sync/application/service.rs (`remember_run`), docs/contracts/sync-contract.md (Events, `SyncCompleted`), src/features/shell/sync_indicator/useSyncIndicator.ts
- Severity: 🔵
- Observation: `SyncCompleted` is raised when a sync applied at least one change or when its failures or paused state changed. A sync that applied nothing but held changes back (SYN-041), or whose only outcome is a conflict notice on a losing incoming creation (SYN-066, CFR-060), changes what the sync status reports — last sync time, held-back count, notices — without raising the event, so the header indicator's attention badge follows only at the next status read.
- User value: The attention badge appears as soon as a sync leaves something for the user to look at, not at the next launch or the next sync that applies a change.
- Done when: a sync whose held-back count or notice count differs from the previous one raises `SyncCompleted`, covered by a `remember_run` unit test, and SYN-064 and the sync contract state the full trigger set.

## 2026-09-19 — TD-031 — The account contract's shared types lag the bindings

- Found by: contract-reviewer (FEE-028/029 review on #018, pre-existing section)
- Where: docs/contracts/account-contract.md (Shared Types, command tables), src/bindings.ts
- Severity: 🔵
- Observation: `RecordInterestDTO` and `CancelTransactionDTO` are used as command arguments but never defined in Shared Types; `Account`, `CreateAccountDTO` and `UpdateAccountDTO` omit `management_fees_enabled`; `HoldingDetail` omits `market_value` and `fee_rate_percent_micros`, `AccountDetailsResponse` omits `total_net_cash_input`; the buy / sell / correct DTOs omit `total_amount` while a note still says it is intentionally absent; `record_split` has no command row in any contract; the `apply_due_fee_deductions` note omits FEE-078 (schedules of an account with management fees disabled are skipped).
- User value: None — the contract is what reviewers and planners read instead of the code.
- Done when: every command registered for the account domain has a row, and every type the rows name is defined with the fields the bindings carry.

## 2026-09-19 — TD-032 — Two features carry the same two-way entry-mode toggle

- Found by: reviewer-frontend (#018)
- Where: src/features/transactions/shared/EntryModeToggle.tsx, src/features/account_details/management_fee_transaction/ManagementFeeModal.tsx
- Severity: 🔵
- Observation: the buy / sell forms' price-or-total toggle and the management fee form's percentage-or-resulting-quantity toggle are the same radiogroup of two small buttons, written twice; a change to the look or to its keyboard behaviour has two places to drift.
- User value: None — one look for every two-way entry toggle.
- Done when: a generic two-option toggle lives in `src/ui/components/` and both forms use it.

## 2026-09-20 — TD-036 — The vocabulary has no entry for the update feature's terms

- Found by: spec-reviewer (UPD-028 review on #037)
- Where: docs/ubiquitous-language.md (no Update section); docs/spec/update.md, docs/adr/020-one-extension-file-per-build.md, docs/contracts/update-contract.md
- Severity: 🔵
- Observation: "update channel", "update server", "update file", "credentials", "refused access", "distribution channel", "public build" and "private build" are used as terms by the spec, the ADR, the contract and the code, but the vocabulary defines none of them. Terms are the owner's to confirm (B5).
- User value: None.
- Done when: The vocabulary carries an Update section whose terms the owner has confirmed, and the spec, ADR, contract and code use them.

## 2026-09-23 — TD-038 — A build without an External provider refuses fetch commands with the runtime's own error

- Found by: reviewer-security (#036)
- Where: `src-tauri/src/lib.rs` (the three fetching use cases are managed only with an External provider), `src-tauri/src/core/specta_builder.rs` (their commands stay registered in every build)
- Severity: 🔵
- Observation: in such a build `fetch_all_asset_prices`, `fetch_account_asset_prices`, `configure_scheduled_fetch`, `get_scheduled_fetch_status` and `backfill_holding_price_history` reach no managed state, so Tauri refuses the call before the command body runs. That satisfies MKT-210 and SPF-070 — nothing is fetched or written, and the message names only the command and its argument, both public in `bindings.ts` — but the refusal is a plain string, not a `{ code }` error, so the error model's typed pipeline does not see it. Nothing calls these commands today: the interface hides every control that would (MKT-212).
- User value: None today.
- Done when: a call to any of those commands in a build without an External provider answers a typed code the frontend presenter can map, through one shared guard rather than five hand-written checks — worth doing when a second optional capability (a bank feed, the advice module) makes the pattern repeat.

## 2026-09-27 — TD-041 — Clickable table rows have no interactive element of their own

- Found by: the main agent (TD-039 review — nine E2E steps click `td:first-child` to open a row)
- Where: `src/features/currency/currency_rates_view/CurrencyRatesView.tsx` (`pair-row-*`), the account rows of `src/features/accounts/`, and every row whose `<tr>` carries `onClick`; the E2E specs clicking `… td:first-child`
- Severity: 🔵
- Observation: The rows are opened by an `onClick` on the `<tr>`, made focusable with `tabIndex` and a key handler. WebDriver cannot click a `<tr>` (its centre hit-tests to a cell), so specs click the first cell and rely on the event bubbling — a click on the wrong cell, or a cell that stops propagation, breaks them. Assistive technology meets a row announced as a row, not as a control that opens something.
- User value: Screen-reader and keyboard users meet a real link or button to open each row; the E2E suite clicks that control directly.
- Done when: each clickable row holds one link or button (with an `id` and an accessible name) that opens it, the row stays clickable for the mouse, and no E2E spec clicks `td:first-child`.

## 2026-09-28 — TD-046 — Feature `account_details` still makes 10 business decision(s) in the interface

- Found by: architecture rule A12 (#047), first freeze
- Where: `account_details/account_details_view/useAccountDetails.ts` (1), `account_details/account_details_view/useAccountDetailsView.ts` (3), `account_details/open_balance/OpenBalanceModal.tsx` (1), `account_details/shared/presenter.ts` (2), `account_details/shared/validateCashForm.ts` (1), `account_details/shared/validateFeeForm.ts` (1), `account_details/shared/validatePriceForm.ts` (1)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `account_details` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `account_details` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `account_details` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).

## 2026-09-28 — TD-047 — Feature `accounts` still makes 3 business decision(s) in the interface

- Found by: architecture rule A12 (#047), first freeze
- Where: `accounts/account_table/useAccountTable.ts` (2), `accounts/shared/validateAccount.ts` (1)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `accounts` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `accounts` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `accounts` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).

## 2026-09-28 — TD-048 — Feature `assets` still makes 6 business decision(s) in the interface

- Found by: architecture rule A12 (#047), first freeze
- Where: `assets/asset_table/useAssetTable.ts` (4), `assets/shared/validateAsset.ts` (1), `assets/useAssets.ts` (1)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `assets` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `assets` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `assets` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).

## 2026-09-28 — TD-049 — Feature `categories` still makes 2 business decision(s) in the interface

- Found by: architecture rule A12 (#047), first freeze
- Where: `categories/category_table/useCategoryTable.ts` (2)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `categories` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `categories` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `categories` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).

## 2026-09-28 — TD-050 — Feature `performance` still makes 9 business decision(s) in the interface

- Found by: architecture rule A12 (#047), first freeze
- Where: `performance/account_view/useAccountPerformance.ts` (2), `performance/global_view/useGlobalPerformance.ts` (3), `performance/shared/globalPresenter.ts` (3), `performance/shared/presenter.ts` (1)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `performance` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `performance` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `performance` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).

## 2026-09-28 — TD-052 — Feature `unpriced_prices` still makes 2 business decision(s) in the interface

- Found by: architecture rule A12 (#047), first freeze
- Where: `unpriced_prices/useUnpricedPrices.ts` (2)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `unpriced_prices` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `unpriced_prices` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `unpriced_prices` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).

## 2026-09-28 — TD-053 — Feature `whats_new` still makes 3 business decision(s) in the interface

- Found by: architecture rule A12 (#047), first freeze
- Where: `whats_new/parseChangelog.ts` (3)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `whats_new` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `whats_new` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `whats_new` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).

## 2026-09-28 — TD-057 — The performance contract lags the response

- Found by: spec-reviewer on #012
- Where: `docs/contracts/account-contract.md` — `PerformancePeriod`, `PerformanceMetric`; no row for `get_global_performance`
- Severity: 🔵
- Observation: the contract's `PerformancePeriod` lacks `annualized_yield` and the bridge fields (`previous_value`, `cash_flow`, `asset_flow`, `dividends`, `pnl`), `PerformanceMetric.pct` says "denominator is 0" where PRF-032 says not positive, and `get_global_performance` has no command row in any contract.
- User value: None directly — the contract describes what the interface receives.
- Done when: the contract's performance types match `src-tauri/src/use_cases/shared/performance.rs` and `get_global_performance` has its row, checked by contract-reviewer.

## 2026-10-03 — TD-063 — The spec lets a correction change the asset; the code does not

- Found by: spec-reviewer and contract-reviewer on TD-056
- Where: `docs/spec/financial-asset-transaction.md` — TRX-032; `CorrectTransactionDTO` in `src-tauri/src/use_cases/holding_transaction/api.rs`
- Severity: 🔵
- Observation: TRX-032 says changing a transaction's asset or account is permitted and recalculates both holdings. A correction carries neither: it keeps the asset and the account of its transaction. CSH-062 states what the code does.
- User value: None directly — the spec says what a correction can change.
- Done when: TRX-032 lists the fields a correction changes, and no rule mentions moving a transaction to another asset or account.

## 2026-10-03 — TD-066 — An asset's name, reference and category have no length or character rule

- Found by: reviewer-security on #055
- Where: `Asset::validate` in `src-tauri/src/context/asset/domain/asset.rs`; `AssetService::add_named_asset` (the command line's path, with the same rules)
- Severity: 🔵
- Observation: the asset rules check that a name and a reference are not empty, and nothing else: no maximum length, and a control character is accepted from the window's form or from another device's synced change. The command line escapes what it prints.
- User value: None directly — a name pasted with a stray control character or of unreasonable length is refused where it is typed.
- Done when: `Asset::validate` refuses a control character and a name or reference above a stated length, with a typed error the form shows and the command line reports.

## 2026-10-03 — TD-067 — The correction form of an opening balance still decides in the interface

- Found by: reviewer-frontend on TD-054
- Where: `src/features/transactions/edit_transaction_modal/useEditTransactionModal.ts` — the branch of an opening balance: saving enabled on `Boolean(date && quantity && unitPrice)`, the total shown as typed
- Severity: 🔵
- Observation: a purchase, a sale and a dividend being corrected follow the core's draft check; a new opening balance does too (TRX-066). The correction of an opening balance is the last transaction form that decides in the interface when it can be saved; recording rejects what is wrong on save.
- User value: None directly — the form says what is wrong before saving, like the others (TRX-067).
- Done when: the opening balance draft check covers a correction, the form follows it, and the hook holds no condition of its own.

## 2026-10-03 — TD-069 — The interface decides whether sync needs attention, in two places

- Found by: the main agent, building the sync page (#010)
- Where: `syncHealth` in `src/features/settings/shared/presenter.ts` (the sync page's leading word) and `needsAttention` in `src/features/shell/sync_indicator/useSyncIndicator.ts` (the shell indicator)
- Severity: 🔵
- Observation: both read the sync status and decide from its failures, notices, inconsistent holdings — and, for the page only, held-back changes — whether sync needs attention. The two rules already differ by one case. The status is assembled in several places of the core (the sync service, the run, the portfolio sync use case), so a health field stored on it would go stale; the core has no single point where a status leaves for the interface.
- User value: The indicator in the header and the sync page never disagree about whether something needs a look.
- Done when: the core says the health of sync with the status, from one rule; the page and the indicator show it; neither decides.

## 2026-10-04 — TD-073 — Correcting a record without typing can change its figures

- Found by: the main agent, building the dividend correction dialog (TD-068): its test showed an exchange rate of 0.9214 pre-filled as 0.921
- Where: `microToDecimal` (three decimals by default) used to pre-fill a form from a recorded figure — `useEditTransactionModal.ts` (quantity, unit price, exchange rate, fees), the shell mounts of the free shares, management fee, interest and split corrections (quantity or factor), `useDepositTransaction.ts` and `useWithdrawalTransaction.ts` (amount); and `parseFloat(microToDecimal(…))` when a trade's unit price is recorded as the asset's price
- Severity: 🟡
- Observation: a figure is stored with six decimals and shown with three (TRX-024). A correction form is filled with the three-decimal text, so opening a correction and saving it without typing rewrites a quantity of 0.123456 as 0.123 and a rate of 0.9214 as 0.921. The dividend correction dialog fills its fields exactly (`microToExactDecimal`); the other forms do not. The same family as #058 (one conversion reads a typed number, one writes it).
- User value: Opening a correction and saving it changes only what the user typed.
- Done when: every form filled from a recorded figure reads back to the same micro-units when saved untouched, shown by a test per form; a recorded market price carries the trade's unit price to its last decimal.

## 2026-10-04 — TD-074 — A back-dated sale passes its draft check and is refused on save

- Found by: reviewer-backend on TD-070
- Where: `validate_draft` in `src-tauri/src/use_cases/holding_transaction/orchestrator.rs` (the oversell check of a new sale)
- Severity: 🔵
- Observation: the draft check compares the quantity sold with what is held today. A sale dated before later purchases can sell more than was held on its date: the draft is clean, and recording refuses it (`CascadingOversell`). The draft now returns no potential gain in that case, but reports no problem.
- User value: The sell dialog says before saving that the quantity was not held at that date.
- Done when: the draft check of a new sale refuses a quantity above the position as of the sale's date, with the error recording would give, shown on the quantity field.

## 2026-10-04 — TD-075 — A split factor has no upper bound, and the replay's quantity can wrap

- Found by: reviewer-security and reviewer-backend on TD-071
- Where: `Transaction::split` in `src-tauri/src/context/account/domain/transaction.rs` (accepts any positive factor); the split arms of the replay in `src-tauri/src/context/account/domain/account.rs` (`total_quantity as i64`)
- Severity: 🔵
- Observation: the replay multiplies the quantity by the factor in 128 bits and casts the result back to 64 bits without a check. A factor large enough (far beyond any real split) makes the cast wrap: recording and the split check both then show a quantity that is wrong, or refuse a split for the wrong reason.
- User value: None in practice — no real split comes near the bound; an absurd factor typed by mistake is refused with a clear reason.
- Done when: a factor whose rescale cannot be stored is refused when the split is recorded and when it is checked, with one error code, shown by a test on each path.
