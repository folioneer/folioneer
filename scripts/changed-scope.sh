#!/usr/bin/env bash
# changed-scope.sh — classify a list of changed paths (stdin, one per line)
# into the local check scope they need, so the git hooks run only what a
# change can affect. Prints exactly one word:
#
#   none      nothing a local check covers (workflows, scripts, hooks, justfile)
#   docs      Markdown only (the hooks run prettier on Markdown whenever any is
#             present, whatever the scope)
#   frontend  src/, e2e/, package.json, the TS/Vite/Biome configs, root web files
#   backend   src-tauri/ (except Markdown)
#   both      frontend and backend
#
# Use:
#   git diff --cached --name-only --diff-filter=ACMR | bash scripts/changed-scope.sh
set -euo pipefail

frontend=0
backend=0
docs=0

while IFS= read -r path; do
    [[ -z "$path" ]] && continue
    case "$path" in
        *.md) docs=1 ;;
        scripts/*|.github/*|.githooks/*|docs/*|screenshots/*) ;;
        src-tauri/*) backend=1 ;;
        src/*|e2e/*|public/*|package.json|package-lock.json|index.html|biome.json|\
        tsconfig*.json|vite.config.*|vitest.config.*|wdio.conf.*|*.css|*.ts|*.tsx|*.js|*.mjs)
            frontend=1 ;;
        *) ;;
    esac
done

if [[ $frontend == 1 && $backend == 1 ]]; then echo both
elif [[ $frontend == 1 ]]; then echo frontend
elif [[ $backend == 1 ]]; then echo backend
elif [[ $docs == 1 ]]; then echo docs
else echo none
fi
