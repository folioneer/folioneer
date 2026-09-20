#!/bin/bash
# Start Folioneer in development mode
# Usage: ./start-app.sh [--reset-db] [--log-level LEVEL]
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RESET_DB=false
LOG_LEVEL="debug"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

show_help() {
  cat <<EOF
Folioneer - Start development server

Usage: $(basename "$0") [OPTIONS]

OPTIONS:
  --reset-db         Reset the development database on startup
  --log-level LEVEL  Set logging level (trace|debug|info|warn|error)
  --help             Show this help

NOTES:
  - GDK_BACKEND=x11 is automatically enabled for screenshot support
  - A development run keeps its data in its own folder (com.folioneer.desktop.dev),
    never the installed application's; `just dev-seed` fills it with a copy

EXAMPLES:
  $(basename "$0") --reset-db --log-level trace
EOF
  exit 0
}

# Parse arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    -h|--help) show_help ;;
    --reset-db) RESET_DB=true; shift ;;
    --log-level)
      [[ $# -lt 2 ]] && { echo "Error: --log-level requires a value"; exit 1; }
      LOG_LEVEL="$2"
      shift 2
      ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

echo -e "${BLUE}🚀 Starting Folioneer${NC}"
[[ "$RESET_DB" == true ]] && echo -e "${YELLOW}⚠️  Development database will be reset${NC}"
echo -e "${BLUE}📝 Log level: $LOG_LEVEL${NC}"

# Check dependencies
for cmd in node cargo; do
  if ! command -v "$cmd" &>/dev/null; then
    echo -e "${RED}❌ $cmd not installed${NC}"
    exit 1
  fi
done
echo -e "${GREEN}✅ Node.js $(node --version)${NC}"
echo -e "${GREEN}✅ Rust $(rustc --version)${NC}"

cd "$PROJECT_ROOT"

# Install npm dependencies if needed
if [[ ! -d "node_modules" ]]; then
  echo "📦 Installing npm dependencies..."
  npm install
fi

echo -e "${BLUE}🔧 Starting server...${NC}"
export RUST_LOG="$LOG_LEVEL"
export GDK_BACKEND="${GDK_BACKEND:-x11}"
export RESET_DATABASE="false"
[[ "$RESET_DB" == true ]] && export RESET_DATABASE="true"

npm run tauri -- dev
