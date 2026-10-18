#!/usr/bin/env bash
# review-lanes.sh — the reviewer agents a change needs, and only those.
#
# Reads changed paths on stdin (one per line) and prints one lane per line,
# in this order: backend, frontend, arch, sql, infra, security, e2e, spec,
# contract, guides. Nothing for a change to documents that are not guides,
# specs or contracts. `--json` prints {"backend":true,...}.
#
# The code lanes mirror the map in .github/workflows/review.yml, so the local
# run and CI fire the same reviewers; `guides` runs locally only:
#   backend   src-tauri/**/*.rs                 reviewer-backend
#   frontend  src/**/*.ts(x)                    reviewer-frontend
#   arch      backend or frontend               reviewer-arch
#   sql       src-tauri/migrations/             reviewer-sql
#   infra     scripts, hooks, workflows, justfile, package.json, Cargo.toml,
#             tauri.conf.json, capabilities, the gate files    reviewer-infra
#   security  the IPC boundary, capabilities, the command registry, sync
#                                               reviewer-security
#   e2e       e2e/**/*.test.ts                  reviewer-e2e
#   spec      docs/spec/*.md                    spec-reviewer
#   contract  docs/contracts/*.md               contract-reviewer
#   guides    CLAUDE.md, docs/workflow.md, docs/agentic-setup.md,
#             docs/*-rules.md, agent, skill and path-rule prompts
#                                               reviewer-guides
#
# Use (local, before a PR):  bash scripts/branch.sh files | bash scripts/review-lanes.sh
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/usage-log.sh"

files=$(cat)
has() { grep -Eq "$1" <<<"$files" && echo true || echo false; }

backend=$(has '^src-tauri/.*\.rs$')
frontend=$(has '^src/.*\.(ts|tsx)$')
arch=false
if [[ "$backend" == true || "$frontend" == true ]]; then arch=true; fi
sql=$(has '^src-tauri/migrations/')
infra=$(has '^(scripts/[^/]*\.(sh|bat|py|mjs)$|\.githooks/|\.github/workflows/.*\.ya?ml$|justfile$|package\.json$|src-tauri/Cargo\.toml$|src-tauri/tauri\.conf\.json$|src-tauri/capabilities/.*\.json$|coverage-gates\.json$|arch-allowlist\.json$|contract-gaps\.json$|required-checks\.json$)')
security=$(has '^(src-tauri/src/.*/api\.rs$|src-tauri/capabilities/|src-tauri/src/core/specta_builder\.rs$|src-tauri/src/context/sync/)')
e2e=$(has '^e2e/.*\.test\.ts$')
spec=$(has '^docs/spec/.*\.md$')
contract=$(has '^docs/contracts/.*\.md$')
guides=$(has '^(CLAUDE\.md|docs/(workflow|agentic-setup|[^/]*-rules)\.md|\.claude/rules/[^/]*\.md|\.claude/agents/[^/]*\.md|\.claude/skills/[^/]+/SKILL\.md)$')

if [[ "${1:-}" == "--json" ]]; then
    printf '{"backend":%s,"frontend":%s,"arch":%s,"sql":%s,"infra":%s,"security":%s,"e2e":%s,"spec":%s,"contract":%s,"guides":%s}\n' \
        "$backend" "$frontend" "$arch" "$sql" "$infra" "$security" "$e2e" "$spec" "$contract" "$guides"
    exit 0
fi
for lane in backend frontend arch sql infra security e2e spec contract guides; do
    if [[ "${!lane}" == true ]]; then echo "$lane"; fi
done
