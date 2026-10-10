# Workflow — the human sets expectations, the harness holds the line

The operating manual for how work moves from `docs/todo.md` to a release. Decided
on 2026-09-12; this document is the rule set, the git history records how it was built.

## 1. Who owns what

| Who     | Owns                                                                                             |
| ------- | ------------------------------------------------------------------------------------------------ |
| Human   | `docs/todo.md`: what is worth doing, its user value, its done-when, and the **Next** queue.      |
| Human   | Validating a **design** before anything the user sees changes.                                   |
| Human   | Cutting a **release**. Several merged branches may wait for one.                                 |
| Agent   | `docs/techdebt.md`: every observation, smell and proposal. The human queues from it.             |
| Agent   | `docs/flow.md`: what the workflow itself owes. The human queues from it.                         |
| Agent   | The task, end to end: tests, code, review, merge. **No pull request is validated by a human.**   |
| Agent   | Coding the right way: logic in Rust, dumb frontend, gold layouts, typed errors, stable ids.      |
| Harness | Proving it, mechanically, on every pull request. Nothing merges that the harness has not passed. |

Four human touchpoints. Everything else is either the agent's job or a machine gate.

## 2. The two files

A reference says what it points at: a todo entry is `TODO-NNN`, a tech-debt entry
`DEBT-NNN`, a flow entry `FLOW-NNN`, a GitHub issue `ghNN`. A pull request keeps GitHub's
own `#NN`. The forms these replaced are refused in every tracked file by
`scripts/reference-forms.py` (harness and CI); `CHANGELOG.md` keeps what was released
under them.

### `docs/todo.md` — human-owned

- `## Next` at the top holds the queue: `TODO-NNN` and `DEBT-NNN` references in the order to
  work them. The agent takes the first **ready** one. Order and additions are the
  human's; the agent only removes a reference, when it closes the entry.
- Every entry ends with `**User value:**`, `**Done when:**`, `**Design:**` and
  `**Open questions:**`.
- **Ready** means: queued, has a Done when, `Open questions: none`, and `Design` is
  `none` (no proposal needed yet) or `validated`.
- The agent writes to this file in four places only: it flips `Design` to
  `proposed (…)`, it adds open questions, and in the closure commit of the pull request
  that merges an entry it removes the entry, and its reference in Next. It never creates
  or reorders entries.

### `docs/techdebt.md` — agent-owned

- Every entry carries a permanent `DEBT-NNN` reference (`## YYYY-MM-DD — DEBT-NNN — title`).
- The agent files here: reviewer findings it did not fix, smells met on the way,
  proposals for new work, surviving mutants, coverage holes, frozen architecture debt.
- Entries are observations, not commitments: an entry says what is odd, not what to do.
  The human promotes one by queuing its reference in Next.
- An entry is `Found by`, `Where`, `Severity` (🔴 🟡 🔵, omitted when unknown),
  `Observation`, `User value` and `Done when`. Nothing in it is invented: a missing fact
  is left out.
- A queued entry that needs the owner's answer carries `**Open questions:**` like a todo
  entry; while a box is unticked it is not ready and the agent skips it.

### `docs/flow.md` — agent-owned

- What the workflow itself owes: observations about the harness, the checks, the waits
  and the human's part, each with its figure, what is proposed (or the verdict to keep
  things as they are), what it costs and what it protects. Reference `FLOW-NNN`.
- It also holds the todo and tech-debt entries that are about how work moves; each keeps
  the reference it was filed under.
- The human promotes one by queuing its reference in Next, like any other.

## 3. The loop — one run, one entry

1. **Pick** — headless: the first ready entry in Next. In chat: the entry the human
   names (`/next-todo TODO-NNN`); the sentence is the queue, and open questions are asked
   before anything starts — one at a time, each with its context — their answers
   written into the entry.
   Branch `<type>/todo-NNN-slug` (or `<type>/debt-NNN-slug`, `<type>/flow-NNN-slug`) off fresh `main`, where `<type>` is
   the commit type the change will carry (`feat`, `fix`, `refactor`, `chore`, `docs`, `test`,
   `ci`); work outside an entry is `<type>/slug`.
2. **Design gate** — if the entry changes what the user sees (a new screen, a moved or
   added control, a changed layout or wording pattern): produce the proposal (§ 4).
   Headless: set `Design: proposed (screenshots/design/NNN-*.png)`, merge that as a
   docs change, and **move on to the next ready entry**; the human validates by editing
   the line. In chat: show the mocks, ask, and on a yes set the line to `validated`
   and continue in the same run.
3. **Acceptance first** — turn Done when into failing tests: Rust for logic, Vitest for
   rendering, E2E for what a user does. Test names carry `TODO-NNN` (or the `TRIGRAM-NNN`
   rule when the domain has a spec; the agent writes the rule from Done when in the
   same commit). A feature in a domain without a spec starts one (`docs/spec/<feature>.md`,
   its trigram registered in `docs/spec-index.md`). A new or changed command updates the
   domain's contract (`docs/contracts/<domain>-contract.md`, created with the domain's
   first command) in the same commit, with `/contract`. Confirm red.
4. **Implement** to green, the right way (§ 6). Logic that lands in the frontend is a
   harness failure, not a style remark.
5. **Self-check** — while working, the recipes take a scope: `just test <paths>`,
   `just test-rust <filter>`, `just check --frontend` or `--backend` for lint and types;
   never the native command.
   Then `just harness`: architecture rules, lint, type-check, build, both suites;
   `just harness --coverage` adds the coverage floors, run before the first push.
   A run is judged by its exit code, never by a line picked out of its output: a suite
   can report every test passing and still fail. `just quiet <recipe>` (so `just quiet
harness`) prints that
   verdict in one line, keeps the whole output in `tmp/<recipe>.log` and shows its end
   on a failure.
6. **Self-review** — run the reviewer agents that match the diff (§ 7); apply the
   triage policy; run them again until no 🔴 remains. CI runs them again on every
   push as the enforced record.
7. **Evidence** — `/visual-proof` for every changed component; the E2E run in CI
   captures the real app and links the screenshots from the pull request.
8. **PR** — opened for the record, not for approval. Body under 20 lines: the entry,
   each Done when clause with the test that proves it, findings that changed
   something, techdebt filed, screenshots. The commit title is the changelog line.
9. **Merge**: `just merge`, which refuses until every check on the pull request is green.
10. **Closure** in the same PR: the entry removed from the file it lives in
    (`docs/todo.md`, `docs/techdebt.md` or `docs/flow.md`), and its reference from Next;
    techdebt
    updated; `ARCHITECTURE.md` if a module appeared; the spec if a rule changed.
11. **Next** entry, or stop (§ 8).

**A batch in chat** — when the human says go to several queued entries, the agent runs
`/next-todo` for each rather than re-deriving its steps, and the human is asked twice,
not once per entry:

- **Opening block**, before the first entry starts: the open questions of every queued
  entry, and the mock-ups (§ 4) of every queued entry that changes what the user sees,
  in one docs pull request. For each: what to look at, what a yes means, and what is
  assumed if nothing is said. A mock-up drawn this early may be redrawn once the code is
  read; the change is said in the entry's pull request.
- **Closing block**, in the release brief: the vocabulary terms added or widened, the
  reviewer findings declined and why, the debt filed, and the screenshots of what
  shipped. It blocks nothing; it is what the human reads to disagree.

The opening block is built by reading, before the first branch, every queued entry and
the code it touches for four things, asked together:

- the words the entry brings to a screen, the code or a spec that
  `docs/ubiquitous-language.md` does not hold;
- a technical choice that will need an ADR;
- a platform, or a part of the Done when, that may be left out;
- an entry large enough to be split.

A queued `DEBT-NNN` is challenged against the current code in the same reading, so that
what the challenge asks is in the block. A question is asked once, at its widest ("the
term for every aggregate edited from a form", not one aggregate at a time). A choice
that has a standing answer is not asked again: the answer is written where its rule
lives, and read there.

A question that only appears mid-batch is written on its entry, the entry is skipped,
and the question joins the closing block — unless nothing else in the queue can run. It
is right only for what the reading could not show, and it says what made it unknowable
at the start (what CI found, what an answer of the block opened).

**Release** — when the queue empties the agent runs `/dep-audit` without being asked (a
known vulnerability in what ships blocks the release; the agent fixes it or files it),
then `spec-checker` once on every spec the batch touched — `bash scripts/batch-specs.sh`
lists them — and files what it finds as tech debt: a promise the code does not keep is
not fixed in the release it was found in. It hands over a clean `main`. The release itself is the human's only part: they run
`just release -y` when they choose. After
it, `/prune` runs once and `/flow-audit` measures the batch against the release before
(`docs/flow.md`). The release re-runs the full
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

`just harness` locally; the same set as required checks on every pull request, except
coverage and the E2E suite, which are CI's (`just harness --coverage` measures coverage
locally too). Both run only the layers a change touched (`scripts/changed-scope.sh`): a
docs, strings or CI change pays lint and build, never a coverage run. The full SQLx
check (`cargo sqlx prepare --check`) runs only in CI and for a release: it makes cargo
rebuild the crate, and the local offline build already fails on a query missing from
`.sqlx/`. The git hooks
run only the fast checks for the same scope; a Markdown-only change costs Prettier
and nothing else.

| Check                     | Where                                                                                         | Gate                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| ------------------------- | --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Lint, format, type-check  | `scripts/check.py`, Quality                                                                   | any error                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Architecture rules A1–A11 | `scripts/arch-check.py`, Quality                                                              | any violation; frozen debt may only shrink (`arch-allowlist.json`)                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Unit and integration      | Vitest, cargo test, Quality                                                                   | any failure                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Coverage floors           | `scripts/coverage-gate.py`, Quality                                                           | frontend features < 80 %, backend logic < floor (`coverage-gates.json`, ratchets to 90 %); the backend report comes from `cargo llvm-cov` with inline test modules stripped (`scripts/coverage-strip-tests.py`)                                                                                                                                                                                                                                                                                                          |
| Golden portfolio          | `src-tauri/tests/golden_portfolio.rs`, Quality                                                | any drift in a pinned figure (`tests/golden/expected.json`); a change that moves one names it in its entry's Done when and regenerates with `GOLDEN_UPDATE=1`                                                                                                                                                                                                                                                                                                                                                            |
| Sync written form         | `src-tauri/tests/sync_two_devices.rs` (`syn_038_*`) + `scripts/sync-format-check.sh`, Quality | what this build writes for any synced record, enum, folder header, manifest or segment differs from `tests/sync_format/v{DATA_FORMAT_VERSION}.json`, or a diff edits a snapshot already on main — a new written form arrives as a bumped version with its own snapshot (SYN-038)                                                                                                                                                                                                                                         |
| Contracts and bindings    | `scripts/contract-check.py` + `contract-gaps.json`, Quality                                   | a command of `src/bindings.ts` without a row in a contract, a row without a command, arguments, a return type, a struct or an enum that are not the generated ones, or a row promising an error code its command's error type cannot carry; the gaps known when the check arrived are listed and the list only shrinks. Which codes a command returns is not in the bindings: that half is read (`/contract`, `contract-reviewer`)                                                                                       |
| Dependency licences       | `scripts/licence-check.py` + `licence-allowlist.json`, Quality                                | a crate reached without a dev-dependency, or an npm package not flagged dev, carries a licence outside the allow-list and is not accepted there by name under that exact licence; an exception nothing uses any more fails too                                                                                                                                                                                                                                                                                           |
| E2E on the real app       | `.github/workflows/e2e.yml`, every PR (records-only PRs skip it, `scripts/e2e-scope.py`)      | any failure; screenshots linked                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Reviewer prompts          | `.github/workflows/review.yml`, every PR                                                      | any 🔴 in a lane the diff touches; the report is a sticky PR comment; fails closed without the `CLAUDE_CODE_OAUTH_TOKEN` subscription secret                                                                                                                                                                                                                                                                                                                                                                             |
| Visual regression         | `.github/workflows/e2e.yml`, every PR                                                         | a screen differing from main's last green run beyond 0.3 % of pixels while the PR touched no frontend file; expected differences are listed in the PR comment                                                                                                                                                                                                                                                                                                                                                            |
| Commit hygiene            | Quality `pr-checks`                                                                           | title > 72, wrong type, trailer                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Merge guard               | `scripts/merge.py`, `required-checks.json`                                                    | `just merge` refuses unless the branch is the head of an open pull request with every check green and every required check present; a rebase that moves the commits pushes them and stops until CI has run on them, unless the rebase left the tested tree as it was, record files (todo, techdebt, lessons, plans, ADRs) aside, so a branch stacked on merged commits lands without one; the rebase folds `fixup!` commits into the commits they name, so an entry lands as one commit whatever was pushed after review |
| Security audit            | `security-audit.yml`                                                                          | new advisory                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Mutation sweep            | `.github/workflows/mutants.yml`, monthly                                                      | none; cargo-mutants on the logic code (`src-tauri/.cargo/mutants.toml`), survivors listed in the sticky issue "Mutation sweep — surviving mutants" in the techdebt entry format, `mutants.out` kept 90 days                                                                                                                                                                                                                                                                                                              |

The `main-protection` ruleset lists the same checks; the owner's laptop bypasses it for
`just release`, so the guard in `just merge` is the enforced gate.

## 6. The right way to code

- **Logic in Rust, dumb frontend** (`docs/frontend-rules.md` F32, guarded by A12).
- **Gold layouts** for new code (`docs/backend-rules.md` B0/B37–B43,
  `docs/frontend-rules.md` F0/F26–F28); bit-by-bit for existing code (§ 10).
- **Typed errors** on the wire (`docs/error-model.md`); factories and aggregate-root
  methods on domain objects (`docs/backend-rules.md` B7, B44).
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
`e2e/**`, `spec-reviewer` and `contract-reviewer` when `docs/spec/` or `docs/contracts/`
change (the checks `reviewer-spec` and `reviewer-contract` in CI), `adr-reviewer` when an
ADR changes and `spec-checker` before closing an entry that carries spec rules — these
two locally only.

Every finding, local or from a lane's CI comment, is graded and the outcome recorded
in the PR body. The grade comes from four questions, in order; the one that settles it
is the row's reason:

1. **Did this branch introduce it?** Yes → (a), unless question 3 or 4 says otherwise.
2. **If it was there before: is it in a file the pull request touches, small (about ten
   lines) and mechanical?** All three → (a). Otherwise → (b).
3. **Does it need a design judgment, or reach files outside the pull request?** → (b).
4. **Is it wrong on the facts, outside the entry, speculative, or a taste no rule
   states?** → (c), whatever the earlier answers. It is a pattern only when it recurs
   across the batch or would bind later work; in doubt, one-off.

Each finding gets its own row — never a silent fix, never a silent pass.

The CI reviewers run on the subscription the chat session uses. A lane stopped by its
usage limit says "did not run (usage limit)" in its comment and in `just merge`'s
refusal, and fails: an unreviewed pull request does not merge. It is re-run once the
limit has reset (`gh run rerun <run> --failed`). Late in a long session, a first push
waits for the reset rather than spend a round on it.

| Grade        | Action                                                            |
| ------------ | ----------------------------------------------------------------- |
| (a)          | Fix in the PR                                                     |
| (b)          | `DEBT-NNN` entry in `docs/techdebt.md`, linked from the PR body   |
| (c) one-off  | Inline comment `// <reviewer> FP: <reason> — see PR #NN`          |
| (c) pattern  | Edit the reviewer prompt in the same PR; note it in the PR body   |
| `[DECISION]` | Open question on the entry; the PR stays open; the agent moves on |

No halt for the human. A review comment from the human on a merged or open PR is
treated as a new open question on the entry.

## 8. Budget and stop rules

- One entry, one run, a wall-clock budget of three hours. Over budget → open question
  "larger than estimated: split?", move on.
- The same gate failing three times on one entry → open question, move on.
- A test fails for a reason the change cannot explain → it is flaky, and is never
  accepted by re-running it: that one test is skipped in its own pull request and a
  `DEBT-NNN` is filed with the failure text and the run's link, to find out why; the fix
  that re-enables it closes the entry. The run continues.
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

## 10. Gold layouts, bit by bit

Three gold targets exist: the backend layout (B0, B37–B43), the frontend layout (F0, F28; the
remaining crossings frozen in `arch-allowlist.json`) and the error model (landed,
`docs/error-model.md`). New code follows them. Existing code touched by a task is brought to gold
in the same pull request only when all three hold — otherwise it keeps the standard around it:

- **Size**: about 50 lines of conformance change at most.
- **Locality**: inside the files the task already touches.
- **Mechanical**: a rename, an import, a signature or type swap; any design judgement ("which
  layer?", "what name?") is its own entry.

The "two stories" check overrides the number: a pull request that tells the feature and a layout
migration tells two stories, and the migration waits. A mixed-standard codebase is acceptable; a
half-done migration that leaves neither standard intact is not. The larger migrations are
tracked in `docs/techdebt.md` (DEBT-008, DEBT-009) and run when queued.

## 11. Splitting a feature by layer

A feature touching backend and frontend ships as one pull request per layer when either layer
exceeds about 20 files or 500 lines, in this order: spec, contract, migration, backend and the
generated bindings (mergeable alone — the bindings are unused); then gateway, hooks, presenter,
components and strings, branched off the merged backend; then E2E and closure. Each diff stays
one story and CI signs each off on its own.
