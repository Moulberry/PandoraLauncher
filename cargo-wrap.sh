#!/bin/bash
set -euo pipefail

PATH_CLEAN="$HOME/.cargo/bin:/usr/bin:/usr/local/bin:/bin:/usr/lib/rustup/bin"
CARGO_BIN="$(PATH="$PATH_CLEAN" command -v cargo)"

exec env -i \
  HOME="$HOME" \
  USER="$USER" \
  PATH="$PATH_CLEAN" \
  RUSTUP_HOME="$HOME/.rustup" \
  CARGO_HOME="$HOME/.cargo" \
  DISPLAY="${DISPLAY:-:0}" \
  WAYLAND_DISPLAY="${WAYLAND_DISPLAY}" \
  XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR}" \
  "$CARGO_BIN" "$@"
