import type { CircleZone, ZoneRect } from './types';

const PIANO_NOTES = [
  'C3',
  'D3',
  'E3',
  'F3',
  'G3',
  'A3',
  'B3',
  'C4',
  'D4',
  'E4',
  'F4',
  'G4',
  'A4',
  'B4',
] as const;

export function createPianoZones(): ZoneRect[] {
  const count = PIANO_NOTES.length;
  const margin = 0.04;
  const totalWidth = 1 - margin * 2;
  const keyWidth = totalWidth / count;
  const y = 0.68;
  const h = 0.24;

  return PIANO_NOTES.map((note, index) => ({
    id: `piano:${note}`,
    label: note,
    x: margin + index * keyWidth,
    y,
    w: keyWidth * 0.92,
    h,
    color: index % 2 === 0 ? '#38bdf8' : '#a78bfa',
  }));
}

export function createDrumZones(): CircleZone[] {
  return [
    { id: 'drums:kick', label: 'Kick', x: 0.5, y: 0.78, r: 0.09, color: '#f472b6' },
    { id: 'drums:snare', label: 'Snare', x: 0.32, y: 0.7, r: 0.075, color: '#38bdf8' },
    { id: 'drums:hat', label: 'Hat', x: 0.68, y: 0.7, r: 0.07, color: '#34d399' },
    { id: 'drums:tom', label: 'Tom', x: 0.2, y: 0.82, r: 0.07, color: '#a78bfa' },
    { id: 'drums:crash', label: 'Crash', x: 0.8, y: 0.82, r: 0.075, color: '#fbbf24' },
  ];
}

export { createGuitarChordZones } from './instruments/guitarVoicings';

export function pointInRect(x: number, y: number, zone: ZoneRect): boolean {
  return x >= zone.x && x <= zone.x + zone.w && y >= zone.y && y <= zone.y + zone.h;
}

export function pointInCircle(x: number, y: number, zone: CircleZone): boolean {
  return Math.hypot(x - zone.x, y - zone.y) <= zone.r;
}
