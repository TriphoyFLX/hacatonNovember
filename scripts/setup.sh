#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "AIR CONTROL setup (macOS / Linux)"

PYTHON=""
for candidate in python3.12 python3.11 python3; do
  if command -v "$candidate" >/dev/null 2>&1; then
    if "$candidate" -c 'import sys; raise SystemExit(0 if (3,11) <= sys.version_info[:2] < (3,13) else 1)'; then
      PYTHON="$candidate"
      break
    fi
  fi
done

if [[ -z "$PYTHON" ]]; then
  echo "Нужен Python 3.11 или 3.12. MediaPipe не ставится на 3.13+/3.14." >&2
  exit 1
fi

VENV="$ROOT/vision/.venv"
if [[ ! -x "$VENV/bin/python" ]]; then
  "$PYTHON" -m venv "$VENV"
fi

"$VENV/bin/python" -m pip install --upgrade pip
"$VENV/bin/python" -m pip install -r "$ROOT/vision/requirements.txt"

MODEL="$ROOT/vision/models/hand_landmarker.task"
if [[ ! -f "$MODEL" ]]; then
  echo "Downloading hand landmarker model"
  mkdir -p "$(dirname "$MODEL")"
  curl -L --fail -o "$MODEL" \
    "https://storage.googleapis.com/mediapipe-models/hand_landmarker/hand_landmarker/float16/1/hand_landmarker.task"
fi

# Voice (vosk + RU model). Falls back if pip breaks on newer macOS.
bash "$ROOT/scripts/install-voice.sh"

"$VENV/bin/python" "$ROOT/scripts/make_icon.py" || true

echo
echo "Готово. Дальше:"
echo "  cargo build -p air-control-app"
echo "  ./scripts/run.sh"
echo
echo "На macOS разрешите:"
echo "  • Камера — при первом запуске"
echo "  • Микрофон — для голоса"
echo "  • Универсальный доступ (Accessibility) — чтобы двигать курсор"
echo "  • Для сцены MIDI: Audio MIDI Setup → IAC Driver (включить), либо любой выход midir"
