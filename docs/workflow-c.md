# Workflow C — the human sets expectations, the harness holds the line

The operating manual for how work moves from `docs/todo.md` to a release. Decided
on 2026-09-12; this document is the rule set, the git history records how it was built.

## 1. Who owns what

| Who     | Owns                                                                                             |
| ------- | ------------------------------------------------------------------------------------------------ |
| Human   | `docs/todo.md`: what is worth doing, its user value, its done-when, and the **Next** queue.      |
| Human   | Validating a **design** before anything the user sees changes.                                   |
| Human   | Cutting a **release**. Several merged branches may wait for one.                                 |
| Agent   | `docs/techdebt.md`: every observation, smell and proposal. The human queues from it.             |
| Agent   | The task, end to end: tests, code, review, merge. **No pull request is validated by a human.**   |
| Agent   | Coding the right way: logic in Rust, dumb frontend, gold layouts, typed errors, stable ids.      |
| Harness | Proving it, mechanically, on every pull request. Nothing merges that the harness has not passed. |

Four human touchpoints. Everything else is either the agent's job or a machine gate.

## 2. The two files

### `docs/todo.md` — human-owned

- `## Next` at the top holds the queue: `#NNN` and `TD-NNN` references in the order to
  work them. The agent takes the first **ready** one and never edits the list.
- Every entry ends with `**User value:**`, `**Done when:**`, `**Design:**` and
  `**Open questions:**`.
- **Ready** means: queued, has a Done when, `Open questions: none`, and `Design` is
  `none` (no proposal needed yet) or `validated`.
- The agent writes to this file in three places only: it flips `Design` to
  `proposed (…)`, it adds open questions, and it removes an entry in the closure commit
  of the pull request that merges it. It never creates or reorders entries.

### `docs/techdebt.md` — agent-owned

- Every entry carries a permanent `TD-NNN` reference (`## YYYY-MM-DD — TD-NNN — title`).
- The agent files here: reviewer findings it did not fix, smells met on the way,
  proposals for new work, surviving mutants, coverage holes, frozen architecture debt.
- Entries are observations, not commitments. The human promotes one by queuing its
  reference in Next.

## 3. The loop — one run, one entry

1. **Pick** — headless: the first ready entry in Next. In chat: the entry the human
   names (`/next-todo #NNN`); the sentence is the queue, and open questions are asked
   together, once, before anything starts, their answers written into the entry.
   Branch `c/NNN-slug` (or `c/td-NNN-slug`) off fresh `main`.
2. **Design gate** — if the entry changes what the user sees (a new screen, a moved or
   added control, a changed layout or wording pattern): produce the proposal (§ 4).
   Headless: set `Design: proposed (screenshots/design/NNN-*.png)`, merge that as a
   docs change, and **move on to the next ready entry**; the human validates by editing
   the line. In chat: show the mocks, ask, and on a yes set the line to `validated`
   and continue in the same run.
3. **Acceptance first** — turn Done when into failing tests: Rust for logic, Vitest for
   rendering, E2E for what a user does. Test names carry `#NNN` (or the `TRIGRAM-NNN`
   rule when the domain has a spec; the agent writes the rule from Done when in the
   same commit). Confirm red.
4. **Implement** to green, the right way (§ 6). Logic that lands in the frontend is a
   harness failure, not a style remark.
5. **Self-check** — `just harness`: architecture rules, lint, type-check, build, both
   suites with coverage, the coverage floors.
6. **Self-review** — run the reviewer agents that match the diff (§ 7); apply the
   triage policy; run them again until no 🔴 remains. CI runs them again on every
   push as the enforced record.
7. **Evidence** — `/visual-proof` for every changed component; the E2E run in CI
   captures the real app and links the screenshots from the pull request.
8. **PR** — opened for the record, not for approval. Body under 20 lines: the entry,
   each Done when clause with the test that proves it, findings that changed
   something, techdebt filed, screenshots. The commit title is the changelog line.
9. **Merge**: `just merge`, which refuses until every check on the pull request is green.
10. **Closure** in the same PR: the entry removed from `docs/todo.md`, and its plan
    when this pull request is the plan's last; techdebt updated;
    `ARCHITECTURE.md` if a module appeared; the spec if a rule changed.
11. **Next** entry, or stop (§ 8).

**Release** — the human runs `just release -y` when they choose. It re-runs the full
harness on `main`, computes the version from the merged titles, writes the changelog,
tags and pushes; CI builds and leaves the draft; the human publishes it. The
changelog and the git history are the record of what shipped.

## 4. Design proposal

Before any change to what the user sees, the agent commits rendered mocks of the
target state, built with the real components and tokens, under
`screenshots/design/NNN-{state}-{light|dark}.png`, plus `screenshots/design/NNN.md`
(five lines: what moves, what is added, what is removed, what stays). The
`/design-proposal` skill drives it: the same preview pipeline as `/visual-proof`,
written to the design folder instead of the proof folder.

The human validates by editing the entry's `Design` line to `validated`, or writes an
open question; in a chat run, a yes in the conversation is the validation and the
agent writes the line. Nothing else is needed. Once the work ships, the proposal images are
deleted in the closure commit; the visual proofs are the record.

## 5. The harness

`just harness` locally; the same set as required checks on every pull request. Both
measure coverage only for a layer the change touched (`scripts/changed-scope.sh`): a
docs, strings or CI change pays lint and build, never a coverage run. The git hooks
run only the fast checks for the same scope; a Markdown-only change costs Prettier
and nothing else.

| Check                    | Where                                                                                         | Gate                                                                                                                                                                                                                                                                                                                                                            |
| ------------------------ | --------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Lint, format, type-check | `scripts/check.py`, Quality                                                                   | any error                                                                                                                                                                                                                                                                                                                                                       |
| Architecture rules A1–A8 | `scripts/arch-check.py`, Quality                                                              | any violation; frozen debt may only shrink (`arch-allowlist.json`)                                                                                                                                                                                                                                                                                              |
| Unit and integration     | Vitest, cargo test, Quality                                                                   | any failure                                                                                                                                                                                                                                                                                                                                                     |
| Coverage floors          | `scripts/coverage-gate.py`, Quality                                                           | frontend features < 80 %, backend logic < floor (`coverage-gates.json`, ratchets to 90 %); the backend report comes from `cargo llvm-cov` with inline test modules stripped (`scripts/coverage-strip-tests.py`)                                                                                                                                                 |
| Golden portfolio         | `src-tauri/tests/golden_portfolio.rs`, Quality                                                | any drift in a pinned figure (`tests/golden/expected.json`); a change that moves one names it in its entry's Done when and regenerates with `GOLDEN_UPDATE=1`                                                                                                                                                                                                   |
| Sync written form        | `src-tauri/tests/sync_two_devices.rs` (`syn_038_*`) + `scripts/sync-format-check.sh`, Quality | what this build writes for any synced record, enum, folder header, manifest or segment differs from `tests/sync_format/v{DATA_FORMAT_VERSION}.json`, or a diff edits a snapshot already on main — a new written form arrives as a bumped version with its own snapshot (SYN-038)                                                                                |
| Dependency licences      | `scripts/licence-check.py` + `licence-allowlist.json`, Quality                                | a crate reached without a dev-dependency, or an npm package not flagged dev, carries a licence outside the allow-list and is not accepted there by name under that exact licence; an exception nothing uses any more fails too                                                                                                                                  |
| E2E on the real app      | `.github/workflows/e2e.yml`, every PR                                                         | any failure; screenshots linked                                                                                                                                                                                                                                                                                                                                 |
| Reviewer prompts         | `.github/workflows/review.yml`, every PR                                                      | any 🔴 in a lane the diff touches; the report is a sticky PR comment; fails closed without the `CLAUDE_CODE_OAUTH_TOKEN` subscription secret                                                                                                                                                                                                                    |
| Visual regression        | `.github/workflows/e2e.yml`, every PR                                                         | a screen differing from main's last green run beyond 0.3 % of pixels while the PR touched no frontend file; expected differences are listed in the PR comment                                                                                                                                                                                                   |
| Commit hygiene           | Quality `pr-checks`                                                                           | title > 72, wrong type, trailer                                                                                                                                                                                                                                                                                                                                 |
| Merge guard              | `scripts/merge.py`, `required-checks.json`                                                    | `just merge` refuses unless the branch is the head of an open pull request with every check green and every required check present; a rebase that moves the commits pushes them and stops until CI has run on them, unless the rebase changed record files only (todo, techdebt, lessons, plans, ADRs), so a branch stacked on merged commits lands without one |
| Security audit           | `security-audit.yml`                                                                          | new advisory                                                                                                                                                                                                                                                                                                                                                    |
| Mutation sweep           | `.github/workflows/mutants.yml`, monthly                                                      | none; cargo-mutants on the logic code (`src-tauri/.cargo/mutants.toml`), survivors listed in the sticky issue "Mutation sweep — surviving mutants" in the techdebt entry format, `mutants.out` kept 90 days                                                                                                                                                     |

The `main-protection` ruleset lists the same checks; the owner's laptop bypasses it for
`just release`, so the guard in `just merge` is the enforced gate.

## 6. The right way to code

- **Logic in Rust, dumb frontend.** The frontend renders, holds ephemeral UI state,
  and maps error codes to i18n keys. It never decides, validates business rules,
  aggregates or derives. If a step can skip the UI, it lives in Rust.
- **Gold layouts** for new code (`docs/backend-rules.md` B0/B37–B43,
  `docs/frontend-rules.md` F0/F26–F28); bit-by-bit for existing code (CLAUDE.md).
- **Typed errors** on the wire (`docs/error-model.md`); factories and aggregate-root
  methods on domain objects (CLAUDE.md § Critical Patterns).
- **Stable ids** on every interactive element; text from i18n; one event per action on
  the bus.
- **Ubiquitous language** in every identifier (`docs/ubiquitous-language.md`).
- **Surgical**: touch the file set the entry needs. Boyscout inside it, never beyond.

## 7. Reviewers and the triage policy

Reviewers run locally on the diff before the PR, until no 🔴 remains, and CI runs them
again on every push (`.github/workflows/review.yml`, one check per lane the diff
touches; the report is a sticky comment on the PR and any 🔴 fails the check). The
lanes: `reviewer-backend` and `reviewer-arch` for
`.rs`, `reviewer-frontend` and `reviewer-arch` for `.ts`/`.tsx`, `reviewer-sql` for
migrations, `reviewer-infra` for scripts, hooks, config and workflows,
`reviewer-security` for commands, capabilities and secret handling, `reviewer-e2e` for
`e2e/**`, `spec-reviewer` / `contract-reviewer` / `adr-reviewer` when those documents
change, `spec-checker` before closing an entry that carries spec rules.

Every finding, local or from a lane's CI comment, is graded and the outcome recorded
in the PR body:

| Grade        | Action                                                            |
| ------------ | ----------------------------------------------------------------- |
| (a)          | Fix in the PR                                                     |
| (b)          | `TD-NNN` entry in `docs/techdebt.md`, linked from the PR body     |
| (c) one-off  | Inline comment `// <reviewer> FP: <reason> — see PR #NN`          |
| (c) pattern  | Edit the reviewer prompt in the same PR; note it in the PR body   |
| `[DECISION]` | Open question on the entry; the PR stays open; the agent moves on |

No halt for the human. A review comment from the human on a merged or open PR is
treated as a new open question on the entry.

## 8. Budget and stop rules

- One entry, one run, a wall-clock budget of three hours. Over budget → open question
  "larger than estimated: split?", move on.
- The same gate failing three times on one entry → open question, move on.
- E2E failed once and passed on re-run → the run continues; the flake is filed as
  `TD-NNN` with the failure screenshot.
- Never touch the live portfolio database, never force-push, never bypass a hook,
  never edit a released changelog line, never reorder Next, never cut a release.

## 9. Where the loop runs

The chat session is for writing entries together, design conversations and answering
open questions. The loop runs headless, one entry per run, never asks, and keeps its
state in git and the two files.

- **Cloud routine** `next-todo` on claude.ai/code, created disabled: weekdays at 05:37
  UTC, the repository's environment, the `/next-todo` prompt. E2E and screenshots
  come from CI, so the sandbox needs no display. The human switches it on.
- **Laptop**: `just next-todo` (`scripts/next-todo.sh`): one run at a time, a clean
  and fresh `main`, a non-empty Next queue, three hours of wall clock
  (`NEXT_TODO_BUDGET`), the log under `logs/next-todo/`. It runs
  `claude -p "/next-todo" --permission-mode acceptEdits`: nobody answers a prompt in a
  print-mode run, so a command outside the allow list in `.claude/settings.json` is
  denied and the run fails instead of waiting. The allow list scopes the flow, it is
  not a sandbox; the deny list refuses the usual spellings of the absolute rules (push
  to `main`, force push, hook bypass, release, tag, `gh pr merge`, editing the list
  itself) as a reminder, not a proof. What is enforced is the harness and the merge
  guard. The run shares the plan's session limit with any open session. A crontab
  line: `37 7 * * 1-5 cd ~/project/folioneer && just next-todo`.

Until a runner is switched on, a human types `/next-todo` in a session and the same
rules apply.
