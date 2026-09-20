#!/usr/bin/env bash
# from-vaultcompass.sh — carry a VaultCompass installation's data over to Folioneer (Linux).
#
# One-off, run once per computer with the application closed (todo #033). Folioneer keeps
# its data under a new identifier, so a fresh Folioneer opens empty until this has run.
#
# It copies, never moves: the database (with its write-ahead log), the interface's saved
# preferences and the window position go to the new folders; logs and caches stay behind.
# The old folder is left exactly as it is, as a backup to delete by hand later. An existing
# Folioneer database is never overwritten. The scheduled download registered under the old
# name is removed; Folioneer registers its own at its next start.
#
# Use: bash scripts/migration/from-vaultcompass.sh
#      [--data-home DIR] [--config-home DIR] [--no-systemctl] [--no-running-check]   (for tests)
set -euo pipefail

OLD_ID="com.phileggel.vault-compass"
NEW_ID="com.folioneer.desktop"
OLD_UNIT="vaultcompass-fetch"

data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
config_home="${XDG_CONFIG_HOME:-$HOME/.config}"
use_systemctl=1
check_running=1
while [[ $# -gt 0 ]]; do
    case "$1" in
        --data-home) data_home="$2"; shift 2 ;;
        --config-home) config_home="$2"; shift 2 ;;
        --no-systemctl) use_systemctl=0; shift ;;
        --no-running-check) check_running=0; shift ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

if [[ -n "${NO_COLOR:-}" ]]; then RED='' GREEN='' BLUE='' NC=''; else RED='\033[0;31m' GREEN='\033[0;32m' BLUE='\033[0;34m' NC='\033[0m'; fi

old_data="$data_home/$OLD_ID"
new_data="$data_home/$NEW_ID"
old_config="$config_home/$OLD_ID"
new_config="$config_home/$NEW_ID"

if [[ ! -f "$old_data/portfolio" ]]; then
    echo -e "${BLUE}ℹ No VaultCompass database at $old_data — nothing to carry over.${NC}"
    exit 0
fi

if [[ -e "$new_data/portfolio" ]]; then
    echo -e "${RED}❌ Folioneer already has a database at $new_data/portfolio — nothing was copied.${NC}" >&2
    echo "   If Folioneer was opened before this script and holds nothing you want, close it," >&2
    echo "   delete $new_data, and run this script again." >&2
    exit 1
fi

if [[ "$check_running" -eq 1 ]] && command -v pgrep >/dev/null 2>&1; then
    for process in tauri-app vault-compass folioneer; do
        if pgrep -x "$process" >/dev/null 2>&1; then
            echo -e "${RED}❌ The application is running ($process). Close it and run this script again.${NC}" >&2
            exit 1
        fi
    done
fi

mkdir -p "$new_data"
# The write-ahead log travels with the database it belongs to; the database file goes last,
# so an interrupted run leaves no database behind and can simply be run again.
for file in portfolio-wal portfolio-shm portfolio; do
    if [[ -f "$old_data/$file" ]]; then
        cp -p "$old_data/$file" "$new_data/$file"
    fi
done
for folder in localstorage storage; do
    if [[ -d "$old_data/$folder" ]]; then
        cp -a "$old_data/$folder" "$new_data/$folder"
    fi
done
if [[ -f "$old_config/.window-state.json" ]]; then
    mkdir -p "$new_config"
    cp -p "$old_config/.window-state.json" "$new_config/.window-state.json"
fi

for file in portfolio portfolio-wal portfolio-shm; do
    if [[ -f "$old_data/$file" ]] && ! cmp -s "$old_data/$file" "$new_data/$file"; then
        echo -e "${RED}❌ $file differs from its copy — was the application running? Delete $new_data and run again.${NC}" >&2
        exit 1
    fi
done

units="$config_home/systemd/user"
if [[ -f "$units/$OLD_UNIT.timer" ]] || [[ -f "$units/$OLD_UNIT.service" ]]; then
    if [[ "$use_systemctl" -eq 1 ]] && command -v systemctl >/dev/null 2>&1; then
        systemctl --user disable --now "$OLD_UNIT.timer" >/dev/null 2>&1 || true
    fi
    rm -f "$units/$OLD_UNIT.timer" "$units/$OLD_UNIT.service"
    if [[ "$use_systemctl" -eq 1 ]] && command -v systemctl >/dev/null 2>&1; then
        systemctl --user daemon-reload >/dev/null 2>&1 || true
    fi
    echo -e "${GREEN}✓ The scheduled download registered as $OLD_UNIT was removed.${NC}"
fi

echo -e "${GREEN}✅ Carried over to $new_data${NC}"
echo "   Left in place, untouched: $old_data (your backup — delete it once Folioneer has run for a while)."
echo "   Next: install Folioneer, open it, check your accounts; if the daily price download was on,"
echo "   it registers itself again at that first start (Settings shows it)."
