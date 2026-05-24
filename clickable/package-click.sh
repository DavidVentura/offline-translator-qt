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
if [ -d "$translator_host_path" ]; then
  extra_mount="${translator_host_path}:${translator_host_path}:ro"
  if [ -n "${CLICKABLE_EXTRA_MOUNTS:-}" ]; then
    export CLICKABLE_EXTRA_MOUNTS="${CLICKABLE_EXTRA_MOUNTS},${extra_mount}"
  else
    export CLICKABLE_EXTRA_MOUNTS="${extra_mount}"
  fi
fi

canonical_ort_arch() {
  case "$1" in
    arm64|aarch64|arm64-v8a)
      echo "aarch64"
      ;;
    amd64|x86_64)
      echo "x86_64"
      ;;
    *)
      return 1
      ;;
  esac
}

ort_build_arch="${ORT_BUILD_ARCH:-}"
if [ -n "$ort_build_arch" ]; then
  if ! ort_build_arch="$(canonical_ort_arch "$ort_build_arch")"; then
    echo "Unsupported ONNX Runtime target architecture: ${ORT_BUILD_ARCH}" >&2
    exit 1
  fi
fi

if [ -z "$ort_build_arch" ]; then
  args=("$@")
  for ((i=0; i<${#args[@]}; i++)); do
    if { [ "${args[$i]}" = "--arch" ] || [ "${args[$i]}" = "-a" ]; } && [ $((i + 1)) -lt ${#args[@]} ]; then
      if ! ort_build_arch="$(canonical_ort_arch "${args[$((i + 1))]}")"; then
        echo "Unsupported ONNX Runtime target architecture: ${args[$((i + 1))]}" >&2
        exit 1
      fi
    fi
  done
fi

if [ -z "$ort_build_arch" ]; then
  if ! ort_build_arch="$(canonical_ort_arch "$(uname -m)")"; then
    ort_build_arch=""
  fi
fi

if [ -n "$ort_build_arch" ]; then
  ort_build_triplet="${ORT_BUILD_TRIPLET:-}"
  if [ -z "$ort_build_triplet" ] && [ "$ort_build_arch" = "aarch64" ]; then
    ort_build_triplet="aarch64-linux-gnu"
  elif [ -z "$ort_build_triplet" ] && [ "$ort_build_arch" = "x86_64" ]; then
    ort_build_triplet="x86_64-linux-gnu"
  fi
  ORT_BUILD_ARCH="$ort_build_arch" ORT_BUILD_TRIPLET="$ort_build_triplet" "$repo_root/scripts/prepare_onnxruntime.sh"
else
  "$repo_root/scripts/prepare_onnxruntime.sh"
fi

runtime_dir="$repo_root/runtime-lib"
runtime_backup_dir=""
restore_runtime_dir() {
  if [ -n "$runtime_backup_dir" ] && [ -d "$runtime_backup_dir" ]; then
    rm -rf "$runtime_dir"
    mv "$runtime_backup_dir" "$runtime_dir"
  fi
}

if [ -n "$ort_build_arch" ] && [ -d "$runtime_dir" ]; then
  selected_lib="$runtime_dir/$ort_build_arch/libonnxruntime.so"
  if [ ! -f "$selected_lib" ]; then
    echo "Missing staged ONNX Runtime library for $ort_build_arch: $selected_lib" >&2
    exit 1
  fi

  runtime_backup_dir="$(mktemp -d "$repo_root/.runtime-lib-backup.XXXXXX")"
  rm -rf "$runtime_backup_dir"
  mv "$runtime_dir" "$runtime_backup_dir"
  mkdir -p "$runtime_dir/$ort_build_arch"
  cp "$runtime_backup_dir/$ort_build_arch/libonnxruntime.so" "$runtime_dir/$ort_build_arch/libonnxruntime.so"
  trap restore_runtime_dir EXIT
fi

if [ -n "${ort_build_triplet:-}" ]; then
  rm -rf "$repo_root/build/$ort_build_triplet/app/install/runtime-lib"
fi

"$clickable_bin" build -c clickable.yaml "$@"
