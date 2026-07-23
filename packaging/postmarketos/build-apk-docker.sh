#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
pkgname="offline-translator-linux"
pkgver="0.1.0"
tarball="$script_dir/distfiles/$pkgname-$pkgver.tar.gz"

mkdir -p "$script_dir/distfiles" "$script_dir/packages"

tar \
  --sort=name \
  --mtime='UTC 2026-01-01' \
  --owner=0 \
  --group=0 \
  --numeric-owner \
  --exclude=.git \
  --exclude=target \
  --exclude=venv \
  --exclude=.claude \
  --exclude=.codex \
  --exclude=clickable/build \
  --exclude=build \
  --exclude=packaging \
  --exclude=.cache \
  -cf - \
  --transform "s,^,$pkgname-$pkgver/," \
  -C "$repo_root" . | gzip -n > "$tarball"

platform_args=()
if [ -n "${DOCKER_PLATFORM:-}" ]; then
  platform_args=(--platform "$DOCKER_PLATFORM")
fi

image_tag="offline-translator-linux-apk${DOCKER_PLATFORM:+-${DOCKER_PLATFORM//\//-}}"

nice -n 19 docker build "${platform_args[@]}" -t "$image_tag" -f "$script_dir/Dockerfile" "$repo_root"

nice -n 19 docker run --rm "${platform_args[@]}" \
  -v "$repo_root:/work" \
  -w /work/packaging/postmarketos \
  "$image_tag" \
  sh -lc '
    set -euo pipefail
    uid=$(stat -c %u /work)
    gid=$(stat -c %g /work)
    addgroup -g "$gid" builder 2>/dev/null || true
    adduser -D -u "$uid" -G builder builder 2>/dev/null || true
    addgroup builder abuild 2>/dev/null || true
    mkdir -p /home/builder/.abuild /work/packaging/postmarketos/packages /work/packaging/postmarketos/distfiles
    chown -R "$uid:$gid" /home/builder /work/packaging/postmarketos
    su builder -c "abuild-keygen -a -n -q"
    cp /home/builder/.abuild/*.pub /etc/apk/keys/
    cp distfiles/'"$pkgname-$pkgver"'.tar.gz .
    su builder -c "export SRCDEST=/work/packaging/postmarketos/distfiles REPODEST=/work/packaging/postmarketos/packages PACKAGER=\"David <david@davidv.dev>\" MAINTAINER=\"David <david@davidv.dev>\"; cd /work/packaging/postmarketos; abuild -r"
  '
