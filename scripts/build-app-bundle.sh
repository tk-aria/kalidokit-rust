#!/bin/bash
# Build KalidoKit.app with embedded Camera Extension.
#
# This creates a macOS .app bundle containing:
#   - The kalidokit-rust host binary
#   - The KalidoKitCamera.appex Camera Extension
# Then ad-hoc signs everything so OSSystemExtensionManager accepts it.
#
# Usage:
#   ./scripts/build-app-bundle.sh [--release]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
EXT_SRC="$PROJECT_ROOT/crates/virtual-camera/macos-extension"

PROFILE="debug"
if [[ "${1:-}" == "--release" ]]; then
    PROFILE="release"
fi

APP_DIR="$PROJECT_ROOT/target/$PROFILE/KalidoKit.app"
CONTENTS="$APP_DIR/Contents"
MACOS="$CONTENTS/MacOS"
EXT_BUNDLE="$CONTENTS/Library/SystemExtensions/com.kalidokit.rust.camera-extension.systemextension"
EXT_CONTENTS="$EXT_BUNDLE/Contents"

echo "=== Building KalidoKit.app (profile=$PROFILE) ==="

# 1. Build host binary
echo "Building host binary..."
if [[ "$PROFILE" == "release" ]]; then
    cargo build --release -p kalidokit-rust
else
    cargo build -p kalidokit-rust
fi

# 2. Build Camera Extension binary
echo "Building Camera Extension..."
mkdir -p "$EXT_CONTENTS/MacOS"

clang -fobjc-arc -fmodules \
    -framework CoreMediaIO \
    -framework CoreMedia \
    -framework CoreVideo \
    -framework Foundation \
    -I "$EXT_SRC" \
    -o "$EXT_CONTENTS/MacOS/com.kalidokit.rust.camera-extension" \
    "$EXT_SRC/main.m" \
    "$EXT_SRC/ProviderSource.m" \
    "$EXT_SRC/DeviceSource.m" \
    "$EXT_SRC/StreamSource.m" \
    "$EXT_SRC/SinkStreamSource.m"

cp "$EXT_SRC/Info.plist" "$EXT_CONTENTS/Info.plist"

# 3. Build installer binary (replaces host binary for Extension activation)
echo "Building installer binary..."
clang -fobjc-arc -fmodules \
    -framework SystemExtensions \
    -framework Foundation \
    -o "$PROJECT_ROOT/target/$PROFILE/install-extension" \
    "$SCRIPT_DIR/install-extension.m"

# 4. Create .app bundle
echo "Creating .app bundle..."
mkdir -p "$MACOS"

# Use installer as the main executable (for Extension activation via open)
cp "$PROJECT_ROOT/target/$PROFILE/install-extension" "$MACOS/kalidokit-rust"

# Create host Info.plist
cat > "$CONTENTS/Info.plist" << 'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>com.kalidokit.rust</string>
    <key>CFBundleName</key>
    <string>KalidoKit</string>
    <key>CFBundleExecutable</key>
    <string>kalidokit-rust</string>
    <key>CFBundleVersion</key>
    <string>15.0</string>
    <key>CFBundleShortVersionString</key>
    <string>15.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>12.3</string>
</dict>
</plist>
PLIST

# 4. Ad-hoc sign extension first, then host app
echo "Signing extension..."
codesign --force --sign - \
    --entitlements "$EXT_SRC/Extension.entitlements" \
    "$EXT_BUNDLE"

echo "Signing host app..."
HOST_ENT="$PROJECT_ROOT/crates/virtual-camera/macos-extension/host.entitlements"
if [[ ! -f "$HOST_ENT" ]]; then
    HOST_ENT="/tmp/host.entitlements"
fi
codesign --force --sign - \
    --entitlements "$HOST_ENT" \
    "$APP_DIR"

echo ""
echo "=== Done ==="
echo "App bundle: $APP_DIR"
echo ""
echo "To run:"
echo "  open $APP_DIR"
echo "  # or: $MACOS/kalidokit-rust"
