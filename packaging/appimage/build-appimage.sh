#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

app_id="dev.davidv.translator"
app_name="offline-translator-linux"

host_arch="$(uname -m)"
case "${APPIMAGE_ARCH:-$host_arch}" in
  x86_64|amd64)
    arch="x86_64"
    clickable_triplet="x86_64-linux-gnu"
    rust_target="x86_64-unknown-linux-gnu"
    ;;
  aarch64|arm64)
    arch="aarch64"
    clickable_triplet="aarch64-linux-gnu"
    rust_target="aarch64-unknown-linux-gnu"
    ;;
  *)
    echo "Unsupported AppImage architecture: ${APPIMAGE_ARCH:-$host_arch}" >&2
    exit 1
    ;;
esac

# linuxdeploy resolves the app's libraries and QML imports against the host, so
# it can only deploy for the architecture it is running on.
if [ "$arch" != "$host_arch" ]; then
  echo "Cannot build a $arch AppImage on a $host_arch host." >&2
  echo "Run this script on a $arch machine or in a $arch container." >&2
  exit 1
fi

find_first_file() {
  for path in "$@"; do
    if [ -f "$path" ]; then
      printf '%s\n' "$path"
      return 0
    fi
  done
  return 1
}

binary="${APPIMAGE_BIN:-}"
if [ -z "$binary" ]; then
  binary="$(find_first_file \
    "$repo_root/build/$clickable_triplet/app/$rust_target/release/$app_name" \
    "$repo_root/clickable/build/$clickable_triplet/app/install/translator" \
    "$repo_root/target/release/$app_name")" || {
    echo "Could not find a prebuilt $app_name binary for $arch." >&2
    echo "Build it first (./clickable/package-click.sh -a ${arch/x86_64/amd64}), or set APPIMAGE_BIN=/path/to/$app_name." >&2
    exit 1
  }
fi

gst_plugin_dir="${GSTREAMER_PLUGINS_DIR:-/usr/lib/$arch-linux-gnu/gstreamer-1.0}"
if [ ! -d "$gst_plugin_dir" ]; then
  echo "GStreamer plugin directory not found: $gst_plugin_dir" >&2
  echo "The live camera goes through Qt's gstreamer mediaservice backend; install" >&2
  echo "gstreamer1.0-plugins-{base,good,bad} or set GSTREAMER_PLUGINS_DIR." >&2
  exit 1
fi

# Only the plugins camerabin needs to drive a v4l2 viewfinder and still capture.
# linuxdeploy-plugin-gstreamer bundles every plugin it is pointed at along with
# their dependencies, and the full Debian plugin set drags in ffmpeg, x265,
# aom, openblas and gtk3 — roughly 300 MB of libraries nothing here calls.
gst_wanted=(
  libgstapp.so
  libgstaudioconvert.so
  libgstaudiorate.so
  libgstaudioresample.so
  libgstautodetect.so
  libgstcamerabin.so
  libgstcoreelements.so
  libgstencoding.so
  libgstjpeg.so
  libgstjpegformat.so
  libgstmultifile.so
  # camerabin builds its video-capture branch even in still-image mode, and its
  # default encoding profile is ogg/theora/vorbis.
  libgstogg.so
  libgsttheora.so
  libgstvorbis.so
  libgstplayback.so
  libgstpulseaudio.so
  libgsttypefindfunctions.so
  libgstvideo4linux2.so
  libgstvideoconvertscale.so
  libgstvideocrop.so
  libgstvideofilter.so
  libgstvideorate.so
  libgstvolume.so
)

version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$repo_root/Cargo.toml" | head -1)"

tools_dir="$script_dir/tools"
mkdir -p "$tools_dir"

fetch_tool() {
  local dest="$1"
  local url="$2"
  if [ ! -x "$dest" ]; then
    echo "Fetching $(basename "$dest")" >&2
    curl -fsSL -o "$dest" "$url"
    chmod +x "$dest"
  fi
  printf '%s\n' "$dest"
}

linuxdeploy_base="https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous"
linuxdeploy="${LINUXDEPLOY:-$(fetch_tool "$tools_dir/linuxdeploy-$arch.AppImage" \
  "$linuxdeploy_base/linuxdeploy-$arch.AppImage")}"
plugin_qt="${LINUXDEPLOY_PLUGIN_QT:-$(fetch_tool "$tools_dir/linuxdeploy-plugin-qt-$arch.AppImage" \
  "https://github.com/linuxdeploy/linuxdeploy-plugin-qt/releases/download/continuous/linuxdeploy-plugin-qt-$arch.AppImage")}"
plugin_appimage="${LINUXDEPLOY_PLUGIN_APPIMAGE:-$(fetch_tool "$tools_dir/linuxdeploy-plugin-appimage-$arch.AppImage" \
  "https://github.com/linuxdeploy/linuxdeploy-plugin-appimage/releases/download/continuous/linuxdeploy-plugin-appimage-$arch.AppImage")}"
plugin_gstreamer="${LINUXDEPLOY_PLUGIN_GSTREAMER:-$(fetch_tool "$tools_dir/linuxdeploy-plugin-gstreamer.sh" \
  "https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gstreamer/master/linuxdeploy-plugin-gstreamer.sh")}"

# linuxdeploy discovers --plugin arguments by scanning PATH.
export PATH="$tools_dir:$PATH"

# The gstreamer plugin shells out to patchelf, which linuxdeploy ships inside
# its own AppImage; borrow that copy rather than requiring a system package.
if ! command -v patchelf >/dev/null 2>&1; then
  extract_dir="$(mktemp -d)"
  (cd "$extract_dir" && "$linuxdeploy" --appimage-extract usr/bin/patchelf >/dev/null)
  install -Dm755 "$extract_dir/squashfs-root/usr/bin/patchelf" "$tools_dir/patchelf"
  rm -rf "$extract_dir"
fi

appdir="$script_dir/AppDir"
rm -rf "$appdir"

install -Dm755 "$binary" "$appdir/usr/bin/$app_name"
mkdir -p "$appdir/usr/share/$app_name"
cp -a "$repo_root/qml" "$appdir/usr/share/$app_name/qml"
cp -a "$repo_root/assets" "$appdir/usr/share/$app_name/assets"

stage_dir="$script_dir/stage"
rm -rf "$stage_dir"
mkdir -p "$stage_dir"
install -Dm644 "$repo_root/assets/logo.png" "$stage_dir/$app_id.png"

gst_stage_dir="$stage_dir/gstreamer-1.0"
mkdir -p "$gst_stage_dir"
gst_missing=()
for plugin in "${gst_wanted[@]}"; do
  if [ ! -f "$gst_plugin_dir/$plugin" ]; then
    gst_missing+=("$plugin")
    continue
  fi
  cp "$gst_plugin_dir/$plugin" "$gst_stage_dir/"
done
if [ "${#gst_missing[@]}" -gt 0 ]; then
  echo "GStreamer plugins missing from $gst_plugin_dir: ${gst_missing[*]}" >&2
  echo "Install gstreamer1.0-plugins-{base,good,bad}, or adjust gst_wanted if this" >&2
  echo "distro's GStreamer splits them differently." >&2
  exit 1
fi

cat >"$stage_dir/$app_id.desktop" <<DESKTOP
[Desktop Entry]
Name=Offline translator
Exec=$app_name
Icon=$app_id
Terminal=false
Type=Application
Categories=Education;Languages;
Keywords=translate;translation;offline;
DESKTOP

out_dir="$script_dir/out"
mkdir -p "$out_dir"

# The Ubports*.qml files import Lomiri.Content, which only exists on Ubuntu
# Touch; they are never loaded in desktop mode, so keep them out of the import
# scan that would otherwise fail the qt plugin.
qml_scan_dir="$stage_dir/qml-scan"
mkdir -p "$qml_scan_dir"
for qml in "$repo_root"/qml/*.qml; do
  case "$(basename "$qml")" in
    Ubports*) continue ;;
  esac
  cp "$qml" "$qml_scan_dir/"
done

export ARCH="$arch"
export VERSION="$version"
export QMAKE="${QMAKE:-/usr/bin/qmake}"
export QML_SOURCES_PATHS="$qml_scan_dir"
export EXTRA_QT_PLUGINS="sensors;platforminputcontexts;wayland-decoration-client;wayland-graphics-integration-client;wayland-shell-integration"
export EXTRA_PLATFORM_PLUGINS="libqwayland-egl.so;libqwayland-generic.so"
export GSTREAMER_PLUGINS_DIR="$gst_stage_dir"
export GSTREAMER_HELPERS_DIR="${GSTREAMER_HELPERS_DIR:-/usr/lib/$arch-linux-gnu/gstreamer1.0/gstreamer-1.0}"
# Avoids needing FUSE to run the tool AppImages (containers, CI).
export APPIMAGE_EXTRACT_AND_RUN=1
export OUTPUT="Offline_translator-$version-$arch.AppImage"

cd "$out_dir"
"$linuxdeploy" \
  --appdir "$appdir" \
  --executable "$appdir/usr/bin/$app_name" \
  --desktop-file "$stage_dir/$app_id.desktop" \
  --icon-file "$stage_dir/$app_id.png" \
  --plugin qt \
  --plugin gstreamer

# The qt plugin deploys whatever the build host has installed: on a Plasma
# desktop that means KDE's kimageformats plugins (avif, jxl, exr) and the
# org.kde.desktop Quick Controls style with its KF5 stack. The app only ever
# renders png/jpeg/svg and skins its own controls.
rm -f "$appdir"/usr/plugins/imageformats/kimg_*.so
rm -rf "$appdir/usr/qml/org"
mkdir -p "$appdir/apprun-hooks"
cat >"$appdir/apprun-hooks/quick-controls-style.sh" <<'HOOK'
# Plasma sessions export org.kde.desktop, which is not bundled.
export QT_QUICK_CONTROLS_STYLE=Default
HOOK

# Pruning plugins leaves their dependencies behind, so drop every library that
# nothing in the AppDir links against any more.
prune_orphan_libs() {
  local lib_dir="$appdir/usr/lib"
  while :; do
    local needed removed=0
    needed="$(find "$appdir/usr/bin" "$appdir/usr/plugins" "$appdir/usr/qml" \
      "$lib_dir" -type f \( -name '*.so*' -o -perm -u+x \) -print0 2>/dev/null \
      | xargs -0 -r objdump -p 2>/dev/null \
      | awk '/NEEDED/ {print $2}' | sort -u)"
    local lib
    for lib in "$lib_dir"/*.so*; do
      [ -f "$lib" ] || continue
      # QtNetwork dlopens OpenSSL, so no ELF in here records it as NEEDED.
      case "$(basename "$lib")" in
        libssl.so*|libcrypto.so*) continue ;;
      esac
      if ! grep -qxF "$(basename "$lib")" <<<"$needed"; then
        echo "Pruning orphaned library: $(basename "$lib")"
        rm -f "$lib"
        removed=1
      fi
    done
    [ "$removed" -eq 0 ] && break
  done
}
prune_orphan_libs

"$plugin_appimage" --appdir "$appdir"

echo "Built $out_dir/$OUTPUT"
