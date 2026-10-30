"""Versioned newline-delimited JSON. The vision process never emits commands."""

from __future__ import annotations

import time
from typing import Any

PROTOCOL_VERSION = 1


def _base(kind: str) -> dict[str, Any]:
    return {"version": PROTOCOL_VERSION, "type": kind}


def hello_message(process_id: int, mediapipe: str) -> dict[str, Any]:
    message = _base("hello")
    message["process_id"] = process_id
    message["mediapipe"] = mediapipe
    return message


def heartbeat_message() -> dict[str, Any]:
    message = _base("heartbeat")
    message["timestamp_ms"] = int(time.time() * 1000)
    return message


def error_message(code: str, text: str) -> dict[str, Any]:
    message = _base("error")
    message["code"] = code
    message["message"] = text[:300]
    return message


def cameras_message(devices: list[dict[str, Any]]) -> dict[str, Any]:
    message = _base("cameras")
    message["devices"] = devices
    return message


def camera_status(state: str, index: int, width: int, height: int, fps: int, text: str) -> dict[str, Any]:
    message = _base("camera_status")
    message.update({"state": state, "index": index, "width": width, "height": height, "fps": fps, "message": text[:240]})
    return message


def tracking_message(
    frame_id: int,
    width: int,
    height: int,
    capture_ms: float,
    inference_ms: float,
    hands: list[dict[str, Any]],
    preview_jpeg_b64: str | None,
) -> dict[str, Any]:
    message = _base("tracking")
    message.update(
        {
            "timestamp_ms": int(time.time() * 1000),
            "capture_ms": round(capture_ms, 3),
            "inference_ms": round(inference_ms, 3),
            "frame_id": frame_id,
            "width": width,
            "height": height,
            "hands": hands,
            "preview_jpeg_b64": preview_jpeg_b64,
        }
    )
    return message


def goodbye_message() -> dict[str, Any]:
    return _base("goodbye")
