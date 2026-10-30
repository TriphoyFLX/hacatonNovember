import type { Point2D } from './types';

export class EmaPoint {
  private value: Point2D | null = null;

  constructor(private readonly alpha: number) {}

  next(point: Point2D): Point2D {
    if (!this.value) {
      this.value = { ...point };
      return { ...point };
    }

    this.value = {
      x: this.value.x + (point.x - this.value.x) * this.alpha,
      y: this.value.y + (point.y - this.value.y) * this.alpha,
    };
    return { ...this.value };
  }

  reset(): void {
    this.value = null;
  }
}

export function velocity(prev: Point2D | null, next: Point2D, dtMs: number): Point2D {
  if (!prev || dtMs <= 0) {
    return { x: 0, y: 0 };
  }
  const dt = dtMs / 1000;
  return {
    x: (next.x - prev.x) / dt,
    y: (next.y - prev.y) / dt,
  };
}

export function distance(a: Point2D, b: Point2D): number {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  return Math.hypot(dx, dy);
}
