#!/bin/bash
# Build the KalidoKit Camera Extension as a .appex bundle.
#
# The Camera Extension is a System Extension that must be embedded
# inside a .app bundle. This script compiles the Objective-C sources
# and creates the .appex bundle structure.
#
# Usage:
#   ./scripts/build-camera-extension.sh [--sign IDENTITY]
#
# Options:
#   --sign IDENTITY   Code sign with the given identity (e.g., "Developer ID Application: ...")
#                     If omitted, the extension is built unsigned (requires SIP disabled).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
EXT_SRC="$PROJECT_ROOT/crates/virtual-camera/macos-extension"
BUILD_DIR="$PROJECT_ROOT/target/camera-extension"
APPEX_DIR="$BUILD_DIR/KalidoKitCamera.appex/Contents"

SIGN_IDENTITY=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --sign)
            SIGN_IDENTITY="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

echo "=== Building KalidoKit Camera Extension ==="

# Clean and create bundle structure
rm -rf "$BUILD_DIR"
mkdir -p "$APPEX_DIR/MacOS"

# Compile Objective-C sources
echo "Compiling Objective-C sources..."
clang -fobjc-arc -fmodules \
    -framework CoreMediaIO \
    -framework CoreMedia \
    -framework CoreVideo \
    -framework Foundation \
    -I "$EXT_SRC" \
    -o "$APPEX_DIR/MacOS/kalidokit-camera-extension" \
    "$EXT_SRC/main.m" \
    "$EXT_SRC/ProviderSource.m" \
    "$EXT_SRC/DeviceSource.m" \
    "$EXT_SRC/StreamSource.m" \
    "$EXT_SRC/SinkStreamSource.m"

# Copy Info.plist and entitlements
cp "$EXT_SRC/Info.plist" "$APPEX_DIR/Info.plist"
cp "$EXT_SRC/Extension.entitlements" "$BUILD_DIR/Extension.entitlements"

echo "Bundle created at: $BUILD_DIR/KalidoKitCamera.appex"

# Code sign if identity provided
if [[ -n "$SIGN_IDENTITY" ]]; then
    echo "Signing with: $SIGN_IDENTITY"
    codesign --force --sign "$SIGN_IDENTITY" \
        --entitlements "$BUILD_DIR/Extension.entitlements" \
        --timestamp \
        "$BUILD_DIR/KalidoKitCamera.appex"
    echo "Signed successfully."
else
    echo "WARNING: Extension is unsigned. SIP must be disabled to load."
    echo "  To sign: $0 --sign 'Developer ID Application: Your Name (TEAMID)'"
fi

echo "=== Done ==="
