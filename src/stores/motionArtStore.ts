import { create } from 'zustand';
import type { BrushId, ColorMode } from '@/modules/motion-art/types';

interface MotionArtState {
  brush: BrushId;
  colorMode: ColorMode;
  trailFade: number;
  thickness: number;
  showSkeleton: boolean;
  showDebug: boolean;
  panelOpen: boolean;
  setBrush: (brush: BrushId) => void;
  setColorMode: (colorMode: ColorMode) => void;
  setTrailFade: (trailFade: number) => void;
  setThickness: (thickness: number) => void;
  setShowSkeleton: (show: boolean) => void;
  setShowDebug: (show: boolean) => void;
  setPanelOpen: (open: boolean) => void;
}

export const useMotionArtStore = create<MotionArtState>((set) => ({
  brush: 'neon',
  colorMode: 'aurora',
  trailFade: 0.045,
  thickness: 5.5,
  showSkeleton: true,
  showDebug: false,
  panelOpen: true,
  setBrush: (brush) => set({ brush }),
  setColorMode: (colorMode) => set({ colorMode }),
  setTrailFade: (trailFade) => set({ trailFade }),
  setThickness: (thickness) => set({ thickness }),
  setShowSkeleton: (showSkeleton) => set({ showSkeleton }),
  setShowDebug: (showDebug) => set({ showDebug }),
  setPanelOpen: (panelOpen) => set({ panelOpen }),
}));
