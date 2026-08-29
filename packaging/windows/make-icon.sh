#!/usr/bin/env bash
# Regenerates app.ico from assets/logo.svg. Run this when the logo changes; the
# .ico is committed so the build itself needs no SVG rasteriser.
#
#   ./packaging/windows/make-icon.sh
#
# Sizes below 256 are stored as uncompressed BMP because Explorer on older
# Windows only accepts PNG-compressed frames at 256x256.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
SVG="$repo_root/assets/logo.svg"
ICO="$repo_root/packaging/windows/app.ico"
SIZES="16 32 48 64 128 256"

command -v inkscape >/dev/null || { echo "!! inkscape not found" >&2; exit 1; }

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

for s in $SIZES; do
  inkscape "$SVG" -w "$s" -h "$s" -o "$tmp/$s.png" >/dev/null 2>&1
done

python3 - "$ICO" "$tmp" $SIZES <<'PY'
import struct, sys
from io import BytesIO
from PIL import Image

ico, tmp, sizes = sys.argv[1], sys.argv[2], [int(s) for s in sys.argv[3:]]

def bmp_frame(img):
    r, g, b, a = img.split()
    xor = Image.merge("RGBA", (b, g, r, a)).transpose(Image.FLIP_TOP_BOTTOM).tobytes()
    w, h = img.size
    and_mask = b"\0" * (((w + 31) // 32) * 4 * h)
    header = struct.pack("<IiiHHIIiiII", 40, w, h * 2, 1, 32, 0,
                         len(xor) + len(and_mask), 0, 0, 0, 0)
    return header + xor + and_mask

def png_frame(img):
    buf = BytesIO()
    img.save(buf, "PNG", optimize=True)
    return buf.getvalue()

frames = []
for size in sizes:
    img = Image.open(f"{tmp}/{size}.png").convert("RGBA")
    if img.size != (size, size):
        raise SystemExit(f"{size}.png rendered as {img.size}")
    frames.append((size, png_frame(img) if size == 256 else bmp_frame(img)))

offset = 6 + 16 * len(frames)
directory, blobs = b"", b""
for size, data in frames:
    directory += struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32,
                             len(data), offset)
    blobs += data
    offset += len(data)

with open(ico, "wb") as f:
    f.write(struct.pack("<HHH", 0, 1, len(frames)) + directory + blobs)
PY

echo "==> $ICO ($(du -h "$ICO" | cut -f1))"
