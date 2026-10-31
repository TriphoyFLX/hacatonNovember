# AIR CONTROL на macOS

Краткая шпаргалка для разработки на Mac рядом с Windows-репо.

## Быстрый старт

```bash
brew install python@3.12 rust
chmod +x scripts/setup.sh scripts/run.sh
./scripts/setup.sh
cargo build -p air-control-app
./scripts/run.sh
```

## Права

| Что | Зачем |
|-----|--------|
| Камера | MediaPipe / OpenCV |
| Универсальный доступ | Enigo двигает курсор и жмёт клавиши |
| IAC Driver / MIDI out | Звук сцены через midir |

## Что отличается от Windows

- Камера: `CAP_AVFOUNDATION`, без `pygrabber`
- Python venv: `vision/.venv/bin/python`
- MIDI: `midir` (не WinMM mapper)
- Глобальный hotkey `Ctrl+Shift+F12` пока только на Windows — на Mac пауза из UI
- Размер экрана: CoreGraphics

## Переменные

```bash
export AIR_CONTROL_ROOT="$PWD"
export AIR_CONTROL_PYTHON="$PWD/vision/.venv/bin/python"
export AIR_CONTROL_MODEL="$PWD/vision/models/hand_landmarker.task"  # опционально
```

## Тесты

```bash
cargo test --workspace
vision/.venv/bin/python -m unittest discover -s vision/tests
```
