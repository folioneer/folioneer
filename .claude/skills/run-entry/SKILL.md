---
name: run-entry
description: Runs one task end to end under docs/workflow.md — branch, design gate, acceptance tests, implementation, harness, reviewers, PR, merge on green, closure. The task is the first ready Queued item of the Project (headless, no argument), an issue the human names (#123), or a plain request typed in chat. One task per invocation.
argument-hint: "[#123 | a request]"
---

# Skill — `run-entry`

One invocation, one task, through the ceremony to `main`. The rules are `docs/workflow.md`;
this file is the checklist.

## Step 0 — Preconditions and mode

- `git status --short` is empty and `main` is fresh (`git pull --ff-only`). Otherwise
  stop and report; never work on a dirty tree.
- **Headless** (no argument): `gh project item-list 1 --owner folioneer --format json --limit 500`;
  take the first Queued item, in row order, that is ready. Nobody to ask: a
  question becomes a comment on the issue with `needs-owner` (`docs/workflow.md` § 2).
- **Chat, named issue** (`#123`): `gh issue view 123 --comments`.
- **Chat, plain request**: the request is the task; no issue is created — the PR body
  is its record. Write its Done when into the opening brief.
- **Decision scan**, for an issue (a pull request has none), before the branch exists:
  run it as `docs/workflow.md` § 3 states it. Chat: the questions it leaves are asked
  then, and the answers written on the issue. Headless: each becomes a comment with
  `needs-owner`, and the issue is no longer ready.
- **Ready** as `docs/workflow.md` § 2 defines it; a flow issue's Decision and a task's
  Observation are their Done when. Headless: skip what is not ready; if nothing is, say
  what blocks each issue and stop. Chat: ask each open question alone, with the context
  to answer it cold, and wait for the answer before the next; write the answers on the
  issue. For an issue with a spec: spec draft, `spec-reviewer`, then the questions.

## Step 1 — Opening brief, branch, task list

- Opening brief (chat: first message; headless: top of the PR body):
  **Task** — the issue or the request in one line · **Scope** — commit type and layers ·
  **Design** — none / validated / needed · **Touching** — the paths.
- `git checkout -b <type>/123-<slug>` (`<type>/<slug>` for a plain request), `<type>`
  being the commit type the change will carry; the item's Status becomes In progress
  (`gh project item-edit`; the field and option ids from `gh project field-list 1
--owner folioneer --format json`).
- `TaskCreate` one task per step below, each `in_progress` / `completed` (none under `/run-batch`).

## Step 2 — Design gate

If the task changes anything the user sees and `Design` is not `validated`: run
`/design-proposal <id>`.

- Chat: point the human to the mocks posted on the issue and ask. Nothing is built before a yes; on a yes, set the issue's
  Design to `validated`. On a no, write what the human wants on the issue and stop.
- Headless: post the proposal on the issue (`docs/workflow.md` § 4), then back to Step 0
  for the next ready item. Do not implement.

## Step 3 — Acceptance first

- A new or changed business rule goes into its spec, a changed command into its contract,
  with their reviewer, before the code (their folder's `README.md`).
- Every Done-when clause becomes a failing test (`docs/workflow.md` § 3, step 3).
- A fault met in production: ask for the input that shows it before designing a fix,
  and reproduce it in a failing test first (`docs/workflow.md` § 3).
- Run the suites; confirm the new tests are red. A clause the agent cannot run becomes
  a test of the harness — an E2E scenario when it needs the real window — or a `task`
  issue that asks for it (`docs/workflow.md` § 1).

## Step 4 — Implement

Smallest change to green, the right way (`docs/workflow.md` § 6). Never the live portfolio
database (`CLAUDE.md` § Core rules 1).

## Step 5 — Harness

`just harness` until green, `just harness --coverage` before the first push: a coverage floor met with tests, an architecture violation fixed
in code — never `coverage-gates.json` lowered nor `arch-allowlist.json` raised.

## Step 6 — Reviewers

Exactly the lanes `bash scripts/branch.sh files | bash scripts/review-lanes.sh` prints, in
one call, each given the branch, the spec and the vocabulary; every finding graded
(`docs/workflow.md` § 7); again on the fixes until no 🔴 remains.

## Step 7 — Evidence and commit

`/visual-proof` for every changed `.tsx` / `.css`. One commit (`docs/commit-rules.md`).

## Step 8 — Pull request and merge

- Push, `gh pr create --body-file`: the body of `docs/workflow.md` § 3, step 8.
- `just watch-pr` in the background: exit 0 is green; `scripts/watch-pr.py` lists the others.
- A red reviewer lane: read its newest comment; grade, apply and re-read locally as
  `docs/workflow.md` § 7 says; push.
- A lane whose comment says "did not run (usage limit)": nothing to fix. Wait for the reset
  time it names (a wake-up in chat, a sleep headless; outside Rule 4), then
  `just rerun-failed <the run id>` and watch again.
- Any other red: read the log first. If it shows the cause, fix it; if not, the next
  push adds observation and makes the failure cheap, never a fix (`docs/workflow.md`
  § 8). Fix with `git commit --fixup <sha>` (never a new titled commit, never a
  force-push), push, watch again. The same gate red three times: open question, leave
  the PR open, stop.
- All green: every lane's newest comment, green or not, graded in a PR comment
  (`docs/workflow.md` § 3, step 9); `just merge` folds the fixups.

## Step 9 — Closure

In the same PR, before the merge, then the closing brief (`CLAUDE.md`) with the PR number:

- **The source docs still match the code**: spec, contract, an ADR for a hard technical
  choice, `ARCHITECTURE.md` for a new module. The PR body says which changed, or "docs:
  none needed".
- A spec touched or named: `spec-checker` on it, and what it reports fixed.
- What the task met and did not fix is filed as issues (`docs/workflow.md` § 2, headless
  too), never left in chat; a task issue the work resolved is `Closes #NN` too.

## Rules

1. Headless never asks the human; questions are comments on the issue.
2. `CLAUDE.md` § Core rules 1 and Step 5 hold, and the batch is never edited.
3. One task per invocation. Stop after Step 9.
4. Three hours of wall clock per task; over that, open question and stop.
