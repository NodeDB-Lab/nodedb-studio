#!/usr/bin/env bash
# Capture every Studio screen, light and dark, as PNG.
#
# Usage: scripts/screenshots.sh [out_dir]    (default: <cargo target dir>/screenshots)
#
# Builds the app with the `screenshots` feature and runs it under a virtual X
# display at 2x scale. The app walks its scene list and writes
# <out_dir>/light/<scene>.png and <out_dir>/dark/<scene>.png.
#
# Linux only: the app snapshots WebKitGTK's own pixels. Needs Xvfb
# (`xvfb-run`). For output that matches CI, install the Inter and
# JetBrains Mono fonts.
set -euo pipefail

cd "$(dirname "$0")/.."
# Cargo config can move the target directory, so ask Cargo where it is.
target_dir="$(cargo metadata --format-version 1 --no-deps |
  grep -o '"target_directory":"[^"]*"' | cut -d'"' -f4)"
out="${1:-$target_dir/screenshots}"

if [[ "$(uname -s)" != Linux ]]; then
  echo "screenshots.sh: needs Linux (WebKitGTK); this host is $(uname -s)" >&2
  exit 2
fi
if ! command -v xvfb-run >/dev/null; then
  echo "screenshots.sh: xvfb-run not found; install Xvfb (Debian/Ubuntu: apt install xvfb)" >&2
  exit 2
fi

# A removed scene must not leave its old image behind.
rm -rf "$out/light" "$out/dark"
mkdir -p "$out"

# `cargo run`, not the bare binary: Dioxus serves `asset!` files from the
# source tree only when CARGO_MANIFEST_DIR is set at run time. A bare binary
# counts as a `dx` bundle and finds no stylesheet.
#
# GDK_BACKEND=x11 and the unset WAYLAND_DISPLAY keep GTK on the virtual
# display, even when this runs inside a Wayland session. The screen is larger
# than the 2880x1800 window so nothing clips it.
env -u WAYLAND_DISPLAY \
  GDK_BACKEND=x11 \
  GDK_SCALE=2 \
  WEBKIT_DISABLE_DMABUF_RENDERER=1 \
  xvfb-run --auto-servernum --server-args="-screen 0 3200x2000x24" \
  cargo run -p nodedb-studio --features screenshots --profile capture -- --screenshots "$out"
