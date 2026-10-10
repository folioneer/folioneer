# CLAUDE.md

Guidance for Claude Code in this repository. It is an index: every rule lives in one home — [`docs/README.md`](docs/README.md) maps them — and this file points there. Code map: [`ARCHITECTURE.md`](ARCHITECTURE.md). How work moves: [`docs/workflow.md`](docs/workflow.md).

## Setup

After cloning: `git config core.hooksPath .githooks`. The hooks block commits to `main`, check the commit format, reject `Co-Authored-By` lines and run the fast checks for what a commit or push touches. Tests, coverage, E2E and the build are CI's job on the pull request.

## Who decides what

- The **human** writes `docs/todo.md` (user value, done-when) and its **Next** queue, validates a **design** before anything the user sees changes, validates the vocabulary, and cuts **releases**.
- The **agent** owns `docs/techdebt.md`, does the task end to end and merges on green. No pull request is validated by a human.
- The **harness** (`just harness` locally, `--coverage` before a first push; required checks in CI) proves the code.

Headless, a question only the human can answer goes into the entry's `**Open questions:**`, never asked. In a chat run (`/next-todo TODO-NNN`) the open questions are asked before anything starts, and a design is validated by a yes; the answers are written into the entry either way. In a chat conversation, ask as you would a colleague: state assumptions, name what is unclear. Whenever the human is asked: one question at a time, each with the context needed to answer it — what the thing is, where it shows, what each answer changes.

## Core rules

1. **Authority follows the entry.** Running a queued entry, a queued `DEBT-NNN`, or a phase the human said "go" to, the agent branches, commits, pushes, opens the PR and runs `just merge` on green without asking. In an open-ended chat, ask once for the task, not per step. Always: never push to `main`, never force-push, never bypass a hook, never cut a release, never touch the live portfolio database (`~/.local/share/com.folioneer.desktop/` and `~/.local/share/com.phileggel.vault-compass/` are read-only reference data).
2. **Always use `just`** when a recipe exists; never the native command (`cargo build`, `npm install`, `sqlx migrate`).
3. **Every change goes through the harness**: branch (`<type>/todo-NNN-slug`, `<type>/debt-NNN-slug`) → PR → every check green → `just merge`. Docs-only changes too. The entry's Done when is the plan. Anything the user sees changing goes through the design gate first (`/design-proposal`).
4. **Only prescribed agents.** Launch only the agents this file, a skill or `docs/workflow.md` names. Implementation and review fixes are done by the main agent — never a general-purpose "implementer" or "fix" agent.

## Per-task discipline (in priority order)

1. **Surgical** — touch only the file set the task requires; every PR tells one story.
2. **Gold for new code, bit by bit for existing** — `docs/workflow.md` § 10. When in doubt, defer.
3. **Boyscout** — small mechanical fixes inside the files already edited ship in the same PR. Known dead code is removed in the same commit (live-vs-dead table in the PR body). No transition comments: code and docs describe what is, not what was.
4. **Coverage when a real gap surfaces** — add a focused test; the floors in `coverage-gates.json` only rise.
5. **Challenge reviewer returns** — every finding graded (`docs/workflow.md` § 7) and recorded in the PR body: (a) in scope → fix; (b) bigger → `DEBT-NNN`; (c) false positive → inline `// <reviewer> FP: <reason> — see PR #NN`, or edit the reviewer prompt when it is a pattern. A `[DECISION]` critical becomes an open question on the entry.
6. **PR size ≤ 1000 lines** as a target; split when a PR crosses it or tells two stories (`docs/workflow.md` § 11).

## Opening and closing a piece of work

**Opening brief** — four lines before the first edit (first message in chat; top of the PR body headless):

    **Task**     — the entry (`TODO-NNN` / `DEBT-NNN`) or the request, in one line
    **Scope**    — the commit type and the layers (backend / frontend / E2E / docs / CI)
    **Design**   — none, validated, or needed (then the mocks come before anything else)
    **Touching** — the paths, so the reviewer lanes are known before the diff exists

**Closing brief** — the last message (and the PR body's first lines), two parts, nothing else; what is still owed goes in `docs/techdebt.md` or an open question:

1. **What changed for whoever reads this next.** For `feat` / `fix`: what a user notices, or "nothing — internal". Otherwise: what can now be done or trusted that could not before.
2. **What the project accumulated.** Tests added and the coverage or golden figure that moved; for harness work, the guarantee and how you know it can fail (you watched it go red).

## Where things are

- **Workflow**: `/next-todo` runs one entry end to end (`docs/workflow.md` § 3); `just next-todo` does it headless.
- **Before implementing**, read the rules for the layers touched (`.claude/rules/` brings the pointer in when a matching file is read; a new file triggers nothing, so this list is the fallback) — backend (`backend-rules`, `error-model`, `backend-patterns`), frontend (`frontend-rules`, `i18n-rules`, `visual-proof-rules`), E2E (`e2e-rules`), any test (`test-rules`), commits (`commit-rules`). When a rule changes, its doc changes in the same PR.
- **After completing**, update the source docs in the same PR: the spec rules (+ `spec-reviewer`), the contract (`/contract` + `contract-reviewer`), an ADR only for a technical choice (`/adr-writer` + `adr-reviewer`), `docs/lessons.md` for an empirical failure worth teaching, `ARCHITECTURE.md` when a top-level folder appears.
- **Vocabulary**: `docs/ubiquitous-language.md` — use confirmed terms in identifiers, comments and logs; never extend a discrepant one; changes need the owner. Give it to every reviewer you launch.
- **Skills**: `/next-todo`, `/design-proposal NNN`, `/visual-proof`, `/adr-writer`, `/contract`, `/dep-audit`, `/prune`, `/whats-next`, `/flow-audit` (after a release).
- **Agents**: the reviewers matched to the diff (locally until no 🔴, and in CI on every push); `reviewer-security` before every release; `spec-checker` before closing an entry with spec rules, and before a release on every spec the batch touched (`bash scripts/batch-specs.sh`); `spec-reviewer` / `contract-reviewer` / `adr-reviewer` when those documents change.
- **References**: `TODO-NNN` (todo), `DEBT-NNN` (tech debt), `FLOW-NNN` (flow), `ghNN` (GitHub issue); a pull request stays `#NN` (`docs/workflow.md` § 2).
- **Task tracking**: `TaskCreate` / `TaskUpdate` for any task of more than one file or step.
- **Plans** (asked in chat): exact paths, functions and components per layer, gold work with its size, the tests for each clause. Once the human says go, the plan is the authority for the batch.

## Commands

- `just dev` (a debug build uses its own data folder and never touches the daily fetch; `--reset-db` resets only that database) · `just dev-seed [--replace]` (a detached copy of the installed database).
- `just test [paths]` · `just test-rust [filter]` (a part of a suite while working; two build jobs) · `just test-unit`. The E2E suite has no recipe: CI runs it (`npm run test:e2e:ci`) and is its gate; the local run is broken on this machine (`docs/lessons.md` L-011).
- `just quiet <recipe> [args]` (one line: the verdict by exit code; the output in `tmp/<recipe>.log`) · `just harness [--coverage]` (the coverage run holds the foreground; without a terminal, `HARNESS_FOREGROUND=1`) · `just check [--frontend|--backend]` · `just check-full` · `just format` · `just arch-check` (`--write-allowlist` only lowers the frozen debt) · `just coverage-gate` · `just generate-types` · `just watch-pr [number]` (waits for a pull request's checks; the verdict by exit code) · `just merge`.
- `just install` · `just stat` · `just worktree <branch>`. A recipe exists because CI, a hook, a skill, an agent or a script calls it, or because it is listed here; otherwise it goes.
- Release (human): `/dep-audit` → `just release [--preview] [-y]`; the workflow builds Windows then Linux and leaves a draft — publish with `gh release edit vX.Y.Z --draft=false` once both attached their assets.

## Standards

- **Nothing about credentials in public text** — PR bodies, comments and commit messages never mention secrets, tokens, keys or their handling.
- **Concise by default** — each fact once, in the fewest words that keep it verifiable. A finding is one line: location, claim, fix. A PR body says what changed and what proves it, under 20 lines; the triage table lists only findings that changed something.
- **Commits** — `docs/commit-rules.md`: conventional types; `feat` / `fix` titles are user-facing changelog lines (≤ 72 characters); one entry, one commit; fixes after a push are `--fixup` commits.
- **Visual proof** — any `.tsx` / `.css` change carries screenshots (`/visual-proof`, `docs/visual-proof-rules.md`); a change with none states "No visual impact" and points to a screen that consumes it.
