export type BrushId = 'neon' | 'ribbon' | 'spark';
export type ColorMode = 'spectrum' | 'speed' | 'emerald' | 'aurora';

export interface MotionArtSettings {
  brush: BrushId;
  colorMode: ColorMode;
  trailFade: number;
  thickness: number;
  showSkeleton: boolean;
  showDebug: boolean;
  panelOpen: boolean;
}

export const BRUSHES: Array<{ id: BrushId; label: string }> = [
  { id: 'neon', label: 'Неон' },
  { id: 'ribbon', label: 'Лента' },
  { id: 'spark', label: 'Искры' },
];

export const COLOR_MODES: Array<{ id: ColorMode; label: string }> = [
  { id: 'spectrum', label: 'Спектр' },
  { id: 'speed', label: 'Скорость' },
  { id: 'emerald', label: 'Изумруд' },
  { id: 'aurora', label: 'Аврора' },
];
