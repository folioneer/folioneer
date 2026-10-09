#!/usr/bin/env bash
# harness.sh — the merge gate, locally, scoped to what moved.
#
# The diff against main (committed, staged, unstaged and untracked files) is
# classified by scripts/changed-scope.sh, and only the layers it touches pay
# their lint, build and tests. Architecture rules, the sync format snapshot
# invariant, the scripts' own unit tests and the dependency licence check always
# run. Coverage is CI's gate; `--coverage` measures it here too, against the
# floors — before a first push, when patch coverage must hold.
#
# Use: just harness [--coverage]   (or: bash scripts/harness.sh [--coverage])
set -euo pipefail

coverage=false
case "$#:${1:-}" in
    0:) ;;
    1:--coverage) coverage=true ;;
    *) echo "usage: harness.sh [--coverage]" >&2; exit 2 ;;
esac

PROJECT_ROOT="$(git rev-parse --show-toplevel)"
cd "$PROJECT_ROOT"

if [ "$coverage" = true ]; then
    bash scripts/coverage-preflight.sh
fi

if [ -n "${NO_COLOR:-}" ]; then BLUE='' GREEN='' NC=''; else BLUE='\033[0;34m' GREEN='\033[0;32m' NC='\033[0m'; fi

for tool in python3 just; do
    command -v "$tool" >/dev/null 2>&1 || { echo "$tool not found" >&2; exit 1; }
done

base=$(git merge-base HEAD origin/main 2>/dev/null || git merge-base HEAD main)
changed=$(
    {
        git diff --name-only --diff-filter=ACMRD "$base" HEAD
        git diff --name-only --diff-filter=ACMRD HEAD
        git ls-files --others --exclude-standard
    } | sort -u
)
scope=$(printf '%s\n' "$changed" | bash scripts/changed-scope.sh)
echo -e "${BLUE}🔍 Harness scope: ${scope}${NC}"

python3 scripts/arch-check.py
python3 scripts/rule-homes.py
python3 scripts/reference-forms.py
bash scripts/sync-format-check.sh "$base"
python3 -m unittest discover -s scripts/tests -p "test_*.py"
python3 scripts/licence-check.py

if [ "$scope" = none ] || [ "$scope" = docs ]; then
    echo -e "${GREEN}✅ No code moved — architecture rules only.${NC}"
    exit 0
fi

case "$scope" in
    frontend) layer=(--frontend) ;;
    backend) layer=(--backend) ;;
    both) layer=() ;;
    *) echo "unknown scope: $scope" >&2; exit 1 ;;
esac

if [ "$coverage" = false ]; then
    python3 scripts/check.py "${layer[@]}"
    exit 0
fi

python3 scripts/check.py "${layer[@]}" --skip-tests
case "$scope" in
    frontend) just coverage-fe; just coverage-gate --frontend ;;
    backend) just coverage-be; just coverage-gate --backend ;;
    both) just coverage-fe; just coverage-be; just coverage-gate ;;
esac
