# Folioneer — Command Runner
# Install just: https://github.com/casey/just

# A recipe's arguments reach its commands as "$@": a path with a space stays one argument and
# nothing in an argument is read by the shell.
set positional-arguments

# List all available commands
default:
    @just --list

# Install all dependencies
install:
    npm install

# Start the application with hot reload
dev *ARGS:
    ./scripts/start-app.sh {{ARGS}}

# Seed the development data folder with a copy of the installed application's database (read-only on the source)
dev-seed *ARGS:
    python3 scripts/dev-seed.py {{ARGS}}

# Regenerate Specta TypeScript bindings (run after adding or changing Tauri commands)
generate-types:
    cd src-tauri && cargo run --bin generate_bindings

# Run frontend tests; pass paths or a vitest filter to run a part (just test src/features/settings)
test *ARGS:
    npm test -- "$@"

# Run backend tests; pass a test-name filter or cargo test flags to run a part (just test-rust div_040,
# just test-rust --lib sync). Two build jobs unless CARGO_BUILD_JOBS (or KIT_CHECK_JOBS, the check
# script's own knob) says otherwise: a full parallel build overloads a small machine.
test-rust *ARGS:
    cd src-tauri && CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-${KIT_CHECK_JOBS:-2}}" cargo test "$@"

# Run frontend tests with lcov coverage (output: coverage/frontend/lcov.info)
coverage-fe:
    npm run test:coverage

# Run backend tests with coverage (output: coverage/backend/lcov.info); requires: cargo install cargo-llvm-cov
coverage-be:
    mkdir -p coverage/backend && cd src-tauri && cargo llvm-cov --lib --tests --lcov --output-path ../coverage/backend/lcov.info --ignore-filename-regex '(^|/)build\.rs$|dev/generate_bindings\.rs$|/src-tauri/tests/'
    python3 scripts/coverage-strip-tests.py coverage/backend/lcov.info

# Run unit tests only (excludes E2E, which runs in CI, and coverage; see coverage-fe/coverage-be)
test-unit: test test-rust

# Check the coverage reports against the floors in coverage-gates.json (run coverage-fe / coverage-be first); pass --frontend or --backend for one layer
coverage-gate *ARGS:
    python3 scripts/coverage-gate.py {{ARGS}}

# Refuse a shipped dependency under a licence outside licence-allowlist.json
licence-check:
    python3 scripts/licence-check.py

# Unit tests of the repository's own scripts
test-scripts:
    python3 -m unittest discover -s scripts/tests -p "test_*.py"

# Check the mechanical architecture rules (scripts/arch-check.py); pass --write-allowlist to shrink arch-allowlist.json to today's state
arch-check *ARGS:
    python3 scripts/arch-check.py {{ARGS}}

# The merge gate, locally, scoped to what moved (scripts/harness.sh): architecture rules always; lint, build and tests for the layers the diff touches; --coverage adds coverage and its floors
harness *ARGS:
    bash scripts/harness.sh {{ARGS}}

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

# Release new version (interactive)
release *ARGS:
    @[ -f scripts/release.py ] || { echo "❌ scripts/release.py not found — restore it from git history"; exit 1; }
    python3 scripts/release.py {{ARGS}}

# Count lines of code per language (cloc)
stat:
    cloc . --vcs=git

# Refuses with a specific diagnostic + recovery command if FF is not safe
# (squash/rebase merge on GitHub, divergence, dirty tree, etc.).
# Rebase, fast-forward merge the current branch into main, push, delete the branch
merge:
    @[ -f scripts/merge.py ] || { echo "❌ scripts/merge.py not found — restore it from git history"; exit 1; }
    python3 scripts/merge.py

# Run one ready entry of docs/todo.md § Next headless (docs/workflow.md § 9)
next-todo:
    @[ -f scripts/next-todo.sh ] || { echo "❌ scripts/next-todo.sh not found — restore it from git history"; exit 1; }
    bash scripts/next-todo.sh

# SQLX_OFFLINE=false forces online mode so `prepare` hits the dev DB even
# though .cargo/config.toml sets SQLX_OFFLINE=true globally.
# Regenerate the SQLx offline query cache (run after schema or query changes): brings the
# check database up to the latest migration, then prepares against it
prepare-sqlx:
    @if [ -d src-tauri ]; then cd src-tauri && mkdir -p .local && DATABASE_URL="sqlite:.local/dev_check.sqlite" sqlx database create && DATABASE_URL="sqlite:.local/dev_check.sqlite" sqlx migrate run; fi
    @if [ -d src-tauri ]; then cd src-tauri && SQLX_OFFLINE=false DATABASE_URL="sqlite:.local/dev_check.sqlite" cargo sqlx prepare -- --tests; else echo "ℹ skipping prepare-sqlx (no src-tauri/)"; fi

# The markdown fixer runs prettier with the same args as check.py's
# _PRETTIER_DOCS_CMD (`--write` mirroring its `--check`), so `just format`
# always satisfies `just check`.
# Auto-fix formatting and linting on both layers
format:
    @if [ -d src-tauri ]; then cd src-tauri && cargo fmt; else echo "ℹ skipping cargo fmt (no src-tauri/)"; fi
    @if [ -d src-tauri ]; then cd src-tauri && cargo clippy --fix --allow-dirty --quiet; else echo "ℹ skipping clippy (no src-tauri/)"; fi
    @if [ -f package.json ]; then npm run format:fix; else echo "ℹ skipping format:fix (no package.json)"; fi
    @if [ -f package.json ]; then npx prettier --write "**/*.md" --ignore-path .gitignore; else echo "ℹ skipping prettier docs (no package.json)"; fi

# Create a git worktree beside this checkout on a new branch off main, sharing node_modules so its git hooks run (usage: just worktree <branch>)
worktree BRANCH:
    #!/usr/bin/env bash
    set -euo pipefail
    dir="../$(basename "$PWD")-$(echo "{{BRANCH}}" | tr '/' '-')"
    [ -d node_modules ] || { echo "❌ no node_modules here — run just install first"; exit 1; }
    git worktree add -b "{{BRANCH}}" "$dir" main
    ln -s "$PWD/node_modules" "$dir/node_modules"
    echo "✅ $dir on {{BRANCH}} — remove with: git worktree remove $dir"
