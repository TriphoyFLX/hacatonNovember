import { create } from 'zustand';
import type { GuitarTone, InstrumentId, PianoPreset } from '@/modules/air-instruments/types';

interface AirInstrumentsState {
  instrument: InstrumentId;
  volume: number;
  sensitivity: number;
  muted: boolean;
  showSkeleton: boolean;
  showDebug: boolean;
  pianoPreset: PianoPreset;
  guitarTone: GuitarTone;
  panelOpen: boolean;
  activeChord: string;
  lastHitLabel: string | null;
  hasPlayed: boolean;
  setInstrument: (instrument: InstrumentId) => void;
  setVolume: (volume: number) => void;
  setSensitivity: (sensitivity: number) => void;
  setMuted: (muted: boolean) => void;
  setShowSkeleton: (show: boolean) => void;
  setShowDebug: (show: boolean) => void;
  setPianoPreset: (preset: PianoPreset) => void;
  setGuitarTone: (tone: GuitarTone) => void;
  setPanelOpen: (open: boolean) => void;
  setActiveChord: (chord: string) => void;
  setLastHitLabel: (label: string | null) => void;
  markPlayed: () => void;
}

export const useAirInstrumentsStore = create<AirInstrumentsState>((set) => ({
  instrument: 'piano',
  volume: 0.8,
  sensitivity: 1,
  muted: false,
  showSkeleton: true, // always show hand skeleton by default
  showDebug: false,
  pianoPreset: 'bright-keys',
  guitarTone: 'crunch',
  panelOpen: true,
  activeChord: 'Em',
  lastHitLabel: null,
  hasPlayed: false,
  setInstrument: (instrument) => set({ instrument, lastHitLabel: null }),
  setVolume: (volume) => set({ volume }),
  setSensitivity: (sensitivity) => set({ sensitivity }),
  setMuted: (muted) => set({ muted }),
  setShowSkeleton: (showSkeleton) => set({ showSkeleton }),
  setShowDebug: (showDebug) => set({ showDebug }),
  setPianoPreset: (pianoPreset) => set({ pianoPreset }),
  setGuitarTone: (guitarTone) => set({ guitarTone }),
  setPanelOpen: (panelOpen) => set({ panelOpen }),
  setActiveChord: (activeChord) => set({ activeChord }),
  setLastHitLabel: (lastHitLabel) => set({ lastHitLabel }),
  markPlayed: () => set({ hasPlayed: true }),
}));
