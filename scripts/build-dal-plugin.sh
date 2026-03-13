#!/bin/bash
# Build KalidoKitCamera.plugin (CMIO DAL Plugin) for browser-compatible virtual camera.
#
# Usage:
#   ./scripts/build-dal-plugin.sh
#   sudo cp -R target/debug/KalidoKitCamera.plugin /Library/CoreMediaIO/Plug-Ins/DAL/
#
# After deploying, restart the app that should see the camera (e.g., Chrome).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DAL_SRC="$PROJECT_ROOT/crates/virtual-camera/macos-dal"
OUT_DIR="$PROJECT_ROOT/target/debug"

PLUGIN_DIR="$OUT_DIR/KalidoKitCamera.plugin"
CONTENTS="$PLUGIN_DIR/Contents"
MACOS="$CONTENTS/MacOS"

echo "=== Building KalidoKitCamera.plugin (CMIO DAL) ==="

# Clean previous build
rm -rf "$PLUGIN_DIR"
mkdir -p "$MACOS"

# Compile
echo "Compiling DAL plugin..."
clang -fobjc-arc -fmodules \
    -framework CoreMediaIO \
    -framework CoreMedia \
    -framework CoreVideo \
    -framework CoreFoundation \
    -framework IOKit \
    -framework Foundation \
    -bundle \
    -o "$MACOS/KalidoKitCamera" \
    "$DAL_SRC/KalidoKitDALPlugin.m"

# Copy Info.plist
cp "$DAL_SRC/Info.plist" "$CONTENTS/Info.plist"

# Ad-hoc sign
echo "Signing..."
codesign --force --sign - "$PLUGIN_DIR"

echo ""
echo "=== Done ==="
echo "Plugin: $PLUGIN_DIR"
echo ""
echo "To deploy:"
echo "  sudo rm -rf /Library/CoreMediaIO/Plug-Ins/DAL/KalidoKitCamera.plugin"
echo "  sudo cp -R $PLUGIN_DIR /Library/CoreMediaIO/Plug-Ins/DAL/"
echo "  # Then restart Chrome / Safari / target app"
