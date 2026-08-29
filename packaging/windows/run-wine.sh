#!/usr/bin/env bash
# Runs the deployed Windows build under wine.
#
#   source packaging/windows/env.sh
#   cargo xwin build --release --target x86_64-pc-windows-msvc
#   ./packaging/windows/deploy.sh
#   ./packaging/windows/run-wine.sh
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
DEPLOY="${1:-$repo_root/target/windows-deploy}"
export WINEPREFIX="${WINEPREFIX:-$HOME/.wine-otl}"
export WINEARCH=win64

[ -f "$DEPLOY/offline-translator-linux.exe" ] || {
  echo "!! nothing deployed at $DEPLOY — run packaging/windows/deploy.sh first" >&2; exit 1; }

# A first run initialises the prefix and prints a lot of unrelated noise; the
# wine32/rundll32/ole errors during that step are expected and harmless.
[ -d "$WINEPREFIX" ] || echo "==> creating wine prefix at $WINEPREFIX (first run is slow)"

cd "$DEPLOY"
exec wine offline-translator-linux.exe "$@"
