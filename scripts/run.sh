#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ -x "$ROOT/target/release/air-control" ]]; then
  exec "$ROOT/target/release/air-control"
fi
if [[ -x "$ROOT/target/debug/air-control" ]]; then
  exec "$ROOT/target/debug/air-control"
fi

exec cargo run -p air-control-app --release
