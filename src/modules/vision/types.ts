export interface Point2D {
  x: number;
  y: number;
}

export interface TrackedHand {
  id: number;
  handedness: 'Left' | 'Right' | 'Unknown';
  score: number;
  landmarks: Point2D[];
  tips: {
    thumb: Point2D;
    index: Point2D;
    middle: Point2D;
    ring: Point2D;
    pinky: Point2D;
  };
  wrist: Point2D;
  palm: Point2D;
  tipVelocity: {
    thumb: Point2D;
    index: Point2D;
    middle: Point2D;
    ring: Point2D;
    pinky: Point2D;
    wrist: Point2D;
  };
}

export type FingerName = 'thumb' | 'index' | 'middle' | 'ring' | 'pinky';

export const FINGER_NAMES: FingerName[] = ['thumb', 'index', 'middle', 'ring', 'pinky'];

export interface HandTrackerSnapshot {
  hands: TrackedHand[];
  fps: number;
  status: 'idle' | 'loading' | 'ready' | 'running' | 'error';
  errorMessage: string | null;
  hasHands: boolean;
}

export const HAND_CONNECTIONS: Array<[number, number]> = [
  [0, 1],
  [1, 2],
  [2, 3],
  [3, 4],
  [0, 5],
  [5, 6],
  [6, 7],
  [7, 8],
  [5, 9],
  [9, 10],
  [10, 11],
  [11, 12],
  [9, 13],
  [13, 14],
  [14, 15],
  [15, 16],
  [13, 17],
  [0, 17],
  [17, 18],
  [18, 19],
  [19, 20],
];
