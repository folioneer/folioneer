#!/usr/bin/env bash
# coverage-preflight.sh — refuses a coverage run that this machine would kill.
#
# The backend coverage build needs most of 8 GiB. Detached from a terminal on a machine
# with 8 GiB or less, it is stopped for lack of memory partway through, after several
# minutes and without a verdict. `harness.sh --coverage` asks here before it starts.
#
# A run with no terminal that still holds the foreground — an agent's shell waiting on
# it, `just quiet` — says so with HARNESS_FOREGROUND=1.
#
# Use: bash scripts/coverage-preflight.sh   (exit 0: go; exit 3: refused, reason on stderr)
# HARNESS_MEMINFO names the file read instead of /proc/meminfo (the tests' own).
set -euo pipefail

limit_kib=$((8 * 1024 * 1024))
meminfo="${HARNESS_MEMINFO:-/proc/meminfo}"

[ -t 1 ] && exit 0
[ "${HARNESS_FOREGROUND:-}" = 1 ] && exit 0
[ -r "$meminfo" ] || exit 0

total_kib=$(awk '/^MemTotal:/ { print $2; exit }' "$meminfo")
case "$total_kib" in
    '' | *[!0-9]*) exit 0 ;;
esac
[ "$total_kib" -gt "$limit_kib" ] && exit 0

total_gib=$(awk -v kib="$total_kib" 'BEGIN { printf "%.1f", kib / 1024 / 1024 }')
cat >&2 <<MESSAGE
harness --coverage: refused — no terminal is attached and this machine has $total_gib GiB of memory.
Detached, the coverage build is stopped for lack of memory before it gives a verdict.
Run it in the foreground. A foreground run without a terminal (an agent's shell that
waits for it, \`just quiet\`) says so: HARNESS_FOREGROUND=1 just harness --coverage
MESSAGE
exit 3
