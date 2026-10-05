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
free: FLOW-022) so the owner can queue it in `docs/todo.md` § Next like any other. An
entry is removed once it is settled.

---

## Measured — the 0.6.0 batch (2026-10-03 → 2026-10-05)

Counted by `scripts/flow-audit.py v0.5.0 v0.6.0 --previous v0.4.0`; "was" is the 0.5.0
batch counted the same way, which is why its figures differ from those gathered by hand
a release ago.

| What                                                    | Figure                                                                                                                                                                                                                                              |
| ------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Pull requests merged (one closed and redone, each time) | 36 (#95–#131), 9 100 lines added — was 27, 7 476                                                                                                                                                                                                    |
| By type                                                 | test 10, refactor 8, docs 8, chore 4, fix 4, feat 1, ci 1 — was docs 9, feat 5, refactor 5, ci 3, chore 2, fix 2, test 1                                                                                                                            |
| Time from opening a pull request to merging             | median 10.9 min, mean 15.4 — was 17.5 and 22.8                                                                                                                                                                                                      |
| CI rounds                                               | 61 for 37 branches, 24 beyond the first — was 46 for 28, 18 beyond                                                                                                                                                                                  |
| What caused the 24 extra rounds                         | not counted                                                                                                                                                                                                                                         |
| Workflow failures on pull requests                      | 8: Review 7, E2E 1 — was 9: Review 6, Quality 2, E2E 1                                                                                                                                                                                              |
| E2E run / Quality run / Review run (median, green)      | 4.6 / 2.9 / 2.1 min — was 9.6 / 3.5 / 2.0                                                                                                                                                                                                           |
| Real defects found by reviewers before merge            | at least 5 (a conversion done in the command layer, a reset device read as paused without the spec saying so, a cursor that jumped in a number field, a hint showing a figure that was no number, a list cut short without notice) — was at least 9 |
| Real defects found by writing the missing tests         | 2 (a corrected deposit was refused before its own balance counted; a failed price write stalled the fetch progress)                                                                                                                                 |
| Real defects found by challenging or closing an entry   | 1 (TD-078: the spec says a corrected purchase unarchives its asset, the code does not) — was 2                                                                                                                                                      |
| Mutation sweep on `main`                                | 63 changes no test notices out of 1 634 — was 218                                                                                                                                                                                                   |

Reading: a pull request merges in two thirds of the time, because the E2E run halved and
because most of this batch was tests and refactors. Four rounds in ten still come after
the first, the same share as before, and nobody counted why. The tests written for the
sweep found two bugs no reviewer had.

---

## FLOW-003 — Every merge sends the other open pull requests round again

- Kind: speed
- Observed: in 0.5.0 about 10 of the rounds re-tested a pull request whose own files had
  not changed, because another had merged first. In 0.6.0, 24 rounds of 61 came after the
  first (39 %; was 18 of 46, 39 %), and what caused them was not counted. `just merge`
  already waives the round when the rebase changes only record files.
- Verdict: **keep, and count the causes in 0.7.0 (FLOW-020).** A round costs about 5 min
  where it cost 25, and the median pull request merges in 11 min. A merge queue would
  cost a rewrite of `just merge` for a saving that is now small.

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
- 0.6.0: `spec-checker` was not run once, although three entries closed with spec rules
  (TD-071, TD-063, #058), and closing TD-063 found a third promise the code does not keep
  (TD-078). The figures argue for the proposal.

## FLOW-008 — The same reviewers run locally and in CI

- Kind: quality + speed
- Observed: each lane costs about a minute locally and about two in CI. They do not find
  the same things. In 0.6.0 the architecture lane was launched locally 4 times for 24
  backend and frontend launches, and CI's architecture lane then refused #124 (a
  conversion in the command layer): one round. The same skip cost a round on #77 in
  0.5.0.
- Verdict: **keep both.** The local run is what makes the first CI round green; CI is
  the one that cannot be skipped. The rule — the architecture lane goes with the backend
  or frontend lane every time — is written and was not followed twice: see FLOW-021.

## FLOW-010 — The local machine cannot carry a mutation sweep

- Kind: speed
- Observed: a local sweep with the commit hooks and another project's builds took the
  load to 53; the pre-push lint timed out three times at 600 s, and stopping it by
  process name killed the other project's builds.
- Proposal: sweeps run in CI only (`mutants.yml`); a background job is stopped by its
  task or process id, never by name. One line each in `docs/workflow.md`.
- Costs: nothing. Protects: an hour, and a neighbour's build.
- 0.6.0: the sweep ran in CI (about four hours, no local load). The two lines are still
  to write in `docs/workflow.md`.

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

## FLOW-020 — Nobody counts why a pull request goes round again

- Kind: speed
- Observed: 24 rounds of 61 came after the first in 0.6.0, the same share as in 0.5.0,
  and the cause — a rebase after another merge, a fix after a review, a failed check —
  was counted by hand once and not at all this time. FLOW-003's verdict rests on it.
- Proposal: `scripts/flow-audit.py` sorts each later round by comparing its commit with
  the round before: same changed files (a rebase) or different ones (a fix), and whether
  the round before had a failed check.
- Costs: about two hours, with tests; one `gh` call per later round at audit time.
  Protects: the decision on a merge queue being taken on a figure.

## FLOW-021 — Two written rules were not followed, and nothing noticed

- Kind: quality + speed
- Observed: (1) the architecture lane goes with the backend or frontend lane every time
  (FLOW-008): launched 4 times for 24, and CI refused #124 for it. (2) The coverage
  harness runs in the foreground on this machine (a note of 2026-09-15): it was started
  in the background, stopped for lack of memory, and the last entry waited for the
  owner's word to run it again.
- Proposal: the rules move from prose to the place that acts — the `/next-todo` skill
  names the architecture lane in its reviewer step, and `just harness --coverage`
  refuses to start without a terminal when the machine has 8 GiB of memory or less.
- Costs: about an hour. Risk: a refusal in a legitimate background run, which the
  message must explain how to override. Protects: a CI round, and a stalled batch.

## Used and not used — the 0.6.0 batch

Counted from the session's transcript, 2026-10-03 → 2026-10-05: what the agent invoked
itself; "was" is the 0.5.0 batch. Hooks and CI ran their own share on every commit and
push; that is not counted.

| What                  | Used (times)                                                                                                                                                                                                                                  | Not used                                                               |
| --------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| Skills, invoked       | `/next-todo` 4, `/whats-next` 1, `/design-proposal` 1, `/flow-audit` 1; `/dep-audit` and `/visual-proof` followed without being invoked — was none                                                                                            | `/adr-writer`, `/prune`                                                |
| Agents, local         | reviewer-backend 16, reviewer-frontend 8, reviewer-infra 4, reviewer-arch 4, spec-reviewer 3, reviewer-security 2, contract-reviewer 2, reviewer-e2e 1 — was 9, 6, 4, 4, 3, 3, 1, 2                                                           | spec-checker, adr-reviewer, reviewer-sql                               |
| Recipes               | `format` 97, `check` 96, `test-rust` 70, `harness` 64, `merge` 55, `test` 55, `test-scripts` 27, `generate-types` 10, `check-full` 6, `arch-check` 6, `test-unit` 4, `stat` 1, `licence-check` 1 — was `merge` 47 and little else             | `coverage-gate`, `dev`, `dev-seed`, `install`, `worktree`, `next-todo` |
| Native commands       | `npx prettier` 76, `cargo test` 37, `ruff` 17, `cargo check` 4, `npx biome` 3, `cargo fmt` 3, `cargo clippy` 1 — was `cargo test` 71, `cargo fmt` 53, `npx vitest run` 45, `npx prettier` 44, `npx biome` 42, `npx tsc` 40, `cargo clippy` 35 | —                                                                      |
| Scripts, run directly | `whats-next.py` 9, `visual-proof-capture.mjs` 3, `flow-audit.py` 3                                                                                                                                                                            | every other script is called by a recipe, a hook, CI or a skill        |

Reading: the recipes replaced the native commands, as the 0.5.0 audit asked. What is left
is `npx prettier` on documents, which `just format` also does, and `cargo test` for the
loop that sorts one mutant. `spec-checker` is prescribed and was never launched.

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

## 2026-09-27 — TD-043 — The reviewer agents run without a turn cap and load the whole CLAUDE.md

- Found by: the main agent, comparing the agent files with the Claude Code sub-agent documentation
- Where: `.claude/agents/reviewer-*.md` (frontmatter), `.github/workflows/review.yml`
- Severity: 🔵
- Observation: The sub-agent format now offers `maxTurns` (stop a runaway review), `effort` (per agent) and `omitClaudeMd` (skip the project `CLAUDE.md` when the prompt is self-contained). None is set: every CI review loads `CLAUDE.md` seven times per pull request, and nothing bounds a review that loops.
- User value: None directly — cheaper, bounded reviews on every pull request.
- Done when: one reviewer runs with `omitClaudeMd: true` and a `maxTurns` cap on a sample diff and reports the same findings as without; if it does, the settings extend to every reviewer, with the review time and cost before and after recorded.

## 2026-10-03 — TD-064 — Sync, joining and leaving: 15 changes in the code that no test notices

- Found by: the mutation sweep of 2026-10-04 on `main` (run 37151658259: 218 missed). Twelve pull requests of 2026-10-04 sorted 165 of them (valuation, performance, account page, conflict rules, transaction recording, fee periods, update checker, lookup client, scheduled sweep, sync reading, price fetch task, price dates, small domain rules) and found two real bugs. What is left was split in three by the owner's decision of 2026-10-04, one part a version: this entry in 0.7.0, TD-076 in 0.8.0, TD-077 in 0.9.0.
- Where: `context/sync/application/join.rs` (4: a manifest or a segment in a newer format, the clock a joined device starts at, the area taken back out when the enrolment fails), `first_publish.rs` (3: the clock and the sequence of a first publish), `service.rs` (3: leaving sync), `publisher.rs` (2: the settling delay), `use_cases/portfolio_sync/orchestrator.rs` (3: an installation that holds user data, resuming)
- Severity: 🟡
- Observation: each needs a two-device scenario of its own (`tests/sync_two_devices.rs` has the fixture: `two_devices_sharing`). A manifest or a segment in a newer format is sealed, so the test must write one with the format's own encoder. To sort a change, apply it by hand and watch the test fail: a small loop (replace, `cargo test`, restore) takes 20 seconds a change on a unit test and a minute on a scenario.
- User value: None directly — a device that joins, leaves or resumes wrongly would be caught by a test.
- Done when: each of the 15 is a test added, dead code deleted, or code simplified so the change no longer exists; the sweep of 2026-10-04 on this entry's own commit (run 37222025189: 63 missed out of 1 634) confirms these 15.

## 2026-10-04 — TD-076 — Sync, what a run applies and reports: 20 changes in the code that no test notices

- Found by: the split of TD-064 (owner, 2026-10-04); this part is for 0.8.0
- Where: `context/sync/application/run.rs` (7: a header whose check does not match this device's key, the count of a notice's kind, the applied and refused counts of an intake, the device named in a notice), `apply.rs` (1), `intake.rs` (4: a segment in a newer format; three that re-read what is already applied, which the conflict rules ignore — nothing to observe), `use_cases/portfolio_sync/applier.rs` (4: a name clash on an account or a category, the currency of a cash line), `rank_stamper.rs` (3: the rows ranked at first publish), `snapshot.rs` (1)
- Severity: 🟡
- Observation: as TD-064. The three re-reads in `intake.rs` change nothing a user or a test can see; they are listed so nobody investigates them again — sort them by simplifying the comparison if it can be done without changing the read.
- User value: None directly — a wrong notice, or a change applied under the wrong rank, would be caught by a test.
- Done when: each of the 20 (counted by the sweep of 2026-10-04, run 37222025189) is a test added, dead code deleted, code simplified, or recorded here as having nothing to observe with the reason.

## 2026-10-04 — TD-077 — Account service, transaction checks, backfills and the headless fetch: 28 changes in the code that no test notices

- Found by: the split of TD-064 (owner, 2026-10-04); this part is for 0.9.0
- Where: `context/account/service.rs` (5: the bounds of a fee deduction and of an interest credit, a management fee given as a rate alone, the fee schedules of an account, the event after a schedule changes), `use_cases/holding_transaction/orchestrator.rs` (5: a sale of exactly the quantity held, a split whose ratio holds a zero), `context/asset/domain/yahoo_symbol.rs` (1), `context/account/domain/journal.rs` (3: the balance held at the largest amount), `context/currency/application/service.rs` (1), `use_cases/price_history_backfill/orchestrator.rs` (2: the week that must hold a trading day), `use_cases/scheduled_fetch/headless.rs` (3), `use_cases/account_details/orchestrator.rs` (3: a fee dated on the as-of day, the as-of loop's skip of cash lines — nothing to observe —, the window-start dates given to the rate lookup), `use_cases/fee_generation/orchestrator.rs` (1: a schedule ending on a period's last day), `use_cases/update_checker/service.rs` (3: the download and the installation themselves, which need the running application), the last one in `use_cases/shared/valuation.rs` (the date a holding closed)
- Severity: 🔵
- Observation: mostly unit tests on existing fixtures. The three of the update checker cannot be reached without the running application: record them as such, or move the download behind a seam a test can drive.
- User value: None directly.
- Done when: each of the 28 (counted by the sweep of 2026-10-04, run 37222025189) is a test added, dead code deleted, code simplified, or recorded here as out of a test's reach with the reason; issue 65 is closed.
