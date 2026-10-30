import type { ColorMode } from './types';

export function colorForStroke(
  mode: ColorMode,
  x: number,
  speed: number,
  now: number,
  handIndex: number,
): string {
  if (mode === 'emerald') {
    const t = 0.45 + Math.min(1, speed) * 0.4;
    return `hsla(158, 84%, ${38 + t * 28}%, 0.95)`;
  }

  if (mode === 'aurora') {
    const hue = (now / 40 + x * 140 + handIndex * 55) % 360;
    return `hsla(${hue}, 85%, 62%, 0.92)`;
  }

  if (mode === 'speed') {
    const hue = 200 - Math.min(1, speed / 3.2) * 160;
    return `hsla(${hue}, 90%, 60%, 0.95)`;
  }

  const hue = (x * 320 + handIndex * 40) % 360;
  return `hsla(${hue}, 92%, 58%, 0.95)`;
}
