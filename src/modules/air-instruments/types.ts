import type { GuitarTone } from './instruments/guitarVoicings';

export type InstrumentId = 'piano' | 'drums' | 'guitar';

export type PianoPreset = 'soft-pad' | 'bright-keys' | 'pluck';

export type { GuitarTone };

export const INSTRUMENT_ORDER: InstrumentId[] = ['piano', 'drums', 'guitar'];

export interface HitEvent {
  id: string;
  instrument: InstrumentId;
  label: string;
  x: number;
  y: number;
  velocity: number;
  color: string;
  at: number;
}

export interface InstrumentSwitchEvent {
  direction: 'prev' | 'next';
  at: number;
}

export interface ZoneRect {
  id: string;
  label: string;
  x: number;
  y: number;
  w: number;
  h: number;
  color: string;
}

export interface CircleZone {
  id: string;
  label: string;
  x: number;
  y: number;
  r: number;
  color: string;
}

export interface GuitarFingerCursor {
  finger: 'thumb' | 'index' | 'middle' | 'ring' | 'pinky';
  x: number;
  y: number;
  active: boolean;
}

export interface GuitarPlayState {
  chord: string;
  muted: boolean;
  power: boolean;
  direction: 'down' | 'up' | null;
  stringEnergy: number[];
  pickX: number;
  pickY: number;
  fingers: GuitarFingerCursor[];
}

export interface AirInstrumentsSettings {
  instrument: InstrumentId;
  volume: number;
  sensitivity: number;
  muted: boolean;
  showSkeleton: boolean;
  showDebug: boolean;
  pianoPreset: PianoPreset;
  guitarTone: GuitarTone;
  panelOpen: boolean;
}
