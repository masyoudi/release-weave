#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SVG_SRC="$REPO_ROOT/assets/icons/release-weave.svg"
ICONS_DIR="$REPO_ROOT/assets/icons"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

if [ ! -f "$SVG_SRC" ]; then
    echo "Error: SVG icon not found at $SVG_SRC"
    exit 1
fi

echo "==> Generating PNG assets from SVG..."
SIZES=(16 32 48 64 128 256 512 1024)
for sz in "${SIZES[@]}"; do
    magick -background none -density 300 "$SVG_SRC" -resize "${sz}x${sz}" "$TMP_DIR/icon_${sz}.png"
done

# Copy standard PNGs to assets/icons
cp "$TMP_DIR/icon_256.png" "$ICONS_DIR/release-weave-256x256.png"
cp "$TMP_DIR/icon_512.png" "$ICONS_DIR/release-weave-512x512.png"

echo "==> Generating Windows ICO (release-weave.ico)..."
magick \
    "$TMP_DIR/icon_16.png" \
    "$TMP_DIR/icon_32.png" \
    "$TMP_DIR/icon_48.png" \
    "$TMP_DIR/icon_64.png" \
    "$TMP_DIR/icon_128.png" \
    "$TMP_DIR/icon_256.png" \
    "$ICONS_DIR/release-weave.ico"

echo "==> Generating macOS ICNS (release-weave.icns)..."
python3 - << 'EOF' "$TMP_DIR" "$ICONS_DIR/release-weave.icns"
import sys, os, struct

tmp_dir = sys.argv[1]
output_icns = sys.argv[2]

# Apple ICNS chunk types for PNG payloads:
# ic07 = 128x128
# ic08 = 256x256
# ic09 = 512x512
# ic10 = 1024x1024
# ic11 = 32x32 (16x16@2x)
# ic12 = 64x64 (32x32@2x)
# ic13 = 256x256 (128x128@2x)
# ic14 = 512x512 (256x256@2x)

type_map = [
    (b'ic07', 'icon_128.png'),
    (b'ic08', 'icon_256.png'),
    (b'ic09', 'icon_512.png'),
    (b'ic10', 'icon_1024.png'),
    (b'ic11', 'icon_32.png'),
    (b'ic12', 'icon_64.png'),
    (b'ic13', 'icon_256.png'),
    (b'ic14', 'icon_512.png'),
]

chunks = []
for tag, filename in type_map:
    filepath = os.path.join(tmp_dir, filename)
    if os.path.exists(filepath):
        with open(filepath, 'rb') as f:
            png_bytes = f.read()
        chunk_len = 8 + len(png_bytes)
        chunk = tag + struct.pack('>I', chunk_len) + png_bytes
        chunks.append(chunk)

total_len = 8 + sum(len(c) for c in chunks)
header = b'icns' + struct.pack('>I', total_len)

with open(output_icns, 'wb') as f:
    f.write(header)
    for c in chunks:
        f.write(c)

print(f"Wrote {total_len} bytes to {output_icns}")
EOF

echo "==> Icon generation completed:"
ls -lh "$ICONS_DIR/release-weave.ico" "$ICONS_DIR/release-weave.icns"
