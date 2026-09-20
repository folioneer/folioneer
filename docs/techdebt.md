# Tech Debt

Observations of code smells, brittle patterns, or pre-existing issues surfaced
during work that don't warrant immediate action. Format produced by the
`/techdebt` skill.

Entries are observations, not commitments, and this file is the agent's: it
files here what it notices and what it did not fix. Each entry carries a
permanent `TD-NNN` reference (never renumbered, never reused; next free:
TD-036) so the human can queue it in `docs/todo.md` § Next like any todo.
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

## 2026-05-16 — TD-007 — ADR status vocabulary lacks an "amends" relationship

- Found by: adr-reviewer
- Where: docs/adr/003-cross-context-use-case-orchestration.md, docs/adr/005-account-details-inject-transaction-service.md, docs/adr/README.md
- Severity: 🔵
- Observation: ADR-003 carries `Status: Accepted — amended by ADR-005` and ADR-005 carries `Status: Accepted — amends ADR-003`. The `adr-writer` skill permits only three status values (`Accepted`, `Accepted — supersedes ADR-{NNN}`, `Superseded by ADR-{NNN}`), so "this ADR refines another without superseding it" has no permitted encoding and the two files use a vocabulary the strict reviewer checks refuse. Converting ADR-003 to `Superseded by ADR-005` would lose the "still partly valid" nuance.
- User value: None — ADR vocabulary.
- Done when: The `adr-writer` skill permits an “amends” status, or ADR-003 and ADR-005 are converted to a permitted one.

---

## 2026-05-10 — TD-008 — Migrate to FE gold layout

- Found by: manual (frontend architecture delta scan)
- Where: src/ (top-level structure + features/account_details cross-imports)
- Severity: 🟡
- Observation: Three FE layout/coupling deltas from the gold layout (F26/F27/F28 in `docs/frontend-rules.md`). The current shape works but encodes implicit conventions that diverge from it. Migration is bit-by-bit per `CLAUDE.md` § Gold Standards & Bit-by-Bit Trajectory — apply gold to new code; defer existing-code reshape unless it fits the 50-LOC + locality + mechanical gates.
  1. **`features/account_details/{buy,sell}_transaction/` cross-imports from `features/transactions/`.** Today the imports are `RecordPriceCheckbox` (component), `TransactionFormData` (type), `validateTransactionForm` / `validateSellForm` (pure functions), and `useTransactions` (hook with state). Under F26, the first three (primitives) become fine; the fourth (behavior coupling via a hook) remains a code smell. Either `account_details` owns its own thin wrapper around the gateway calls it needs, or the two features consolidate. Worth deciding _with_ the consolidation question (delta 2) rather than fixing the hook coupling alone.

  2. **`account_details` sub-feature bloat (8 sub-features).** Half of them — `buy_transaction`, `sell_transaction`, `deposit_transaction`, `withdrawal_transaction` — are conceptually transaction-recording flows and overlap with the `transactions/` feature. Two reasonable shapes: (a) consolidate the four into `transactions/` and let `account_details` stay focused on the holdings view, or (b) formalize the split — `account_details` owns "modals invoked from the holding row," `transactions/` owns "the transaction list page and its CRUD." Pick (b) as the lighter move; (a) is a bigger refactor.

  3. **`src/lib/*Storage.ts` adapters belong in `src/infra/settings/`.** The browser-`localStorage` UI-preference adapters (`autoFetchStorage.ts`, `autoRecordPriceStorage.ts`, `lastOperationDateStorage.ts`, `closedSectionStorage.ts`) are platform adapters per F28's Store-kinds table and should move to `src/infra/settings/`. New ones keep landing in `src/lib/` to stay consistent with their siblings (a partial move would orphan one file mid-migration). Mechanical folder move + import-path update; fold into the same `lib/ → infra/` rename PR.

  Migration is mechanical for delta 3 (folder move + import sites) and conventional for deltas 1 and 2 (depends on the consolidation decision). Cleanest as one or two dedicated pull requests.

- User value: None — internal layout.
- Done when: `src/lib/*Storage.ts` moves to `src/infra/settings/`, the `useTransactions` cross-feature coupling is removed, and the `account_details` / `transactions` split is formalised.

## 2026-05-09 — TD-009 — Migrate to gold DDD layout

- Found by: manual (backend layout design discussion)
- Where: src-tauri/src/ (top-level structure)
- Severity: 🟡
- Observation: Three layout deltas from the gold target (B0/B37–B43 in `docs/backend-rules.md`). The current shape works but documents the architecture imperfectly to newcomers. Migration is bit-by-bit per `CLAUDE.md` § Gold Standards & Bit-by-Bit Trajectory — apply gold to new code; defer existing-code reshape unless it fits the 50-LOC + locality + mechanical gates.
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

## 2026-09-12 — TD-015 — Three E2E specs select by text or duplicate a shared helper

- Found by: reviewer-e2e (`.review/reviewer-e2e-2026-09-12-01.md`, pre-existing section)
- Where: e2e/accounts/accounts.test.ts:23 (local `navigateToAccounts` next to the shared one in e2e/helpers/navigation.ts), e2e/asset_web_lookup/asset_web_lookup.test.ts:47 (`button[aria-label="Fill manually"]`), e2e/assets/assets.test.ts:79 and :102 (XPath on `normalize-space(text())`)
- Severity: 🔵
- Observation: Two specs locate elements by their English label or cell text rather than a stable id (E4), which ties them to the forced `en_US` locale and to copy that the i18n files own; one spec carries its own copy of a navigation helper the shared module already provides, so a change to the accounts route has two places to drift.
- User value: None — suite robustness.
- Done when: the three sites select by `id` (adding the ids on the frontend elements in the same commit) and the local helper is replaced by the shared import.

## 2026-09-12 — TD-016 — A controlled-input value can be lost once in the E2E buy flow

- Found by: manual (first pull-request E2E run, attempt 1)
- Where: e2e/account_details/buy_sell.test.ts (TRX-010), e2e/helpers/react.ts (`setReactInputValue`), src/ui/components/field/CalcField.tsx
- Severity: 🟡
- Observation: TRX-010 failed with `submit still not enabled after 5000ms`; the failure screenshot shows the date and the unit price filled and the quantity field empty, so the value set by `setReactInputValue("buy-trx-quantity", "10")` between the two others did not stick. The same spec passed six times on `main` the same day and the field's own state sync guards against prop clobbering, so no deterministic path is known. With E2E as a required check, a once-in-N loss of a set value is a merge blocked for a reason unrelated to the change.
- User value: None — suite reliability.
- Done when: the loss is reproduced (or its trigger understood) and either the helper waits for the field to report the value back before returning, or the field's handling is changed so a dispatched `input` event can never be dropped; TRX-010 no longer needs a re-run to pass.

## 2026-09-12 — TD-017 — Backend logic coverage sits at 89 % against the 90 % target

- Found by: manual (`python3 scripts/coverage-gate.py --backend`; figures refreshed 2026-09-19)
- Where: src-tauri/src/use_cases/update_checker/service.rs (0 %), src-tauri/src/use_cases/scheduled_fetch/headless.rs (5 %), src-tauri/src/use_cases/portfolio_sync/applier.rs (60 %), src-tauri/src/use_cases/asset_web_lookup/orchestrator.rs (62 %), src-tauri/src/context/sync/application/join.rs (74 %), src-tauri/src/use_cases/holding_transaction/orchestrator.rs (77 %), src-tauri/src/context/asset/service.rs (86 %), src-tauri/src/context/account/service.rs (87 %)
- Severity: 🟡
- Observation: 89.06 % of the 11,680 lines in domain, application, service and use-case code are covered; the gate's floor is 85.5 % and its target 90 %, about 110 more covered lines. Two files carry almost no test at all because they talk to the network or run the app headless; the other six are orchestration paths with untested branches. The floor in `coverage-gates.json` is a ratchet — raise it in the same change that lifts coverage, never lower it.
- User value: None — a harness that catches logic regressions in these paths.
- Mutation survivors: the 2026-09-14 sweep (issue #137) found 301 logic changes no test notices — `context/account/domain/account.rs` 52, `use_cases/shared/valuation.rs` 33, `context/sync/domain/resolution.rs` 24, `use_cases/global_performance/orchestrator.rs` 21; each names an assertion that is missing or too weak.
- Done when: the backend floor in `coverage-gates.json` reads 90.0 and the gate passes on `main`.

## 2026-09-13 — TD-018 — 118 interactive components in feature code carry no id

- Found by: manual (`python3 scripts/arch-check.py`, rule A6, first run)
- Where: 34 files under src/features/ listed in arch-allowlist.json `missing_ids`; mostly Cancel and secondary buttons in modals, the price-history and update-banner actions, and the design-system dev page
- Severity: 🔵
- Observation: E1–E4 ask every interactive element for a stable id, and the E2E suite selects by id, yet 118 (119 at the first run) of the 317 `Button` / `IconButton` / `TextField` / `DateField` / `CalcField` / `FAB` tags rendered by feature code have none. The architecture check freezes today's count per file and refuses any growth; the count can only go down. The 18 sibling-feature imports the same check freezes belong to the FE gold layout migration entry above; the 8 `Math.` uses are display rounding and the documented split preview (SPL-061) and need no action.
- User value: None — every control becomes addressable by tests and assistive tech.
- Done when: `missing_ids` in arch-allowlist.json is empty.

## 2026-09-13 — TD-019 — The assets spec's before-each hook can hit a stale element

- Found by: manual (an E2E run, attempt 1, on a pull request that touched no app code)
- Seen again: a `main` push run, attempt 1 (2026-09-15, after a change that touched no E2E or assets code) — same `before each` hook, same stale node handle on an `element` call; attempt 2 green.
- Where: e2e/assets/assets.test.ts (`beforeEach`), e2e/helpers/modal.ts (`dismissLeftoverModal`), e2e/helpers/navigation.ts (`navigateToAssets`)
- Severity: 🟡
- Observation: The hook failed with `stale element reference` while creating a node handle for an `element` call — an element located by one step had been replaced by a re-render before the next step used it. It is the second distinct once-only E2E failure in two days (TD-016 is the first); both sit in setup or navigation code shared by many specs, so each has many chances to fire per run. With E2E as a required check, every such failure costs a re-run before a green PR can merge.
- User value: None — suite reliability.
- Done when: the hook re-locates elements after each navigation step instead of reusing handles across renders, or the shared helpers wait for the route to settle before returning; a month of pull-request runs shows no before-each failure.

## 2026-09-13 — TD-021 — The rust-cache pin is labelled with the wrong tag in three workflows

- Found by: reviewer-infra (phase 12 review, `.review/reviewer-infra-2026-09-13-10.md`)
- Where: `.github/workflows/quality.yml`, `e2e.yml`, `release.yml` (twice) — `Swatinem/rust-cache@e18b4977…` labelled v2.9.1, while that tag peels to `c1937114…`; `security-audit.yml` pins `taiki-e/install-action@f48d2f8b…` with no version label at all
- Severity: 🔵
- Observation: the commits are real upstream commits, so nothing is compromised, but a reader trusting the comment audits the wrong release notes. `mutants.yml` carries the correct pins, and the `install-action` labels were corrected to v2.79.6 since; the rust-cache label is what remains. Tag verified with `git ls-remote --tags` on 2026-09-13, file state on 2026-09-19.
- User value: None — whoever audits a pinned action reads the release notes of the version actually running.
- Done when: every pinned action in `.github/workflows/` carries the label of the tag its commit belongs to, or the pin moves to the commit of the labelled tag.

## 2026-09-14 — TD-023 — E2E specs still locate controls by label, role or form attribute

- Found by: manual (selector count while fixing dangling references in the E2E headers)
- Where: `e2e/asset_web_lookup/asset_web_lookup.test.ts` (`button[aria-label="Add asset"]`, `"Back"`, `"Fill manually"`), `e2e/account_details/manual_price_fill.test.ts` and `e2e/account_details/auto_fetch.test.ts` (`[role="dialog"]`, `[role="status"]`, `body`), `e2e/open_balance/open_balance.test.ts` and `e2e/account_details/buy_sell.test.ts` (`button[type="submit"][form="…"]`)
- Severity: 🔵
- Observation: `docs/e2e-rules.md` asks every selector to be a stable `id`; these specs still find controls by an accessible label (which changes with the locale and the wording), by role, or by the form a submit button belongs to, because the elements carry no id of their own.
- User value: None — E2E specs that survive a wording or locale change.
- Done when: every selector in those five specs is an `id`, the controls they target carry one, and `reviewer-e2e` passes on them.

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

## 2026-09-20 — TD-033 — Two figures are still derived in the frontend

- Found by: a scan of production frontend code for derivation (chat, 2026-09-20)
- Where: src/features/account_details/shared/presenter.ts (`currentValue`, MKT-143), src/features/transactions/edit_transaction_modal/useEditTransactionModal.ts (`unitPriceMicro`, TRX-051)
- Severity: 🟡
- Observation: the holdings presenter computes a holding's market value as `current_price / 1_000_000 × quantity` in floating point, while the backend already values every holding for the account totals; and the edit form of an opening balance computes the unit price it sends to the backend as `floor(total × 1_000_000 / quantity)`, so a stored figure is decided by the frontend, with an intermediate that leaves JavaScript's exact-integer range once the total passes about 9,007 currency units. Both are among the 8 `Math.` / arithmetic uses the architecture check freezes (A7); the other derivation it freezes, the split preview, is documented (SPL-061). Nothing else in feature code aggregates or derives: no `reduce`, and every `sort` is a table's column order.
- User value: None visible today — the market value column and an edited opening balance stop depending on frontend arithmetic, so the two layers cannot disagree on a figure.
- Done when: `HoldingDetail` carries the market value computed by the backend and the presenter only formats it; the opening-balance correction sends the total and the backend derives the unit price with the rule the creation path uses (TRX-047); the two `math_usage` entries leave `arch-allowlist.json`.

## 2026-09-20 — TD-034 — The contributor documents describe a workflow that no longer exists

- Found by: the main agent, while adding the contribution terms
- Where: CONTRIBUTING.md (Quality Check, Alternative: Direct Commands, the sample output), .github/pull_request_template.md (the Tests checklist), COMMIT_POLICY.md
- Severity: 🔵
- Observation: CONTRIBUTING.md still sends a contributor to `./scripts/check.sh` (gone; the script is `scripts/check.py` behind `just check`), quotes test counts from long ago (110 React, 50 Rust) and offers raw `npm` / `cargo` commands the project rules forbid; the pull-request template asks for a hand-ticked test checklist that CI now answers; none of them mentions the harness, the reviewers or `just merge`; COMMIT_POLICY.md still opens with the name of another project it was copied from. The dead link to a pull-request policy was replaced in #029; the rest was left alone as a second story. Harmless while the only contributor is the owner; it is the first thing an outside contributor reads.
- User value: None — a newcomer can follow the documents and arrive at a mergeable pull request.
- Done when: CONTRIBUTING.md and the pull-request template describe `just` recipes, the harness and the checks a pull request passes as they are today, with no command the project forbids.

## 2026-09-20 — TD-035 — A release leaves the lockfile's own version behind

- Found by: reviewer-infra
- Where: scripts/release.py (`update_version_files`), package-lock.json (the two top-level `version` fields)
- Severity: 🔵
- Observation: A release writes the new version into `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json` and `Cargo.lock`, but not into `package-lock.json`, whose own `version` fields keep the value of the last `npm install`. Nothing reads them, so nothing breaks; the file simply states a version the project is not at.
- User value: None.
- Done when: A release leaves `package-lock.json` stating the released version, and a test of the release script proves it.
