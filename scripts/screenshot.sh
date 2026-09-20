#!/bin/bash

# Configuration
# 1. Try to get the title from tauri.conf.json (v2)
if [ -f "src-tauri/tauri.conf.json" ]; then
    # Try to extract the productName or the title of the first window
    APP_TITLE=$(grep -oP '"productName":\s*"\K[^"]+' src-tauri/tauri.conf.json)
    if [ -z "$APP_TITLE" ]; then
        APP_TITLE=$(grep -oP '"title":\s*"\K[^"]+' src-tauri/tauri.conf.json | head -1)
    fi
fi

# Fallback to the parameter or "folioneer"
WINDOW_NAME=${1:-${APP_TITLE:-"folioneer"}}
OUTPUT_DIR=".screenshots"
TIMESTAMP=$(date +%Y-%m-%d_%H-%M-%S)
FILENAME="$OUTPUT_DIR/$TIMESTAMP.png"

# 1. Dependency check
if ! command -v xdotool &> /dev/null; then
    echo "❌ Error: 'xdotool' is not installed. Run: sudo apt install xdotool"
    exit 1
fi

if ! command -v import &> /dev/null; then
    echo "❌ Error: 'imagemagick' is not installed. Run: sudo apt install imagemagick"
    exit 1
fi

# 2. Create directory if it doesn't exist
mkdir -p "$OUTPUT_DIR"

# 3. Search for the window
echo "Searching for window: $WINDOW_NAME..."

# Search by exact name, then by class as backup
WID=$(xdotool search --name "$WINDOW_NAME" 2>/dev/null | tail -1)

if [ -z "$WID" ]; then
    WID=$(xdotool search --class "$WINDOW_NAME" 2>/dev/null | tail -1)
fi

# 4. Capture
if [ -n "$WID" ]; then
    echo "✅ Window found (ID: $WID). Capturing..."
    # Brief pause to ensure rendering is ready if called in sequence
    sleep 0.2
    if import -window "$WID" "$FILENAME"; then
        echo "📸 Screenshot saved: $FILENAME"
    else
        echo "❌ Error during capture with ImageMagick."
        exit 1
    fi
else
    echo "❌ Error: Could not find a window with the name or class '$WINDOW_NAME'."
    echo "Make sure the application is running and visible on the desktop."
    exit 1
fi
