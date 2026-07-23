#!/usr/bin/env bash
# Fast iteration build: same Alpine/musl env as the APK build, but runs
# `cargo build` directly against the bind-mounted repo so `target/` persists
# across runs. No abuild, no tarball.
#
# Usage:
#   ./packaging/postmarketos/dev-build.sh                  # cargo build --release
#   ./packaging/postmarketos/dev-build.sh shell            # interactive shell
#   ./packaging/postmarketos/dev-build.sh cargo check      # any cargo subcommand
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

nice -n 19 docker build -t offline-translator-linux-apk -f "$script_dir/Dockerfile" "$repo_root"

if [ "$#" -eq 0 ]; then
  cmd=(cargo build --release)
elif [ "$1" = "shell" ]; then
  cmd=(sh)
else
  cmd=("$@")
fi

tty_flags=()
if [ -t 0 ] && [ -t 1 ]; then
  tty_flags=(-it)
fi

nice -n 19 docker run --rm "${tty_flags[@]}" \
  -v "$repo_root:/work" \
  -w /work \
  -e CARGO_NET_GIT_FETCH_WITH_CLI=true \
  -e CMAKE_POLICY_VERSION_MINIMUM=3.5 \
  -e CARGO_HOME=/work/.cache/dev-build/cargo-home \
  -e HOME=/work/.cache/dev-build \
  -u "$(id -u):$(id -g)" \
  offline-translator-linux-apk \
  "${cmd[@]}"
