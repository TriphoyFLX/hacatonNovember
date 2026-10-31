"""Local Russian command listener.

The process prints newline-delimited JSON only: heard text and a confidence.
It never names a system command. The Rust core maps the text through its allowlist.
"""

from __future__ import annotations

import json
import os
import queue
import sys
import threading
from pathlib import Path

PROTOCOL_VERSION = 1
SAMPLE_RATE = 16000
MODEL_NAME = "vosk-model-small-ru-0.22"

# Keep this list aligned with intent_from_phrase in commands.rs.
PHRASES = [
    "открой проводник",
    "открой файлы",
    "проводник",
    "скопируй",
    "копировать",
    "вставь",
    "вставить",
    "отмени",
    "отменить",
    "громче",
    "тише",
    "без звука",
    "выключи звук",
    "следующий трек",
    "следующая песня",
    "предыдущий трек",
    "предыдущая песня",
    "следующее окно",
    "предыдущее окно",
    "сверни окно",
    "разверни окно",
    "правый клик",
    "двойной клик",
    "прокрути вверх",
    "прокрути вниз",
    "клик",
    "нажми",
    "пауза",
    "плей",
    "play",
    "[unk]",
]


def emit(kind: str, **fields: object) -> None:
    message = {"version": PROTOCOL_VERSION, "type": kind, **fields}
    sys.stdout.write(json.dumps(message, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def model_dir() -> Path | None:
    root = Path(__file__).resolve().parents[1]
    candidate = root / "models" / MODEL_NAME
    if (candidate / "conf" / "model.conf").exists():
        return candidate
    override = os.environ.get("AIR_CONTROL_ROOT")
    if override:
        other = Path(override) / "voice" / "models" / MODEL_NAME
        if (other / "conf" / "model.conf").exists():
            return other
    return None


class Listener:
    def __init__(self) -> None:
        self.model = None
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None
        self._stream = None

    def prepare(self) -> None:
        if self.model is not None:
            emit("status", ready=True, listening=False, message="модель уже загружена")
            return
        try:
            from vosk import Model, SetLogLevel
        except ImportError:
            emit(
                "error",
                code="import",
                ready=False,
                message="Нет модуля vosk. Один раз запустите ./scripts/setup.sh (macOS/Linux) или setup.ps1 (Windows).",
            )
            return
        folder = model_dir()
        if folder is None:
            emit(
                "error",
                code="model",
                ready=False,
                message="Модель голоса не найдена. Один раз запустите ./scripts/setup.sh (macOS/Linux) или setup.ps1 (Windows).",
            )
            return
        SetLogLevel(-1)
        self.model = Model(str(folder))
        emit("status", ready=True, listening=False, message="модель загружена")

    def start(self) -> None:
        if self.model is None:
            self.prepare()
        if self.model is None:
            return
        if self._thread is not None and self._thread.is_alive():
            emit("status", ready=True, listening=True, message="микрофон уже включён")
            return
        try:
            import sounddevice as sd
            from vosk import KaldiRecognizer
        except ImportError:
            emit(
                "error",
                code="import",
                ready=False,
                message="Нет модуля sounddevice. Один раз запустите setup.ps1 рядом с exe.",
            )
            return
        recognizer = KaldiRecognizer(self.model, SAMPLE_RATE, json.dumps(PHRASES, ensure_ascii=False))
        audio: queue.Queue[bytes] = queue.Queue(maxsize=12)
        self._stop.clear()

        def callback(indata, _frames, _time_info, _status) -> None:
            try:
                audio.put_nowait(bytes(indata))
            except queue.Full:
                pass

        try:
            stream = sd.RawInputStream(
                samplerate=SAMPLE_RATE,
                blocksize=4000,
                dtype="int16",
                channels=1,
                callback=callback,
            )
            stream.start()
        except Exception as exc:
            emit(
                "error",
                code="mic",
                ready=True,
                message=f"Микрофон не открылся: {exc}"[:240],
            )
            return
        self._stream = stream

        def pump() -> None:
            while not self._stop.is_set():
                try:
                    chunk = audio.get(timeout=0.2)
                except queue.Empty:
                    continue
                if not recognizer.AcceptWaveform(chunk):
                    continue
                try:
                    text = json.loads(recognizer.Result()).get("text", "")
                except json.JSONDecodeError:
                    continue
                text = str(text).strip().lower().replace("ё", "е")
                if not text or text == "[unk]":
                    continue
                emit("voice", text=text[:120], confidence=0.9)

        self._thread = threading.Thread(target=pump, name="air-voice", daemon=True)
        self._thread.start()
        emit("status", ready=True, listening=True, message="микрофон включён")

    def stop(self) -> None:
        self._stop.set()
        stream = self._stream
        self._stream = None
        if stream is not None:
            try:
                stream.stop()
                stream.close()
            except Exception:
                pass
        thread = self._thread
        self._thread = None
        if thread is not None and thread.is_alive():
            thread.join(timeout=1.0)
        ready = self.model is not None
        emit("status", ready=ready, listening=False, message="микрофон выключен")


def main() -> int:
    sys.stdin.reconfigure(encoding="utf-8")
    sys.stdout.reconfigure(encoding="utf-8", newline="\n")
    listener = Listener()
    emit("hello", process_id=os.getpid(), message="голос запущен")
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            command = json.loads(line)
        except json.JSONDecodeError:
            emit("error", code="protocol", ready=listener.model is not None, message="плохая команда")
            continue
        kind = str(command.get("type", ""))
        if kind == "prepare":
            listener.prepare()
        elif kind == "start":
            listener.start()
        elif kind == "stop":
            listener.stop()
        elif kind == "shutdown":
            listener.stop()
            emit("goodbye")
            return 0
    listener.stop()
    return 0
