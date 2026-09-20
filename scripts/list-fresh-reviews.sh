#!/usr/bin/env bash
# List reviewer reports under .review/ that are newer than the HEAD commit.
# Used by the /review-triage skill to find the current batch (reset boundary
# is the most recent commit).
#
# Usage: bash scripts/list-fresh-reviews.sh
# Output: one path per line, or empty if no fresh reports.
#
# Exit code 0 either way (empty output is a valid signal — clean batch).
set -euo pipefail

if [[ ! -d .review ]]; then
    exit 0
fi

HEAD_TIME=$(git log -1 --format=%ct HEAD 2>/dev/null || echo 0)
TODAY=$(date +%Y-%m-%d)

# Glob covers both naming families:
# - Downstream reviewers: reviewer-{arch,backend,...}-DATE-NN.md (prefix)
# - Kit-internal reviewers: {ai,doc,script}-reviewer-DATE-NN.md (suffix)
find .review -name "*reviewer*-${TODAY}-*.md" -newermt "@${HEAD_TIME}" 2>/dev/null | sort
