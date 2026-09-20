#!/usr/bin/env bash
# harness.sh — the merge gate, locally, scoped to what moved.
#
# The diff against main (committed, staged, unstaged and untracked files) is
# classified by scripts/changed-scope.sh, and only the layers it touches pay
# their lint, build, tests and coverage. Architecture rules, the sync format
# snapshot invariant, the scripts' own unit tests and the dependency licence
# check always run. A
# docs or strings change measures no coverage: nothing it did can move the
# number, and CI measures it the same way.
#
# Use: just harness          (or: bash scripts/harness.sh)
set -euo pipefail

PROJECT_ROOT="$(git rev-parse --show-toplevel)"
cd "$PROJECT_ROOT"

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
bash scripts/sync-format-check.sh "$base"
python3 -m unittest discover -s scripts/tests -p "test_*.py"
python3 scripts/licence-check.py

case "$scope" in
    none|docs)
        echo -e "${GREEN}✅ No code moved — architecture rules only.${NC}"
        ;;
    frontend)
        python3 scripts/check.py --frontend --skip-tests
        just coverage-fe
        just coverage-gate --frontend
        ;;
    backend)
        python3 scripts/check.py --backend --skip-tests
        just coverage-be
        just coverage-gate --backend
        ;;
    both)
        python3 scripts/check.py --skip-tests
        just coverage-fe
        just coverage-be
        just coverage-gate
        ;;
    *)
        echo "unknown scope: $scope" >&2
        exit 1
        ;;
esac
