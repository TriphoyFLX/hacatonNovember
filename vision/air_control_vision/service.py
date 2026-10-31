"""Camera capture and MediaPipe Hand Landmarker.

The process prints JSON lines on stdout and logs on stderr. It accepts only
configure, list_cameras, and shutdown on stdin.
"""

from __future__ import annotations

import json
import os
import sys
import threading
import time
from pathlib import Path
from typing import Any

from air_control_vision.protocol import (
    camera_status,
    cameras_message,
    error_message,
    goodbye_message,
    heartbeat_message,
    hello_message,
    tracking_message,
)

_PRINT_LOCK = threading.Lock()


def emit(message: dict[str, Any]) -> None:
    line = json.dumps(message, separators=(",", ":"))
    with _PRINT_LOCK:
        sys.stdout.write(line + "\n")
        sys.stdout.flush()


class State:
    def __init__(self) -> None:
        self.lock = threading.Lock()
        self.stop = False
        self.config: dict[str, Any] | None = None
        self.need_cameras = False
        self.generation = 0

    def request_stop(self) -> None:
        with self.lock:
            self.stop = True

    def stopped(self) -> bool:
        with self.lock:
            return self.stop

    def request_cameras(self) -> None:
        with self.lock:
            self.need_cameras = True

    def take_cameras(self) -> bool:
        with self.lock:
            needed = self.need_cameras
            self.need_cameras = False
            return needed

    def set_config(self, config: dict[str, Any]) -> None:
        """Apply config. Reopen the device only when index/size change.

        preview / preview_every / mirror / fps are soft — flipping them must not
        tear down VideoCapture (that was reopening the camera on every page switch).
        """
        hard_keys = ("camera_index", "width", "height")
        with self.lock:
            prev = self.config
            hard = prev is None or any(prev.get(key) != config.get(key) for key in hard_keys)
            self.config = config
            if hard:
                self.generation += 1

    def clear_config(self) -> None:
        with self.lock:
            self.config = None
            self.generation += 1

    def current_config(self) -> tuple[int, dict[str, Any] | None]:
        with self.lock:
            return self.generation, None if self.config is None else dict(self.config)


def local_model_copy() -> Path:
    base = Path(os.environ.get("LOCALAPPDATA") or (Path.home() / "AppData" / "Local"))
    return base / "AirControl" / "models" / "hand_landmarker.task"


def model_candidates() -> list[Path]:
    paths: list[Path] = []
    override = os.environ.get("AIR_CONTROL_MODEL")
    if override:
        paths.append(Path(override))
    paths.append(local_model_copy())
    paths.append(Path(__file__).resolve().parents[1] / "models" / "hand_landmarker.task")
    unique: list[Path] = []
    seen: set[str] = set()
    for path in paths:
        key = str(path)
        if key not in seen:
            seen.add(key)
            unique.append(path)
    return unique


def model_path() -> Path:
    return ensure_model()


def ensure_model() -> Path:
    """MediaPipe on Windows cannot open a .task file from a Cyrillic/OneDrive path.

    Python can read that file; we copy it to %LOCALAPPDATA%\\AirControl\\models.
    """
    wanted = local_model_copy()
    wanted.parent.mkdir(parents=True, exist_ok=True)
    min_size = 1_000_000
    if wanted.is_file() and wanted.stat().st_size >= min_size:
        return wanted
    for source in model_candidates():
        if source == wanted or not source.is_file():
            continue
        if source.stat().st_size < min_size:
            continue
        import shutil

        shutil.copy2(source, wanted)
        return wanted
    for source in model_candidates():
        if source.is_file():
            return source
    return wanted


def _dshow_names() -> list[str]:
    # DirectShow names are Windows-only; probing pygrabber on macOS just spams stderr.
    if sys.platform != "win32":
        return []
    try:
        try:
            import pythoncom

            pythoncom.CoInitialize()
        except Exception:
            pass
        from pygrabber.dshow_graph import FilterGraph

        return [str(name) for name in FilterGraph().get_input_devices() if str(name).strip()]
    except Exception as exc:  # noqa: BLE001 - device listing is best-effort
        print(f"camera names unavailable: {exc}", file=sys.stderr)
        return []


def _honor_machine() -> bool:
    try:
        import winreg

        key = winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r"HARDWARE\DESCRIPTION\System\BIOS")
        parts: list[str] = []
        for name in ("SystemManufacturer", "SystemProductName", "BaseBoardManufacturer"):
            try:
                parts.append(str(winreg.QueryValueEx(key, name)[0]))
            except OSError:
                continue
        blob = " ".join(parts).lower()
        return "honor" in blob or "magicbook" in blob
    except OSError:
        return False


def empty_camera_hint() -> str:
    if _honor_machine():
        return (
            "Windows не видит съёмный модуль Honor: питание не дошло до камеры. "
            "Снимите её с верхней рамки и поставьте снова до щелчка контактов. "
            "Магнит может держать корпус, а пины — нет."
        )
    return (
        "Windows не показал ни одной камеры. Разрешите доступ классическим приложениям "
        "или закройте программу, которая уже её заняла."
    )


def _named_cameras(cv2: Any) -> list[dict[str, Any]]:
    """Honor's module sometimes answers by name after DirectShow index list is empty."""
    found: list[dict[str, Any]] = []
    seen: set[str] = set()
    names = [
        "FHD Camera",
        "Honor Camera",
        "HD Camera",
        "USB Camera",
        "Integrated Camera",
        "webcam AC310",
    ]
    for name in names:
        key = name.casefold()
        if key in seen:
            continue
        capture = cv2.VideoCapture(name, getattr(cv2, "CAP_DSHOW", 700))
        ok = capture.isOpened() and _grab_frame(capture, 6)
        capture.release()
        if ok:
            seen.add(key)
            found.append({"index": len(found), "name": name, "available": True})
    return found


def _backends(cv2: Any) -> list[int]:
    backends: list[int] = []
    if sys.platform.startswith("win"):
        names = ("CAP_DSHOW", "CAP_MSMF", "CAP_ANY")
    elif sys.platform == "darwin":
        names = ("CAP_AVFOUNDATION", "CAP_ANY")
    else:
        names = ("CAP_V4L2", "CAP_ANY")
    for name in names:
        backend = getattr(cv2, name, None)
        if isinstance(backend, int) and backend not in backends:
            backends.append(backend)
    return backends


def _grab_frame(capture: Any, tries: int = 10) -> bool:
    """Detachable USB cameras often need a few frames after open."""
    for _ in range(max(1, tries)):
        ok, frame = capture.read()
        if ok and frame is not None and getattr(frame, "size", 0):
            return True
        time.sleep(0.04)
    return False


def collect_cameras(cv2: Any) -> list[dict[str, Any]]:
    names = _dshow_names()
    if names:
        return [{"index": index, "name": name, "available": True} for index, name in enumerate(names)]
    devices: list[dict[str, Any]] = []
    backends = _backends(cv2)[:1]
    # macOS: FaceTime is almost always index 0. Probing 1..7 only logs
    # "out device of bound". Still allow a short look at index 1 for Continuity.
    max_index = 2 if sys.platform == "darwin" else 8
    misses = 0
    for index in range(max_index):
        found = None
        for backend in backends:
            found = _try_open(
                cv2,
                index,
                backend,
                640,
                480,
                30,
                mjpg=False,
                lock=False,
                # macOS TCC / warm-up often needs more than 2 grabs on first open.
                reads=8 if sys.platform == "darwin" else 3,
            )
            if found is not None:
                break
        if found is not None:
            label = "FaceTime / камера" if sys.platform == "darwin" and index == 0 else f"Камера {index + 1}"
            devices.append({"index": index, "name": label, "available": True})
            found.release()
            misses = 0
        else:
            misses += 1
            # Only stop after a gap that follows a found device — never abort on
            # the first failed index when the list is still empty (that hid FaceTime).
            if devices and misses >= 1:
                break
    return devices


def list_cameras(cv2: Any) -> list[dict[str, Any]]:
    # Honor MagicBook Art magnetic module often appears a beat after process start.
    attempts = 4 if sys.platform == "darwin" else 6
    for attempt in range(attempts):
        names = _dshow_names()
        if names:
            return [{"index": index, "name": name, "available": True} for index, name in enumerate(names)]
        named = _named_cameras(cv2)
        if named:
            return named
        probed = collect_cameras(cv2)
        if probed:
            return probed
        if attempt + 1 < attempts:
            time.sleep(0.25 if sys.platform == "darwin" else 0.35)
    return _named_cameras(cv2)


def read_commands(state: State) -> None:
    for line in sys.stdin:
        text = line.strip()
        if not text:
            continue
        try:
            message = json.loads(text)
        except json.JSONDecodeError:
            emit(error_message("bad_command", "invalid json"))
            continue
        if message.get("version") != 1:
            emit(error_message("bad_version", "unsupported protocol version"))
            continue
        kind = message.get("type")
        if kind == "configure":
            state.set_config(message)
        elif kind == "list_cameras":
            state.request_cameras()
        elif kind == "release":
            state.clear_config()
        elif kind == "shutdown":
            state.request_stop()
            break
        else:
            emit(error_message("unknown_command", str(kind)))


def _try_open(
    cv2: Any,
    index: int,
    backend: int,
    width: int,
    height: int,
    fps: int,
    *,
    mjpg: bool,
    lock: bool,
    reads: int,
) -> Any | None:
    capture = cv2.VideoCapture(index, backend)
    if not capture.isOpened():
        capture.release()
        return None
    if mjpg:
        try:
            fourcc = cv2.VideoWriter_fourcc(*"MJPG")
            capture.set(cv2.CAP_PROP_FOURCC, fourcc)
        except Exception:  # noqa: BLE001
            pass
    capture.set(cv2.CAP_PROP_FRAME_WIDTH, width)
    capture.set(cv2.CAP_PROP_FRAME_HEIGHT, height)
    capture.set(cv2.CAP_PROP_FPS, fps)
    try:
        capture.set(cv2.CAP_PROP_BUFFERSIZE, 1)
    except Exception:  # noqa: BLE001 - not every backend accepts this hint
        pass
    if lock:
        lock_camera_optics(cv2, capture)
    if _grab_frame(capture, reads):
        return capture
    capture.release()
    return None


def open_capture(cv2: Any, config: dict[str, Any], *, light: bool = False) -> Any | None:
    index = int(config.get("camera_index", 0))
    width = int(config.get("width", 640))
    height = int(config.get("height", 480))
    fps = int(config.get("fps", 30))
    # Magnetic USB modules often reject MJPG or autofocus lock on first try.
    variants = ((False, False), (True, False), (True, True)) if light else ((True, True), (False, True), (False, False))
    reads = 5 if light else 12
    for backend in _backends(cv2):
        for mjpg, lock in variants:
            capture = _try_open(
                cv2,
                index,
                backend,
                width,
                height,
                fps,
                mjpg=mjpg,
                lock=lock,
                reads=reads,
            )
            if capture is not None:
                return capture
    return None


def lock_camera_optics(cv2: Any, capture: Any) -> None:
    """Freeze autofocus / auto-exposure so fingertips stop swimming between frames."""
    try_set = (
        ("CAP_PROP_AUTOFOCUS", 0),
        # DirectShow: 0.25 ≈ manual; MSMF often wants 1 for off — try both later if needed.
        ("CAP_PROP_AUTO_EXPOSURE", 0.25),
        ("CAP_PROP_EXPOSURE", -6),
        # Mid-range fixed focus (~arm length) when the driver exposes absolute focus.
        ("CAP_PROP_FOCUS", 55),
    )
    for name, value in try_set:
        prop = getattr(cv2, name, None)
        if prop is None:
            continue
        try:
            capture.set(prop, value)
        except Exception:  # noqa: BLE001
            pass


def create_landmarker(mp: Any, path: Path, num_hands: int = 2) -> Any:
    data = path.read_bytes()
    if len(data) < 1_000_000:
        raise RuntimeError(f"модель повреждена ({len(data)} байт): {path}")
    hands = max(1, min(2, int(num_hands)))
    # Bytes, not a path: TFLite on Windows fails on non-ASCII folders.
    last_error: Exception | None = None
    for kwargs in (
        {"model_asset_buffer": bytearray(data)},
        {"model_asset_path": str(path)},
    ):
        try:
            options = mp.tasks.vision.HandLandmarkerOptions(
                base_options=mp.tasks.BaseOptions(**kwargs),
                running_mode=mp.tasks.vision.RunningMode.VIDEO,
                num_hands=hands,
                min_hand_detection_confidence=0.5,
                min_hand_presence_confidence=0.45,
                min_tracking_confidence=0.4,
            )
            return mp.tasks.vision.HandLandmarker.create_from_options(options)
        except Exception as exc:  # noqa: BLE001
            last_error = exc
    raise last_error if last_error is not None else RuntimeError(f"не удалось открыть модель: {path}")


def frame_quality(cv2: Any, frame: Any, prev_gray: Any | None) -> tuple[float, float, float, Any]:
    """Return sharpness, brightness, flicker, and the gray frame for the next tick."""
    gray = cv2.cvtColor(frame, cv2.COLOR_BGR2GRAY)
    sharpness = float(cv2.Laplacian(gray, cv2.CV_64F).var())
    brightness = float(gray.mean())
    if prev_gray is not None and prev_gray.shape == gray.shape:
        flicker = float(cv2.absdiff(gray, prev_gray).mean())
    else:
        flicker = 0.0
    return sharpness, brightness, flicker, gray


def hands_from_result(result: Any, mirror: bool) -> list[dict[str, Any]]:
    hands: list[dict[str, Any]] = []
    landmarks = result.hand_landmarks or []
    worlds = getattr(result, "hand_world_landmarks", None) or []
    handedness = result.handedness or []
    for index, points in enumerate(landmarks):
        label = "Unknown"
        score = 0.0
        if index < len(handedness) and handedness[index]:
            label = str(handedness[index][0].category_name)
            score = float(handedness[index][0].score)
        packed = []
        for point in points:
            x = float(point.x)
            if mirror:
                x = 1.0 - x
            packed.append([round(x, 5), round(float(point.y), 5), round(float(point.z), 5)])
        entry: dict[str, Any] = {
            "handedness": label,
            "score": round(max(0.0, min(1.0, score)), 4),
            "landmarks": packed,
        }
        if index < len(worlds) and worlds[index] is not None and len(worlds[index]) == 21:
            entry["world"] = [
                [round(float(point.x), 5), round(float(point.y), 5), round(float(point.z), 5)]
                for point in worlds[index]
            ]
        hands.append(entry)
    return hands


def preview_b64(cv2: Any, frame: Any, mirror: bool, quality: int) -> str:
    image = cv2.flip(frame, 1) if mirror else frame
    ok, encoded = cv2.imencode(".jpg", image, [int(cv2.IMWRITE_JPEG_QUALITY), quality])
    if not ok:
        return ""
    import base64

    return base64.b64encode(encoded.tobytes()).decode("ascii")


def _beat(stop: threading.Event) -> None:
    while not stop.wait(1.0):
        emit(heartbeat_message())


def main() -> int:
    emit(hello_message(os.getpid(), "starting"))
    stop_beat = threading.Event()
    try:
        import cv2
        import mediapipe as mp
    except Exception as exc:  # noqa: BLE001
        emit(error_message("dependency", f"не удалось загрузить MediaPipe или OpenCV: {exc}"))
        return 1
    emit(hello_message(os.getpid(), getattr(mp, "__version__", "unknown")))
    # Threads start after the native libraries load. Starting them earlier
    # deadlocks the Windows loader while OpenCV and MediaPipe import.
    threading.Thread(target=_beat, args=(stop_beat,), name="air-vision-beat", daemon=True).start()
    state = State()
    threading.Thread(target=read_commands, args=(state,), name="air-vision-stdin", daemon=True).start()
    listed_any = False
    scan_count = 0
    last_scan = 0.0
    try:
        first_devices = list_cameras(cv2)
        emit(cameras_message(first_devices))
        listed_any = bool(first_devices)
        scan_count = 1
        last_scan = time.perf_counter()
    except Exception as exc:  # noqa: BLE001
        print(f"camera list failed: {exc}", file=sys.stderr)

    weights = ensure_model()
    if not weights.exists():
        emit(
            error_message(
                "model_missing",
                f"нет файла модели: {weights}. Запустите scripts/setup.ps1 или scripts/setup.sh",
            )
        )
        return 1
    num_hands = 2
    try:
        landmarker = create_landmarker(mp, weights, num_hands)
    except Exception as exc:  # noqa: BLE001
        emit(error_message("model", str(exc)))
        return 1

    capture = None
    applied = -1
    failures = 0
    open_attempts = 0
    last_generation = -1
    frame_id = 0
    started = time.perf_counter()
    last_timestamp = 0
    last_heartbeat = 0.0
    prev_gray = None
    emit(camera_status("waiting", 0, 0, 0, 0, "waiting for configure"))
    try:
        while not state.stopped():
            now = time.perf_counter()
            if now - last_heartbeat >= 1.0:
                emit(heartbeat_message())
                last_heartbeat = now
            want_scan = state.take_cameras()
            # Rare auto-scan only until the first non-empty list — never hammer indices 0..7.
            max_auto = 3 if sys.platform == "darwin" else 12
            if (
                not want_scan
                and not listed_any
                and capture is None
                and scan_count < max_auto
                and now - last_scan >= (1.0 if sys.platform == "darwin" else 0.5)
            ):
                want_scan = True
            if want_scan:
                devices = list_cameras(cv2)
                emit(cameras_message(devices))
                if not devices:
                    emit(error_message("camera_missing", empty_camera_hint()))
                listed_any = bool(devices)
                scan_count += 1
                last_scan = now
            generation, config = state.current_config()
            if config is None:
                if capture is not None:
                    capture.release()
                    capture = None
                    applied = -1
                    emit(camera_status("waiting", 0, 0, 0, 0, "camera off"))
                prev_gray = None
                time.sleep(0.05)
                continue
            if generation != applied:
                if capture is not None:
                    capture.release()
                    capture = None
                index = int(config.get("camera_index", 0))
                if generation != last_generation:
                    open_attempts = 0
                    last_generation = generation
                emit(camera_status("opening", index, 0, 0, 0, "opening"))
                capture = open_capture(cv2, config)
                applied = generation
                failures = 0
                prev_gray = None
                frame_id = 0
                if capture is None:
                    open_attempts += 1
                    if open_attempts == 1:
                        emit(
                            error_message(
                                "camera_open",
                                "Камера не открылась. Закройте программу, которая её заняла, или разрешите доступ в параметрах камеры Windows.",
                            )
                        )
                    emit(camera_status("reconnecting", index, 0, 0, 0, "open failed"))
                    if open_attempts < 5:
                        applied = -1
                        time.sleep(0.6)
                    else:
                        time.sleep(1.5)
                    continue
                open_attempts = 0
                # Listing can fail on first TCC prompt while open still works —
                # publish the live index so the UI does not stay on «не найдена».
                if not listed_any:
                    label = (
                        "FaceTime / камера"
                        if sys.platform == "darwin" and index == 0
                        else f"Камера {index + 1}"
                    )
                    emit(cameras_message([{"index": index, "name": label, "available": True}]))
                    listed_any = True
            if capture is None:
                time.sleep(0.05)
                continue

            target_fps = max(1, int(config.get("fps", 30)))
            loop_started = time.perf_counter()
            captured_at = time.perf_counter()
            ok, frame = capture.read()
            capture_ms = (time.perf_counter() - captured_at) * 1000.0
            if not ok or frame is None:
                failures += 1
                if failures >= 12:
                    emit(camera_status("reconnecting", int(config.get("camera_index", 0)), 0, 0, 0, "device lost"))
                    capture.release()
                    capture = None
                    applied = -1
                    failures = 0
                    time.sleep(0.4)
                continue
            failures = 0
            # MediaPipe landmarks are normalized — full-res inference only burns CPU.
            # Native Camera.app feels smooth because it skips hand tracking entirely.
            infer = frame
            fh, fw = frame.shape[:2]
            max_infer = 480
            if max(fh, fw) > max_infer:
                scale = max_infer / float(max(fh, fw))
                infer = cv2.resize(
                    frame,
                    (max(1, int(fw * scale)), max(1, int(fh * scale))),
                    interpolation=cv2.INTER_AREA,
                )
            sharpness, brightness, flicker, prev_gray = frame_quality(cv2, infer, prev_gray)
            rgb = cv2.cvtColor(infer, cv2.COLOR_BGR2RGB)
            mp_image = mp.Image(image_format=mp.ImageFormat.SRGB, data=rgb)
            # Prefer the capture driver's frame clock so dropped frames keep honest dt.
            pos_msec = 0.0
            try:
                pos_msec = float(capture.get(cv2.CAP_PROP_POS_MSEC) or 0.0)
            except Exception:  # noqa: BLE001
                pos_msec = 0.0
            if pos_msec > 0.0:
                timestamp = int(pos_msec)
            else:
                timestamp = int((time.perf_counter() - started) * 1000)
            if timestamp <= last_timestamp:
                timestamp = last_timestamp + 1
            last_timestamp = timestamp
            inferred_at = time.perf_counter()
            result = landmarker.detect_for_video(mp_image, timestamp)
            inference_ms = (time.perf_counter() - inferred_at) * 1000.0
            frame_id += 1
            mirror = bool(config.get("mirror", True))
            preview_every = max(1, int(config.get("preview_every", 2)))
            preview = None
            if bool(config.get("preview", True)) and frame_id % preview_every == 0:
                # Encode the already-shrunk frame — Rust downscales again for HUD anyway.
                encoded = preview_b64(cv2, infer, mirror, 65)
                preview = encoded or None
            height, width = infer.shape[:2]
            hands = hands_from_result(result, mirror)
            emit(
                tracking_message(
                    frame_id,
                    int(width),
                    int(height),
                    capture_ms,
                    inference_ms,
                    hands,
                    preview,
                    frame_ms=timestamp,
                    sharpness=sharpness,
                    brightness=brightness,
                    flicker=flicker,
                )
            )
            if frame_id == 1:
                emit(camera_status("streaming", int(config.get("camera_index", 0)), int(width), int(height), target_fps, "streaming"))
            elapsed = time.perf_counter() - loop_started
            budget = 1.0 / float(target_fps)
            if elapsed < budget:
                time.sleep(budget - elapsed)
    finally:
        stop_beat.set()
        if capture is not None:
            capture.release()
        landmarker.close()
        emit(goodbye_message())
    return 0
