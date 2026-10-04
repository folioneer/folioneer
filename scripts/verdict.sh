#!/usr/bin/env bash
# verdict.sh — run a command, keep its whole output in a log, print one line.
#
# The verdict is the command's exit code, never a line of its output: a suite can print
# "all tests passed" and still exit 1 (an unhandled error, a file that did not load).
# On failure the end of the log follows the verdict, so the reason is in view.
#
# Use: just quiet <recipe> [args…]     (or: bash scripts/verdict.sh <name> <command> [args…])
# Log: tmp/<name>.log at the repository root, replaced when the run ends.
#
# Not for a command that asks a question or stays up (release, merge, dev): its output is
# hidden and it reads nothing — a prompt fails at once instead of waiting unseen.
#
# No `set -e`: the command's exit code is what this script is for.
set -uo pipefail

if [[ $# -lt 2 ]]; then
    echo "usage: verdict.sh <name> <command> [args…]" >&2
    exit 2
fi

name="$1"
shift
if [[ ! "$name" =~ ^[A-Za-z0-9_-][A-Za-z0-9._-]*$ ]]; then
    echo "verdict.sh: '${name}' cannot name a log (letters, digits, '.', '_' and '-' only)" >&2
    exit 2
fi

root=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
mkdir -p "${root}/tmp"
log="tmp/${name}.log"
partial=""
finish() {
    [[ -n "$partial" ]] && mv -f "$partial" "${root}/${log}"
}
interrupted() {
    finish
    echo "❌ ${name}: interrupted — log: ${log}"
    exit 130
}
trap interrupted INT TERM
# Written aside and moved onto the log at the end: two runs never interleave in one file.
partial=$(mktemp "${root}/tmp/${name}.XXXXXX")

start=$(date +%s)
"$@" >"$partial" 2>&1 </dev/null
code=$?
seconds=$(($(date +%s) - start))
finish

if [[ $code -eq 0 ]]; then
    echo "✅ ${name}: passed in ${seconds}s — log: ${log}"
else
    echo "❌ ${name}: FAILED (exit ${code}) in ${seconds}s — log: ${log}"
    echo "── last lines ──"
    tail -n "${VERDICT_TAIL:-30}" "${root}/${log}"
fi
exit "$code"
