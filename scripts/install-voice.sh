#!/usr/bin/env bash
# Install vosk + sounddevice into vision/.venv and download the RU model.
# Works around broken pip on macOS 26 where platform.mac_ver() is empty.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PY="$ROOT/vision/.venv/bin/python"
if [[ ! -x "$PY" ]]; then
  echo "Сначала создайте venv: ./scripts/setup.sh" >&2
  exit 1
fi

"$PY" -m pip install -r "$ROOT/voice/requirements.txt" && true

if ! "$PY" -c 'import vosk, sounddevice' 2>/dev/null; then
  echo "pip недоступен — ставим колёса вручную"
  ROOT="$ROOT" "$PY" - <<'PY'
import io, json, os, tarfile, urllib.request, zipfile
from pathlib import Path

root = Path(os.environ["ROOT"])
cands = list((root / "vision" / ".venv" / "lib").glob("python*/site-packages"))
if not cands:
    raise SystemExit("site-packages not found")
site = cands[0]

def get_json(url):
    return json.load(urllib.request.urlopen(url))

def install_wheel(name, prefer=("none-any", "macosx_11_0_arm64", "macosx_10_6_universal2", "universal2")):
    data = get_json(f"https://pypi.org/pypi/{name}/json")
    ver = data["info"]["version"]
    files = data["releases"][ver]
    wheels = [u for u in files if u["packagetype"] == "bdist_wheel"]
    chosen = None
    for key in prefer:
        for u in wheels:
            if key in u["filename"]:
                chosen = u
                break
        if chosen:
            break
    if chosen is None and wheels:
        chosen = wheels[0]
    if chosen is None:
        raise SystemExit(f"no wheel for {name}")
    print("wheel", chosen["filename"])
    raw = urllib.request.urlopen(chosen["url"]).read()
    with zipfile.ZipFile(io.BytesIO(raw)) as zf:
        zf.extractall(site)

# Exact vosk mac wheel pin used by air-control
vosk = "https://files.pythonhosted.org/packages/44/19/5e8299237bc2005c3d155d3b48adba6fd6484465ad5c970302fc1d37947d/vosk-0.3.44-py3-none-macosx_10_6_universal2.whl"
print("wheel vosk-0.3.44 macos universal")
raw = urllib.request.urlopen(vosk).read()
with zipfile.ZipFile(io.BytesIO(raw)) as zf:
    zf.extractall(site)

for name in ["sounddevice", "cffi", "pycparser", "websockets", "requests", "tqdm", "certifi", "idna", "urllib3", "charset-normalizer"]:
    install_wheel(name)

# srt is sdist-only
srt = get_json("https://pypi.org/pypi/srt/json")
ver = srt["info"]["version"]
sdist = next(u for u in srt["releases"][ver] if u["packagetype"] == "sdist")
print("sdist", sdist["filename"])
raw = urllib.request.urlopen(sdist["url"]).read()
with tarfile.open(fileobj=io.BytesIO(raw), mode="r:*") as tf:
    member = next(m for m in tf.getmembers() if m.name.endswith("/srt.py") or m.name == "srt.py")
    f = tf.extractfile(member)
    (site / "srt.py").write_bytes(f.read())

print("voice packages installed into", site)
PY
fi

VOICE_MODEL="$ROOT/voice/models/vosk-model-small-ru-0.22"
if [[ ! -f "$VOICE_MODEL/conf/model.conf" ]]; then
  echo "Downloading Russian voice model"
  mkdir -p "$ROOT/voice/models"
  ZIP="$ROOT/voice/models/vosk-model-small-ru-0.22.zip"
  curl -L --fail -o "$ZIP" "https://alphacephei.com/vosk/models/vosk-model-small-ru-0.22.zip"
  unzip -o -q "$ZIP" -d "$ROOT/voice/models"
  rm -f "$ZIP"
fi

"$PY" -c 'import vosk, sounddevice; from vosk import Model; Model("'"$VOICE_MODEL"'"); print("voice OK")'
echo "Готово. Перезапустите AIR CONTROL и нажмите «Слушать»."
