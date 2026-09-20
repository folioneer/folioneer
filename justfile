# Portfolio Manager — Command Runner
# Install just: https://github.com/casey/just

# List all available commands
default:
    @just --list

# Install all dependencies
install:
    npm install

# Start the application with hot reload
dev *ARGS:
    ./scripts/start-app.sh {{ARGS}}

# Start the application using Wayland backend (use when window doesn't appear with just dev)
dev-wayland *ARGS:
    GDK_BACKEND=wayland ./scripts/start-app.sh {{ARGS}}

# Seed the development data folder with a copy of the installed application's database (read-only on the source)
dev-seed *ARGS:
    python3 scripts/dev-seed.py {{ARGS}}

# Regenerate Specta TypeScript bindings (run after adding or changing Tauri commands)
generate-types:
    cd src-tauri && cargo run --bin generate_bindings

# Run frontend tests
test:
    npm test

# Run backend tests
test-rust:
    cd src-tauri && cargo test

# Run frontend tests with lcov coverage (output: coverage/frontend/lcov.info)
coverage-fe:
    npm run test:coverage

# Run backend tests with coverage (output: coverage/backend/lcov.info); requires: cargo install cargo-llvm-cov + rustup component add llvm-tools-preview
coverage-be:
    mkdir -p coverage/backend && cd src-tauri && cargo llvm-cov --lib --tests --lcov --output-path ../coverage/backend/lcov.info --ignore-filename-regex '(^|/)build\.rs$|dev/generate_bindings\.rs$|/src-tauri/tests/'
    python3 scripts/coverage-strip-tests.py coverage/backend/lcov.info

# Run E2E tests against the built binary (opens a window)
test-e2e:
    npm run test:e2e

# Run E2E tests headlessly via Xvfb (Linux / CI, no display required)
test-e2e-headless:
    npm run test:e2e:ci

# Run unit tests only (excludes E2E and coverage; see test-e2e and coverage-fe/coverage-be)
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

# Refuse a diff that edits a published sync format snapshot (SYN-038)
sync-format-check *ARGS:
    bash scripts/sync-format-check.sh {{ARGS}}

# Check the mechanical architecture rules (scripts/arch-check.py); pass --write-allowlist to shrink arch-allowlist.json to today's state
arch-check *ARGS:
    python3 scripts/arch-check.py {{ARGS}}

# The merge gate, locally, scoped to what moved (scripts/harness.sh): architecture rules always; lint + build, tests with coverage and the floor only for the layers the diff touches
harness:
    bash scripts/harness.sh

# Resource-capped check-full: runs the full quality suite in a memory-throttled, low-priority
# cgroup so heavy builds stay responsive on low-RAM machines (requires a systemd user session)
check-safe:
    systemd-run --user --scope -p MemoryHigh=4G -p CPUWeight=20 nice -n19 just check-full

# Resource-capped release: same memory guard around the full release flow
release-safe *ARGS:
    systemd-run --user --scope -p MemoryHigh=4G -p CPUWeight=20 nice -n19 just release {{ARGS}}

# Collect logs for debugging
collect-logs:
    ./scripts/collect-logs.sh

# Take a screenshot of the app
screenshot:
    ./scripts/screenshot.sh

# Run linters only
lint:
    npm run lint
    cd src-tauri && cargo clippy -- -D warnings

# Clean build artifacts
clean:
    rm -rf dist src-tauri/target

# ⚠️  Destructive: resets database and restarts app in dev mode
reset-db:
    ./scripts/start-app.sh --reset-db

# Run pending database migrations. Override `URL=...` to target a different DB.
db-migrate URL="sqlite:.local/dev_check.sqlite":
    cd src-tauri && DATABASE_URL={{URL}} sqlx migrate run

# ---- shared recipes ----------------------------------------------------
# Run fast quality check (lint/format only, no tests)
check:
    @[ -f scripts/check.py ] || { echo "❌ scripts/check.py not found — restore it from git history"; exit 1; }
    python3 scripts/check.py --fast

# Run full quality check (tests + build + lint)
check-full:
    @[ -f scripts/check.py ] || { echo "❌ scripts/check.py not found — restore it from git history"; exit 1; }
    python3 scripts/check.py

# Release new version (interactive)
release *ARGS:
    @[ -f scripts/release.py ] || { echo "❌ scripts/release.py not found — restore it from git history"; exit 1; }
    python3 scripts/release.py {{ARGS}}

# ⚠️  Destructive: removes stale remote-tracking branches
clean-branches:
    #!/usr/bin/env bash
    set -euo pipefail
    git fetch --prune
    # Portable equivalent of `xargs -r` (which is GNU-only — BSD/macOS
    # xargs runs the command once with no arguments instead of skipping).
    stale=$(git branch -vv | grep ': gone]' | awk '{print $1}')
    [ -n "$stale" ] && echo "$stale" | xargs git branch -D || true

# Count lines of code per language (cloc)
stat:
    cloc . --vcs=git

# Refuses with a specific diagnostic + recovery command if FF is not safe
# (squash/rebase merge on GitHub, divergence, dirty tree, etc.).
# Rebase, fast-forward merge the current branch into main, push, delete the branch
merge:
    @[ -f scripts/merge.py ] || { echo "❌ scripts/merge.py not found — restore it from git history"; exit 1; }
    python3 scripts/merge.py

# Mutation sweep of the logic code, in place (requires: cargo install cargo-mutants; scope in src-tauri/.cargo/mutants.toml; args pass through, e.g. `-f src/use_cases/fee_generation`; a killed run leaves a mutated file — `git checkout -- src-tauri/src`)
mutants *ARGS:
    cd src-tauri && cargo mutants --in-place {{ARGS}}

# Run one ready entry of docs/todo.md § Next headless (docs/workflow-c.md § 9)
next-todo:
    @[ -f scripts/next-todo.sh ] || { echo "❌ scripts/next-todo.sh not found — restore it from git history"; exit 1; }
    bash scripts/next-todo.sh

# Prerequisites: sqlx must be on $PATH and DATABASE_URL must be set
# Run pending database migrations
migrate:
    @if [ -d src-tauri ]; then cd src-tauri && sqlx migrate run; else echo "ℹ skipping migrate (no src-tauri/)"; fi

# SQLX_OFFLINE=false forces online mode so `prepare` hits the dev DB even
# though .cargo/config.toml sets SQLX_OFFLINE=true globally.
# Regenerate the SQLx offline query cache (run after schema or query changes)
prepare-sqlx:
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

# ⚠️  Destructive: deletes local database and recreates schema
clean-db:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ ! -d src-tauri ]; then
        echo "ℹ skipping clean-db (no src-tauri/)"
        exit 0
    fi
    rm -rf src-tauri/.local/*
    cd src-tauri && sqlx database setup
