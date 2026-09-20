#!/usr/bin/env bash
set -euo pipefail

# branch.sh — Branch-base git operations.
#
# Single source of truth for "where did this branch diverge from main" plus
# the operations that need that base: diff, log, and files.
#
# Use:
#   bash scripts/branch.sh base                      # print resolved BASE
#   bash scripts/branch.sh diff <path> [<path>...]   # git diff BASE..HEAD -- paths
#   bash scripts/branch.sh log [git-log-flags]       # git log --oneline BASE..HEAD
#   bash scripts/branch.sh files [filter] [--uncommitted-only]
#
# `files` prints a sort-unique list of paths changed on the branch — the branch
# diff (BASE..HEAD) unioned with uncommitted work (staged, unstaged, untracked)
# — one path per line, no headers. Filters narrow the list:
#   --rust          *.rs
#   --frontend      *.ts / *.tsx, excluding e2e/
#   --arch          *.rs / *.ts / *.tsx, excluding e2e/
#   --e2e           e2e/**/*.test.ts
#   --migrations    src-tauri/migrations/* (or migrations/* at the root)
#   --security      *.rs / *.ts / *.tsx OR capabilities/**/*.json
# `--uncommitted-only` drops the branch-diff source (in-flight files only). A
# filter and --uncommitted-only compose, in either order.
#
# Output: stdout-only. base prints a sha-or-sentinel; diff/log/files forward
# the underlying git output verbatim. Never fails on base resolution — falls
# back through merge-base → rev-parse → "HEAD" so detached HEAD / shallow clone
# / missing-main branches still get a usable BASE.
#
# Exit codes: 0 on success; 2 on a usage error (unknown subcommand/flag, or
# `diff` with no paths). Underlying git failures propagate their own status.
#
# Why a script (vs inline shell in agent prompts):
#   Compound shell ($(...), &&, ||, ;) cannot be safely allowlisted in
#   Claude Code's permission system, which matches by literal prefix. A
#   literal `bash scripts/branch.sh diff foo.ts` call IS allowlistable as
#   `Bash(bash scripts/branch.sh *)`. One entry covers every consumer — and
#   the named `files` filters keep reviewer prompts pipe-free for the same
#   reason (`branch.sh files | grep ...` would prompt on every distinct shape).

resolve_base() {
    git merge-base HEAD main 2>/dev/null ||
        git rev-parse main 2>/dev/null ||
        echo "HEAD"
}

case "${1:-}" in
base)
    resolve_base
    ;;
diff)
    shift
    if [ "$#" -lt 1 ]; then
        echo "usage: bash scripts/branch.sh diff <path> [<path> ...]" >&2
        exit 2
    fi
    BASE=$(resolve_base)
    git diff "$BASE"..HEAD -- "$@"
    ;;
log)
    shift
    BASE=$(resolve_base)
    git log --oneline "$@" "$BASE"..HEAD
    ;;
files)
    shift
    FILTER='.' # match-all default (any non-empty line)
    EXCLUDE=''
    UNCOMMITTED_ONLY=''
    while [ "$#" -gt 0 ]; do
        case "$1" in
        --uncommitted-only) UNCOMMITTED_ONLY=1 ;;
        --rust) FILTER='\.rs$' ;;
        --frontend)
            FILTER='\.(ts|tsx)$'
            EXCLUDE='^e2e/'
            ;;
        --arch)
            FILTER='\.(rs|ts|tsx)$'
            EXCLUDE='^e2e/'
            ;;
        --e2e) FILTER='^e2e/.*\.test\.ts$' ;;
        --migrations) FILTER='^(src-tauri/)?migrations/' ;;
        --security) FILTER='\.(rs|ts|tsx)$|capabilities/.*\.json$' ;;
        *)
            echo "usage: bash scripts/branch.sh files [--rust|--frontend|--arch|--e2e|--migrations|--security] [--uncommitted-only]" >&2
            exit 2
            ;;
        esac
        shift
    done

    BASE=$(resolve_base)
    collect() {
        {
            [ -z "$UNCOMMITTED_ONLY" ] && git diff --name-only "$BASE"..HEAD
            git diff --name-only HEAD
            git diff --name-only --cached
            git ls-files --others --exclude-standard
        } | LC_ALL=C sort -u | grep -v '^$' || true
    }

    if [ -n "$EXCLUDE" ]; then
        collect | grep -E "$FILTER" | grep -v -E "$EXCLUDE" || true
    else
        collect | grep -E "$FILTER" || true
    fi
    ;;
*)
    echo "usage: bash scripts/branch.sh {base|diff <paths>|log [flags]|files [filter] [--uncommitted-only]}" >&2
    exit 2
    ;;
esac
