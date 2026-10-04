# Flow

Observations about how work moves — the harness, the checks, the waits, the human's
part — and what to change. `todo.md` says what the application owes its user,
`techdebt.md` what the code owes itself; this file says what the workflow owes the two
people who run it. It also holds the todo and tech-debt entries that are about the flow;
each keeps the reference it was filed under.

The rules for an entry, set by the owner on 2026-10-03:

1. Quality and speed are weighed together: a change that buys speed says what it risks,
   a change that buys safety says what it costs.
2. Where no change is needed, none is proposed. "Keep" is a verdict.
3. Human review is a last resort and never forgotten: when only the owner can judge, the
   questions come in one block for the batch, with what to look at and what a yes means.

Each entry carries a permanent `FLOW-NNN` reference (never renumbered, never reused; next
free: FLOW-019) so the owner can queue it in `docs/todo.md` § Next like any other. An
entry is removed once it is settled.

---

## Measured — the 0.5.0 batch (2026-09-29 → 2026-10-03)

| What                                             | Figure                                                                                                                                                                         |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Pull requests merged (one closed and redone)     | 24 (#66–#91), about 6 900 lines added                                                                                                                                          |
| Mean time from opening a pull request to merging | 25 min                                                                                                                                                                         |
| CI rounds                                        | 43 for 26 branches — 17 beyond the first round                                                                                                                                 |
| What caused the 17 extra rounds                  | a rebase after another merge (about 10), a fix (about 6), 1 lost to the usage limit                                                                                            |
| Workflow failures on pull requests               | 9: Review 6 (2 of them the usage limit), Quality 2, E2E 1                                                                                                                      |
| E2E run, before and after TD-044                 | 15.3 min → 5.0 min (median of the day's runs)                                                                                                                                  |
| Quality run / Review run                         | about 3 min / about 2 min                                                                                                                                                      |
| Real defects found by reviewers before merge     | at least 9 (the core accepted a Cash asset, a wrong sign in a conversion, the Cash Category could be deleted, a stale error in a dialog, a readiness rule that failed open, …) |
| Real defects found by challenging an entry first | 2 (TD-062 hid a refused correction; TD-056 aimed at the wrong layer)                                                                                                           |

Reading: the reviewers and the challenge step paid for themselves on almost every pull
request. The time went to waiting on CI and to rounds that tested nothing new.

---

## FLOW-003 — Every merge sends the other open pull requests round again

- Kind: speed
- Observed: about 10 of the 43 rounds re-tested a pull request whose own files had not
  changed, because another had merged first. `just merge` already waives the round when
  the rebase changes only record files.
- Verdict: **keep, and re-measure in 0.6.0.** The round now costs about 12 min instead
  of 25 (FLOW-004), and what worked in this batch — one code pull request in CI at a
  time, the next one stacked locally — needs no tool. A merge queue would cost a
  rewrite of `just merge` for a saving that has already shrunk by half.

## FLOW-004 — The E2E suite's start-up wait

- Kind: speed
- Observed: 30 s lost at each of 23 application starts, for a missing session bus.
- Verdict: **settled** by TD-044 (lesson L-020): 15.3 → 5.0 min a run. What is worth
  keeping is how it was found — timestamps of one CI log, then one experiment run started
  by hand on a branch with no pull request. A suspicion about CI costs one such run to
  test; say so in `docs/workflow.md`.

## FLOW-006 — An entry whose Done-when is an audit cannot close

- Kind: speed
- Observed: TD-065 ("every figure listed with where it is computed") took four pull
  requests and is still open: the audit kept finding work. #055 promised a refusal that
  an existing rule (AST-009) forbids, found only when the owner read the result.
- Proposal: the challenge step, already run before starting a TD or an issue, also (a)
  splits an entry whose Done-when is a survey into "list them" and one entry per thing
  found, and (b) reads the entry against the spec rules of its domain before coding.
- Costs: minutes per entry. Protects: a pull request written to a promise the rules
  refuse.

## FLOW-007 — A spec promised what the code did not do

- Kind: quality
- Observed: CSH-015 said the core refuses the Cash class (it did not), TRX-032 that a
  correction can change the asset (it cannot — TD-063). `spec-checker` runs only when an
  entry with spec rules closes, so a rule nobody touches is never re-read.
- Proposal: before a release, `spec-checker` runs once on every spec the batch touched,
  and its findings are filed as debt, not fixed in the release.
- Costs: one agent run per touched spec at release time. Protects: the specs' claim to
  describe the application.

## FLOW-008 — The same reviewers run locally and in CI

- Kind: quality + speed
- Observed: each lane costs about a minute locally and about two in CI. They did not
  find the same things: CI's architecture lane caught a critical on #77 that the local
  run had not been asked — the lane was skipped locally.
- Verdict: **keep both.** The local run is what makes the first CI round green; CI is
  the one that cannot be skipped. One change: the architecture lane is launched locally
  with the backend or frontend lane every time, as CLAUDE.md already says — the skip
  cost a round.

## FLOW-010 — The local machine cannot carry a mutation sweep

- Kind: speed
- Observed: a local sweep with the commit hooks and another project's builds took the
  load to 53; the pre-push lint timed out three times at 600 s, and stopping it by
  process name killed the other project's builds.
- Proposal: sweeps run in CI only (`mutants.yml`); a background job is stopped by its
  task or process id, never by name. One line each in `docs/workflow.md`.
- Costs: nothing. Protects: an hour, and a neighbour's build.

## FLOW-011 — Parts of the harness run only on the day they matter

- Kind: quality
- Observed: CI floated on the latest Rust until a new lint broke `main` (TD-059, fixed);
  the visual reference went stale when the month changed (TD-060, fixed); the release
  workflow's Windows steps run only at a release; the golden portfolio had never been
  read through the live account page (fixed in #87).
- Proposal: a weekly scheduled run of what otherwise runs only at a release — the
  Windows build without publishing.
- Costs: about 20 runner-minutes a week. Protects: a release day spent on a workflow
  bug.

## Used and not used — the 0.5.0 batch

Counted from the session's transcript, 2026-09-29 → 2026-10-03: what the agent invoked
itself. Hooks and CI ran their own share on every commit and push; that is not counted.

| What                  | Used (times)                                                                                                                                          | Not used                                                                                                                                                                                                                         |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Skills, invoked       | none                                                                                                                                                  | all nine — `/next-todo`, `/design-proposal`, `/visual-proof`, `/review-triage`, `/techdebt`, `/whats-next`, `/adr-writer`, `/dep-audit`, `/prune`                                                                                |
| Agents, local         | reviewer-backend 9, reviewer-frontend 6, reviewer-infra 4, reviewer-arch 4, spec-reviewer 3, reviewer-security 3, reviewer-e2e 2, contract-reviewer 1 | spec-checker, adr-reviewer, reviewer-sql                                                                                                                                                                                         |
| Recipes               | `merge` 47, `arch-check` 12, `generate-types` 9, `test-scripts` 9, `worktree` 4, `release` 3, `licence-check` 2, `test-rust` 1                        | `harness`, `check`, `check-full`, `format`, `test`, `test-unit`, `coverage-fe`, `coverage-be`, `coverage-gate`, `dev`, `dev-seed`, `install`, `stat`, `db-migrate`, `prepare-sqlx`, `next-todo`, `test-e2e`, `test-e2e-headless` |
| Native commands       | `cargo test` 71, `cargo fmt` 53, `npx vitest run` 45, `npx prettier` 44, `npx biome` 42, `npx tsc` 40, `cargo clippy` 35                              | —                                                                                                                                                                                                                                |
| Scripts, run directly | `whats-next.py` 8, `visual-proof-capture.mjs` 1                                                                                                       | `build.sh` and the two E2E recipes, which nothing ran — deleted; every other script is called by a recipe, a hook, CI or a skill                                                                                                 |

Reading: the reviewers were used as the workflow says. The skills and the recipes were
not — the work was done by hand, in the skills' spirit, with native commands.

## FLOW-018 — The flow is audited once, by hand

- Kind: quality + speed
- Observed: this file's figures — pull requests and their time to merge, CI rounds and
  what caused them, run durations, what the agent invoked — were gathered by hand after
  0.5.0, from `gh` and the session's transcript, in about an hour. Nothing will gather
  them after 0.6.0 unless someone thinks of it. Raised by the owner on 2026-10-04.
- Proposal: a `/flow-audit` skill run after each release, on a script
  (`scripts/flow-audit.py`) that measures the same things between two tags and prints
  them beside the previous release's. The skill writes the "Measured" and "Used and not
  used" sections anew, proposes entries only where a figure moved or a rule was not
  followed, and re-reads the open entries: settled ones leave, "keep" verdicts are
  re-measured. A skill, not an agent: the judging needs the session's context, the
  counting does not.
- Costs: about half a day for the script and its tests; a few minutes per release.
  Protects: the flow being improved on figures, release after release.
- Decided: go, built last in the 0.6.0 batch and run first at the 0.6.0 release (owner, 2026-10-04).

## Moved here from the todo and the tech debt

Entries that are about how work moves, not about the application. Each keeps the
reference it was filed under and is queued by it.

## #049 — (tooling) — References name what they point at: TODO-NNN, DEBT-NNN, ghNN

Decided by the owner on 2026-09-27: a todo entry becomes `TODO-NNN` (today `#NNN`), a tech-debt entry `DEBT-NNN` (today `TD-NNN`), and a GitHub issue `ghNN`. Today `#043` reads like pull request #43 on GitHub, where it also links to the wrong thing. Measured the same day: `TD-NNN` appears 53 times in 10 files, a `#0NN` todo reference in 47 files, `gh#NN` 16 times; three scripts parse the formats (`scripts/next-todo.sh`, `scripts/check.py`, `scripts/release.py`), as do the skills (`/next-todo`, `/techdebt`, `/review-triage`, `/whats-next`), `CLAUDE.md` and `docs/workflow-c.md`.

**User value:** None directly — a reference says what it points at and no longer collides with a pull request number.
**Done when:** The three formats are written into `CLAUDE.md`, `docs/workflow-c.md` and the skills; the scripts parse and emit the new forms, with a test each; every live file uses them (`docs/todo.md`, `docs/techdebt.md`, specs, lessons, code comments and test names); a check fails on an old-form reference in a changed file (seen red); `CHANGELOG.md` and git history stay as they are.
**Design:** none
**Open questions:**

- [ ] Rewrite existing references, or only new ones? (Recommended: rewrite every live file in one pull request, so one format exists at a time; the changelog and git history keep the old form.)

## #014 — (e2e) — Drive a second device in the E2E suite

The multi-device sync E2E covers the single-device critical path only (plan § Halt Artifact H1): `wdio.conf.ts` launches one binary with one `VAULT_COMPASS_E2E_DATA_DIR` and `maxInstances: 1`, so joining a folder another device created (SYN-014/036) is proven by the two-database integration test `src-tauri/tests/sync_two_devices.rs`, not through the UI. A real two-device E2E needs an `e2e/helpers/second_device.ts` that launches a second binary against its own data directory plus a wdio multi-remote configuration — a separate, pre-requisite task before any join scenario is written.

**User value:** None directly — test infrastructure.
**Done when:** A wdio multi-remote config and `e2e/helpers/second_device.ts` launch a second binary on its own data directory, and a join scenario (SYN-014/036) passes through the UI.
**Design:** none
**Open questions:** none

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

## 2026-09-21 — TD-037 — The settings capture still carries a random folder path

- Found by: the main agent (PR #8, a Markdown-only pull request, visual gate red)
- Where: `e2e/sync/sync.test.ts:114` (`mkdtempSync(join(tmpdir(), "folioneer-sync-"))`), captured in `sync-settings-{light,dark}`
- Severity: 🔵
- Observation: the sync section renders the shared folder, whose `mkdtemp` suffix differs every run, so those pixels (columns 611–651, 0.106 % of the screen) always differ between two runs of the same commit. Below the workflow's 0.3 % threshold on its own, so nothing fails today; it was a third of the budget when the fading scrollbar took the rest and pushed the total to 0.327 %. It leaves the gate that much closer to a false regression on any screen that shares it.
- User value: None — the merge gate's headroom.
- Done when: `sync-settings` is byte-identical across two runs of the same commit; the E2E sync folder carries a fixed name (the suite runs one instance, `maxInstances: 1`) or the value is not rendered into the capture.

## 2026-09-27 — TD-039 — The currency rates spec's rate edit can hit a stale element

- Found by: the main agent (PR #18, a records-only pull request, E2E attempt 1 after a rebase; attempt 2 green)
- Where: e2e/currency/currency_rates.test.ts (`FXR-052: editing a rate via the UI updates the rate row`)
- Severity: 🟡
- Observation: The test failed with `stale element reference` while creating a node handle for an `element` call — the same failure as TD-019, in a different spec. The Currency Rates view re-fetches its pairs and rates on `CurrencyRateUpdated`, so the row a step located can be re-rendered before the next step uses it. The same pull request hit TD-019 on its first run: two once-only failures on a change the suite cannot execute (see #041).
- User value: None — suite reliability.
- Cause fixed (2026-09-27): views no longer unmount their rows on an event-driven re-fetch (F29), and the currency rates drill-ins locate a row's cell with one selector instead of chaining from a row handle. The entry closes after a month of pull-request runs without this failure.
- Done when: the test re-locates the row after the edit is saved, or waits for the view's re-fetch to settle; a month of pull-request runs shows no failure of it.

## 2026-09-27 — TD-043 — The reviewer agents run without a turn cap and load the whole CLAUDE.md

- Found by: the main agent, comparing the agent files with the Claude Code sub-agent documentation
- Where: `.claude/agents/reviewer-*.md` (frontmatter), `.github/workflows/review.yml`
- Severity: 🔵
- Observation: The sub-agent format now offers `maxTurns` (stop a runaway review), `effort` (per agent) and `omitClaudeMd` (skip the project `CLAUDE.md` when the prompt is self-contained). None is set: every CI review loads `CLAUDE.md` seven times per pull request, and nothing bounds a review that loops.
- User value: None directly — cheaper, bounded reviews on every pull request.
- Done when: one reviewer runs with `omitClaudeMd: true` and a `maxTurns` cap on a sample diff and reports the same findings as without; if it does, the settings extend to every reviewer, with the review time and cost before and after recorded.

## 2026-10-03 — TD-064 — 60 changes in the logic code that no test notices, none of them in the account, the valuation or the performance

- Found by: the mutation sweep of 2026-10-01 (issue 65); the 50 of `account.rs` were sorted on 2026-10-03, and the sweep of 2026-10-04 on `main` (run 37151658259: 1 890 mutants — 1 307 caught, 218 missed, 354 unviable, 11 timed out) reports none left in `account.rs` nor in `context/account/service.rs`
- Where: `use_cases/account_details/orchestrator.rs` (3: a fee dated on the as-of day, the as-of loop's skip of cash lines, the window-start dates given to the rate lookup), `use_cases/update_checker/service.rs` (3: the download and the installation themselves, which need the running application), `use_cases/fee_generation/orchestrator.rs` (1: a schedule ending on a period's last day), `context/sync/application/` (`run.rs` 5, `intake.rs` 3 — two of them re-read what is already applied, which the conflict rules ignore: nothing to observe, `join.rs` 5, `first_publish.rs` 4), and the files with fewer — the full list is in the sweep's artifacts
- Severity: 🟡
- Observation: sorting `account.rs` found one real bug (a corrected deposit refused), tests that only ever used one asset per account, a synced removal no test checked, unreachable code and three comparisons that changed nothing; the sweep that followed finds nothing there. The 68 of `use_cases/shared/valuation.rs`, `use_cases/shared/performance.rs` and `use_cases/global_performance/orchestrator.rs` were sorted on 2026-10-04: tests on figures computed by hand for the terms of a period's bridge (no test checked a withdrawal, interest or a position's trades there), for a foreign holding's value and for the weight of a flow; two lines simplified where the change made no difference. Of the 14 of the account page, 11 are sorted: the as-of view had no test with a transaction dated on the day itself, a closed position, a cash balance back to nothing or a position that cost nothing. The 24 of `context/sync/domain/resolution.rs` are sorted: no test met an equal rank, an edit over a concurrent removal, an overruled change of the application's, an observation through the full decision, a removed catch-up position, or each condition of a collision on its own. The 10 of `use_cases/holding_transaction/orchestrator.rs` were one check written five times — is the asset still held — now written once (`holds`) and tested with a closed position; 8 of the 9 of the fee generation were the quarter arithmetic, which no test reached. The 10 of `use_cases/asset_web_lookup/orchestrator.rs` were the client of the lookup service, which no test ever called: it is now tested against a local server (what it asks, what it reads, a rate limit told apart from a failure). The 10 of the scheduled fetch's sweep are sorted: no test counted a sweep with more than one asset, nor measured the wait before a retry. Seven of the sync run's reading and counting are sorted by two-device scenarios: each published change applied once and counted, a gap stopping the read, an unreadable manifest counted, a dropped change counted, the roster dated at the run. The other files are not sorted. The local machine cannot run the sweep (five minutes a mutant under load): CI's sweep, four hours, is the proof.
- User value: None directly — a wrong merge of two devices' changes, or a wrong figure in an account's history or performance, would be caught by a test.
- Done when: each remaining missed mutant is sorted, file by file, into a test added, dead code deleted, or code simplified so the change no longer exists; a sweep on `main` confirms it; issue 65 is closed.
