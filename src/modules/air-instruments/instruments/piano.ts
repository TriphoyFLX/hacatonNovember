import type { PianoPreset } from '../types';
import { createPianoZones } from '../zones';

export const pianoPresets: Array<{ id: PianoPreset; label: string }> = [
  { id: 'soft-pad', label: 'Soft Pad' },
  { id: 'bright-keys', label: 'Bright Keys' },
  { id: 'pluck', label: 'Pluck' },
];

export const pianoHint = 'Коснитесь клавиш кончиками пальцев движением вниз';

export function getPianoZones() {
  return createPianoZones();
}
