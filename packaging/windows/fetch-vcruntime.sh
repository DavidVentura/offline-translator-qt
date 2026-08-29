#!/usr/bin/env bash
# Populates the MSVC runtime DLLs that deploy.sh bundles app-locally.
#
# These are needed because Qt5Core.dll imports MSVCP140 regardless of how our
# own exe is linked, so `-C target-feature=+crt-static` does not avoid them.
# Bundling them beside the exe is Microsoft's documented "local deployment" and
# means users do not have to install vc_redist.x64.exe first.
#
# cargo-xwin gives us the CRT *import libraries*, not the redistributable DLLs,
# so they have to come from somewhere that has actually installed them.
set -euo pipefail

ROOT="${OTL_WIN_ROOT:-$HOME/.cache/otl-windows}"
DEST="${VCRT_DIR:-$ROOT/vcruntime}"
DLLS=(msvcp140.dll msvcp140_1.dll vcruntime140.dll vcruntime140_1.dll)
VM="${WINVM_DIR:-$HOME/vm/winvm}"

mkdir -p "$DEST"

if [ -x "$VM/scp-vm.sh" ]; then
  echo "==> copying from the Windows VM ($VM)"
  for dll in "${DLLS[@]}"; do
    "$VM/scp-vm.sh" "vm:C:/Windows/System32/$dll" "$DEST/" >/dev/null
    echo "    $dll"
  done
  chmod 644 "$DEST"/*.dll
  echo "==> $DEST ($(du -ch "$DEST"/*.dll | tail -1 | cut -f1))"
  exit 0
fi

cat >&2 <<EOF
!! No Windows VM at $VM, so the MSVC runtime cannot be fetched automatically.

Get these four files and drop them in $DEST:
    ${DLLS[*]}

Any of these sources works — they are the same binaries:
  * A Visual Studio install:
      VC/Redist/MSVC/<version>/x64/Microsoft.VC143.CRT/
    This is the canonical redistributable folder and the right provenance for a
    release build.
  * Any Windows machine with the redistributable installed: C:\\Windows\\System32\\
  * vc_redist.x64.exe, but note it is a WiX Burn bundle — 7z only reaches the
    bootstrapper UI. Unpacking the CRT payload needs WiX's \`dark.exe\`.

deploy.sh works without them; it warns and the resulting build then requires
users to install vc_redist.x64.exe themselves.
EOF
exit 1
