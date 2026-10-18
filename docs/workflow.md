# Workflow — the human sets expectations, the harness holds the line

How work moves from a request or an issue to a batch end, and the conventions the
agents and skills rely on. This document is the rule set; git history records how it
was built.

## 1. Who owns what

- **Shared** — the feature and bug issues, each what a user will see; the human confirms
  its user value and done-when, and approves the batch (§ 2).
- **Human** — validating a **design** before anything the user sees changes.
- **Human** — cutting a **release**; several merged branches may wait for one.
- **Agent** — the task issues: every observation, smell and proposal about the code. The
  human queues from them.
- **Agent** — the flow issues: the same about the workflow itself (checks, waits, tools,
  the human's part), audited after each batch (§ 12).
- **Agent** — the task, end to end: tests, code, review, merge. **No pull request is
  validated by a human.**
- **Harness** — proving it, mechanically, on every pull request. Nothing merges that
  the harness has not passed.

Three human touchpoints, plus answering open questions. Everything else is the agent's
job or a machine gate.

The human is not the QA (owner, 2026-10-11): no issue waits for the owner to check by
hand that what was built works. A Done-when clause the agent cannot run becomes a test of
the harness — an E2E scenario when it needs the real window — or, when that test is
larger than the issue, a `task` issue that asks for it, and the issue closes on what the
harness proves. What the owner notices while using the application is welcome and is no
gate.

## 2. Issues and the Project (owner, 2026-10-18)

The backlog is on GitHub, never in git: the issues and the organisation's Project
(github.com/orgs/folioneer/projects/1). No commit touches it. In prose an issue is
"ticket #123", a pull request "PR #123"; a pull request says `Closes #123`, so its merge
closes the issue. An old `TODO-NNN`, `DEBT-NNN` or `FLOW-NNN` is found by searching the
issues for `Migrated from <id>`. A spec rule is `TRIGRAM-NNN` (Conventions).

### Filing an issue

- **Four kinds, one label each**: `feature` and `bug`, what a user will see (a `feat`, a
  `fix`); `task`, what a user will not (`refactor`, `chore` on the application, an
  enabler for a feature); `flow`, how the work moves (guides, `scripts/`, `justfile`,
  hooks, workflows). A spec, contract, glossary, ADR or `ARCHITECTURE.md` drifting from
  right code is a task gated "when next touched", alone only when it blocks something.
- **From its form** (`.github/ISSUE_TEMPLATE/`), the CLI included: `gh issue create` with
  its labels (`--label <kind> --label size:<S|M|L|XL>`) and the form's fields as
  `### <field>` sections, then `gh project item-add 1 --owner folioneer --url <url>`. Its
  title is one plain sentence stating the observation, no kind prefix; it carries its
  evidence (what was seen, where, a figure or a pull request), one observation only.
- **Size** (the `size:` label), by facts and not by effort — **S**: one pull request, one
  layer, nothing from the owner; **M**: any other single pull request — several lanes, a
  spec or a contract, an answer from the owner; **L**: two or three pull requests, or
  mocks or a spec interview with the owner; **XL**: a programme, sliced into sub-issues
  before it is queued. The agent sets it on a task or a flow issue; on a feature or a bug
  `/plan-batch` proposes it and the owner confirms it. A Design needing a mock: `mock`.
- **What waits on the owner** is a comment on the issue and the `needs-owner` label; the
  owner answers in the thread and the label goes once nothing is open. A feature or bug
  the agent files waits for the owner to confirm its User value; a flow issue waits for
  its Decision. The agent does not change, without asking, what the owner confirmed.
- **A bug is always filed**, even when fixed in the same session: its pull request
  closes it. A chat request that is not a bug needs no issue: the pull request is its
  record. An issue someone else opens is given its label, its form's fields and its size.
- **A dependency** is GitHub's "blocked by"; the slices of a programme are its
  sub-issues. A roadmap stage (MVP, Phase 2, Phase 3) is a milestone.
- **A gate** an issue carries ("when next touched", "not as a sweep", "when a third
  consumer appears") is honoured, or the override is put to the owner — never swept
  silently. A task the agent files is an observation, not a commitment.

### The Project — Status and Batch

- **Status**: Inbox (filed) → Ready → Queued (in the batch) → In progress (its branch
  exists) → In review (a pull request is linked) → Done (closed). GitHub sets the last two.
- **Ready** (owner, 2026-10-18): its `size:` label is set, its mock is made when it
  changes what a user sees, and its open questions are answered — no `needs-owner`. A
  flow issue is ready once it carries its Decision.
- **A batch** is the Queued items in row order, their Batch field its date. `/plan-batch`
  proposes it in chat; the owner's yes sets Batch, order and Status, and nothing else
  makes an item Queued. A cut during a batch is the owner's.
- **Sized by its work**, not its number of items: `/plan-batch` sums the sizes of what it
  proposes and stops at what the last batches merged (their `batch-audit` issues), saying
  both figures. A programme enters as its first slice.
- **The order `/plan-batch` proposes**: what changes the tools first; a dependency
  before what depends on it; items that touch the same files next to each other, so one
  can be prepared on the other; small fixes a user notices early; what needs the owner
  flagged at the top. Where two of these disagree, the proposal says so in one line.
- **Nothing open is passed over**: every open issue and every open Dependabot pull
  request is on the Project, queued or named as left out with its reason.

## 3. The loop — one run, one task

A task — a Queued item (`/run-entry`, headless), an issue the human names, or a chat
request (its record is the pull request body) — lands on `main` through the **ceremony**:
here a pull request, every check green, merged by `just merge` (steps 8–10).

1. **Pick and brief.** In chat, a question to the human goes through `AskUserQuestion`,
   one per call: what happened, what each option changes and costs, the recommended one
   first; the next waits for its answer. Headless, a question is a comment with
   `needs-owner` (§ 2) and the run moves on.
   - **The questions come before the work — the decision scan**: the agent reads the
     issue and the code it names, lists every choice that is the owner's — what a user
     sees, a term, a rule's scope, where a programme stops — and asks each once and at
     its widest, writing the answers into the issue. `/plan-batch` runs it while
     refining, leaving what a user sees (§ 4) and the code to the run, which asks what is
     still open before its branch exists. Two readings are named, because the code an
     issue changes shows neither: **the sentences that list** — for an issue that adds or
     changes what an interface can do, the locales (`src/i18n/locales/`), the specs and
     the `README.md` are searched for the sentences that list what it can do (a consent
     dialog, a description, a help page); one that would say less than what is allowed is
     a question — and **the ADRs of the records** — for every record the issue writes,
     the ADRs that name it (`docs/adr/`); a Done when that stands against one is a
     question.
   - A choice with a standing answer is not asked: when one option keeps what a user
     sees today, it is taken and stated in the pull request; a release-sweep finding
     that is not critical and predates the batch is filed and reported (Conventions).
   - **Banked**: a question is asked alone, with its context, only where the options
     differ in cost, risk or what is seen — an ambiguity or a deep choice; a trivial one
     is the agent's. A flow issue's proposal so answered is recorded as accepted
     ("banked" in its Decision, § 12) and reported in one line.
   - Each queued size is checked against the code the issue names and corrected on it
     before the branch (a feature's or a bug's as a question); one that changes during
     the work is written before closure.
   - During the work, a question remains right for what that reading could not show — a
     reviewer's finding, a failing machine, a behaviour found in the code — and the
     agent asks rather than guess, for a spec point as for a vocabulary term.
   - For an issue with a spec the order is: spec draft, `spec-reviewer`, then the
     questions, new vocabulary included, so a decision is taken once, knowing its
     consequences.

   The opening brief states Task, Scope, Design, Touching. Branch `<type>/123-slug`, the
   issue's number leading the slug, or `<type>/slug`, off a fresh `main`, where `<type>`
   is the commit type the change will carry; the item moves to In progress.

2. **Design gate** (§ 4) for anything the user sees.
3. **Acceptance first.** Each Done-when clause becomes a failing test: Rust for logic,
   Vitest for rendering, E2E for what a user does. Test names carry the issue number or the
   spec rule. A new business rule goes into `docs/spec/<feature>.md` in the same commit;
   a new or changed command updates `docs/contracts/<domain>-contract.md`
   (`docs/contracts/README.md`). Confirm red.
   - **A fault met in production**: the input that shows it is asked for before any fix
     is designed — a log says what failed, not why — and a failing test that reproduces
     it is the acceptance test of the fix.
4. **Implement** to green, the right way (§ 6). Logic that lands in the frontend is a
   harness failure, not a style remark.
5. **Self-check:** while working, the recipes take a scope (`just test <paths>`,
   `just test-rust <filter>`, `just check --frontend` or `--backend`); then `just harness`,
   and `just harness --coverage` before the first push (§ 5). A run is judged by its exit
   code, never by a line picked out of its output; `just quiet <recipe>` prints that
   verdict in one line and keeps the output in `tmp/<recipe>.log`.
6. **Self-review:** the reviewer lanes the diff needs (§ 7), again on the fixes until no
   🔴 remains.
7. **Evidence:** `/visual-proof` for every changed component; the E2E run in CI captures
   the real app and links its screenshots from the pull request.
8. **PR**, opened for the record, not for approval. Body under 20 lines: `Closes #123`, the
   task, each Done-when clause with the test that proves it and where that test was seen
   red (or why it could not be), findings that changed something, issues filed,
   screenshots. The commit title is the changelog line.
9. **Merge:** `just watch-pr` waits for the checks; each lane's newest comment, green or
   not, is read and its findings graded (§ 7); then `just merge` — it refuses until every
   check is green and folds `fixup!` commits, so the task lands as one commit.
10. **Closure** in the same PR: on an issue that touched a spec, `spec-checker` runs
    first and what it finds is fixed; a coverage floor the run measured more than a point
    above rises to half a point under that figure (`coverage-gates.json`, never down);
    what the task met and did not fix is filed as issues; `ARCHITECTURE.md` if a module
    appeared, the spec if a rule changed. The merge closes the issue. The closing brief
    says what changed for the user and what the project gained.

**Ending a batch** — every batch ends alike: the closing brief, then the release sweep
(Conventions) with `spec-checker` on every spec the batch touched (`bash
scripts/batch-specs.sh` lists them), what they find filed as `task` issues; a security
issue that puts users at risk is a `fix` instead: a `bug` issue put to the owner, worked
before the batch ends, swept again on what it changed, released. A batch with a `feat` or
`fix` is released: `just release-check` runs on `main` what the release validates before
it versions anything, a failure fixed in a pull request of its own; once every pull
request of the batch, and the `release-check` fix if any, has merged and the workflow runs of `main`'s head are green, the
agent runs `/dep-audit` on that `main` (a known vulnerability in what ships blocks the
release) and hands over with its verdict, naming the runs. The human runs `just release
-y`; CI builds the draft, which the human publishes once both platforms attached their
assets; the private build follows the public release (its `follow.yml`). Any other batch
the agent closes with `just batch-end` (a `batch/<date>` tag): it publishes nothing, its
chores ride in the next release, no fix is picked to fill a changelog. `/flow-audit`
audits the range it closes (§ 12). A batch is named by its dates, never a version number.

## 4. Design proposal

Before **any** change to what the user sees — a new screen, a moved or added control, a
changed layout or wording pattern — the agent renders mocks of the target state with
the real components and tokens (`/design-proposal`):
`screenshots/design/{id}-{light|dark}-{state}.png` plus a five-line note (what moves,
is added, is removed, stays).

- **In chat:** the mocks are shown; nothing is built until the human says yes. The yes
  sets the issue's Design field to `validated`.
- **Headless:** the run pushes the mocks on a `design/123` branch, posts them and the note
  on the issue with `needs-owner` (§ 2), and moves on to the next Queued item; the human
  validates in the thread.

The mocks show invented data only and never reach `main`: the issue holds them, the
visual proofs of the pull request are the record.

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

| Check                     | Where                                                                                         | Gate                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------------- | --------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Lint, format, type-check  | `scripts/check.py`, Quality                                                                   | any error                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Architecture rules A1–A11 | `scripts/arch-check.py`, Quality                                                              | any violation; frozen debt may only shrink (`arch-allowlist.json`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Unit and integration      | Vitest, cargo test, Quality                                                                   | any failure                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Windows build             | Quality `windows`, a pull request that touches `src-tauri/`                                   | the application does not compile for Windows, or a test of the agent connection fails there                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Coverage floors           | `scripts/coverage-gate.py`, Quality                                                           | frontend features or backend logic under its floor (`coverage-gates.json`, a ratchet: § 3, step 10); the backend report comes from `cargo llvm-cov` with inline test modules stripped (`scripts/coverage-strip-tests.py`)                                                                                                                                                                                                                                                                                                                                                                                                   |
| Golden portfolio          | `src-tauri/tests/golden_portfolio.rs`, Quality                                                | any drift in a pinned figure (`tests/golden/expected.json`); a change that moves one names it in its issue's Done when and regenerates with `GOLDEN_UPDATE=1`                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Sync written form         | `src-tauri/tests/sync_two_devices.rs` (`syn_038_*`) + `scripts/sync-format-check.sh`, Quality | what this build writes for any synced record, enum, folder header, manifest or segment differs from `tests/sync_format/v{DATA_FORMAT_VERSION}.json`, or a diff edits a snapshot already on main — a new written form arrives as a bumped version with its own snapshot (SYN-038)                                                                                                                                                                                                                                                                                                                                            |
| Contracts and bindings    | `scripts/contract-check.py` + `contract-gaps.json`, Quality                                   | a command of `src/bindings.ts` without a row in a contract, a row without a command, arguments, a return type, a struct or an enum that are not the generated ones, or a row promising an error code its command's error type cannot carry; the gaps known when the check arrived are listed and the list only shrinks. Which codes a command returns is not in the bindings: that half is read (a hand amendment per `docs/contracts/README.md`, then `contract-reviewer`)                                                                                                                                                 |
| Dependency licences       | `scripts/licence-check.py` + `licence-allowlist.json`, Quality                                | a crate reached without a dev-dependency, or an npm package not flagged dev, carries a licence outside the allow-list and is not accepted there by name under that exact licence; an exception nothing uses any more fails too                                                                                                                                                                                                                                                                                                                                                                                              |
| E2E on the real app       | `.github/workflows/e2e.yml`, every PR (records-only PRs skip it, `scripts/e2e-scope.py`)      | any failure; screenshots linked                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Reviewer prompts          | `.github/workflows/review.yml`, every PR                                                      | any 🔴 in a lane the diff touches; the report is a sticky PR comment; fails closed without the `CLAUDE_CODE_OAUTH_TOKEN` subscription secret                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Visual regression         | `.github/workflows/e2e.yml`, every PR                                                         | a screen differing from main's last green run beyond 0.3 % of pixels while the PR touched no frontend file; expected differences are listed in the PR comment                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Commit hygiene            | Quality `pr-checks`, and `main-commits` on a push to main (`scripts/commit-check.sh`)         | title > 72, wrong type, trailer, an author other than the owner                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Merge guard               | `scripts/merge.py`, `required-checks.json`                                                    | `just merge` refuses unless the branch is the head of an open pull request with every check green and every required check present; a rebase that moves the commits pushes them and stops until CI has run on them, unless the rebase left the tested tree as it was, record files (lessons, plans, ADRs) aside, so a branch stacked on merged commits lands without one; the rebase folds `fixup!` commits into the commits they name, so a task lands as one commit whatever was pushed after review; GitHub merges the tested head, squashed under the folded commit when there were `fixup!` commits, rebased otherwise |
| Security audit            | `security-audit.yml`                                                                          | new advisory                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Mutation sweep            | `.github/workflows/mutants.yml`, monthly                                                      | none; cargo-mutants on the logic code (`src-tauri/.cargo/mutants.toml`), survivors listed in the sticky issue "Mutation sweep — surviving mutants", filed as `task` issues, `mutants.out` kept 90 days                                                                                                                                                                                                                                                                                                                                                                                                                      |

The `main-protection` ruleset requires the same checks of every update of `main`: `just merge`
and `just release` land through a pull request GitHub merges, and neither pushes to `main`.

**The usage log.** The audit after a batch reads `logs/usage.log`: one line per run of
a script or a recipe (time, tool, duration, result), written by `scripts/usage_log.py`.
The file is local, never committed, capped at 5 000 lines, and emptied once its figures
are in the batch's audit. A new Python or shell script logs itself
(`usage_log.start()` first thing in its `__main__` block, `. scripts/usage-log.sh` after
its `set` line); a recipe that runs no script depends on `(_used "<recipe>")`. Nothing
is logged in CI, nor by the script tests (`USAGE_LOG=off`). A script that only CI
runs does not log. A script that runs another logs both (`harness.sh` and each check it
calls), and a worktree keeps its own file: the audit reads the main checkout's. Skills and agents are
counted from the session's transcript.

**Heavy runs.** A mutation sweep runs in CI only (`mutants.yml`): the local machine cannot carry it beside the hooks and other builds. A background job is stopped by its task or process id, never by name.

## 6. The right way to code

- **Logic in Rust, dumb frontend** (`docs/frontend-rules.md` F32, guarded by A12).
- **Gold layouts** for new code (`docs/backend-rules.md` B0/B37–B43,
  `docs/frontend-rules.md` F0/F26–F28); bit-by-bit for existing code (§ 10).
- **Typed errors** on the wire (`docs/error-model.md`); factories and aggregate-root
  methods on domain objects (`docs/backend-rules.md` B7, B44).
- **Stable ids** on every interactive element; text from i18n; one event per action on
  the bus.
- **Ubiquitous language** in every identifier (`docs/ubiquitous-language.md`).
- **Surgical**: touch the file set the task needs. Boyscout inside it, never beyond.

- **Read the gold first.** Before changing a pattern that has a named reference (B0, F0,
  the error model), open and read it; if it contradicts the design in mind, the gold wins.
- **Check the ADRs before writing "cannot".** Before stating that something cannot be
  done, tested or automated, grep `docs/adr/`: a solved constraint is recorded there.

## 7. Reviewers and the triage policy

Before the PR, run exactly the lanes
`bash scripts/branch.sh files | bash scripts/review-lanes.sh` prints (none for documents
that are not guides, specs or contracts), and again on every fix before its push, until
no 🔴 remains: a file edited in answer to a finding goes back to its lane — a spec above
all, whose text a fix rewrites. CI runs the same code lanes on every push
(`.github/workflows/review.yml`, one check per lane, the report a sticky comment, any 🔴
fails), prompts from `main`; `reviewer-guides` runs locally only. A local reviewer gets
the branch, the spec and the vocabulary, never a description of the change. Also:
`adr-reviewer` when an ADR changes, `spec-checker` before closing an issue with spec
rules. What every reviewer does is in `.claude/agents/review-protocol.md`.

Every finding, local or from CI, is graded before any is applied; one line per finding that
changed something or was rejected goes in the PR body, or in a PR comment once pushed:

- **(a)** in scope → fix in the PR.
- **(b)** bigger, or outside the file set → a `task` issue, linked from that line.
- **(c)** is never "pre-existing" alone: a rejection cites what overrides the finding —
  a rule, a file, an ADR, a measurement, the owner's decision.
- **(c) one-off** false positive → inline comment `<reviewer> FP: <reason> — see PR #NN`.
- **(c) pattern** → a "not a finding" rule in the reviewer's prompt, same PR.
- **A CI finding the local run missed** → a rule in that reviewer's prompt, same PR.
- **`[DECISION]`** → see Conventions below.
- **A 🔴 security finding on the agent's own change** is fixed, or put to the human with
  its options. The agent never clears it with a "not a finding" rule it writes itself:
  that would soften the gate that grades its own work.
- **A 🔴 security finding that already exists on `main`**, outside the task: shown to the
  human with that context and three options — file it as a `task` issue and ship the scoped
  change (the default), widen this pull request, or block. Never deferred silently. A
  finding the pull request introduces blocks.

The CI reviewers run on the subscription the chat session uses. A lane stopped by its
usage limit says "did not run (usage limit)" in its comment and in `just merge`'s
refusal, and fails: an unreviewed pull request does not merge. It is re-run once the
limit has reset (`just rerun-failed <run>`).

## 8. Budget and stop rules

- One task, one run, three hours of wall clock; a wait for the usage limit to reset is
  not the task's time. Over budget → open question "larger than estimated: split?", stop.
- A gate fails → read its log. If the log shows the cause, fix it. If it does not, the
  next push adds observation and makes the failure cheap to reproduce, never a fix.
- The same gate failing three times on one task → open question, stop (a trivial one is
  the agent's, § 3).
- A test fails for a reason the change cannot explain → it is flaky, and is never
  accepted by re-running it: that one test is skipped in its own pull request and a
  `task` issue is filed with the failure text and the run's link; the fix that re-enables
  it closes the issue. The run continues.
- **Never:** what `CLAUDE.md` § Core rules 1 forbids; nor change the batch without the
  human's yes (§ 2), nor edit `.claude/settings.json` (the human edits it; the agent
  commits that edit through a pull request).

## 9. Where the loop runs

The chat session is for filing issues together, design conversations and answering
open questions. The loop runs one task per run and keeps its state in git and the Project.

- **In chat:** `/run-entry #123`, or a plain request; `/run-batch` for a whole batch:
  the questions first, each remaining item through `/run-entry`, the batch's end
  (§ 3), then `/flow-audit`.
- **Laptop:** `just run-entry` (`scripts/run-entry.sh`) — the first Queued item, headless,
  three hours of budget (`RUN_ENTRY_BUDGET`), the log under `logs/run-entry/`. It runs
  `claude -p "/run-entry" --permission-mode acceptEdits`: a command outside the allow list
  in `.claude/settings.json` is denied, so the run fails instead of waiting. The allow
  list scopes the flow, it is not a sandbox; the deny list refuses the usual spellings of
  the absolute rules as a reminder, not a proof. What is enforced is the harness and the
  merge guard.
- **Cloud routine** `run-entry` on claude.ai/code, created disabled; the human switches
  it on.

---

## 10. Gold for new code, bit by bit for existing

Three golds: the backend layout (`backend-rules.md` B0, B37–B43), the frontend layout
(`frontend-rules.md` F0, F28; the remaining crossings frozen in `arch-allowlist.json`)
and the error model (`error-model.md`). New code follows them. Existing code touched by a
task is made conformant in the same PR only when all three hold:

- **Size** — at most about 50 lines of conformance changes.
- **Locality** — inside the files the task already touches.
- **Mechanical** — a rename, an import, a signature or type swap; any design judgement
  (which layer, what name) defers it.

Otherwise the touched code keeps its current standard; a mixed codebase is fine, a
half-migrated file is not. The deciding test is **two stories**: if a reviewer would
read the PR as the task plus a migration, the migration is a separate issue.

## 11. PR size and split

One story per pull request. The target is about 400 hand-written lines per reviewer
lane; above about 800 in a lane, split. Generated files, lockfiles, screenshots and spec
documents are not counted, and the total has no limit: a mechanical change that one
check proves is one story whatever its size. A feature whose backend or frontend exceeds
about 20 files or 500 lines ships as one PR per layer, in order: spec, contract,
migration, backend and bindings; then the frontend; then E2E and closure. Each is
mergeable on its own.

While a pull request waits for its checks, the next task may be prepared on a branch
not pushed yet, so pull requests that change the same file do not conflict: its harness
and reviewers run there, the reviewers given its last commit only, and it is rebased
onto `main` before its first push — a local rebase, never a force-push.

## 12. The flow audit

After a batch ends (§ 3), `/flow-audit` audits it once, as a comment on that batch's
`batch-audit` issue: quality first, weighed against speed and effort. Nothing of it runs
during a batch.

- **An auditor of its own** — the skill runs forked, with a fresh context, and judges
  from the figures, the batch's pull requests and the issues filed and closed during it —
  never the session agent's account. What it cannot rebuild from those records is a
  finding; the questions put to the owner are the dated answers in the issue threads.
- **Measures** — `just flow-audit` counts every figure beside the batch before: the
  pull requests, CI and its rounds, the usage log. The auditor counts from the pull
  requests what the reviewers caught, got wrong and left to CI, and reads each generic
  piece of `docs/agentic-setup.md` against its line.
- **Six questions**, asked of every batch: what would have avoided a conflict; where it
  waited, and what would shorten it without costing quality; what can be simplified;
  which script, recipe, skill or agent went unused (keep, fold or remove); where a guide
  was false, contradicted another, or grew without earning it; what was done by hand that
  a tool should do, and which finding belongs to another repository.
- **Hard points** — every difficulty met becomes a `flow` issue with its evidence
  and a proposal; where nothing should change, "keep" is a verdict said in the audit,
  and no issue is filed.

A flow issue (its form) carries `Observed` with its evidence — a count, a duration, a
pull request — then a `Proposal` with what it `Costs` and what it protects. A change that
buys speed says what it risks; one that buys safety says what it costs. A tool nobody
used is named, with a verdict: keep, fold or remove. The human decides each in its
thread, or has banked it (§ 3); the agent copies the decision, dated, into its Decision
field, and it can be queued. One done, or whose verdict is to keep things as they are, is
closed.

## Conventions

### Spec rule numbering (TRIGRAM-NNN)

The scheme and the registered trigrams live in [`spec/README.md`](spec/README.md).

### `[DECISION]` criticals

A reviewer critical tagged `[DECISION]` needs an architectural choice, not a mechanical
fix. It is never fixed unilaterally: it becomes a `needs-owner` question on the issue, or goes
to the human in chat, and the PR stays open. The choice made is recorded before the fix: a
technical one as an ADR (`docs/adr/README.md`), a domain one in the spec or the glossary.

### Reviewer reports

Locally a reviewer's reply is its report; nothing is written. In CI the reviewer saves
its output to `.review/{slug}-{date}-{NN}.md` (`bash scripts/review-path.sh {slug}`) and
the workflow posts it as the lane's sticky pull request comment. `.review/` is gitignored
and safe to delete.

### Tools the flow needs

A tool the flow needs is added for good — a script under `scripts/` with its tests, a
recipe in the `justfile` — never rewritten in a session's scratch folder. Each is
judged again at the audit that follows a batch: keep, fold or remove. The usage log is
§ 5's.

### Shell calls

A headless run has nobody to approve a command, so every shell call is one command the
allow list in `.claude/settings.json` names:

- no `cd … &&`, no `&&`, `||`, `;` or `$(...)` chain, in a prompt as in a call (the
  allow list matches a literal prefix): `git -C <path>` reaches another directory, a
  multi-line edit goes through `python3 - <<'EOF'`, more logic into a script by name;
- a message or a body goes in a file (`gh pr create --body-file`), never on the command
  line;
- calls that depend on each other run one after another, never in one parallel batch:
  the first failure cancels the rest;
- an empty result is read as a result, not probed again with `echo` or `pwd`;
- a command the list lacks is named to the human, who adds it; the agent does not.

### Release sweep

Before a batch ends (§ 3), run in `release-sweep` mode the lanes
`git diff --name-only <tag>...HEAD | bash scripts/review-lanes.sh` prints, `<tag>` the last
batch end (`just batch-ends`), plus `reviewer-security`, and
`spec-checker` on every spec the batch touched (`bash scripts/batch-specs.sh`); `/dep-audit`
runs at a release's hand-over, on the final `main` (§ 3). `reviewer-security` reads the whole
command surface; every other lane reads what changed since the last batch end's tag
(`just batch-ends`); each lane's `## Files read` goes into the hand-over, or into the closing brief of a batch
that ends with `just batch-end`. What
they find is filed as `task` issues, not fixed in the batch, but for § 3's user-risk case;
one not critical and older than the batch is filed and reported, not asked (§ 7 keeps a
🔴 security finding for the owner).
