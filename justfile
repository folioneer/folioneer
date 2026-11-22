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
# Not for a recipe that asks or stays up (dev, run-entry): it would read nothing.
quiet RECIPE *ARGS:
    @bash scripts/verdict.sh "$1" just "$@"

# Run fast quality check (lint/format only, no tests); pass --frontend or --backend for one layer
check *ARGS:
    python3 scripts/check.py --fast "$@"

# What a release validates before it versions anything — every test, the build, the full query check — run at the hand-over of a batch (docs/workflow.md § Ending a batch)
release-check:
    python3 scripts/check.py --release

# The owner publishes the newest draft release (scripts/publish-release.sh)
publish-release:
    bash scripts/publish-release.sh

# Count lines of code per language (cloc)
stat: (_used "stat")
    cloc . --vcs=git

# One Queued item, headless: one run at a time, a 3-hour budget, logged under logs/run-entry/ (daneel's plugins/app/README.md § Headless runs)
run-entry: (_used "run-entry")
    mkdir -p logs/run-entry && flock -n -E 0 logs/run-entry/.lock timeout --signal=INT --kill-after=60 ${RUN_ENTRY_BUDGET:-3h} claude -p "/app:run-entry" --permission-mode acceptEdits --output-format text >> logs/run-entry/$(date -u +%Y%m%dT%H%M%SZ).log 2>&1

# SQLX_OFFLINE=false forces online mode so `prepare` hits the dev DB even
# though .cargo/config.toml sets SQLX_OFFLINE=true globally.
# Regenerate the SQLx offline query cache (run after schema or query changes): brings the
# check database up to the latest migration, then prepares against it
prepare-sqlx: (_used "prepare-sqlx")
    cd src-tauri && mkdir -p .local && DATABASE_URL="sqlite:.local/dev_check.sqlite" sqlx database create && DATABASE_URL="sqlite:.local/dev_check.sqlite" sqlx migrate run
    cd src-tauri && SQLX_OFFLINE=false DATABASE_URL="sqlite:.local/dev_check.sqlite" cargo sqlx prepare -- --tests

# The markdown fixer runs prettier with the same args as check.py's
# _PRETTIER_DOCS_CMD (`--write` mirroring its `--check`), so `just format`
# always satisfies `just check`.
# Auto-fix formatting and linting on both layers
format: (_used "format")
    cd src-tauri && cargo fmt
    cd src-tauri && cargo clippy --fix --allow-dirty --quiet
    npm run format:fix
    npx prettier --write "**/*.md" --ignore-path .gitignore

# Create a git worktree beside this checkout on a new branch off main, sharing node_modules so its git hooks run (usage: just worktree <branch>)
worktree BRANCH: (_used "worktree")
    #!/usr/bin/env bash
    set -euo pipefail
    dir="../$(basename "$PWD")-$(echo "{{BRANCH}}" | tr '/' '-')"
    [ -d node_modules ] || { echo "❌ no node_modules here — run just install first"; exit 1; }
    git worktree add -b "{{BRANCH}}" "$dir" main
    ln -s "$PWD/node_modules" "$dir/node_modules"
    echo "✅ $dir on {{BRANCH}} — remove with: git worktree remove $dir"
