# AIR CONTROL

Desktop-приложение на Rust + Slint и локальном MediaPipe. Рука перед веб-камерой двигает курсор, кликает, перетаскивает и запускает сцену с инструментами. Кадры не уходят с компьютера.

Поддерживаются **Windows** (основная платформа) и **macOS** (сборка и разработка; ввод через Accessibility).

## Windows

```powershell
powershell -ExecutionPolicy Bypass -File scripts\setup.ps1
cargo build --release -p air-control-app
powershell -ExecutionPolicy Bypass -File scripts\run.ps1
```

Нужны Rust stable и Python 3.11 или 3.12.

## macOS

```bash
# Xcode CLT + Homebrew Python 3.11/3.12
xcode-select --install
brew install python@3.12 rust

chmod +x scripts/setup.sh scripts/run.sh
./scripts/setup.sh
cargo build -p air-control-app
./scripts/run.sh
```

После первого запуска:

1. Разрешите **камеру** для `air-control`.
2. В **Системные настройки → Конфиденциальность и безопасность → Универсальный доступ** добавьте бинарь/`Terminal`/`Cursor`, иначе Enigo не сможет двигать мышь.
3. Для звука сцены включите **IAC Driver** в Audio MIDI Setup или подключите любой MIDI-выход — приложение шлёт midir.

Опционально укажите интерпретатор:

```bash
export AIR_CONTROL_PYTHON="$PWD/vision/.venv/bin/python"
export AIR_CONTROL_ROOT="$PWD"
```

Глобальная пауза `Ctrl+Shift+F12` сейчас только на Windows; на Mac — кнопка паузы в окне.

## Жесты

- Указательный палец двигает курсор. Сглаживание — One Euro Filter, плюс короткое предсказание по задержке потока.
- Короткий щипок — левый клик. Щипок с движением — перетаскивание. Два коротких щипка — двойной клик.
- Знак V — правый клик. Свайп вверх и вниз — прокрутка. Большой палец — громкость.
- Две руки: ведущая держит курсор, вторая меняет громкость.
- Профили выбираются по имени процесса активного окна.
- Свой жест — записанный шаблон и динамическое выравнивание, не обучение модели.
- Калибровка запоминает вашу руку: ладонь, указательный, щипок, большой, зона, проверка и тренировка.
- Пауза: кнопка или Ctrl+Shift+F12 (Windows). Аварийная остановка отпускает кнопки.

Команды проходят через один диспетчер и белый список. Жест не запускает shell и не повышает права.

## Проверки

```powershell
cargo test --workspace
vision\.venv\Scripts\python.exe -m unittest discover -s vision\tests
```

```bash
cargo test --workspace
vision/.venv/bin/python -m unittest discover -s vision/tests
```

## Сборка установщика (Windows)

```powershell
powershell -ExecutionPolicy Bypass -File scripts\package.ps1
```
