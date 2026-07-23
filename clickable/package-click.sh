#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"

cd "$repo_root"

clickable_bin="${CLICKABLE_BIN:-}"
if [ -z "$clickable_bin" ]; then
  if [ -x "$repo_root/venv/bin/clickable" ]; then
    clickable_bin="$repo_root/venv/bin/clickable"
  else
    clickable_bin="clickable"
  fi
fi

translator_host_path="/home/david/git/translator-rs"
mnn_host_path="/home/david/git/mnn-sys"
if [ -d "$translator_host_path" ]; then
  extra_mount="${translator_host_path}:${translator_host_path}:ro"
  extra_mount2="${mnn_host_path}:${mnn_host_path}:ro"
  if [ -n "${CLICKABLE_EXTRA_MOUNTS:-}" ]; then
    export CLICKABLE_EXTRA_MOUNTS="${CLICKABLE_EXTRA_MOUNTS},${extra_mount},${extra_mount2}"
  else
    export CLICKABLE_EXTRA_MOUNTS="${extra_mount},${extra_mount2}"
  fi
fi

"$clickable_bin" build -c clickable.yaml "$@"
