# CLAUDE.md

Guidance for Claude Code in this repository. It is an index: every rule lives in one home — [`docs/README.md`](docs/README.md) maps them — and this file points there. Code map: [`ARCHITECTURE.md`](ARCHITECTURE.md).

This project runs daneel's flow (`flow@daneel`) and app (`app@daneel`); its method is the plugins' `method.md` and its loop `app`'s `loop.md`, and this file and the project's documents hold only what is this project's own — how work moves here: [`docs/workflow.md`](docs/workflow.md). Both come from the marketplace tag pinned in `.claude/settings.json` (`DANEEL_REF` in `review.yml` the same); `<flow>` and `<app>` below are their folders, `~/.claude/plugins/cache/daneel/{flow,app}/<version>/`, each plugin's version in its `.claude-plugin/plugin.json`.

## Setup

After cloning: `git config core.hooksPath .githooks`. The hooks block commits to `main`, check the commit format, reject `Co-Authored-By` lines and run the fast checks for what a commit or push touches; a check script that is missing fails them. Tests, coverage, E2E and the build are CI's job on the pull request.

## Who decides what

- The **owner** confirms a feature or bug issue and approves the batch (`method.md` § 2), validates a **design** before anything the user sees changes, validates the vocabulary, and publishes **releases**.
- The **agent** files the issues of every kind (GitHub, the folioneer Project — no commit touches the backlog), does the task end to end and merges on green. No pull request waits for the owner.
- The **harness** (`just harness` locally, `--coverage` before a first push; the required checks in CI) proves the code.

A question only the owner can answer is never guessed: headless, it goes on the issue with `needs-owner`; in chat, through `AskUserQuestion`, one per call, with the context to answer it cold — before the work, and during it for what that reading could not show (`method.md` § 2). A trivial choice is the agent's, only an ambiguity or a deep choice the owner's; state assumptions and name what is unclear.

What the owner is told rests on what was read. A statement that the code does or does not do something names what was read (the component, the command, the service); a statement about what a system will do next is checked, or given as a guess. A reviewer's or a checker's claim about a screen is read in the component before it is repeated.

## Core rules

1. **Authority follows the task.** Given a task (a chat request, `/app:run-entry`, a queued issue), the agent branches (`{type}/{slug}`, the issue number leading the slug: `feat/412-wrapper-ceilings`) off a fresh `main`, commits, pushes, opens the PR (`Closes #412`) and runs `merge.py` on green without asking. In an open-ended chat, ask once for the task, not per step. Always forbidden: committing or pushing to `main`; force-pushing, except `merge.py` pushing the task's own branch with lease once it folded its fixups or rebased it; bypassing a hook (`--no-verify`); publishing a release; editing a released changelog line; and touching the live portfolio database (`~/.local/share/com.folioneer.desktop/` and `~/.local/share/com.phileggel.vault-compass/` are read-only reference data). The agent cuts a release up to its draft, the `v*` tag included (`method.md` § 5).
2. **Always use `just`** when a recipe exists; never the native command (`cargo build`, `npm install`, `sqlx migrate`).
3. **Every change goes through the harness**: branch → PR → every check green → `merge.py`. Docs-only changes too. The task's Done when is the plan. Anything the user sees changing goes through the design gate first (`/app:design-proposal`, `loop.md` § 2).
4. **Only prescribed agents.** Launch only the agents this file, a skill, `loop.md` or `docs/workflow.md` names. Implementation and review fixes are done by the main agent — never a general-purpose "implementer" or "fix" agent.

## Per-task discipline (in priority order)

1. **Surgical** — touch only the file set the task requires; every PR tells one story.
2. **Gold for new code, bit by bit for existing** — `loop.md` § 6; the golds here: `docs/workflow.md` § The loop here. When in doubt, defer.
3. **Boyscout** — small mechanical fixes inside the files already edited ship in the same PR. Known dead code is removed in the same commit. No transition comments: code and docs describe what is, not what was.
4. **Coverage when a real gap surfaces** — add a focused test; the floors in `coverage-gates.json` only rise.
5. **Challenge reviewer returns** — every finding graded (a)/(b)/(c), the PR comment posted before the merge (`loop.md` § 5).
6. **PR size** — one story per pull request; about 400 hand-written lines per reviewer lane, a split above about 800 (`loop.md` § 7).

## Opening and closing a piece of work

**Opening brief** — four lines written before the first edit (first message in chat; headless, at the top of the PR body once it is opened):

    **Task**     — the issue (ticket #123) or the request, in one line
    **Scope**    — the commit type and the layers (backend / frontend / E2E / docs / CI)
    **Design**   — none, validated, or needed (then the mocks come before anything else)
    **Touching** — the paths, so the reviewer lanes are known before the diff exists

**Closing brief** — the last message (`loop.md` § 1, step 10): (1) what changed for whoever reads this next — for `feat` / `fix`, what a user notices, or "nothing — internal"; otherwise what can now be done or trusted; (2) what the project accumulated — tests added and the coverage or golden figure that moved, or the guarantee a harness change buys and how we know it can fail (you watched it go red). What is still owed becomes a `task` issue.

## Where things are

- **Workflow**: `/app:run-entry` runs one task end to end (`loop.md` § 1); `just run-entry` does it headless. `/flow:plan-batch` proposes the batch; the owner approves it in chat. `/app:run-batch` runs it.
- **Before implementing**, read the rules for the layers touched (`.claude/rules/` brings them in when a matching file is read; a new file triggers nothing, so this list is the fallback) — backend (`backend-rules`, `error-model`, `backend-patterns`), frontend (`frontend-rules`, `i18n-rules`, `visual-proof-rules`), E2E (`e2e-rules`), any test (`test-rules`), commits (`commit-rules`), a spec, contract or ADR (the `README.md` of `docs/spec/`, `docs/contracts/`, `docs/adr/`). When a rule changes, its doc changes in the same PR.
- **Source docs** change in the task's PR: the spec (the core's behaviour only), a surface's description (`docs/surfaces/`), the contract, an ADR only for a hard technical choice (a functional decision goes in the spec or the glossary), `ARCHITECTURE.md` for a new module; a failure worth teaching goes beside the rule or the code it explains.
- **Vocabulary**: `docs/ubiquitous-language.md` — confirmed terms in code, specs, comments and logs; never extend a discrepant one. Give it to every reviewer you launch, with the branch and the spec — never a description of the change.
- **Skills, agents, scripts and hooks**: the plugins' are in each plugin's `README.md`; what this project adds, [`docs/agentic-setup.md`](docs/agentic-setup.md) says what it is for and what it must never do; a new one gets its line there in the same PR. The plugins read this project's part from `.claude/daneel.json`.
- **Agents**: the reviewer lanes `bash <app>/scripts/branch.sh files | bash <app>/scripts/review-lanes.sh` prints, again on the fixes until no 🔴 (`loop.md` § 5); `app:adr-reviewer` when an ADR changes; `app:spec-checker` once when the code of a branch that touches or names a spec is finished; before every batch end, the release sweep (`loop.md` § Release sweep) with `app:spec-checker` on every spec the batch touched (`docs/workflow.md` § Ending a batch).
- **Task tracking**: `TaskCreate` / `TaskUpdate` for any task of more than one file or step; under `/app:run-batch`, the batch's list only.
- **Plans** (asked in chat): exact paths, functions and components per layer, gold work with its size, the tests for each clause. Once the owner says go, the plan is the authority for the batch.

## Commands

- `just dev` (a debug build uses its own data folder and never touches the daily fetch; `--reset-db` resets only that database) · `just dev-seed [--replace]` (a detached copy of the installed database).
- `just test [paths]` · `just test-rust [filter]` (two build jobs) · `just test-scripts [files]`. `just e2e [specs]` (under xvfb, as CI runs it; by hand — the harness never runs it, CI stays its gate).
- `just quiet <recipe> [args]` (one line: the verdict by exit code; the output in `tmp/<recipe>.log`) · `just harness [--coverage]` (the coverage run holds the foreground; without a terminal, `HARNESS_FOREGROUND=1`) · `just check [--frontend|--backend]` · `just release-check` · `just format` · `just arch-check` (`--write-allowlist` only lowers the frozen debt) · `just rule-homes` · `just coverage-gate` · `just generate-types`.
- In `<app>/skills/run-entry/scripts/`: `watch-pr.py [number]` (waits for a pull request's checks; the verdict by exit code) · `merge.py` (refuses until every check is green and every CI 🔴 and 🟡 graded; folds `fixup!` commits). In `<flow>/skills/flow-audit/scripts/`: `flow-audit.py` (a batch's figures).
- `just install` · `just stat` · `just worktree <branch>`. A recipe exists because CI, a hook, a skill, an agent or a script calls it, or because it is listed here; otherwise it goes.
- Ending a batch: `method.md` § 5 and `docs/workflow.md` § Ending a batch — `/app:run-batch` cuts the release to its draft (`release.py`) or tags `batch/<date>` (`batch-end.sh`); the owner publishes the draft with `just publish-release`, which checks its tag and its release run first; then `/flow:flow-audit`.

## Standards

- **Nothing about credentials in public text** — PR bodies, issues, comments and commit messages never mention secrets, tokens, keys or their handling.
- **Concise by default** — each fact once, in the fewest words that keep it verifiable. A finding is one line: location, claim, fix. A PR body is `loop.md` § 1, step 8.
- **Where a rule lives** — a rule about how we work is written in the repository (a `flow` issue first, then its home: this file, `docs/workflow.md` or a rules document; a rule of the method or the loop, an issue on daneel). The decisions of one discussion are recorded together, in one pull request.
- **Commits** — `docs/commit-rules.md`: conventional types; `feat` / `fix` titles are user-facing changelog lines (≤ 100 characters after `: `, an optional scope from its list); one task, one commit; fixes after a push are `--fixup` commits.
- **Visual proof** — any `.tsx` / `.css` change carries screenshots (`/app:visual-proof`, `docs/visual-proof-rules.md`); a change with none states "No visual impact" and points to a screen that consumes it.
