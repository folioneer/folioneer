# Workflow — folioneer's additions to daneel's method and loop

This document holds only what is folioneer's own, beside daneel's method and loop
(`CLAUDE.md`); where it and the plugins disagree on what they hold, the plugins are the
rule.

## Issues and the Project

- **The Project** is github.com/orgs/folioneer/projects/1 (`plan-batch` in
  `.claude/daneel.json`). An old `TODO-NNN`, `DEBT-NNN` or `FLOW-NNN` is found by
  searching the issues for `Migrated from <id>`. A spec rule is `TRIGRAM-NNN`; the scheme
  and the registered trigrams live in [`spec/README.md`](spec/README.md).
- **A roadmap stage** (MVP, Phase 2, Phase 3) is a milestone.
- **A drifting document** (`loop.md` § 6) is a `task` gated "when next touched", alone
  only when it blocks something.

## The loop here

- **Acceptance tests** (`loop.md` § 1, step 3): Rust for logic, Vitest for rendering, E2E
  for what a user does; the spec is `docs/spec/<feature>.md`, the contract
  `docs/contracts/<domain>-contract.md` (`docs/contracts/README.md`).
- **Self-check**: while working, the recipes take a scope (`just test <paths>`,
  `just test-rust <filter>`, `just check --frontend` or `--backend`); `just harness
--coverage` runs before the first push (`run-entry.recipes.push_harness`).
- **The branch** is named when it is created: renaming the head branch of an open pull
  request closes the pull request.
- **Evidence**: beside `/app:visual-proof`, the E2E run in CI captures the real
  application and links its screenshots from the pull request.
- **CI reviewers**: the code lanes, and `spec` and `contract` on a changed spec or contract
  (`app:spec-reviewer`, `app:contract-reviewer`, in their changed-lines mode); `guides` and
  `adr` run locally only.
- **The gold here**: logic in Rust, the frontend renders (`docs/frontend-rules.md` F32,
  guarded by A12); the backend layout (`backend-rules.md` B0, B37–B43), the frontend
  layout (`frontend-rules.md` F0, F26–F28; the remaining crossings frozen in
  `arch-allowlist.json`) and the error model (`error-model.md`); factories and
  aggregate-root methods (`backend-rules.md` B7, B44); one event per action on the bus.

## Design proposal

The mocks are pushed to `folioneer/folioneer-design` (owner, 2026-10-19;
`design-proposal.repo`) — one folder per issue, straight to its `main`, no pull request,
review or CI — and posted on the issue with each image inline. A new surface has no current
state to capture (owner, 2026-10-24). Once validated, the mocks are the window's
description: no spec restates them (`docs/surfaces/README.md`).

## Ending a batch

- **The release sweep** runs `app:spec-checker` on every spec the batch touched
  (`git diff --name-only <tag>...HEAD -- docs/spec/`).
- **A batch with a `feat` or `fix`** (`/app:run-batch` Step 4): a `just release-check`
  failure is fixed in a pull request of its own before the release; the `v*` tag starts
  `release.yml`, which builds the draft; the owner publishes it
  (`gh release edit vX.Y.Z --draft=false`) once both platforms attached their assets.

## The harness

`just harness` locally; the same set as required checks on every pull request, except
coverage and the E2E suite, which are CI's (`just harness --coverage` measures coverage
locally too). Both run only the layers a change touched (`scripts/changed-scope.sh`): a
docs, strings or CI change pays lint and build, never a coverage run. The full SQLx
check (`cargo sqlx prepare --check`) runs only in CI and for a release: it makes cargo
rebuild the crate, and the local offline build already fails on a query missing from
`.sqlx/`. The git hooks
run only the fast checks for the same scope; a Markdown-only change costs Prettier
and nothing else.

| Check                     | Where                                                                                         | Gate                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------------- | --------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Lint, format, type-check  | `scripts/check.py`, Quality                                                                   | any error                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Architecture rules A1–A12 | `scripts/arch-check.py`, Quality                                                              | any violation; frozen debt may only shrink (`arch-allowlist.json`)                                                                                                                                                                                                                                                                                                                                                                                                          |
| Unit and integration      | Vitest, cargo test, Quality                                                                   | any failure                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Windows build             | Quality `windows`, a pull request that touches `src-tauri/`                                   | the application does not compile for Windows, or a test of the agent connection fails there                                                                                                                                                                                                                                                                                                                                                                                 |
| Coverage floors           | `scripts/coverage-gate.py`, Quality                                                           | frontend features or backend logic under its floor (`coverage-gates.json`, a ratchet: `loop.md` § 1, step 9); the backend report comes from `cargo llvm-cov` with inline test modules stripped (`scripts/coverage-strip-tests.py`)                                                                                                                                                                                                                                          |
| Golden portfolio          | `src-tauri/tests/golden_portfolio.rs`, Quality                                                | any drift in a pinned figure (`tests/golden/expected.json`); a change that moves one names it in its issue's Done when and regenerates with `GOLDEN_UPDATE=1`                                                                                                                                                                                                                                                                                                               |
| Sync written form         | `src-tauri/tests/sync_two_devices.rs` (`syn_038_*`) + `scripts/sync-format-check.sh`, Quality | what this build writes for any synced record, enum, folder header, manifest or segment differs from `tests/sync_format/v{DATA_FORMAT_VERSION}.json`, or a diff edits a snapshot already on main — a new written form arrives as a bumped version with its own snapshot (SYN-038)                                                                                                                                                                                            |
| Contracts and bindings    | `scripts/contract-check.py` + `contract-gaps.json`, Quality                                   | a command of `src/bindings.ts` without a row in a contract, a row without a command, arguments, a return type, a struct or an enum that are not the generated ones, or a row promising an error code its command's error type cannot carry; the gaps known when the check arrived are listed and the list only shrinks. Which codes a command returns is not in the bindings: that half is read (a hand amendment per `docs/contracts/README.md`, then `contract-reviewer`) |
| Dependency licences       | `scripts/licence-check.py` + `licence-allowlist.json`, Quality                                | a crate reached without a dev-dependency, or an npm package not flagged dev, carries a licence outside the allow-list and is not accepted there by name under that exact licence; an exception nothing uses any more fails too                                                                                                                                                                                                                                              |
| E2E on the real app       | `.github/workflows/e2e.yml`, every PR (records-only PRs skip it, `scripts/e2e-scope.py`)      | any failure; screenshots linked                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Reviewer prompts          | `.github/workflows/review.yml`, every PR                                                      | any 🔴 in a lane the diff touches, the plugin's reviewers at `DANEEL_REF`; a lane reviews a pull request once, then passes when every 🔴 and 🟡 of its report is graded; fails closed without the `CLAUDE_CODE_OAUTH_TOKEN` subscription secret                                                                                                                                                                                                                             |
| Visual regression         | `.github/workflows/e2e.yml`, every PR                                                         | a screen differing from main's last green run beyond 0.3 % of pixels while the PR touched no frontend file; expected differences are listed in the PR comment                                                                                                                                                                                                                                                                                                               |
| Commit hygiene            | Quality `pr-checks`, and `main-commits` on a push to main (`scripts/commit-check.sh`)         | description > 100, a scope off the list, wrong type, trailer, an author other than the owner                                                                                                                                                                                                                                                                                                                                                                                |
| Merge guard               | the plugin's `merge.py`, `required-checks.json`                                               | refuses unless every required check is green and every CI 🔴 and 🟡 graded; folds `fixup!` commits, so a task lands as one commit; record files (`run-entry.record_paths`) moving on `main` leave the checks standing                                                                                                                                                                                                                                                       |
| Security audit            | `security-audit.yml`                                                                          | new advisory                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Mutation sweep            | `.github/workflows/mutants.yml`, monthly                                                      | none; cargo-mutants on the logic code (`src-tauri/.cargo/mutants.toml`), survivors listed in the sticky issue "Mutation sweep — surviving mutants", filed as `task` issues, `mutants.out` kept 90 days                                                                                                                                                                                                                                                                      |

The `main-protection` ruleset (a copy of the plugin's `ruleset.json`, nobody bypassing it)
requires the checks of `required-checks.json` of every update of `main`: `merge.py` and
`release.py` land through a pull request GitHub merges, and neither pushes to `main`.

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

**Heavy runs.** A mutation sweep runs in CI only (`mutants.yml`): the local machine cannot carry it beside the hooks and other builds, and an in-place run that dies leaves a mutated source file behind (`git status` first, `git checkout -- <tree>` restores it). A background job is stopped by its task or process id, never by name.

**The infra lane's own checks.** What `app:reviewer-infra` checks for this project,
beside its own checks:

- `contract-gaps.json` is a harness record: an entry appearing or growing 🔴, as for
  `arch-allowlist.json`;
- a recipe needs a caller — CI, a git hook, a skill, a script — or a line in `CLAUDE.md`
  § Commands as a tool people run 🟡; a change that removes a recipe's last caller removes
  the recipe;
- a workflow a required check comes from has a `workflow_dispatch` entry (with the pull
  request's number when it posts to one) 🟡: GitHub drops pull-request events at times, and
  `gh workflow run <name> --ref <branch>` is then the way to fire the runs;
- `prepare-sqlx` names its prerequisite, the `DATABASE_URL` it reads, in its doc comment 🟡;
- the Windows bundle: WiX artifacts (`release/wix/`) cleared before a release build 🟡, and
  an `icon.ico` over 64 KB 🟡 (WiX `light.exe` can fail without a message).

## Where the loop runs

`loop.md` § 8, with: `just run-entry` takes three hours of budget (`RUN_ENTRY_BUDGET`) and
logs under `logs/run-entry/`. The allow list in `.claude/settings.json` scopes the flow, it
is not a sandbox; the deny list refuses the usual spellings of the absolute rules as a
reminder, not a proof. What is enforced is the harness and the merge guard.
