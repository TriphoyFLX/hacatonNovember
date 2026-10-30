import {
  createGuitarChordZones,
  getVoicing,
  GUITAR_TONES,
  type GuitarTone,
} from './guitarVoicings';

export const guitarHint =
  'Левая — аккорд (кулак = mute/power). Правая — пальцы: бас / середина / верх. Быстрый взмах = strum. Clean · Crunch · Lead в панели.';

export const guitarTones = GUITAR_TONES;

export function getGuitarChordZones() {
  return createGuitarChordZones();
}

export function getChordNotes(chord: string, power = false): Array<string | null> {
  const voicing = getVoicing(chord);
  return power ? voicing.power : voicing.strings;
}

export type { GuitarTone };
