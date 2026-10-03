# Flow

Observations about how work moves — the harness, the checks, the waits, the human's
part — and what to change. `todo.md` says what the application owes its user,
`techdebt.md` what the code owes itself; this file says what the workflow owes the two
people who run it.

The rules for an entry, set by the owner on 2026-10-03:

1. Quality and speed are weighed together: a change that buys speed says what it risks,
   a change that buys safety says what it costs.
2. Where no change is needed, none is proposed. "Keep" is a verdict.
3. Human review is a last resort and never forgotten: when only the owner can judge, the
   questions come in one block for the batch, with what to look at and what a yes means.

Each entry carries a permanent `FLOW-NNN` reference (never renumbered, never reused; next
free: FLOW-014) so the owner can queue it in `docs/todo.md` § Next like any other. An
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

## FLOW-001 — A filtered test output hid a failure three times

- Kind: quality
- Observed: `just merge | tail` ran the next step after a refused merge; `vitest | tail`
  hid a test file that could not load; `vitest | grep "Test Files"` hid four unhandled
  errors that failed CI on #91 with every test passing. Each time the count looked right.
- Proposal: the agent gates on the exit code of a recipe run bare (`just test-unit`,
  `just harness`) and reads a saved log afterwards. One line in `docs/workflow.md` § 5,
  and the harness prints its own one-line verdict so nothing needs filtering.
- Costs: nothing. Protects: a CI round (12 min) per miss, and a wrong "all green" said
  to the owner.

## FLOW-002 — The CI reviewers stop when the session's usage window is spent

- Kind: quality + speed
- Observed: on #91 the six reviewer checks went red with "You've hit your session limit";
  they share the interactive session's window. A red reviewer check then reads like a
  finding. One round lost, and the push had to wait for the reset.
- Proposal: the reviewer job tells "did not run" from "found a critical" — a neutral
  conclusion with the reset time in the sticky comment — and `just merge` refuses a
  neutral reviewer with that message. Before a first push late in a long session, the
  agent says so and holds the push when the limit is near.
- Costs: an hour on `review.yml` and `scripts/merge.py`. Protects: a false red, and a
  merge that would otherwise wait on an unexplained failure.

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
  the visual reference goes stale when the month changes (TD-060, open); the release
  workflow's Windows steps run only at a release; the golden portfolio had never been
  read through the live account page (fixed in #87).
- Proposal: a weekly scheduled run of what otherwise runs only at a release — the
  Windows build without publishing — and TD-060 queued before the next month change.
- Costs: about 20 runner-minutes a week. Protects: a release day spent on a workflow
  bug.

## FLOW-012 — Editing a pull request's text restarts its checks

- Kind: speed
- Observed: `quality.yml` runs on `edited`, so recording the triage table in the body
  after a push restarted every job (#77, #82), and a fixup aimed at a commit already
  merged forced a new pull request (#77 → #78).
- Proposal: only the `pr-checks` job, which reads the title and the body, runs on
  `edited`; the build and test jobs run on new commits. The body is written with the
  push. `just merge` already refuses a fixup that names no commit of the branch.
- Costs: a condition on four jobs. Protects: a 12-minute round per edit.

## FLOW-013 — `just merge` cannot run from a worktree

- Kind: speed
- Observed: docs work done in a worktree while the release ran on `main` (the rule for
  a release hand-over) could not be merged from there: `scripts/merge.py` checks out
  `main`, which the main folder holds. The worktree had to be removed and the branch
  checked out in the main folder.
- Proposal: `scripts/merge.py` merges without checking out `main` when it runs in a
  worktree (it pushes the rebased branch to `main` and lets the main folder pull), or
  says in its refusal what to do.
- Costs: an hour on `scripts/merge.py` and its tests. Protects: the worktree rule being
  usable to its end.

---

## Entries elsewhere that are about the flow

Candidates to move here, for the owner to decide; none is moved yet.

| Entry                                  | What it is about                                     |
| -------------------------------------- | ---------------------------------------------------- |
| #049                                   | how references are written (TODO-NNN, DEBT-NNN)      |
| #014                                   | a second device in the E2E suite                     |
| TD-015, TD-016, TD-019, TD-023, TD-039 | E2E specs that are fragile by construction           |
| TD-017                                 | backend coverage one point under its target          |
| TD-021                                 | a cache pin labelled with the wrong tag in workflows |
| TD-037                                 | a capture that carries a random path                 |
| TD-043                                 | reviewer agents without a turn cap                   |
| TD-060                                 | the capture that changes with the month              |
| TD-064                                 | the mutation sweep outside the account               |

`#016` and `#017` (accepted advisories) are about what ships, not how work moves: they
stay in `todo.md`.
