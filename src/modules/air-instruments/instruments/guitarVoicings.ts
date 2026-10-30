/** Standard tuning E2 A2 D3 G3 B3 E4. `null` = muted string. Order: low E → high e. */
export type GuitarStringNote = string | null;

export interface GuitarVoicing {
  id: string;
  label: string;
  color: string;
  /** Full open / barre shape — all sounding strings filled where possible */
  strings: GuitarStringNote[];
  /** Root + fifth (+ octave) power shape for mute/rock feel */
  power: GuitarStringNote[];
}

/**
 * Beefed open voicings: stronger bass roots, denser mids, singing highs.
 */
export const GUITAR_VOICINGS: GuitarVoicing[] = [
  {
    id: 'Em',
    label: 'Em',
    color: '#34d399',
    strings: ['E2', 'B2', 'E3', 'G3', 'B3', 'E4'],
    power: ['E2', 'B2', 'E3', 'B3', null, null],
  },
  {
    id: 'Am',
    label: 'Am',
    color: '#38bdf8',
    strings: ['E2', 'A2', 'E3', 'A3', 'C4', 'E4'],
    power: ['A2', 'E3', 'A3', 'E4', null, null],
  },
  {
    id: 'C',
    label: 'C',
    color: '#a78bfa',
    strings: ['G2', 'C3', 'E3', 'G3', 'C4', 'E4'],
    power: ['C3', 'G3', 'C4', 'G4', null, null],
  },
  {
    id: 'G',
    label: 'G',
    color: '#fbbf24',
    strings: ['G2', 'B2', 'D3', 'G3', 'B3', 'G4'],
    power: ['G2', 'D3', 'G3', 'D4', null, null],
  },
  {
    id: 'D',
    label: 'D',
    color: '#f472b6',
    strings: ['D2', 'A2', 'D3', 'A3', 'D4', 'F#4'],
    power: ['D3', 'A3', 'D4', 'A4', null, null],
  },
  {
    id: 'A',
    label: 'A',
    color: '#22d3ee',
    strings: ['E2', 'A2', 'E3', 'A3', 'C#4', 'E4'],
    power: ['A2', 'E3', 'A3', 'E4', null, null],
  },
  {
    id: 'E',
    label: 'E',
    color: '#fb7185',
    strings: ['E2', 'B2', 'E3', 'G#3', 'B3', 'E4'],
    power: ['E2', 'B2', 'E3', 'B3', null, null],
  },
  {
    id: 'F',
    label: 'F',
    color: '#c084fc',
    strings: ['F2', 'C3', 'F3', 'A3', 'C4', 'F4'],
    power: ['F2', 'C3', 'F3', 'C4', null, null],
  },
  {
    id: 'Dm',
    label: 'Dm',
    color: '#818cf8',
    strings: ['D2', 'A2', 'D3', 'A3', 'D4', 'F4'],
    power: ['D3', 'A3', 'D4', 'A4', null, null],
  },
  {
    id: 'Bm',
    label: 'Bm',
    color: '#67e8f9',
    strings: ['B2', 'F#2', 'F#3', 'B3', 'D4', 'F#4'],
    power: ['B2', 'F#3', 'B3', 'F#4', null, null],
  },
  {
    id: 'Em7',
    label: 'Em7',
    color: '#86efac',
    strings: ['E2', 'B2', 'E3', 'G3', 'D4', 'E4'],
    power: ['E2', 'B2', 'E3', 'D4', null, null],
  },
  {
    id: 'Cadd9',
    label: 'Cadd9',
    color: '#ddd6fe',
    strings: ['G2', 'C3', 'E3', 'G3', 'D4', 'E4'],
    power: ['C3', 'G3', 'D4', 'G4', null, null],
  },
];

export type GuitarTone = 'clean' | 'crunch' | 'lead';

export const GUITAR_TONES: Array<{ id: GuitarTone; label: string }> = [
  { id: 'clean', label: 'Clean' },
  { id: 'crunch', label: 'Crunch' },
  { id: 'lead', label: 'Lead' },
];

/** Slightly wider string spacing for clearer bass / mid / high finger hits. */
export const GUITAR_STRING_YS = [0.28, 0.36, 0.44, 0.52, 0.6, 0.68];

export function getVoicing(id: string): GuitarVoicing {
  return GUITAR_VOICINGS.find((item) => item.id === id) ?? GUITAR_VOICINGS[0];
}

export function createGuitarChordZones() {
  return GUITAR_VOICINGS.map((chord, index) => {
    const col = index < 6 ? 0 : 1;
    const row = index < 6 ? index : index - 6;
    return {
      id: `guitar:chord:${chord.id}`,
      label: chord.label,
      x: 0.02 + col * 0.12,
      y: 0.16 + row * 0.11,
      w: 0.11,
      h: 0.095,
      color: chord.color,
    };
  });
}
