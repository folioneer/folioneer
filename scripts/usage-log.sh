# usage-log.sh — sourced by a script: one line in the usage log when it exits.
#
# Use, after `set -euo pipefail`:   . "$(dirname "${BASH_SOURCE[0]}")/usage-log.sh"
# The line is written by scripts/usage_log.py (time, tool, duration, result); the
# script's own exit code is left as it was.
_usage_log_py="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/usage_log.py"
_usage_log_start=$(date +%s.%N)
# The code is read first: the command substitution below would overwrite `$?`.
trap '_usage_log_code=$?; python3 "$_usage_log_py" record "$(basename "$0")" "$_usage_log_start" "$_usage_log_code" || true' EXIT
