#!/usr/bin/env bash
# e2e.sh — the E2E suite under xvfb, the one way CI (`npm run test:e2e:ci`) and `just e2e`
# start it. Given spec files, it runs those only.
#
# Use: just e2e [spec…]     (or: bash scripts/e2e.sh [spec…])
#
# wdio is called directly: through `npm run`, a --spec reaches npm and not the runner.
set -uo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/usage-log.sh"
cd "$(dirname "${BASH_SOURCE[0]}")/.."

args=()
for spec in "$@"; do
    args+=(--spec "$spec")
done
env -u WAYLAND_DISPLAY GDK_BACKEND=x11 dbus-run-session -- \
    xvfb-run --auto-servernum npx wdio run wdio.conf.ts "${args[@]}"
