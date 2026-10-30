import type { Point2D } from '@/modules/vision/types';

/** Map MediaPipe image coords to mirrored stage coords (CSS scaleX(-1) on video/canvas). */
export function mirrorX(point: Point2D): Point2D {
  return { x: 1 - point.x, y: point.y };
}
