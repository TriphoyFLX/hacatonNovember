"""Build a small cyan mark for the window and the executable."""

import struct
import zlib
from pathlib import Path


def chunk(tag: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)


def png(size: int) -> bytes:
    rows = []
    for y in range(size):
        row = bytearray([0])
        for x in range(size):
            nx = (x + 0.5) / size
            ny = (y + 0.5) / size
            dx = nx - 0.50
            dy = ny - 0.52
            ring = abs((dx * dx + dy * dy) ** 0.5 - 0.30)
            finger = abs(nx - 0.50) < 0.035 and 0.18 < ny < 0.52
            palm = (nx - 0.50) ** 2 / 0.07 + (ny - 0.62) ** 2 / 0.035 < 1.0
            edge = min(x, y, size - 1 - x, size - 1 - y) < 1
            if edge:
                color = (8, 12, 18, 0)
            elif ring < 0.025 or finger or palm:
                color = (62, 224, 255, 255)
            elif ring < 0.055:
                color = (28, 92, 150, 255)
            else:
                color = (12, 16, 22, 255)
            row.extend(color)
        rows.append(bytes(row))
    raw = b"".join(rows)
    return b"".join(
        [
            b"\x89PNG\r\n\x1a\n",
            chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)),
            chunk(b"IDAT", zlib.compress(raw, 9)),
            chunk(b"IEND", b""),
        ]
    )


def ico(png_bytes: bytes, size: int) -> bytes:
    header = struct.pack("<HHH", 0, 1, 1)
    entry = struct.pack("<BBBBHHII", size if size < 256 else 0, size if size < 256 else 0, 0, 0, 1, 32, len(png_bytes), 22)
    return header + entry + png_bytes


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    image = png(64)
    target = root / "crates" / "air-control-app" / "ui"
    target.mkdir(parents=True, exist_ok=True)
    (target / "icon.png").write_bytes(image)
    (target / "icon.ico").write_bytes(ico(image, 64))
    (root / "assets").mkdir(exist_ok=True)
    (root / "assets" / "icon.png").write_bytes(image)


if __name__ == "__main__":
    main()
