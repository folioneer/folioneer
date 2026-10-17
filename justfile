# Folioneer — Command Runner
# Install just: https://github.com/casey/just

# A recipe's arguments reach its commands as "$@": a path with a space stays one argument and
# nothing in an argument is read by the shell.
set positional-arguments

# List all available commands
default:
    @just --list

# Install all dependencies
install: (_used "install")
    npm install

# Start the application with hot reload
dev *ARGS:
    ./scripts/start-app.sh {{ARGS}}

# Seed the development data folder with a copy of the installed application's database (read-only on the source)
dev-seed *ARGS:
    python3 scripts/dev-seed.py {{ARGS}}

# Regenerate Specta TypeScript bindings (run after adding or changing Tauri commands)
generate-types: (_used "generate-types")
    cd src-tauri && cargo run --bin generate_bindings

# Run frontend tests; pass paths or a vitest filter to run a part (just test src/features/settings)
test *ARGS: (_used "test")
    npm test -- "$@"

# Run backend tests; pass a test-name filter or cargo test flags to run a part (just test-rust div_040,
# just test-rust --lib sync). Two build jobs unless CARGO_BUILD_JOBS (or KIT_CHECK_JOBS, the check
# script's own knob) says otherwise: a full parallel build overloads a small machine.
test-rust *ARGS: (_used "test-rust")
    cd src-tauri && CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-${KIT_CHECK_JOBS:-2}}" cargo test "$@"

# Run frontend tests with lcov coverage (output: coverage/frontend/lcov.info)
coverage-fe: (_used "coverage-fe")
    npm run test:coverage

# Run backend tests with coverage (output: coverage/backend/lcov.info); requires: cargo install cargo-llvm-cov
coverage-be: (_used "coverage-be")
    mkdir -p coverage/backend && cd src-tauri && cargo llvm-cov --lib --tests --lcov --output-path ../coverage/backend/lcov.info --ignore-filename-regex '(^|/)build\.rs$|dev/generate_bindings\.rs$|/src-tauri/tests/'
    python3 scripts/coverage-strip-tests.py coverage/backend/lcov.info

# Run unit tests only (excludes E2E, which is `just e2e`, and coverage; see coverage-fe/coverage-be)
test-unit: test test-rust

# Run the E2E suite under xvfb, as CI does; pass spec files to run some (just e2e e2e/split/split.test.ts)
e2e *SPECS:
    bash scripts/e2e.sh "$@"

# Check the coverage reports against the floors in coverage-gates.json (run coverage-fe / coverage-be first); pass --frontend or --backend for one layer
coverage-gate *ARGS:
    python3 scripts/coverage-gate.py {{ARGS}}

# Refuse a shipped dependency under a licence outside licence-allowlist.json
licence-check:
    python3 scripts/licence-check.py

# Unit tests of the repository's own scripts; pass test files to run some
# (just test-scripts scripts/tests/test_mcpb_bundle.py)
test-scripts *PATHS: (_used "test-scripts")
    if [ "$#" -eq 0 ]; then USAGE_LOG=off python3 -m unittest discover -s scripts/tests -p "test_*.py"; else USAGE_LOG=off python3 -m unittest "$@"; fi

# Check the mechanical architecture rules (scripts/arch-check.py); pass --write-allowlist to shrink arch-allowlist.json to today's state
arch-check *ARGS:
    python3 scripts/arch-check.py {{ARGS}}

# Rule homes and the agentic setup: every rule ID defined once, every skill, agent prompt, script and hook with its line in docs/agentic-setup.md
rule-homes:
    python3 scripts/rule-homes.py

# The merge gate, locally, scoped to what moved (scripts/harness.sh): architecture rules always; lint, build and tests for the layers the diff touches; --coverage adds coverage and its floors
harness *ARGS:
    bash scripts/harness.sh {{ARGS}}

# One line in the usage log (logs/usage.log) for a recipe that runs no script of its own; scripts log themselves
_used RECIPE:
    @python3 scripts/usage_log.py used {{RECIPE}} || true

# Run a recipe and print one line: its verdict by exit code, with the whole output kept in
# tmp/<recipe>.log and its end shown on failure (just quiet harness, just quiet test src/lib).
# Not for a recipe that asks or stays up (release, merge, dev): it would read nothing.
quiet RECIPE *ARGS:
    @bash scripts/verdict.sh "$1" just "$@"

# ---- shared recipes ----------------------------------------------------
# Run fast quality check (lint/format only, no tests); pass --frontend or --backend for one layer
check *ARGS:
    @[ -f scripts/check.py ] || { echo "❌ scripts/check.py not found — restore it from git history"; exit 1; }
    python3 scripts/check.py --fast "$@"

# Run full quality check (tests + build + lint)
check-full:
    @[ -f scripts/check.py ] || { echo "❌ scripts/check.py not found — restore it from git history"; exit 1; }
    python3 scripts/check.py

# What a release validates before it versions anything — every test, the build, the full query check — run at the hand-over of a batch (docs/workflow.md § 3)
release-check:
    @[ -f scripts/check.py ] || { echo "❌ scripts/check.py not found — restore it from git history"; exit 1; }
    python3 scripts/check.py --release

# Release new version (interactive)
release *ARGS:
    @[ -f scripts/release.py ] || { echo "❌ scripts/release.py not found — restore it from git history"; exit 1; }
    python3 scripts/release.py {{ARGS}}

# Count lines of code per language (cloc)
stat: (_used "stat")
    cloc . --vcs=git

# Refuses with a specific diagnostic + recovery command if FF is not safe
# (squash/rebase merge on GitHub, divergence, dirty tree, etc.).
# Rebase, fast-forward merge the current branch into main, push, delete the branch
merge:
    @[ -f scripts/merge.py ] || { echo "❌ scripts/merge.py not found — restore it from git history"; exit 1; }
    python3 scripts/merge.py

# Where the queue stands, as one JSON document (/plan-queue proposes the order); `just queue close TODO-NNN` deletes a shipped entry's file;
# `just queue next-id DEBT` prints the next free id; `just queue remaining` the queued references not shipped yet
queue *ARGS:
    @python3 scripts/work-queue.py "$@"

# Wait for the checks of a pull request (the current branch's, or a number) and end with the verdict: 0 green, 1 failed, 2 GitHub unreachable, 3 timed out, 4 not open
watch-pr *ARGS:
    @python3 scripts/watch-pr.py "$@"

# Run the first ready entry of the queue (docs/work/queue.md) headless (docs/workflow.md § 9); logs under logs/run-entry/
run-entry:
    @[ -f scripts/run-entry.sh ] || { echo "❌ scripts/run-entry.sh not found — restore it from git history"; exit 1; }
    bash scripts/run-entry.sh

# SQLX_OFFLINE=false forces online mode so `prepare` hits the dev DB even
# though .cargo/config.toml sets SQLX_OFFLINE=true globally.
# Regenerate the SQLx offline query cache (run after schema or query changes): brings the
# check database up to the latest migration, then prepares against it
prepare-sqlx: (_used "prepare-sqlx")
    @if [ -d src-tauri ]; then cd src-tauri && mkdir -p .local && DATABASE_URL="sqlite:.local/dev_check.sqlite" sqlx database create && DATABASE_URL="sqlite:.local/dev_check.sqlite" sqlx migrate run; fi
    @if [ -d src-tauri ]; then cd src-tauri && SQLX_OFFLINE=false DATABASE_URL="sqlite:.local/dev_check.sqlite" cargo sqlx prepare -- --tests; else echo "ℹ skipping prepare-sqlx (no src-tauri/)"; fi

# The markdown fixer runs prettier with the same args as check.py's
# _PRETTIER_DOCS_CMD (`--write` mirroring its `--check`), so `just format`
# always satisfies `just check`.
# Auto-fix formatting and linting on both layers
format: (_used "format")
    @if [ -d src-tauri ]; then cd src-tauri && cargo fmt; else echo "ℹ skipping cargo fmt (no src-tauri/)"; fi
    @if [ -d src-tauri ]; then cd src-tauri && cargo clippy --fix --allow-dirty --quiet; else echo "ℹ skipping clippy (no src-tauri/)"; fi
    @if [ -f package.json ]; then npm run format:fix; else echo "ℹ skipping format:fix (no package.json)"; fi
    @if [ -f package.json ]; then npx prettier --write "**/*.md" --ignore-path .gitignore; else echo "ℹ skipping prettier docs (no package.json)"; fi

# Create a git worktree beside this checkout on a new branch off main, sharing node_modules so its git hooks run (usage: just worktree <branch>)
worktree BRANCH: (_used "worktree")
    #!/usr/bin/env bash
    set -euo pipefail
    dir="../$(basename "$PWD")-$(echo "{{BRANCH}}" | tr '/' '-')"
    [ -d node_modules ] || { echo "❌ no node_modules here — run just install first"; exit 1; }
    git worktree add -b "{{BRANCH}}" "$dir" main
    ln -s "$PWD/node_modules" "$dir/node_modules"
    echo "✅ $dir on {{BRANCH}} — remove with: git worktree remove $dir"
