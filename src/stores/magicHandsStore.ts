import { create } from 'zustand';
import type { ElementId } from '@/modules/magic-hands/types';

interface MagicHandsState {
  element: ElementId;
  intensity: number;
  showSkeleton: boolean;
  showDebug: boolean;
  panelOpen: boolean;
  setElement: (element: ElementId) => void;
  setIntensity: (intensity: number) => void;
  setShowSkeleton: (show: boolean) => void;
  setShowDebug: (show: boolean) => void;
  setPanelOpen: (open: boolean) => void;
}

export const useMagicHandsStore = create<MagicHandsState>((set) => ({
  element: 'fire',
  intensity: 1.35,
  showSkeleton: true,
  showDebug: false,
  panelOpen: true,
  setElement: (element) => set({ element }),
  setIntensity: (intensity) => set({ intensity }),
  setShowSkeleton: (showSkeleton) => set({ showSkeleton }),
  setShowDebug: (showDebug) => set({ showDebug }),
  setPanelOpen: (panelOpen) => set({ panelOpen }),
}));
