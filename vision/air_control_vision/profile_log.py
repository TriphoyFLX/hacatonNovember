"""Optional vision pipeline profiler (AIR_CONTROL_PROFILE=1).

Writes JSONL samples to AIR_CONTROL_PROFILE_DIR / vision.jsonl.
Never touches stdout (protocol stays clean).
"""

from __future__ import annotations

import json
import os
import threading
import time
from pathlib import Path
from typing import Any

_ENABLED = os.environ.get("AIR_CONTROL_PROFILE") == "1"
_DIR = Path(os.environ.get("AIR_CONTROL_PROFILE_DIR", "/tmp/air-control-profile"))
_LOCK = threading.Lock()
_FILE: Any = None
_BROKEN = False
_START = time.perf_counter()
_LINES = 0


def enabled() -> bool:
    return _ENABLED and not _BROKEN


def mono_ms() -> float:
    return (time.perf_counter() - _START) * 1000.0


def _mark_broken(exc: BaseException) -> None:
    global _BROKEN, _FILE
    if _BROKEN:
        return
    _BROKEN = True
    _FILE = None
    try:
        sys_stderr = __import__("sys").stderr
        print(f"AIR_CONTROL_PROFILE: vision log disabled: {exc}", file=sys_stderr)
    except Exception:  # noqa: BLE001
        pass


def _writer() -> Any | None:
    global _FILE
    if not enabled():
        return None
    if _FILE is not None:
        return _FILE
    with _LOCK:
        if _FILE is not None or _BROKEN:
            return _FILE
        try:
            _DIR.mkdir(parents=True, exist_ok=True)
            _FILE = open(_DIR / "vision.jsonl", "a", encoding="utf-8", buffering=1024 * 16)
        except OSError as exc:
            _mark_broken(exc)
            return None
    return _FILE


def write_event(payload: dict[str, Any]) -> None:
    global _LINES
    handle = _writer()
    if handle is None:
        return
    payload = dict(payload)
    payload.setdefault("mono_ms", round(mono_ms(), 3))
    try:
        line = json.dumps(payload, separators=(",", ":"))
        with _LOCK:
            handle.write(line + "\n")
            _LINES += 1
            if _LINES % 32 == 0:
                handle.flush()
    except OSError as exc:
        _mark_broken(exc)


def write_camera(props: dict[str, Any]) -> None:
    event = {"type": "camera"}
    event.update(props)
    write_event(event)


def write_sample(fields: dict[str, Any]) -> None:
    event = {"type": "sample"}
    event.update(fields)
    write_event(event)


def fourcc_to_str(value: float | int) -> str:
    try:
        code = int(value)
    except (TypeError, ValueError):
        return ""
    if code <= 0:
        return ""
    chars: list[str] = []
    for shift in (0, 8, 16, 24):
        chars.append(chr((code >> shift) & 0xFF))
    return "".join(chars).strip() or str(code)


def capture_props(cv2: Any, capture: Any, *, backend: int | None, mjpg: bool | None, lock: bool | None) -> dict[str, Any]:
    def get(name: str) -> float | None:
        prop = getattr(cv2, name, None)
        if prop is None:
            return None
        try:
            return float(capture.get(prop))
        except Exception:  # noqa: BLE001
            return None

    width = get("CAP_PROP_FRAME_WIDTH")
    height = get("CAP_PROP_FRAME_HEIGHT")
    fps = get("CAP_PROP_FPS")
    fourcc = get("CAP_PROP_FOURCC")
    buffersize = get("CAP_PROP_BUFFERSIZE")
    return {
        "backend": backend,
        "mjpg_requested": mjpg,
        "lock_requested": lock,
        "width": int(width) if width is not None else None,
        "height": int(height) if height is not None else None,
        "fps": fps,
        "fourcc": fourcc_to_str(fourcc) if fourcc is not None else "",
        "fourcc_raw": fourcc,
        "buffersize": buffersize,
    }
