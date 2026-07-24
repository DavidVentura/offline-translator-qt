#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

case "${1:-$(uname -m)}" in
  x86_64|amd64)
    arch="x86_64"
    platform="linux/amd64"
    ;;
  aarch64|arm64)
    arch="aarch64"
    platform="linux/arm64"
    ;;
  *)
    echo "Usage: $0 [x86_64|aarch64]" >&2
    exit 1
    ;;
esac

image_tag="offline-translator-appimage-${arch}"

nice -n 19 docker build --platform "$platform" -t "$image_tag" -f "$script_dir/Dockerfile" "$script_dir"

nice -n 19 docker run --rm --platform "$platform" \
  -v "$repo_root:/work" \
  -w /work \
  -u "$(id -u):$(id -g)" \
  -e HOME=/tmp \
  "$image_tag" \
  ./packaging/appimage/build-appimage.sh
