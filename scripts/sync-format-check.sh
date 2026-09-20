#!/usr/bin/env bash
# sync-format-check.sh — a published sync format snapshot is never edited (SYN-038).
#
# src-tauri/tests/sync_format/vN.json pins how data format version N writes what is
# synced; the backend test suite fails when this build writes anything else. That check
# is only worth something if the snapshot of a version already on main cannot be
# rewritten to match: a change to the written form has to arrive as a new version —
# DATA_FORMAT_VERSION bumped, a new vN.json added. This script refuses a diff against
# main that modifies, renames or deletes an existing snapshot.
#
# Use: bash scripts/sync-format-check.sh [base-ref]   (default: merge-base with origin/main)
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

if [ -n "${NO_COLOR:-}" ]; then RED='' GREEN='' NC=''; else RED='\033[0;31m' GREEN='\033[0;32m' NC='\033[0m'; fi

base="${1:-$(git merge-base HEAD origin/main 2>/dev/null || git merge-base HEAD main 2>/dev/null || true)}"
if [ -z "$base" ]; then
    echo "sync-format-check: neither origin/main nor main resolves — pass a base ref" >&2
    exit 1
fi
edited=$(git diff --name-status "$base" -- 'src-tauri/tests/sync_format/v*.json' | { grep -v '^A' || true; })

if [ -n "$edited" ]; then
    echo -e "${RED}❌ A published sync format snapshot was edited:${NC}" >&2
    printf '%s\n' "$edited" >&2
    echo "   Restore it, bump DATA_FORMAT_VERSION (src-tauri/src/context/sync/infrastructure/codec.rs)" >&2
    echo "   and create the new version's snapshot: CREATE_SYNC_FORMAT_SNAPSHOT=1 just test-rust" >&2
    exit 1
fi
echo -e "${GREEN}✅ sync format: no published snapshot edited.${NC}"
