#!/usr/bin/env bash
# Swap the YAML `model:` field in the sonnet-pinned agent definitions.
# Usage: scripts/swap-agent-model.sh <from> <to>
#   e.g. scripts/swap-agent-model.sh sonnet opus
#        scripts/swap-agent-model.sh opus sonnet
set -euo pipefail
FROM="${1:?from model required}"
TO="${2:?to model required}"
AGENTS=(
  contract-reviewer reviewer-arch reviewer-backend reviewer-frontend
  reviewer-infra reviewer-security spec-reviewer test-writer-backend
  test-writer-e2e test-writer-frontend
)
cd "$(dirname "$0")/.."
for name in "${AGENTS[@]}"; do
  sed -i "s/^model: ${FROM}$/model: ${TO}/" ".claude/agents/${name}.md"
done
echo "Swapped ${FROM} -> ${TO} on ${#AGENTS[@]} agents."
