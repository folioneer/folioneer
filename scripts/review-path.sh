#!/usr/bin/env bash
# Compute and print the next available reviewer report path for a given reviewer slug.
# Usage: bash scripts/review-path.sh <reviewer-slug>
# Output: .review/<slug>-YYYY-MM-DD-NN.md  (NN is zero-padded, auto-incremented)
#
# Reviewer agents (kit/agents/reviewer-*.md) call this before responding so
# their full report is preserved across the sub-agent → main-agent boundary
# (where only the agent's terminal message would otherwise be visible). The
# main agent reads the file(s) when executing /review-triage.
#
# The `.review/` folder is intentionally separate from `tmp/` (used by
# scripts/report-path.sh for one-shot skill reports) — reviewer reports live
# longer because /review-triage may consult them after the fact. Downstream
# projects should gitignore `.review/`.
#
# Concurrency: the script reads-then-prints without locking, so two parallel
# callers can both compute -NN before either writes the file, and the second
# writer will clobber the first. Callers MUST serialize reviewer invocations
# within a batch (the standard reviewer-batch pattern already does this).
set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "Usage: $0 <reviewer-slug>" >&2
    exit 1
fi

SLUG="$1"
mkdir -p .review
DATE=$(date +%Y-%m-%d)

MAX=0
for f in .review/"${SLUG}-${DATE}"-*.md; do
    [[ -e "$f" ]] || continue
    NN="${f##*-}"
    NN="${NN%.md}"
    NN=$((10#$NN))
    ((NN > MAX)) && MAX=$NN
done

printf ".review/%s-%s-%02d.md\n" "$SLUG" "$DATE" $((MAX + 1))
