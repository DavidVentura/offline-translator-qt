#!/usr/bin/env bash
# Builds the app, then assembles a runnable Windows tree from it: resolves the
# transitive Qt DLL closure, copies the plugins Qt loads by name (which no
# dependency walk can discover), lays out qml/ and assets/ the way
# find_main_qml expects, and zips the result.
#
#   source packaging/windows/env.sh
#   ./packaging/windows/deploy.sh [outdir]
#
# The build is included on purpose. Deploying a stale exe looks exactly like
# "my fix did not take effect", with nothing to indicate the binary is old --
# and it is easy to forget the compile when the change was only to a .qml file,
# which is copied rather than compiled. OTL_SKIP_BUILD=1 skips it.
#
# This is a stand-in for windeployqt, which is a Windows binary we cannot run.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="${1:-$repo_root/target/windows-deploy}"
ROOT="${OTL_WIN_ROOT:-$HOME/.cache/otl-windows}"
QT_DIR="${QT_DIR:-$ROOT/qt/5.15.2/msvc2019_64}"
EXE="$repo_root/target/x86_64-pc-windows-msvc/release/offline-translator-linux.exe"

if [ "${OTL_SKIP_BUILD:-}" != "1" ]; then
  echo "==> building"
  ( cd "$repo_root" && cargo xwin build --release --target x86_64-pc-windows-msvc )
fi
[ -f "$EXE" ] || { echo "!! $EXE missing (OTL_SKIP_BUILD=1 with no prior build?)" >&2; exit 1; }
[ -d "$QT_DIR" ] || { echo "!! Qt not found at $QT_DIR — run setup-toolchain.sh" >&2; exit 1; }

rm -rf "$OUT"
mkdir -p "$OUT/qml" "$OUT/platforms" "$OUT/imageformats" "$OUT/iconengines"
cp "$EXE" "$OUT/"

# Plugins are dlopen'd by filename, so they are invisible to a dependency walk
# and must be listed explicitly. qsvg is not optional: every icon is an SVG, and
# without it Qt reports "Unsupported image format" for all of them.
cp "$QT_DIR/plugins/platforms/qwindows.dll" "$OUT/platforms/"
for fmt in qsvg qjpeg qgif qico; do
  cp "$QT_DIR/plugins/imageformats/$fmt.dll" "$OUT/imageformats/" 2>/dev/null || true
done
cp "$QT_DIR/plugins/iconengines/qsvgicon.dll" "$OUT/iconengines/" 2>/dev/null || true

# Qt loads these by name at runtime too. opengl32sw is the software rasteriser
# Qt falls back to when the machine exposes no OpenGL 2.0 driver;
# libEGL/libGLESv2/d3dcompiler are the ANGLE path. Without them Qt Quick aborts
# at context creation on any machine without GPU drivers — a fresh Windows
# install, an RDP session, a VM. Wine hides this by supplying its own opengl32
# backed by host Mesa, so it only shows up on real Windows.
for gl in opengl32sw.dll libEGL.dll libGLESv2.dll d3dcompiler_47.dll; do
  cp "$QT_DIR/bin/$gl" "$OUT/" 2>/dev/null || true
done

# Only the QML modules actually imported — copying all of Qt's qml/ works but
# drags in Qt3D, Bluetooth, WebView and every *d.dll debug variant. The list is
# derived from the `import` lines rather than hardcoded, because a missing
# module is not a warning: Main.qml fails to load and the window stays blank.
# A dotted import lives under its first component (QtQuick.Controls ->
# QtQuick/Controls.2, Qt.labs.settings -> Qt/labs/settings).
mapfile -t qml_modules < <(
  grep -rhoE "^[[:space:]]*import [A-Za-z][A-Za-z0-9._]*" "$repo_root"/qml/*.qml \
    | awk '{print $2}' | cut -d. -f1 | sort -u
)
for mod in "${qml_modules[@]}"; do
  if [ -e "$QT_DIR/qml/$mod" ]; then
    cp -r "$QT_DIR/qml/$mod" "$OUT/qml/"
  else
    # TranslatorUi is registered from Rust; Lomiri.Content is Ubuntu Touch only.
    echo "    (skipping non-Qt QML module: $mod)"
  fi
done
# QtQuick.2 is the plugin backing `import QtQuick`, and no import line names it.
[ -e "$QT_DIR/qml/QtQuick.2" ] && cp -r "$QT_DIR/qml/QtQuick.2" "$OUT/qml/"

# Prune before the DLL closure runs, so nothing reachable only from this dead
# weight gets pulled in after it.

# Qt ships debug plugins (`<name>d.dll`) beside the release ones inside each QML
# module, and `cp -r` takes both. Only drop one when its release sibling exists,
# so a library legitimately ending in "d" survives.
find "$OUT/qml" -name "*d.dll" | while read -r dbg; do
  [ -f "${dbg%d.dll}.dll" ] && rm -f "$dbg"
done

# QtQuick.Controls ships four alternate styles. Nothing selects one (no
# QQuickStyle call, no QT_QUICK_CONTROLS_STYLE, no qtquickcontrols2.conf), so
# the Default style is used and these are unreachable.
for style in Imagine Material Universal Fusion; do
  rm -rf "$OUT/qml/QtQuick/Controls.2/$style"
done

# Our own QML sits alongside Qt's module directories: find_main_qml looks for
# <exe dir>/qml/Main.qml, and the names never collide (ours are files).
cp "$repo_root"/qml/*.qml "$OUT/qml/"
cp -r "$repo_root/assets" "$OUT/assets"

# The MSVC runtime, deployed app-locally. Microsoft supports this and it spares
# users installing vc_redist.x64.exe first — which they would otherwise have to,
# because Qt5Core.dll imports MSVCP140 independently of how our exe is linked, so
# `-C target-feature=+crt-static` would not avoid it. The trade is that an
# app-local CRT is not serviced by Windows Update; we own updating it.
# Populate with: packaging/windows/fetch-vcruntime.sh
VCRT_DIR="${VCRT_DIR:-$ROOT/vcruntime}"
if [ -d "$VCRT_DIR" ] && [ -n "$(ls -A "$VCRT_DIR"/*.dll 2>/dev/null)" ]; then
  cp "$VCRT_DIR"/*.dll "$OUT/"
else
  echo "    (no MSVC runtime in $VCRT_DIR — users will need vc_redist.x64.exe;"
  echo "     see packaging/windows/fetch-vcruntime.sh)"
fi

# Transitive Qt DLL closure, seeded from the exe AND every plugin copied above.
python3 - "$QT_DIR/bin" "$OUT" <<'PY'
import subprocess, sys, pathlib, shutil
qt_bin, deploy = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
available = {p.name.lower(): p for p in qt_bin.glob("*.dll") if not p.stem.endswith("d")}

def deps(path):
    out = subprocess.run(["objdump", "-p", str(path)], capture_output=True, text=True).stdout
    return [l.split()[-1] for l in out.splitlines() if "DLL Name:" in l]

queue = [deploy / "offline-translator-linux.exe"] + list(deploy.rglob("*.dll"))
seen, copied = set(), []
while queue:
    for name in deps(queue.pop()):
        key = name.lower()
        if key in seen or key not in available:
            continue
        seen.add(key)
        dest = deploy / name
        shutil.copy2(available[key], dest)
        copied.append(name)
        queue.append(dest)
print("Qt DLLs:", " ".join(sorted(copied)))
PY

zip_out="$OUT.zip"
rm -f "$zip_out"
( cd "$(dirname "$OUT")" && zip -qr9 "$zip_out" "$(basename "$OUT")" )

cat <<EOF

==> $OUT ($(du -sh "$OUT" | cut -f1))
==> $zip_out ($(du -h "$zip_out" | cut -f1))

Run under wine:
  ./packaging/windows/run-wine.sh

Run on real Windows:
  cd /home/david/vm/winvm && ./run-in-vm.sh
EOF
