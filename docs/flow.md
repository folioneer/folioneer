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
free: FLOW-033) so the owner can queue it in `docs/todo.md` § Next like any other. An
entry is removed once it is settled.

---

## Measured — the 0.7.1 batch (2026-10-10)

Counted by `scripts/flow-audit.py v0.7.0 v0.7.1 --previous v0.6.0`; "was" is the 0.7.0
batch counted the same way. The batch ran in three and a half hours, the day 0.7.0 was
released. What the script cannot count comes from the session that ran #160 to #172; #156
to #159 were merged before it and are in the script's figures only.

| What                                                       | Figure                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ---------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Pull requests merged                                       | 17 (#156–#172), 2 207 lines added — was 23, 12 961                                                                                                                                                                                                                                                                                                                                                                                              |
| By type                                                    | docs 9, chore 4, fix 2, ci 1, refactor 1 — was docs 9, feat 5, fix 4, chore 3, ci 2                                                                                                                                                                                                                                                                                                                                                             |
| Time from opening a pull request to merging                | median 5.3 min, mean 4.5 — was 7.1 and 30.4                                                                                                                                                                                                                                                                                                                                                                                                     |
| CI rounds                                                  | 18 for 17 branches, 1 beyond the first — was 48 for 24, 24 beyond                                                                                                                                                                                                                                                                                                                                                                               |
| What caused the extra round                                | not counted: it is not among #160–#172, each of which took one round — was not counted                                                                                                                                                                                                                                                                                                                                                          |
| Workflow failures on pull requests                         | 0 — was 14: Review 13, Quality 1                                                                                                                                                                                                                                                                                                                                                                                                                |
| Workflow failures on `main`                                | 1: E2E on the push of #171, the assets spec's before-each hook (DEBT-019); not re-run; the release commit after it was green — was not counted                                                                                                                                                                                                                                                                                                  |
| E2E run / Quality run / Review run (median, green)         | 2.1 / 2.3 / 0.6 min — was 4.8 / 3.6 / 1.1 (13 of the 17 pull requests moved no code, and a records-only one skips the E2E suite)                                                                                                                                                                                                                                                                                                                |
| Questions put to the owner                                 | 5: 3 in the opening block (one asked, two given with the go), 0 during the batch, 2 before queueing DEBT-086 — was 17: 7 and 10                                                                                                                                                                                                                                                                                                                 |
| Real defects found by reviewers before merge               | 6, all in the batch's own new code (a missing gap list read as an empty one; a branch without a pull request watched as an outage for two and a half minutes; a watch ending on a traceback with the exit code of a failed check; a session log nested too deep raising past "always one line"; an E2E step reading the folder before it was published; a rule amended to "at most 100%" against the rule that relies on less) — was at least 4 |
| Real defects found by the exhaustive reading of a contract | 8 rows of the account contract whose code returns other codes than the rules ask for (DEBT-098), two read again by the main agent — was 22 gaps of the document (DEBT-088)                                                                                                                                                                                                                                                                      |
| Real defects found by the spec checks before a release     | 3 (starting over in an empty folder said the folder held a portfolio, from the batch's own TODO-066, fixed in #171 before the release; a resumed fee schedule charges the pause, FEE-061; a recurring removal is floored twice, FEE-041 — the last two the checker's reading, DEBT-100) — was 4                                                                                                                                                 |
| Real defects found by challenging or closing an entry      | 2 (FLOW-024: the bindings give a command's error type, not the codes it returns, so the script holds half of what the entry asked; FLOW-025: the seven lane reviewers already judged the changed lines, only the two document reviewers did not) — was 1                                                                                                                                                                                        |
| Commands of the core named in no contract                  | 0 of 93, `log_frontend` set aside by the check; 4 gaps left in `contract-gaps.json`, was 37 at its first run — was 3 of 93                                                                                                                                                                                                                                                                                                                      |
| Mutation sweep on `main`                                   | not run in this batch — was not run                                                                                                                                                                                                                                                                                                                                                                                                             |

Reading: one round in eighteen came after the first where it was one in two, and no
workflow failed on a pull request where fourteen had: each entry was cut from the branch
in review and pushed once the one below had merged, and the reviewers' lanes no longer
refuse what a pull request did not change. No question reached the owner during the
batch. The one red run was on `main`, on a known intermittent failure, while the owner
had been told `main` was ready (FLOW-031).

---

## FLOW-003 — Every merge sends the other open pull requests round again

- Kind: speed
- Observed: in 0.5.0 about 10 of the rounds re-tested a pull request whose own files had
  not changed, because another had merged first. In 0.6.0, 24 rounds of 61 came after the
  first (39 %; was 18 of 46, 39 %), and what caused them was not counted. `just merge`
  already waives the round when the rebase changes only record files.
  In 0.7.0, 24 rounds of 48 (50 %), the causes not counted again: FLOW-020 was not queued.
  Two of them are known, #153 and #155 on the release day, each sent round by the other's
  merge.
  In 0.7.1, 1 round of 18 (6 %): the entries were stacked (`docs/workflow.md` § 11) and
  no pull request was sent round by another's merge.
- Verdict: **keep, and count the causes (FLOW-020).** A round costs about 5 min where it
  cost 25, and the median pull request merges in 7 min. A merge queue would cost a
  rewrite of `just merge` for a saving that is small. The share rose on the reviewers'
  failures, not on the merges.

## FLOW-006 — An entry whose Done-when is an audit cannot close

- Kind: speed
- Observed: DEBT-065 ("every figure listed with where it is computed") took four pull
  requests and is still open: the audit kept finding work. TODO-055 promised a refusal that
  an existing rule (AST-009) forbids, found only when the owner read the result.
- Proposal: the challenge step, already run before starting a TD or an issue, also (a)
  splits an entry whose Done-when is a survey into "list them" and one entry per thing
  found, and (b) reads the entry against the spec rules of its domain before coding.
- Costs: minutes per entry. Protects: a pull request written to a promise the rules
  refuse.
- 0.7.0: TODO-059 held seven pieces of work and was split into seven entries when the
  owner read it, after the batch; the challenge step had not split it.

## FLOW-008 — The same reviewers run locally and in CI

- Kind: quality + speed
- Observed: each lane costs about a minute locally and about two in CI. They do not find
  the same things. In 0.6.0 the architecture lane was launched locally 4 times for 24
  backend and frontend launches, and CI's architecture lane then refused #124 (a
  conversion in the command layer): one round. The same skip cost a round on #77 in
  0.5.0.
- Verdict: **keep both.** The local run is what makes the first CI round green; CI is
  the one that cannot be skipped. The rule — the architecture lane goes with the backend
  or frontend lane every time — was not followed twice; the `/next-todo` skill now names
  the lane in its reviewer step. In 0.7.0 it was launched 9 times for 13 backend and
  frontend launches and CI's architecture lane refused nothing the local one had passed.
  In 0.7.1 it went with each of the 2 frontend launches; no backend code moved.

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
- 0.7.0: no sweep. The same machine carries the backend coverage run only in the
  foreground (405 s, two build jobs); the owner asked on 2026-10-10 whether another
  computer could do the work.

## FLOW-011 — Parts of the harness run only on the day they matter

- Kind: quality
- Observed: CI floated on the latest Rust until a new lint broke `main` (DEBT-059, fixed);
  the visual reference went stale when the month changed (DEBT-060, fixed); the release
  workflow's Windows steps run only at a release; the golden portfolio had never been
  read through the live account page (fixed in #87).
- Proposal: a weekly scheduled run of what otherwise runs only at a release — the
  Windows build without publishing.
- Costs: about 20 runner-minutes a week. Protects: a release day spent on a workflow
  bug.
- 0.7.0: the Windows-only code of the agent connection was compiled for the first time
  by the release build, by the owner's choice; it passed. DEBT-086 asks for a Windows
  compile check on pull requests, which would cover this proposal's case.
- 0.7.1: DEBT-086 is queued, with a Windows job on the pull requests that touch Rust
  (owner, 2026-10-10). Once it is merged, what is left of this proposal is the release
  workflow's own steps.

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
- 0.7.0: not queued, and the causes were not counted a second time.
- 0.7.1: one later round in eighteen. While the entries are stacked there is little to
  sort; the figure argues for waiting until the share rises again.

## FLOW-030 — Two skills are followed by hand, never invoked

- Kind: quality
- Observed: `docs/workflow.md` § 3 says that in a batch "the agent runs `/next-todo` for
  each" entry. In 0.7.0 it was invoked for 1 entry of 10; in 0.7.1 for none of 10, and
  `/visual-proof` for none of 2: the agent read each skill once and followed it from
  memory. Nothing visible was lost in 0.7.1 (one round in eighteen came after the first),
  but a step of a skill that changes during a batch is not re-read: `/next-todo` Step 8
  changed in #165 and the agent went on running the script it names directly.
- Proposal: the rule says what is done and is checkable: a skill is loaded at the first
  entry of a batch and again after any pull request of the batch that changes it;
  `/flow-audit` counts the loads beside the entries.
- Costs: one sentence in `docs/workflow.md` § 3, one line in the audit. Protects: a batch
  that follows a step it rewrote an hour before.

## FLOW-031 — "`main` is ready" is said before the checks of `main` have ended

- Kind: quality
- Observed: in 0.7.1 the agent told the owner that `main` was ready for the release as
  soon as pull request 171 merged. The E2E run of that push then failed (DEBT-019, an
  intermittent failure of the assets spec), and the owner had already started the
  release. `just release` validates locally and does not look at the runs of the head
  commit; the tag landed on the next commit, whose own runs were green.
- Proposal: `scripts/release.py` refuses to tag while a workflow run of the head commit
  is unfinished or failed, naming the run; the agent's hand-over line names the runs it
  saw end green. `just watch-pr` already knows how to wait for runs.
- Costs: about an hour with tests; a release waits for the runs of the last merge, at
  most a quarter of an hour. Protects: a tag on a commit whose checks nobody saw end.

## FLOW-032 — What `/prune` finds is written where git does not look

- Kind: quality
- Observed: `/prune` runs once after a release (`docs/workflow.md` § 3) and saves its
  report under `tmp/`, which git ignores: a finding nobody applies in the same session is
  lost. It was not run after 0.6.0 or 0.7.0. After 0.7.1 it ran on the three production
  files the release changed and found one mechanical simplification; the owner asked on
  2026-10-10 whether such findings are kept anywhere, and they were not (DEBT-101 holds
  this one, filed by hand).
- Proposal: `docs/workflow.md` § 3 and the skill say it: what `/prune` recommends is filed
  as one tech-debt entry in the hand-over, like what the spec checks find; an audit with no
  finding files nothing. The skill's scope after a release is the production files the
  release changed, which is what was run; a full scan is asked for by name.
- Costs: two sentences and one entry per release at most. Protects: a simplification that
  was found, proven covered, and then forgotten.

## Used and not used — the 0.7.1 batch

Counted from the session's transcript for #160 to #172: what the agent invoked itself;
"was" is the 0.7.0 batch. A figure is given where it was counted. Hooks and CI ran their
own share on every commit and push; that is not counted.

| What                  | Used (times)                                                                                                                                                                                                                                                                                                                                                                                 | Not used                                                                                                                        |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Skills, invoked       | `/whats-next` 2, `/dep-audit` 1, `/flow-audit` 1; `/next-todo` for the ten entries and `/visual-proof` twice followed without being invoked; `/contract`, written in the batch, not used for the contract the batch rewrote — was `/next-todo` 1, `/design-proposal` 1, `/flow-audit` 1                                                                                                      | `/design-proposal`, `/adr-writer`; `/prune` runs after this audit                                                               |
| Agents, local         | reviewer-infra 3, spec-reviewer 3, reviewer-frontend 2, reviewer-arch 2, spec-checker 2, reviewer-e2e 1, contract-reviewer 1 — was 2, 10, 6, 9, 10, 2, 5                                                                                                                                                                                                                                     | reviewer-backend, reviewer-sql, reviewer-security, adr-reviewer: no Rust, migration or ADR moved                                |
| Recipes               | `merge` 13 (one more refused: `main` was checked out in another worktree), `watch-pr` 5 once it existed, `worktree` 1, `harness`, `format`, `test`, `check`, `licence-check` not counted — was `format` 127, `harness` 49, `merge` 29                                                                                                                                                        | `test-rust`, `test-scripts`, `check-full`, `coverage-gate`, `generate-types`, `dev`, `dev-seed`, `install`, `stat`, `next-todo` |
| Native commands       | `python3 -m unittest` about 6 where `just test-scripts` exists; `npx vite` 2 (the preview server of the visual proof); `npx tsc` 1; `git worktree add` 1 for a branch that existed (the recipe only creates one); for the dependency audit, as its skill writes them: `npm audit` 3, `cargo audit` 3, `cargo outdated` 1, `npm outdated` 1 — was `npx tsc` 32, `npx vite` 14, `cargo test` 7 | —                                                                                                                               |
| Scripts, run directly | `watch-pr.py` 6 where the recipe exists, `contract-check.py` about 10 (no recipe: the harness calls it), `whats-next.py` 6, `visual-proof-capture.mjs` 2, `batch-specs.sh` 1, `flow-audit.py` 1 — was `whats-next.py` 31, `visual-proof-capture.mjs` 17, `reference-forms.py` 17                                                                                                             | every other script is called by a recipe, a hook, CI or a skill                                                                 |

Reading: `/next-todo` and `/visual-proof` were followed by hand for a second batch
(FLOW-030). Three hand-written polling loops watched the first three pull requests, none
after `just watch-pr` merged, though the script was then run directly six times. `npx tsc`
went from 32 to 1.

## Moved here from the todo and the tech debt

Entries that are about how work moves, not about the application. Each keeps the
reference it was filed under and is queued by it.

## TODO-014 — (e2e) — Drive a second device in the E2E suite

The multi-device sync E2E covers the single-device critical path only (plan § Halt Artifact H1): `wdio.conf.ts` launches one binary with one `VAULT_COMPASS_E2E_DATA_DIR` and `maxInstances: 1`, so joining a folder another device created (SYN-014/036) is proven by the two-database integration test `src-tauri/tests/sync_two_devices.rs`, not through the UI. A real two-device E2E needs an `e2e/helpers/second_device.ts` that launches a second binary against its own data directory plus a wdio multi-remote configuration — a separate, pre-requisite task before any join scenario is written.

**User value:** None directly — test infrastructure.
**Done when:** A wdio multi-remote config and `e2e/helpers/second_device.ts` launch a second binary on its own data directory, and a join scenario (SYN-014/036) passes through the UI.
**Design:** none
**Open questions:** none

## 2026-09-12 — DEBT-015 — Three E2E specs select by text or duplicate a shared helper

- Found by: reviewer-e2e (`.review/reviewer-e2e-2026-09-12-01.md`, pre-existing section)
- Where: e2e/accounts/accounts.test.ts:23 (local `navigateToAccounts` next to the shared one in e2e/helpers/navigation.ts), e2e/asset_web_lookup/asset_web_lookup.test.ts:47 (`button[aria-label="Fill manually"]`), e2e/assets/assets.test.ts:79 and :102 (XPath on `normalize-space(text())`)
- Severity: 🔵
- Observation: Two specs locate elements by their English label or cell text rather than a stable id (E4), which ties them to the forced `en_US` locale and to copy that the i18n files own; one spec carries its own copy of a navigation helper the shared module already provides, so a change to the accounts route has two places to drift.
- User value: None — suite robustness.
- Done when: the three sites select by `id` (adding the ids on the frontend elements in the same commit) and the local helper is replaced by the shared import.

## 2026-09-12 — DEBT-016 — A controlled-input value can be lost once in the E2E buy flow

- Found by: manual (first pull-request E2E run, attempt 1)
- Where: e2e/account_details/buy_sell.test.ts (TRX-010), e2e/helpers/react.ts (`setReactInputValue`), src/ui/components/field/CalcField.tsx
- Severity: 🟡
- Observation: TRX-010 failed with `submit still not enabled after 5000ms`; the failure screenshot shows the date and the unit price filled and the quantity field empty, so the value set by `setReactInputValue("buy-trx-quantity", "10")` between the two others did not stick. The same spec passed six times on `main` the same day and the field's own state sync guards against prop clobbering, so no deterministic path is known. With E2E as a required check, a once-in-N loss of a set value is a merge blocked for a reason unrelated to the change.
- User value: None — suite reliability.
- Done when: the loss is reproduced (or its trigger understood) and either the helper waits for the field to report the value back before returning, or the field's handling is changed so a dispatched `input` event can never be dropped; TRX-010 no longer needs a re-run to pass.

## 2026-09-12 — DEBT-017 — Backend logic coverage sits at 89 % against the 90 % target

- Found by: manual (`python3 scripts/coverage-gate.py --backend`; figures refreshed 2026-09-19)
- Where: src-tauri/src/use_cases/update_checker/service.rs (0 %), src-tauri/src/use_cases/scheduled_fetch/headless.rs (5 %), src-tauri/src/use_cases/portfolio_sync/applier.rs (60 %), src-tauri/src/use_cases/asset_web_lookup/orchestrator.rs (62 %), src-tauri/src/context/sync/application/join.rs (74 %), src-tauri/src/use_cases/holding_transaction/orchestrator.rs (77 %), src-tauri/src/context/asset/service.rs (86 %), src-tauri/src/context/account/service.rs (87 %)
- Severity: 🟡
- Observation: 89.06 % of the 11,680 lines in domain, application, service and use-case code are covered; the gate's floor is 85.5 % and its target 90 %, about 110 more covered lines. Two files carry almost no test at all because they talk to the network or run the app headless; the other six are orchestration paths with untested branches. The floor in `coverage-gates.json` is a ratchet — raise it in the same change that lifts coverage, never lower it.
- User value: None — a harness that catches logic regressions in these paths.
- Mutation survivors: the 2026-09-14 sweep (issue #137) found 301 logic changes no test notices — `context/account/domain/account.rs` 52, `use_cases/shared/valuation.rs` 33, `context/sync/domain/resolution.rs` 24, `use_cases/global_performance/orchestrator.rs` 21; each names an assertion that is missing or too weak.
- Done when: the backend floor in `coverage-gates.json` reads 90.0 and the gate passes on `main`.

## 2026-09-13 — DEBT-019 — The assets spec's before-each hook can hit a stale element

- Found by: manual (an E2E run, attempt 1, on a pull request that touched no app code)
- Seen again: a `main` push run, attempt 1 (2026-09-15, after a change that touched no E2E or assets code) — same `before each` hook, same stale node handle on an `element` call; attempt 2 green.
- Seen again: PR #18, a records-only pull request, attempt 1 (2026-09-27); attempt 2 green.
- Seen again: the `main` push of pull request 171 (2026-10-10, run 38043257182), a change to the sync dialog; the same tree was green on the pull request, and the release commit after it was green. Not re-run, and not skipped either: the hook guards every scenario of the assets spec, so skipping it is skipping the spec.
- Where: e2e/assets/assets.test.ts (`beforeEach`), e2e/helpers/modal.ts (`dismissLeftoverModal`), e2e/helpers/navigation.ts (`navigateToAssets`)
- Severity: 🟡
- Observation: The hook failed with `stale element reference` while creating a node handle for an `element` call — an element located by one step had been replaced by a re-render before the next step used it. It is the second distinct once-only E2E failure in two days (DEBT-016 is the first); both sit in setup or navigation code shared by many specs, so each has many chances to fire per run. With E2E as a required check, every such failure costs a re-run before a green PR can merge.
- User value: None — suite reliability.
- Cause not found yet (2026-09-27): the F29 fix for DEBT-039 does not reach this hook — the assets spec's navigation and modal helpers depend on none of the hooks it changed. Next step: capture which element the `element` call was locating when it went stale (the failure screenshot and the hook's last command), then fix the component that replaces it.
- Done when: the hook re-locates elements after each navigation step instead of reusing handles across renders, or the shared helpers wait for the route to settle before returning; a month of pull-request runs shows no before-each failure.

## 2026-09-13 — DEBT-021 — The rust-cache pin is labelled with the wrong tag in three workflows

- Found by: reviewer-infra (phase 12 review, `.review/reviewer-infra-2026-09-13-10.md`)
- Where: `.github/workflows/quality.yml`, `e2e.yml`, `release.yml` (twice) — `Swatinem/rust-cache@e18b4977…` labelled v2.9.1, while that tag peels to `c1937114…`; `security-audit.yml` pins `taiki-e/install-action@f48d2f8b…` with no version label at all
- Severity: 🔵
- Observation: the commits are real upstream commits, so nothing is compromised, but a reader trusting the comment audits the wrong release notes. `mutants.yml` carries the correct pins, and the `install-action` labels were corrected to v2.79.6 since; the rust-cache label is what remains. Tag verified with `git ls-remote --tags` on 2026-09-13, file state on 2026-09-19.
- User value: None — whoever audits a pinned action reads the release notes of the version actually running.
- Done when: every pinned action in `.github/workflows/` carries the label of the tag its commit belongs to, or the pin moves to the commit of the labelled tag.

## 2026-09-14 — DEBT-023 — E2E specs still locate controls by label, role or form attribute

- Found by: manual (selector count while fixing dangling references in the E2E headers)
- Where: `e2e/asset_web_lookup/asset_web_lookup.test.ts` (`button[aria-label="Add asset"]`, `"Back"`, `"Fill manually"`), `e2e/account_details/manual_price_fill.test.ts` and `e2e/account_details/auto_fetch.test.ts` (`[role="dialog"]`, `[role="status"]`, `body`), `e2e/open_balance/open_balance.test.ts` and `e2e/account_details/buy_sell.test.ts` (`button[type="submit"][form="…"]`)
- Severity: 🔵
- Observation: `docs/e2e-rules.md` asks every selector to be a stable `id`; these specs still find controls by an accessible label (which changes with the locale and the wording), by role, or by the form a submit button belongs to, because the elements carry no id of their own.
- User value: None — E2E specs that survive a wording or locale change.
- Done when: every selector in those five specs is an `id`, the controls they target carry one, and `reviewer-e2e` passes on them.

## 2026-09-21 — DEBT-037 — The settings capture still carries a random folder path

- Found by: the main agent (PR #8, a Markdown-only pull request, visual gate red)
- Where: `e2e/sync/sync.test.ts:114` (`mkdtempSync(join(tmpdir(), "folioneer-sync-"))`), captured in `sync-settings-{light,dark}`
- Severity: 🔵
- Observation: the sync section renders the shared folder, whose `mkdtemp` suffix differs every run, so those pixels (columns 611–651, 0.106 % of the screen) always differ between two runs of the same commit. Below the workflow's 0.3 % threshold on its own, so nothing fails today; it was a third of the budget when the fading scrollbar took the rest and pushed the total to 0.327 %. It leaves the gate that much closer to a false regression on any screen that shares it.
- User value: None — the merge gate's headroom.
- Done when: `sync-settings` is byte-identical across two runs of the same commit; the E2E sync folder carries a fixed name (the suite runs one instance, `maxInstances: 1`) or the value is not rendered into the capture.

## 2026-09-27 — DEBT-043 — The reviewer agents run without a turn cap and load the whole CLAUDE.md

- Found by: the main agent, comparing the agent files with the Claude Code sub-agent documentation
- Where: `.claude/agents/reviewer-*.md` (frontmatter), `.github/workflows/review.yml`
- Severity: 🔵
- Observation: The sub-agent format now offers `maxTurns` (stop a runaway review), `effort` (per agent) and `omitClaudeMd` (skip the project `CLAUDE.md` when the prompt is self-contained). None is set: every CI review loads `CLAUDE.md` seven times per pull request, and nothing bounds a review that loops.
- User value: None directly — cheaper, bounded reviews on every pull request.
- Done when: one reviewer runs with `omitClaudeMd: true` and a `maxTurns` cap on a sample diff and reports the same findings as without; if it does, the settings extend to every reviewer, with the review time and cost before and after recorded.

## 2026-10-04 — DEBT-076 — Sync, what a run applies and reports: 20 changes in the code that no test notices

- Found by: the mutation sweep of 2026-10-04 (run 37222025189: 63 missed out of 1 634), split in three by the owner on 2026-10-04; the first part (joining and leaving, 15) is done, this one is for 0.8.0
- Where: `context/sync/application/run.rs` (7: a header whose check does not match this device's key, the count of a notice's kind, the applied and refused counts of an intake, the device named in a notice), `apply.rs` (1), `intake.rs` (4: a segment in a newer format; three that re-read what is already applied, which the conflict rules ignore — nothing to observe), `use_cases/portfolio_sync/applier.rs` (4: a name clash on an account or a category, the currency of a cash line), `rank_stamper.rs` (3: the rows ranked at first publish), `snapshot.rs` (1)
- Severity: 🟡
- Observation: most need a two-device scenario of their own (`tests/sync_two_devices.rs` has the fixtures: `two_devices_sharing`, `break_segment_writes`); a file in a newer format is sealed, so a test writes one with the format's own encoder (see the tests of `join.rs`). To sort a change, apply it by hand and watch the test fail: a small loop (replace, `cargo test`, restore, then `touch` the restored file — cargo trusts the modification time, and a file put back with its old one leaves the changed build in place) takes 20 seconds a change on a unit test and a minute on a scenario. The three re-reads in `intake.rs` change nothing a user or a test can see; they are listed so nobody investigates them again — sort them by simplifying the comparison if it can be done without changing the read.
- User value: None directly — a wrong notice, or a change applied under the wrong rank, would be caught by a test.
- Done when: each of the 20 (counted by the sweep of 2026-10-04, run 37222025189) is a test added, dead code deleted, code simplified, or recorded here as having nothing to observe with the reason.

## 2026-10-04 — DEBT-077 — Account service, transaction checks, backfills and the headless fetch: 28 changes in the code that no test notices

- Found by: the mutation sweep of 2026-10-04 (run 37222025189), split in three by the owner on 2026-10-04; this part is for 0.9.0
- Where: `context/account/service.rs` (5: the bounds of a fee deduction and of an interest credit, a management fee given as a rate alone, the fee schedules of an account, the event after a schedule changes), `use_cases/holding_transaction/orchestrator.rs` (5: a sale of exactly the quantity held, a split whose ratio holds a zero), `context/asset/domain/yahoo_symbol.rs` (1), `context/account/domain/journal.rs` (3: the balance held at the largest amount), `context/currency/application/service.rs` (1), `use_cases/price_history_backfill/orchestrator.rs` (2: the week that must hold a trading day), `use_cases/scheduled_fetch/headless.rs` (3), `use_cases/account_details/orchestrator.rs` (3: a fee dated on the as-of day, the as-of loop's skip of cash lines — nothing to observe —, the window-start dates given to the rate lookup), `use_cases/fee_generation/orchestrator.rs` (1: a schedule ending on a period's last day), `use_cases/update_checker/service.rs` (3: the download and the installation themselves, which need the running application), the last one in `use_cases/shared/valuation.rs` (the date a holding closed)
- Severity: 🔵
- Observation: mostly unit tests on existing fixtures. The three of the update checker cannot be reached without the running application: record them as such, or move the download behind a seam a test can drive.
- User value: None directly.
- Done when: each of the 28 (counted by the sweep of 2026-10-04, run 37222025189) is a test added, dead code deleted, code simplified, or recorded here as out of a test's reach with the reason; issue 65 is closed.
