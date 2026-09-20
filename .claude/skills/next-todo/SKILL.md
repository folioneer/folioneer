---
name: next-todo
description: Runs the first ready entry of the human's Next queue in docs/todo.md end to end under Workflow C — branch, design gate, acceptance tests, implementation, harness, reviewers, PR, merge on green, closure — without asking the human anything; questions go into the entry. Use to advance the backlog autonomously, one entry per invocation.
tools: Read, Glob, Grep, Write, Edit, Bash, Agent, TaskCreate, TaskUpdate, Monitor
---

# Skill — `next-todo`

One invocation, one entry, from `docs/todo.md` § Next to a merged pull request. The
rules are `docs/workflow-c.md`; this file is the checklist.

## Step 0 — Preconditions

- `git status --short` is empty and the branch is `main`, fresh (`git pull --ff-only`).
  Otherwise stop and report; never work on a dirty tree.
- With an argument (`/next-todo #NNN` or `/next-todo TD-NNN`): that entry, and the
  human is in the chat — **chat mode**. Without one: read `docs/todo.md` § Next and
  take references in order — **headless mode**, nobody to ask. Load the entry
  (`## #NNN` in `docs/todo.md`, or `## … — TD-NNN — …` in `docs/techdebt.md`).
- **Ready** = has a `**Done when:**`, `**Open questions:** none` (or every box ticked
  with its answer written after it), and `**Design:**` is `none` or `validated`.
  Headless: skip entries that are not ready; if none is ready, print which questions
  block which entries and stop. Chat: ask the open questions together, once, before
  anything else; write the answers into the entry (they land in the same PR as the
  change), then continue.

## Step 1 — Opening brief, branch and task list

- The opening brief first (CLAUDE.md § Opening and closing a piece of work): Task,
  Scope, Design, Touching. Chat: the first message. Headless: the top of the PR body.

- `git checkout -b c/NNN-<slug>` (or `c/td-NNN-<slug>`).
- `TaskCreate` one task per step below; mark each `in_progress` / `completed` as you go.

## Step 2 — Design gate

If the entry changes what the user sees (a new screen, a moved or added control, a
changed layout or wording pattern) and `Design` is `none`: run `/design-proposal NNN`,
which renders the mocks and flips the line to `proposed (…)`.

- Headless: commit the mocks, merge that docs change through a PR, then go back to
  Step 0 for the next ready entry. Do not implement.
- Chat: show the mocks in the conversation and ask. On a yes, set the line to
  `validated` and continue; the mocks and the line land in the same PR as the change.
  On a no, write what the human wants as an open question, commit, stop.

## Step 3 — Acceptance first

- Read the convention docs the entry's layers require (CLAUDE.md § Mandatory pre-read).
- Translate every clause of Done when into a failing test: Rust for logic, Vitest for
  rendering, E2E for what a user does. Name each test with the ref (`#NNN`) or the spec
  rule (`TRIGRAM-NNN`); when the domain has a spec and the entry adds a rule, write the
  rule in `docs/spec/<feature>.md` in the same commit.
- Run the suites; confirm the new tests are red. If a clause cannot become a test,
  write it as an open question on the entry, commit that, and stop.

## Step 4 — Implement

Smallest change to green. Logic in Rust; the frontend renders. Stable ids on every
control; i18n for every string; one event per action. Gold layout for new files.

## Step 5 — Harness

`just harness`. Fix until green. A coverage floor under its value is fixed with
tests, never by editing `coverage-gates.json` downward; an architecture violation is
fixed in code, never by editing `arch-allowlist.json` upward.

## Step 6 — Reviewers

Launch the reviewer agents that match the diff (`docs/workflow-c.md` § 7) in one
batch. Grade every finding with `/review-triage`'s axes and apply the policy: (a) fix,
(b) `TD-NNN` entry, (c) one-off inline comment, (c) pattern → edit the reviewer prompt,
`[DECISION]` → open question on the entry. Re-run the reviewers until no 🔴 remains.
No question to the human. CI runs the same reviewers on every push (Step 8).

## Step 7 — Evidence

`/visual-proof` for every changed `.tsx`/`.css`; stage the screenshots. Write the
commit: conventional title as the changelog line (≤ 72 characters, user-facing words
for `feat`/`fix`), body ≤ 2 lines with the ref.

## Step 8 — Pull request and merge

- Commit, push, `gh pr create`. Body under 20 lines: the entry ref and title, each
  Done when clause with the test that proves it, findings that changed something,
  techdebt filed, the visual proofs. Nothing else.
- Watch the checks with a `Monitor` on
  `gh api repos/{owner}/{repo}/commits/<sha>/check-runs` (one line per completed
  run, exit when all are completed) until every run completes.
- A red `reviewer-<lane>` check: read its sticky comment on the PR (`gh api
repos/{owner}/{repo}/issues/<n>/comments`), grade every finding with
  `/review-triage`'s axes and apply the policy — (a) fix, (b) `TD-NNN` entry, (c)
  one-off inline comment, (c) pattern → edit the reviewer prompt, `[DECISION]` → open
  question on the entry — then record the table in the PR body, push, and watch again.
- One E2E failure that passes on re-run: file the flake as `TD-NNN` with the failure
  screenshot and continue. Any other red: fix, push, watch again. The same gate red
  three times: open question on the entry, leave the PR open, stop.
- All green: `just merge`.

## Step 9 — Closure

On the same branch before the merge, or as a follow-up docs PR if forgotten: the
entry is removed from `docs/todo.md` (and its plan, when this is the plan's last pull
request); techdebt entries the work resolved are removed; `ARCHITECTURE.md` if a
module appeared; design proposal images deleted. Then the closing brief (CLAUDE.md §
Opening and closing a piece of work): what changed for the user, what the project
accumulated; the PR number and where anything still owed was filed.

## Rules

1. Never ask the human. Questions are lines in the entry.
2. Never lower a floor, never raise an allowlist, never bypass a hook, never
   force-push, never touch the live database, never cut a release, never edit Next.
3. One entry per invocation. Stop after Step 9.
4. Three hours wall-clock per entry; over that, open question and stop.
