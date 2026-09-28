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
- Seen again: PR #18, a records-only pull request, attempt 1 (2026-09-27); attempt 2 green.
- Where: e2e/assets/assets.test.ts (`beforeEach`), e2e/helpers/modal.ts (`dismissLeftoverModal`), e2e/helpers/navigation.ts (`navigateToAssets`)
- Severity: 🟡
- Observation: The hook failed with `stale element reference` while creating a node handle for an `element` call — an element located by one step had been replaced by a re-render before the next step used it. It is the second distinct once-only E2E failure in two days (TD-016 is the first); both sit in setup or navigation code shared by many specs, so each has many chances to fire per run. With E2E as a required check, every such failure costs a re-run before a green PR can merge.
- User value: None — suite reliability.
- Cause not found yet (2026-09-27): the F29 fix for TD-039 does not reach this hook — the assets spec's navigation and modal helpers depend on none of the hooks it changed. Next step: capture which element the `element` call was locating when it went stale (the failure screenshot and the hook's last command), then fix the component that replaces it.
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

## 2026-09-20 — TD-035 — A release leaves the lockfile's own version behind

- Found by: reviewer-infra
- Where: scripts/release.py (`update_version_files`), package-lock.json (the two top-level `version` fields)
- Severity: 🔵
- Observation: A release writes the new version into `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json` and `Cargo.lock`, but not into `package-lock.json`, whose own `version` fields keep the value of the last `npm install`. Nothing reads them, so nothing breaks; the file simply states a version the project is not at.
- User value: None.
- Done when: A release leaves `package-lock.json` stating the released version, and a test of the release script proves it.

## 2026-09-20 — TD-036 — The vocabulary has no entry for the update feature's terms

- Found by: spec-reviewer (UPD-028 review on #037)
- Where: docs/ubiquitous-language.md (no Update section); docs/spec/update.md, docs/adr/020-one-extension-file-per-build.md, docs/contracts/update-contract.md
- Severity: 🔵
- Observation: "update channel", "update server", "update file", "credentials", "refused access", "distribution channel", "public build" and "private build" are used as terms by the spec, the ADR, the contract and the code, but the vocabulary defines none of them. Terms are the owner's to confirm (B5).
- User value: None.
- Done when: The vocabulary carries an Update section whose terms the owner has confirmed, and the spec, ADR, contract and code use them.

## 2026-09-21 — TD-037 — The settings capture still carries a random folder path

- Found by: the main agent (PR #8, a Markdown-only pull request, visual gate red)
- Where: `e2e/sync/sync.test.ts:114` (`mkdtempSync(join(tmpdir(), "folioneer-sync-"))`), captured in `sync-settings-{light,dark}`
- Severity: 🔵
- Observation: the sync section renders the shared folder, whose `mkdtemp` suffix differs every run, so those pixels (columns 611–651, 0.106 % of the screen) always differ between two runs of the same commit. Below the workflow's 0.3 % threshold on its own, so nothing fails today; it was a third of the budget when the fading scrollbar took the rest and pushed the total to 0.327 %. It leaves the gate that much closer to a false regression on any screen that shares it.
- User value: None — the merge gate's headroom.
- Done when: `sync-settings` is byte-identical across two runs of the same commit; the E2E sync folder carries a fixed name (the suite runs one instance, `maxInstances: 1`) or the value is not rendered into the capture.

## 2026-09-23 — TD-038 — A build without an External provider refuses fetch commands with the runtime's own error

- Found by: reviewer-security (#036)
- Where: `src-tauri/src/lib.rs` (the three fetching use cases are managed only with an External provider), `src-tauri/src/core/specta_builder.rs` (their commands stay registered in every build)
- Severity: 🔵
- Observation: in such a build `fetch_all_asset_prices`, `fetch_account_asset_prices`, `configure_scheduled_fetch`, `get_scheduled_fetch_status` and `backfill_holding_price_history` reach no managed state, so Tauri refuses the call before the command body runs. That satisfies MKT-210 and SPF-070 — nothing is fetched or written, and the message names only the command and its argument, both public in `bindings.ts` — but the refusal is a plain string, not a `{ code }` error, so the error model's typed pipeline does not see it. Nothing calls these commands today: the interface hides every control that would (MKT-212).
- User value: None today.
- Done when: a call to any of those commands in a build without an External provider answers a typed code the frontend presenter can map, through one shared guard rather than five hand-written checks — worth doing when a second optional capability (a bank feed, the advice module) makes the pattern repeat.

## 2026-09-27 — TD-039 — The currency rates spec's rate edit can hit a stale element

- Found by: the main agent (PR #18, a records-only pull request, E2E attempt 1 after a rebase; attempt 2 green)
- Where: e2e/currency/currency_rates.test.ts (`FXR-052: editing a rate via the UI updates the rate row`)
- Severity: 🟡
- Observation: The test failed with `stale element reference` while creating a node handle for an `element` call — the same failure as TD-019, in a different spec. The Currency Rates view re-fetches its pairs and rates on `CurrencyRateUpdated`, so the row a step located can be re-rendered before the next step uses it. The same pull request hit TD-019 on its first run: two once-only failures on a change the suite cannot execute (see #041).
- User value: None — suite reliability.
- Cause fixed (2026-09-27): views no longer unmount their rows on an event-driven re-fetch (F29), and the currency rates drill-ins locate a row's cell with one selector instead of chaining from a row handle. The entry closes after a month of pull-request runs without this failure.
- Done when: the test re-locates the row after the edit is saved, or waits for the view's re-fetch to settle; a month of pull-request runs shows no failure of it.

## 2026-09-27 — TD-041 — Clickable table rows have no interactive element of their own

- Found by: the main agent (TD-039 review — nine E2E steps click `td:first-child` to open a row)
- Where: `src/features/currency/currency_rates_view/CurrencyRatesView.tsx` (`pair-row-*`), the account rows of `src/features/accounts/`, and every row whose `<tr>` carries `onClick`; the E2E specs clicking `… td:first-child`
- Severity: 🔵
- Observation: The rows are opened by an `onClick` on the `<tr>`, made focusable with `tabIndex` and a key handler. WebDriver cannot click a `<tr>` (its centre hit-tests to a cell), so specs click the first cell and rely on the event bubbling — a click on the wrong cell, or a cell that stops propagation, breaks them. Assistive technology meets a row announced as a row, not as a control that opens something.
- User value: Screen-reader and keyboard users meet a real link or button to open each row; the E2E suite clicks that control directly.
- Done when: each clickable row holds one link or button (with an `id` and an accessible name) that opens it, the row stays clickable for the mouse, and no E2E spec clicks `td:first-child`.

## 2026-09-27 — TD-042 — A quick change of key can show the previous key's data as loaded

- Found by: reviewer-frontend (the F29 change to the view hooks)
- Where: `src/features/account_details/account_details_view/useAccountDetails.ts`, `account_details/price_history/usePriceHistory.ts`, `accounts/useAccountSummaries.ts`, `transactions/account_journal/useAccountJournal.ts`, and `fetchRates` in `currency/currency_rates_view/useCurrencyRatesView.ts`
- Severity: 🔵
- Observation: These hooks have no request-sequence guard, unlike the two performance hooks. When the key changes twice quickly (another account, another asset), an earlier fetch that answers last overwrites the newer data, and its `finally` clears the loading flag while the current fetch is still pending — the previous key's rows show as loaded for a moment. Predates F29; the change only moved where the flag is raised.
- User value: Switching accounts or assets quickly never shows the one left behind.
- Done when: each listed hook drops a response that a newer fetch has superseded, as the performance hooks do, with a test per hook.

## 2026-09-27 — TD-043 — The reviewer agents run without a turn cap and load the whole CLAUDE.md

- Found by: the main agent, comparing the agent files with the Claude Code sub-agent documentation
- Where: `.claude/agents/reviewer-*.md` (frontmatter), `.github/workflows/review.yml`
- Severity: 🔵
- Observation: The sub-agent format now offers `maxTurns` (stop a runaway review), `effort` (per agent) and `omitClaudeMd` (skip the project `CLAUDE.md` when the prompt is self-contained). None is set: every CI review loads `CLAUDE.md` seven times per pull request, and nothing bounds a review that loops.
- User value: None directly — cheaper, bounded reviews on every pull request.
- Done when: one reviewer runs with `omitClaudeMd: true` and a `maxTurns` cap on a sample diff and reports the same findings as without; if it does, the settings extend to every reviewer, with the review time and cost before and after recorded.

## 2026-09-27 — TD-044 — Every E2E spec file waits about 30 seconds before the app appears

- Found by: the main agent, measuring CI for #046
- Where: `wdio.conf.ts` (a session and a fresh app per spec file), the E2E job in `.github/workflows/e2e.yml`
- Severity: 🟡
- Observation: The 23 spec files run their tests in about 80 seconds in total, yet the step takes 13–15 minutes: each file starts the app and waits ~30 s (10 s for one of them) between `RUNNING` and the webview's first log line. Disabling the accessibility bus (`NO_AT_BRIDGE=1`) removed its warning but not the wait (run of 2026-09-27), so the cause lies elsewhere — the WebDriver session start, the app's own start-up, or a timeout in the webview.
- User value: None directly — code pull requests merge ten minutes sooner; the wait may also be felt by a user if it is the app's own start-up.
- Done when: an E2E run with timestamped start-up logs (driver, app setup, first render) names the step that waits; it is removed or shortened at its cause, and the E2E job's time before and after is recorded.

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

## 2026-09-28 — TD-051 — Feature `transactions` still makes 6 business decision(s) in the interface

- Found by: architecture rule A12 (#047), first freeze
- Where: `transactions/account_journal/useAccountJournal.ts` (3), `transactions/add_transaction/AddTransactionModal.tsx` (1), `transactions/edit_transaction_modal/EditTransactionModal.tsx` (1), `transactions/transaction_list/useTransactionList.ts` (1)
- Severity: 🟡
- Observation: validation, grouping, ordering or filtering by business meaning still runs in the frontend of `transactions` (F32). Each site is frozen in `arch-allowlist.json`; a new one fails the check.
- User value: None directly — `transactions` behaves the same in the window, the command line and any later interface.
- Done when: every frozen site of `transactions` moves into a core query, a draft check or a query parameter, with its rule tested in Rust; its `decision_sites` entries leave the allowlist (`just arch-check --write-allowlist`).

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

## 2026-09-28 — TD-054 — A dividend correction still previews its total in the interface

- Found by: #047 part 3 — the draft check covers a purchase and a sale only (TRX-062)
- Where: `transactions/edit_transaction_modal/useEditTransactionModal.ts` — `computeTotalMicro` for every type but a purchase or a sale, and the save button enabled once date, quantity and amount are filled
- Severity: 🟡
- Observation: the correction form shows a dividend's total computed in the frontend with the purchase formula (F32); the backend recomputes it on save (DIV-040). The architecture check does not see a formula call, so nothing stops a new one.
- User value: None directly — the total shown while editing a dividend comes from the rule that records it.
- Done when: the dividend correction takes its displayed total from a backend check, `computeTotalMicro` has no caller left in `src/` and is deleted.
