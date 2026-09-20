#!/bin/bash
# Collect logs from app directory to project logs folder
# Usage: ./collect-logs.sh [--dry-run]
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DRY_RUN=false

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

show_help() {
  cat <<EOF
Collect logs from app directory

Usage: $(basename "$0") [OPTIONS]

OPTIONS:
  --dry-run    Preview files without copying
  --help       Show this help

EXAMPLES:
  $(basename "$0")          # Copy logs to project logs/ folder
  $(basename "$0") --dry-run  # Preview files that would be copied
EOF
  exit 0
}

# Parse arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    -h|--help) show_help ;;
    --dry-run) DRY_RUN=true; shift ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

# Determine app log directory
case "$OSTYPE" in
  linux-gnu*) APP_LOG_DIR="$HOME/.local/share/com.folioneer.desktop/logs" ;;
  darwin*)    APP_LOG_DIR="$HOME/Library/Logs/Folioneer" ;;
  *)          echo -e "${RED}❌ Unsupported OS: $OSTYPE${NC}"; exit 1 ;;
esac

PROJECT_LOG_DIR="$PROJECT_ROOT/logs"

# Check if source exists
if [[ ! -d "$APP_LOG_DIR" ]]; then
  echo -e "${YELLOW}⚠️  Log directory not found: $APP_LOG_DIR${NC}"
  exit 0
fi

# Show operation
echo -e "${BLUE}📋 Collecting logs${NC}"
echo "From: $APP_LOG_DIR"
echo "To:   $PROJECT_LOG_DIR"

if [[ "$DRY_RUN" == true ]]; then
  echo -e "${YELLOW}[DRY RUN]${NC} Files:"
  find "$APP_LOG_DIR" -type f -exec basename {} \; 2>/dev/null | sed 's/^/  → /' || true
else
  mkdir -p "$PROJECT_LOG_DIR"
  mv -v "$APP_LOG_DIR"/* "$PROJECT_LOG_DIR/" 2>/dev/null || true
  echo -e "${GREEN}✅ Done${NC}"
fi

echo ""
ls -lh "$PROJECT_LOG_DIR" 2>/dev/null || echo "No files"
