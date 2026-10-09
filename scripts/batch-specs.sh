#!/usr/bin/env bash
# batch-specs.sh — the specs the batch touched: every file under docs/spec/ changed since
# the last release tag and still there, one path per line.
#
# Before a release `spec-checker` runs once on each (docs/workflow.md § 3, Release): a
# rule nobody touched is never re-read otherwise, and a spec that promises what the code
# does not do is only found by reading both.
#
# Use: bash scripts/batch-specs.sh [<since>]   (<since> defaults to the last `v*` tag)
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

if [ "$#" -gt 1 ]; then
    echo "usage: batch-specs.sh [<since>]" >&2
    exit 2
fi
since="${1:-}"
if [ -z "$since" ]; then
    since=$(git describe --tags --abbrev=0 --match 'v*' 2>/dev/null) || {
        echo "batch-specs.sh: no release tag to start from; name a commit" >&2
        exit 1
    }
fi

git diff --name-only --diff-filter=ACMR "$since" HEAD -- docs/spec/
