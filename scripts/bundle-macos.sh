#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY="${1:-}"
OUTPUT_DIR="${2:-$REPO_ROOT/dist}"

if [ -z "$BINARY" ] || [ ! -f "$BINARY" ]; then
    echo "Usage: $0 <path-to-binary> [output-dir]"
    echo "Error: Binary not found at '$BINARY'"
    exit 1
fi

VERSION="$(grep '^version' "$REPO_ROOT/Cargo.toml" | head -n1 | cut -d'"' -f2)"
APP_BUNDLE="$OUTPUT_DIR/ReleaseWeave.app"

echo "==> Creating macOS App Bundle for ReleaseWeave v${VERSION}..."
rm -rf "$APP_BUNDLE"
mkdir -p "$APP_BUNDLE/Contents/MacOS"
mkdir -p "$APP_BUNDLE/Contents/Resources"

# 1. Install binary
cp "$BINARY" "$APP_BUNDLE/Contents/MacOS/release-weave"
chmod +x "$APP_BUNDLE/Contents/MacOS/release-weave"
if command -v strip >/dev/null 2>&1; then
    strip -S "$APP_BUNDLE/Contents/MacOS/release-weave" 2>/dev/null || true
fi

# 2. Install icon
if [ -f "$REPO_ROOT/assets/icons/release-weave.icns" ]; then
    cp "$REPO_ROOT/assets/icons/release-weave.icns" "$APP_BUNDLE/Contents/Resources/release-weave.icns"
fi

# 3. Install and customize Info.plist
sed -e "s/<string>0.1.0<\/string>/<string>${VERSION}<\/string>/g" \
    "$REPO_ROOT/assets/macos/Info.plist" > "$APP_BUNDLE/Contents/Info.plist"

echo "==> App bundle created at: $APP_BUNDLE"

# 4. Create ZIP distribution
echo "==> Packaging ZIP archive..."
(
    cd "$OUTPUT_DIR"
    zip -r -y "ReleaseWeave-macos-universal.zip" "ReleaseWeave.app"
)

# 5. Create DMG if hdiutil is available (native macOS)
if command -v hdiutil >/dev/null 2>&1; then
    echo "==> Packaging Apple Disk Image (DMG)..."
    DMG_STAGE="$(mktemp -d)"
    cp -R "$APP_BUNDLE" "$DMG_STAGE/"
    ln -s /Applications "$DMG_STAGE/Applications"
    
    STAGE_SIZE_KB="$(du -sk "$DMG_STAGE" | cut -f1)"
    # Add generous headroom (minimum 150MB buffer) to avoid "No space left on device"
    DMG_SIZE_MB=$(( (STAGE_SIZE_KB / 1024) + 150 ))

    TEMP_DMG="$OUTPUT_DIR/ReleaseWeave-temp.dmg"
    DMG_OUTPUT="$OUTPUT_DIR/ReleaseWeave-macos-universal.dmg"
    rm -f "$TEMP_DMG" "$DMG_OUTPUT"

    echo "==> Creating temporary DMG volume (${DMG_SIZE_MB}MB)..."
    hdiutil create -size "${DMG_SIZE_MB}m" \
        -volname "ReleaseWeave" \
        -fs HFS+ \
        -srcfolder "$DMG_STAGE" \
        -format UDRW \
        "$TEMP_DMG"

    echo "==> Compressing DMG to final UDZO format..."
    hdiutil convert "$TEMP_DMG" -format UDZO -ov -o "$DMG_OUTPUT"
    rm -f "$TEMP_DMG"
    rm -rf "$DMG_STAGE"
    echo "==> DMG created at: $DMG_OUTPUT"
else
    echo "Note: hdiutil not available on this host; skipped DMG creation."
fi

echo "==> macOS bundling finished successfully."
