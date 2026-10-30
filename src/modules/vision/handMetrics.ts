import { distance } from './smoothing';
import type { TrackedHand } from './types';

/** 0 = fist, 1 = fully open palm. */
export function handOpenness(hand: TrackedHand): number {
  const tips = [hand.tips.thumb, hand.tips.index, hand.tips.middle, hand.tips.ring, hand.tips.pinky];
  const avg = tips.reduce((sum, tip) => sum + distance(tip, hand.wrist), 0) / tips.length;
  // Empirically: fist ~0.12, open ~0.32 in normalized image space.
  return Math.max(0, Math.min(1, (avg - 0.12) / 0.2));
}

export function isFist(hand: TrackedHand, threshold = 0.32): boolean {
  return handOpenness(hand) < threshold;
}
