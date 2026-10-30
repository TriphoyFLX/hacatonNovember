import type { CameraStatus, ThemeMode } from '@/stores/appStore';

export const cameraStatusLabel: Record<CameraStatus, string> = {
  idle: 'ожидание',
  requesting: 'запрос',
  active: 'активна',
  denied: 'отказано',
  unavailable: 'недоступна',
};

export const aiStatusLabel = {
  ready: 'готов',
  calibrating: 'калибровка',
  offline: 'офлайн',
} as const;

export const themeLabel: Record<ThemeMode, string> = {
  dark: 'тёмная',
  light: 'светлая',
};

export const modeStatusLabel = {
  planned: 'в планах',
  ready: 'готов',
} as const;
