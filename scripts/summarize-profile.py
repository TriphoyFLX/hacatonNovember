#!/usr/bin/env python3
"""Aggregate AIR_CONTROL_PROFILE JSONL samples into avg/p50/p95/max tables."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import Any


def percentile(sorted_vals: list[float], p: float) -> float | None:
    if not sorted_vals:
        return None
    if len(sorted_vals) == 1:
        return sorted_vals[0]
    # Nearest-rank.
    rank = max(1, math.ceil((p / 100.0) * len(sorted_vals)))
    return sorted_vals[rank - 1]


def stats(values: list[float]) -> dict[str, Any]:
    if not values:
        return {"count": 0, "avg": None, "p50": None, "p95": None, "max": None}
    ordered = sorted(values)
    return {
        "count": len(ordered),
        "avg": sum(ordered) / len(ordered),
        "p50": percentile(ordered, 50),
        "p95": percentile(ordered, 95),
        "max": ordered[-1],
    }


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    rows: list[dict[str, Any]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    return rows


def collect(rows: list[dict[str, Any]], key: str) -> list[float]:
    out: list[float] = []
    for row in rows:
        value = row.get(key)
        if isinstance(value, (int, float)) and math.isfinite(float(value)):
            out.append(float(value))
    return out


def fps_from_mono(mono: list[float]) -> float | None:
    if len(mono) < 2:
        return None
    span_s = (mono[-1] - mono[0]) / 1000.0
    if span_s <= 0:
        return None
    return (len(mono) - 1) / span_s


def fmt_ms(value: float | None) -> str:
    if value is None:
        return "—"
    return f"{value:.3f}"


def print_stage(name: str, data: dict[str, Any]) -> None:
    print(
        f"{name:<28} {data['count']:>7}  "
        f"{fmt_ms(data['avg']):>8}  {fmt_ms(data['p50']):>8}  "
        f"{fmt_ms(data['p95']):>8}  {fmt_ms(data['max']):>8}"
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "dir",
        nargs="?",
        default="/tmp/air-control-profile",
        help="AIR_CONTROL_PROFILE_DIR",
    )
    args = parser.parse_args()
    root = Path(args.dir)
    vision = load_jsonl(root / "vision.jsonl")
    rust = load_jsonl(root / "rust.jsonl")
    ui = load_jsonl(root / "ui.jsonl")
    cpu_lines = (root / "cpu.txt").read_text(encoding="utf-8") if (root / "cpu.txt").exists() else ""

    cameras = [row for row in vision if row.get("type") == "camera"]
    samples = [row for row in vision if row.get("type") == "sample"]
    parses = [row for row in rust if row.get("type") == "parse"]
    renders = [row for row in rust if row.get("type") == "render"]
    ui_rows = [row for row in ui if row.get("type") == "ui_refresh"]

    print(f"Profile dir: {root}")
    print()
    if cameras:
        print("Camera after open (last record):")
        print(json.dumps(cameras[-1], indent=2, ensure_ascii=False))
        print()
    else:
        print("Camera after open: (no camera event — was the camera started?)")
        print()

    print(f"{'stage':<28} {'count':>7}  {'avg_ms':>8}  {'p50_ms':>8}  {'p95_ms':>8}  {'max_ms':>8}")
    print("-" * 78)
    stages = [
        ("1 capture.read", collect(samples, "capture_ms")),
        ("2 infer resize", collect(samples, "infer_resize_ms")),
        ("3 BGR→RGB", collect(samples, "bgr2rgb_ms")),
        ("4 MediaPipe detect", collect(samples, "mediapipe_ms")),
        ("5 preview resize", collect(samples, "preview_resize_ms")),
        ("6 JPEG encode", collect(samples, "jpeg_ms")),
        ("7 base64 encode", collect(samples, "b64_ms")),
        ("8a JSON dumps", collect(samples, "json_ms")),
        ("8b stdout write", collect(samples, "stdout_ms")),
        ("9 Rust JSON parse", collect(parses, "parse_ms")),
        ("10 Rust base64 decode", collect(parses, "b64_ms")),
        ("11 JPEG decode", collect(renders, "jpeg_decode_ms")),
        ("12a Rust downscale", collect(renders, "downscale_ms")),
        ("12b paint overlays", collect(renders, "paint_ms")),
        ("13 Slint frame_image", collect(ui_rows, "frame_image_ms")),
    ]
    computed = []
    for name, values in stages:
        data = stats(values)
        computed.append((name, data))
        print_stage(name, data)

    print()
    print("Payload sizes")
    print(f"{'metric':<28} {'count':>7}  {'avg':>8}  {'p50':>8}  {'p95':>8}  {'max':>8}")
    print("-" * 78)
    for name, key, rows in (
        ("JPEG bytes (vision)", "jpeg_bytes", samples),
        ("JSON bytes (vision)", "json_bytes", samples),
        ("JPEG bytes (rust parse)", "jpeg_bytes", parses),
        ("JSON line bytes (rust)", "json_bytes", parses),
    ):
        data = stats(collect(rows, key))
        print(
            f"{name:<28} {data['count']:>7}  "
            f"{fmt_ms(data['avg']):>8}  {fmt_ms(data['p50']):>8}  "
            f"{fmt_ms(data['p95']):>8}  {fmt_ms(data['max']):>8}"
        )

    print()
    capture_mono = collect(samples, "mono_ms")
    preview_mono = collect([row for row in samples if row.get("had_preview")], "mono_ms")
    tracking_mono = collect(parses, "mono_ms") or capture_mono
    ui_mono = collect(ui_rows, "mono_ms")
    print("Independent FPS (from sample mono timestamps, not HUD):")
    print(f"  capture FPS  (vision samples):     {fps_from_mono(capture_mono)}")
    print(f"  tracking FPS (rust parse / vision): {fps_from_mono(tracking_mono)}")
    print(f"  preview FPS  (had_preview samples): {fps_from_mono(preview_mono)}")
    print(f"  UI refresh FPS (successful set_*):  {fps_from_mono(ui_mono)}")
    if capture_mono:
        print(
            f"  vision sample span: {(capture_mono[-1] - capture_mono[0]) / 1000.0:.2f}s, "
            f"n={len(capture_mono)}"
        )
    if ui_mono:
        print(
            f"  UI refresh span: {(ui_mono[-1] - ui_mono[0]) / 1000.0:.2f}s, n={len(ui_mono)}"
        )

    costly = [
        (name, data)
        for name, data in computed
        if data["count"] > 0 and data["avg"] is not None
    ]
    if costly:
        costly.sort(key=lambda item: item[1]["avg"], reverse=True)
        print()
        print("Most expensive by average:")
        for name, data in costly[:5]:
            print(f"  {name}: avg={data['avg']:.3f} ms (n={data['count']})")
        over = [
            (name, data)
            for name, data in costly
            if data["p95"] is not None and data["p95"] > 33.3
        ]
        print()
        print("Stages with P95 > 33.3 ms budget:")
        if over:
            for name, data in over:
                print(f"  {name}: p95={data['p95']:.3f} ms")
        else:
            print("  (none)")

    if cpu_lines.strip():
        print()
        print("CPU sampler (raw tail):")
        print("\n".join(cpu_lines.strip().splitlines()[-20:]))
    else:
        print()
        print("CPU sampler: cpu.txt missing (use scripts/profile-camera.sh)")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
