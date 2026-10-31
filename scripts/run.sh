#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ "$(uname -s)" == "Darwin" ]]; then
  PROFILE=debug
  BIN="$ROOT/target/$PROFILE/air-control"
  if [[ ! -x "$ROOT/target/release/air-control" && ! -x "$BIN" ]]; then
    cargo build -p air-control-app
  fi
  if [[ -x "$ROOT/target/release/air-control" ]]; then
    PROFILE=release
  fi
  APP="$("$ROOT/scripts/macos-app.sh" "$PROFILE")"
  exec open -n "$APP"
fi

if [[ -x "$ROOT/target/release/air-control" ]]; then
  exec "$ROOT/target/release/air-control"
fi
if [[ -x "$ROOT/target/debug/air-control" ]]; then
  exec "$ROOT/target/debug/air-control"
fi

exec cargo run -p air-control-app --release
