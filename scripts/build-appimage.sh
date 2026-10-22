#!/usr/bin/env bash
set -euo pipefail

# Script to build ReleaseWeave AppImage for Linux x86_64
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

SKIP_BUILD=false
for arg in "$@"; do
    case "$arg" in
        --skip-build)
            SKIP_BUILD=true
            ;;
    esac
done

if [ "$SKIP_BUILD" = false ]; then
    echo "==> Building ReleaseWeave release binary..."
    cargo build --release
fi

BINARY="$REPO_ROOT/target/release/release-weave"
if [ ! -f "$BINARY" ]; then
    echo "Error: Release binary not found at $BINARY"
    exit 1
fi

APPDIR="$REPO_ROOT/target/AppDir"
OUTPUT_IMAGE="$REPO_ROOT/target/ReleaseWeave-x86_64.AppImage"

echo "==> Assembling AppDir structure..."
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"
mkdir -p "$APPDIR/usr/share/applications"
mkdir -p "$APPDIR/usr/share/icons/hicolor/scalable/apps"
mkdir -p "$APPDIR/usr/share/metainfo"

# Install binary
cp "$BINARY" "$APPDIR/usr/bin/release-weave"
chmod +x "$APPDIR/usr/bin/release-weave"
if command -v strip >/dev/null 2>&1; then
    strip -s "$APPDIR/usr/bin/release-weave" 2>/dev/null || true
fi

# Install desktop entry
cp "$REPO_ROOT/assets/release-weave.desktop" "$APPDIR/usr/share/applications/release-weave.desktop"
cp "$REPO_ROOT/assets/release-weave.desktop" "$APPDIR/release-weave.desktop"

# Install metadata
if [ -f "$REPO_ROOT/assets/com.release_weave.ReleaseWeave.metainfo.xml" ]; then
    cp "$REPO_ROOT/assets/com.release_weave.ReleaseWeave.metainfo.xml" "$APPDIR/usr/share/metainfo/com.release_weave.ReleaseWeave.metainfo.xml"
fi

# Install icon
cp "$REPO_ROOT/assets/icons/release-weave.svg" "$APPDIR/usr/share/icons/hicolor/scalable/apps/release-weave.svg"
cp "$REPO_ROOT/assets/icons/release-weave.svg" "$APPDIR/release-weave.svg"

# Create AppRun launcher
cat << 'EOF' > "$APPDIR/AppRun"
#!/bin/sh
SELF=$(readlink -f "$0")
HERE=${SELF%/*}
export PATH="${HERE}/usr/bin:${PATH}"
export XDG_DATA_DIRS="${HERE}/usr/share:${XDG_DATA_DIRS}"
exec "${HERE}/usr/bin/release-weave" "$@"
EOF
chmod +x "$APPDIR/AppRun"

# Locate or download appimagetool
APPIMAGETOOL=""
if command -v appimagetool >/dev/null 2>&1; then
    APPIMAGETOOL="appimagetool"
elif [ -x "$REPO_ROOT/target/tools/squashfs-root/AppRun" ]; then
    APPIMAGETOOL="$REPO_ROOT/target/tools/squashfs-root/AppRun"
else
    echo "==> Downloading appimagetool..."
    mkdir -p "$REPO_ROOT/target/tools"
    TOOL_URL="https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
    curl -fsSL -o "$REPO_ROOT/target/tools/appimagetool.AppImage" "$TOOL_URL"
    chmod +x "$REPO_ROOT/target/tools/appimagetool.AppImage"
    
    echo "==> Extracting appimagetool for FUSE-less execution..."
    (
        cd "$REPO_ROOT/target/tools"
        ./appimagetool.AppImage --appimage-extract >/dev/null 2>&1
    )
    APPIMAGETOOL="$REPO_ROOT/target/tools/squashfs-root/AppRun"
fi

echo "==> Packaging AppImage with $APPIMAGETOOL (XZ compression)..."
rm -f "$OUTPUT_IMAGE"
ARCH=x86_64 "$APPIMAGETOOL" --comp xz -n "$APPDIR" "$OUTPUT_IMAGE"

echo "==> AppImage created successfully:"
ls -lh "$OUTPUT_IMAGE"
