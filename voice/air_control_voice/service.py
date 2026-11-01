"""Local Russian command listener.

The process prints newline-delimited JSON only: heard text and a confidence.
It never names a system command. The Rust core maps the text through its allowlist.
"""

from __future__ import annotations

import array
import json
import math
import os
import queue
import sys
import threading
from pathlib import Path

PROTOCOL_VERSION = 1
SAMPLE_RATE = 16000
MODEL_NAME = "vosk-model-small-ru-0.22"


def emit(kind: str, **fields: object) -> None:
    message = {"version": PROTOCOL_VERSION, "type": kind, **fields}
    sys.stdout.write(json.dumps(message, ensure_ascii=False) + "\n")
    sys.stdout.flush()


class NoiseGate:
    """High-pass plus a noise floor. Quiet room noise becomes silence for Vosk."""

    def __init__(self) -> None:
        self.floor = 280.0
        self.prev = 0.0
        self.hp = 0.0
        self.hang = 0

    def process(self, raw: bytes) -> tuple[bytes, bool]:
        samples = array.array("h")
        samples.frombytes(raw)
        if not samples:
            return raw, False
        acc = 0.0
        prev = self.prev
        hp = self.hp
        for index, sample in enumerate(samples):
            current = float(sample)
            hp = 0.97 * (hp + current - prev)
            prev = current
            value = int(max(-32767, min(32767, hp)))
            samples[index] = value
            acc += hp * hp
        self.prev = prev
        self.hp = hp
        rms = math.sqrt(acc / len(samples))
        if rms < self.floor * 1.6:
            self.floor = self.floor * 0.94 + rms * 0.06
        self.floor = min(2200.0, max(160.0, self.floor))
        if rms >= max(380.0, self.floor * 3.1):
            self.hang = 2
            return samples.tobytes(), True
        if self.hang > 0:
            self.hang -= 1
            return samples.tobytes(), True
        return bytes(len(raw)), False


def utterance_confidence(payload: dict) -> float:
    words = payload.get("result") or []
    scores = []
    for word in words:
        if isinstance(word, dict):
            try:
                scores.append(float(word.get("conf", 0.0)))
            except (TypeError, ValueError):
                continue
    if not scores:
        return 0.0
    return sum(scores) / len(scores)


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
        recognizer = KaldiRecognizer(self.model, SAMPLE_RATE)
        recognizer.SetWords(True)
        audio: queue.Queue[bytes] = queue.Queue(maxsize=12)
        gate = NoiseGate()
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
            speech_chunks = 0
            while not self._stop.is_set():
                try:
                    chunk = audio.get(timeout=0.2)
                except queue.Empty:
                    continue
                cleaned, voiced = gate.process(chunk)
                if voiced:
                    speech_chunks += 1
                if not recognizer.AcceptWaveform(cleaned):
                    continue
                heard = speech_chunks
                speech_chunks = 0
                try:
                    payload = json.loads(recognizer.Result())
                except json.JSONDecodeError:
                    continue
                text = str(payload.get("text", "")).strip().lower().replace("ё", "е")
                if not text or text == "[unk]" or heard < 1:
                    continue
                confidence = utterance_confidence(payload)
                if confidence == 0.0 and heard >= 2:
                    confidence = 0.7
                if confidence < 0.55:
                    continue
                emit("voice", text=text[:120], confidence=round(confidence, 3))

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
