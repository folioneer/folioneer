#!/usr/bin/env bash
# Generate TypeScript types from Rust domain models
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "Generating TypeScript bindings..."
cd "$PROJECT_ROOT/src-tauri" && SQLX_OFFLINE=true cargo run --bin generate_bindings
