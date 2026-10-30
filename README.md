# FEEL AI / AIR CONTROL

Ощути мир. Управляй им по-своему.

Feel the digital world.

В репозитории две части одной темы.

## FEEL AI

Оболочка Electron + React. Локальная авторизация, навигация по режимам и **Воздушные инструменты**: камера, MediaPipe Hand Landmarker, Tone.js (Piano / Drums / Guitar).

```bash
pnpm install
pnpm dev
```

Нужны Node.js 20+ и pnpm 9+.

### Архитектура оболочки

```
electron/
  main/          Main Process Electron + IPC-обработчики
  preload/       API через contextBridge (window.feelAI)
shared/
  ipc/           Типизированные IPC-каналы и контракты
src/
  components/    UI и layout
  pages/         Экраны маршрутов
  modules/       Камера, vision, воздушные инструменты
  stores/        Состояние Zustand
  styles/        Глобальные токены
public/
  models/        Hand Landmarker
  mediapipe/     Локальный WASM
  audio/         Сэмплы гитары
```

Безопасность по умолчанию:

- `contextIsolation: true`, `nodeIntegration: false`
- Renderer → Node только через типизированный preload IPC
- Локальные профили: пароль как scrypt-хеш в `userData`

| Команда | Описание |
| --- | --- |
| `pnpm dev` | Окно Electron |
| `pnpm typecheck` | Проверка TypeScript |
| `pnpm lint` | ESLint |
| `pnpm format` | Prettier |
| `pnpm build` | Production-сборка |
| `pnpm dist` | Установщики |

### Воздушные инструменты

- Левая рука — аккорд / mute, правая — пальцы по струнам или strum
- Guitar: bass / mid / high шины + one-shot сэмпл
- Взмах двумя руками вверх — смена инструмента

## AIR CONTROL

Отдельное Windows-приложение на Rust и локальном MediaPipe. Оно уже управляет курсором, кликами, перетаскиванием и прокруткой через веб-камеру. Кадры не уходят с компьютера.

```powershell
powershell -ExecutionPolicy Bypass -File scripts\setup.ps1
cargo build --release -p air-control-app
powershell -ExecutionPolicy Bypass -File scripts\run.ps1
```

Нужны Rust stable и Python 3.11 или 3.12.

## Жесты AIR CONTROL

- Указательный палец двигает курсор. Сглаживание — One Euro Filter, плюс короткая добавка скорости, которая возвращается к положению пальца.
- Короткий щипок — левый клик. Щипок с движением — перетаскивание. Два коротких щипка Windows получает как двойной клик.
- Знак V — правый клик. Свайп вверх и вниз — прокрутка. Большой палец — громкость.
- Две руки: ведущая держит курсор, вторая меняет громкость. Две открытые ладони меняют масштаб.
- Профили выбираются по имени процесса активного окна. Незнакомое приложение остаётся в профиле рабочего стола.
- Свой жест — записанный шаблон и динамическое выравнивание, не обучение модели.
- Калибровка меняет зону руки и порог щипка. MediaPipe при этом не дообучается.
- Пауза: кнопка или Ctrl+Shift+F12. Аварийная остановка отпускает кнопки.

Команды проходят через один диспетчер и белый список. Жест не запускает shell и не повышает права.

## Проверки AIR CONTROL

```powershell
cargo test --workspace
vision\.venv\Scripts\python.exe -m unittest discover -s vision\tests
```

Числа задержки в окне диагностики — измерения текущей сессии.
