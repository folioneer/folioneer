#!/usr/bin/env bash
# Compute and print the next available report path for a given agent/skill slug.
# Usage: bash scripts/report-path.sh <slug>
# Output: tmp/<slug>-YYYY-MM-DD-NN.md  (NN is zero-padded, auto-incremented)
set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "Usage: $0 <slug>" >&2
    exit 1
fi

SLUG="$1"
mkdir -p tmp
DATE=$(date +%Y-%m-%d)

MAX=0
for f in tmp/"${SLUG}-${DATE}"-*.md; do
    [[ -e "$f" ]] || continue
    NN="${f##*-}"
    NN="${NN%.md}"
    NN=$((10#$NN))
    ((NN > MAX)) && MAX=$NN
done

printf "tmp/%s-%s-%02d.md\n" "$SLUG" "$DATE" $((MAX + 1))
