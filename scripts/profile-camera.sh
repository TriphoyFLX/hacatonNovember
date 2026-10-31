#!/usr/bin/env bash
# Launch AIR CONTROL with pipeline profiling enabled.
# Logs: $AIR_CONTROL_PROFILE_DIR (default /tmp/air-control-profile)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export AIR_CONTROL_PROFILE=1
export AIR_CONTROL_PROFILE_DIR="${AIR_CONTROL_PROFILE_DIR:-/tmp/air-control-profile}"

rm -rf "$AIR_CONTROL_PROFILE_DIR"
mkdir -p "$AIR_CONTROL_PROFILE_DIR"

cat >"$AIR_CONTROL_PROFILE_DIR/README.txt" <<EOF
AIR CONTROL profile run
=======================
1. Start camera in the app (open a page that shows the camera / instruments).
2. Keep it running for ~60 seconds with your typical scene.
3. Quit the app.
4. Summarize:

   python3 scripts/summarize-profile.py "$AIR_CONTROL_PROFILE_DIR"

Files:
  vision.jsonl  — capture / MediaPipe / JPEG / stdout timings
  rust.jsonl    — parse / decode / paint timings
  ui.jsonl      — successful Slint image updates only
  cpu.txt       — 1 Hz process CPU samples (python + air-control)
EOF

echo "Profile dir: $AIR_CONTROL_PROFILE_DIR"
echo "Building air-control-app..."
cargo build -p air-control-app

# Sample CPU of the app + vision python while the UI runs.
(
  while true; do
    printf '--- %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    ps -axo pid,%cpu,rss,comm 2>/dev/null | grep -E '[a]ir-control|[P]ython|[p]ython' || true
    sleep 1
  done
) >"$AIR_CONTROL_PROFILE_DIR/cpu.txt" &
CPU_PID=$!
cleanup() {
  kill "$CPU_PID" 2>/dev/null || true
}
trap cleanup EXIT

echo "Starting app. Record ~60s with the camera on, then quit."
echo "After quit, run:"
echo "  python3 \"$ROOT/scripts/summarize-profile.py\" \"$AIR_CONTROL_PROFILE_DIR\""
./target/debug/air-control

cleanup
trap - EXIT
echo
echo "Summarizing..."
python3 "$ROOT/scripts/summarize-profile.py" "$AIR_CONTROL_PROFILE_DIR"
