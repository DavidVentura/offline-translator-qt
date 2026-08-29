#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${repo_root}"

export QMAKE="${QMAKE:-/usr/bin/qmake}"
export CLICKABLE_DESKTOP_MODE=1
export START_SCREEN="${START_SCREEN:-1}"
export AUTOMATION_FROM="${AUTOMATION_FROM:-${1:-English}}"
export AUTOMATION_TO="${AUTOMATION_TO:-${2:-Dutch}}"
export AUTOMATION_TEXT="${AUTOMATION_TEXT:-${3:-hello}}"
export AUTOMATION_SCREENSHOT_PATH="${AUTOMATION_SCREENSHOT_PATH:-${4:-${repo_root}/screenshot.png}}"
export AUTOMATION_QUIT="${AUTOMATION_QUIT:-1}"

cargo build --features live,pdf
rm -f "${AUTOMATION_SCREENSHOT_PATH}"
xvfb-run -a cargo run --features live,pdf

if [ -f "${AUTOMATION_SCREENSHOT_PATH}" ]; then
  echo "saved screenshot to ${AUTOMATION_SCREENSHOT_PATH}"
else
  echo "screenshot was not created: ${AUTOMATION_SCREENSHOT_PATH}" >&2
  exit 1
fi
