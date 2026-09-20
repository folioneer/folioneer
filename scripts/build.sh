#!/usr/bin/env bash

# Build production executable for Tauri desktop application
# Usage: ./build.sh
#
# Creates a distributable binary for your operating system.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Colors
BLUE='\033[0;34m'
GREEN='\033[0;32m'
NC='\033[0m'

echo -e "${BLUE}🏗️  Building Folioneer${NC}"
echo ""

cd "$PROJECT_ROOT"

# Install dependencies
echo "📦 Installing dependencies..."
npm install
echo ""

# Build
echo -e "${BLUE}🔨 Building application...${NC}"
npm run tauri -- build

echo ""
echo -e "${GREEN}✅ Build complete!${NC}"
echo ""
echo "📦 Executable location: src-tauri/target/release/bundle/"
echo ""
echo "Distributions:"
echo "  - Windows: src-tauri/target/release/bundle/msi/"
echo "  - macOS: src-tauri/target/release/bundle/dmg/"
echo "  - Linux: src-tauri/target/release/bundle/appimage/"
