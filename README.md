# FEEL AI

Feel the world. Control it your way.

Interactive desktop platform for camera-driven experiences: face, hands, body tracking, generative visuals, and local personalization.

## Stack

- Electron (desktop shell, secure IPC)
- React 19 + TypeScript + Vite
- Zustand, Zod, CSS Modules
- MediaPipe / Three.js / Tone.js / Prisma — planned for later stages

## Requirements

- Node.js 20+
- pnpm 9+
- macOS or Windows with a webcam for later vision stages

## Setup

```bash
pnpm install
```

## Development

```bash
pnpm dev
```

Starts Vite and launches the Electron window with hot reload.

## Scripts

| Command | Description |
| --- | --- |
| `pnpm dev` | Development app |
| `pnpm typecheck` | TypeScript project references check |
| `pnpm lint` | ESLint |
| `pnpm format` | Prettier write |
| `pnpm build` | Production renderer + Electron bundles |
| `pnpm pack` | Unpackaged electron-builder output |
| `pnpm dist` | Packaged installers |

## Architecture

```
electron/
  main/          Electron Main Process + IPC handlers
  preload/       contextBridge API (window.feelAI)
shared/
  ipc/           Typed IPC channels and contracts
src/
  components/    UI + layout
  pages/         Route screens
  modules/       Feature modules (camera, vision, modes…)
  stores/        Zustand state
  styles/        Global tokens
```

Security defaults:

- `contextIsolation: true`
- `nodeIntegration: false`
- Renderer talks to Node only through typed preload IPC

## Current stage

Stage 0 / base shell:

- Electron + React app bootstrap
- Typed IPC
- App shell, Home, Settings, mode routes
- Folder structure for upcoming vision modes

Camera tracking and interactive modes are intentionally not implemented yet.
