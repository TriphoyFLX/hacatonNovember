# FEEL AI

Ощути мир. Управляй им по-своему.

Интерактивная desktop-платформа для взаимодействия через камеру: лицо, руки, тело, генеративная графика и локальная персонализация.

Репозиторий: [hacatonNovember](https://github.com/TriphoyFLX/hacatonNovember)

## Стек

- Electron (оболочка, безопасный IPC)
- React 19 + TypeScript + Vite
- Zustand, Zod, CSS Modules
- MediaPipe / Three.js / Tone.js / Prisma — на следующих этапах

## Требования

- Node.js 20+
- pnpm 9+
- macOS или Windows с веб-камерой для этапов с vision

## Установка

```bash
pnpm install
```

## Разработка

```bash
pnpm dev
```

Запускает Vite и открывает окно Electron с горячей перезагрузкой.

## Скрипты

| Команда | Описание |
| --- | --- |
| `pnpm dev` | Приложение в режиме разработки |
| `pnpm typecheck` | Проверка TypeScript |
| `pnpm lint` | ESLint |
| `pnpm format` | Prettier |
| `pnpm build` | Production-сборка renderer и Electron |
| `pnpm pack` | Распакованная сборка electron-builder |
| `pnpm dist` | Установщики |

## Архитектура

```
electron/
  main/          Main Process Electron + IPC-обработчики
  preload/       API через contextBridge (window.feelAI)
shared/
  ipc/           Типизированные IPC-каналы и контракты
src/
  components/    UI и layout
  pages/         Экраны маршрутов
  modules/       Функциональные модули (камера, vision, режимы…)
  stores/        Состояние Zustand
  styles/        Глобальные токены
```

Безопасность по умолчанию:

- `contextIsolation: true`
- `nodeIntegration: false`
- Renderer обращается к Node только через типизированный preload IPC

## Авторизация

Локальные профили без облака:

- регистрация и вход;
- пароль хранится как scrypt-хеш в `userData`;
- сессия восстанавливается при запуске;
- защищённые маршруты приложения.

## Текущий этап

Этап 0 + авторизация:

- Bootstrap Electron + React
- Типизированный IPC
- Локальная авторизация (регистрация / вход / выход)
- Оболочка приложения, Главная, Настройки, маршруты режимов
- Структура папок под будущие vision-режимы

Трекинг камеры и интерактивные режимы пока намеренно не реализованы.
