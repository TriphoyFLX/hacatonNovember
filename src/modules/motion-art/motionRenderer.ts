import { mirrorX } from '@/modules/air-instruments/coords';
import { handOpenness, isFist } from '@/modules/vision/handMetrics';
import { FINGER_NAMES, HAND_CONNECTIONS, type TrackedHand } from '@/modules/vision/types';
import { colorForStroke } from './palette';
import type { BrushId, ColorMode } from './types';

interface TrailPoint {
  x: number;
  y: number;
  width: number;
  color: string;
  at: number;
}

interface Spark {
  x: number;
  y: number;
  vx: number;
  vy: number;
  life: number;
  color: string;
}

export interface MotionRenderState {
  hands: TrackedHand[];
  brush: BrushId;
  colorMode: ColorMode;
  trailFade: number;
  thickness: number;
  showSkeleton: boolean;
  showDebug: boolean;
  handFps: number;
  cameraFps: number;
  hint: string;
  drawing: boolean;
}

/**
 * Persistent ink layer + live tips. Fist pauses ink so you can reposition.
 */
export class MotionRenderer {
  private ink: HTMLCanvasElement | null = null;
  private inkCtx: CanvasRenderingContext2D | null = null;
  private prevTips = new Map<string, { x: number; y: number }>();
  private sparks: Spark[] = [];
  private ribbons: TrailPoint[] = [];

  clear(): void {
    if (this.ink && this.inkCtx) {
      this.inkCtx.clearRect(0, 0, this.ink.width, this.ink.height);
    }
    this.prevTips.clear();
    this.sparks = [];
    this.ribbons = [];
  }

  draw(canvas: HTMLCanvasElement, state: MotionRenderState): void {
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      return;
    }

    const width = canvas.clientWidth;
    const height = canvas.clientHeight;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    if (canvas.width !== Math.floor(width * dpr) || canvas.height !== Math.floor(height * dpr)) {
      canvas.width = Math.floor(width * dpr);
      canvas.height = Math.floor(height * dpr);
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    this.ensureInk(width, height, dpr);
    this.fadeInk(state.trailFade);

    const now = performance.now();
    let anyDrawing = false;

    for (const [handIndex, hand] of state.hands.entries()) {
      const fist = isFist(hand, 0.34);
      const open = handOpenness(hand);
      const drawing = state.drawing && !fist && open > 0.38;
      if (drawing) {
        anyDrawing = true;
      }

      for (const finger of FINGER_NAMES) {
        if (finger === 'thumb' && state.brush !== 'spark') {
          continue;
        }
        const tip = mirrorX(hand.tips[finger]);
        const key = `${hand.id}:${finger}`;
        const prev = this.prevTips.get(key);
        const speed = Math.hypot(hand.tipVelocity[finger].x, hand.tipVelocity[finger].y);
        const color = colorForStroke(state.colorMode, tip.x, speed, now, handIndex);
        const widthPx =
          state.thickness *
          (0.7 + Math.min(1.8, speed / 2.2)) *
          (finger === 'index' || finger === 'middle' ? 1.15 : 0.75);

        if (drawing && prev) {
          this.strokeInk(prev.x * width, prev.y * height, tip.x * width, tip.y * height, {
            brush: state.brush,
            color,
            width: widthPx,
            speed,
          });

          if (state.brush === 'ribbon') {
            this.ribbons.push({
              x: tip.x * width,
              y: tip.y * height,
              width: widthPx * 1.4,
              color,
              at: now,
            });
          }

          if (state.brush === 'spark' && speed > 0.8) {
            for (let i = 0; i < 3; i += 1) {
              this.sparks.push({
                x: tip.x * width,
                y: tip.y * height,
                vx: (Math.random() - 0.5) * speed * 18,
                vy: (Math.random() - 0.5) * speed * 18 - 4,
                life: 0.55 + Math.random() * 0.4,
                color,
              });
            }
          }
        }

        this.prevTips.set(key, { x: tip.x, y: tip.y });
      }

      // Palm “sculpt” bloom while open and moving.
      if (drawing && state.brush === 'ribbon') {
        const palm = mirrorX(hand.palm);
        const palmSpeed = Math.hypot(hand.tipVelocity.wrist.x, hand.tipVelocity.wrist.y);
        if (palmSpeed > 0.55 && this.inkCtx) {
          const px = palm.x * width;
          const py = palm.y * height;
          const radius = 18 + open * 42 + palmSpeed * 10;
          const gradient = this.inkCtx.createRadialGradient(px, py, 2, px, py, radius);
          gradient.addColorStop(0, colorForStroke(state.colorMode, palm.x, palmSpeed, now, handIndex));
          gradient.addColorStop(1, 'rgba(0,0,0,0)');
          this.inkCtx.globalCompositeOperation = 'lighter';
          this.inkCtx.fillStyle = gradient;
          this.inkCtx.beginPath();
          this.inkCtx.arc(px, py, radius, 0, Math.PI * 2);
          this.inkCtx.fill();
          this.inkCtx.globalCompositeOperation = 'source-over';
        }
      }
    }

    // Drop tips for hands that disappeared.
    const liveKeys = new Set(
      state.hands.flatMap((hand) => FINGER_NAMES.map((finger) => `${hand.id}:${finger}`)),
    );
    for (const key of [...this.prevTips.keys()]) {
      if (!liveKeys.has(key)) {
        this.prevTips.delete(key);
      }
    }

    ctx.clearRect(0, 0, width, height);

    // Dim camera plate so trails read clearly.
    ctx.fillStyle = 'rgba(4, 8, 12, 0.22)';
    ctx.fillRect(0, 0, width, height);

    if (this.ink) {
      ctx.drawImage(this.ink, 0, 0, width, height);
    }

    this.drawRibbons(ctx, now);
    this.drawSparks(ctx);

    if (state.showSkeleton) {
      this.drawSkeleton(ctx, width, height, state.hands);
    }

    this.drawLiveTips(ctx, width, height, state, now);
    this.drawHud(ctx, width, height, state, anyDrawing);
  }

  private ensureInk(width: number, height: number, dpr: number): void {
    const w = Math.floor(width * dpr);
    const h = Math.floor(height * dpr);
    if (!this.ink || this.ink.width !== w || this.ink.height !== h) {
      this.ink = document.createElement('canvas');
      this.ink.width = w;
      this.ink.height = h;
      this.inkCtx = this.ink.getContext('2d');
      if (this.inkCtx) {
        this.inkCtx.setTransform(dpr, 0, 0, dpr, 0, 0);
      }
    }
  }

  private fadeInk(fade: number): void {
    if (!this.ink || !this.inkCtx) {
      return;
    }
    const amount = Math.max(0.01, Math.min(0.35, fade));
    this.inkCtx.save();
    this.inkCtx.setTransform(1, 0, 0, 1, 0, 0);
    this.inkCtx.globalCompositeOperation = 'destination-out';
    this.inkCtx.fillStyle = `rgba(0,0,0,${amount})`;
    this.inkCtx.fillRect(0, 0, this.ink.width, this.ink.height);
    this.inkCtx.restore();
    this.inkCtx.globalCompositeOperation = 'source-over';
  }

  private strokeInk(
    x0: number,
    y0: number,
    x1: number,
    y1: number,
    options: { brush: BrushId; color: string; width: number; speed: number },
  ): void {
    const ctx = this.inkCtx;
    if (!ctx) {
      return;
    }

    ctx.save();
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';
    ctx.strokeStyle = options.color;
    ctx.shadowColor = options.color;
    ctx.shadowBlur = options.brush === 'neon' ? 18 : options.brush === 'ribbon' ? 10 : 8;
    ctx.globalCompositeOperation = 'lighter';
    ctx.lineWidth = Math.max(1.5, options.width);

    if (options.brush === 'neon') {
      ctx.beginPath();
      ctx.moveTo(x0, y0);
      ctx.lineTo(x1, y1);
      ctx.stroke();
      ctx.lineWidth = Math.max(1, options.width * 0.35);
      ctx.shadowBlur = 4;
      ctx.strokeStyle = 'rgba(255,255,255,0.85)';
      ctx.beginPath();
      ctx.moveTo(x0, y0);
      ctx.lineTo(x1, y1);
      ctx.stroke();
    } else if (options.brush === 'ribbon') {
      const midX = (x0 + x1) / 2 + (y1 - y0) * 0.35;
      const midY = (y0 + y1) / 2 - (x1 - x0) * 0.35;
      ctx.lineWidth = Math.max(2, options.width * 1.35);
      ctx.beginPath();
      ctx.moveTo(x0, y0);
      ctx.quadraticCurveTo(midX, midY, x1, y1);
      ctx.stroke();
    } else {
      ctx.lineWidth = Math.max(1.2, options.width * 0.55);
      ctx.beginPath();
      ctx.moveTo(x0, y0);
      ctx.lineTo(x1, y1);
      ctx.stroke();
    }

    ctx.restore();
  }

  private drawRibbons(ctx: CanvasRenderingContext2D, now: number): void {
    this.ribbons = this.ribbons.filter((point) => now - point.at < 420);
    for (const point of this.ribbons) {
      const age = (now - point.at) / 420;
      ctx.beginPath();
      ctx.arc(point.x, point.y, point.width * (1 - age), 0, Math.PI * 2);
      ctx.fillStyle = point.color;
      ctx.globalAlpha = 0.35 * (1 - age);
      ctx.fill();
      ctx.globalAlpha = 1;
    }
  }

  private drawSparks(ctx: CanvasRenderingContext2D): void {
    const next: Spark[] = [];
    for (const spark of this.sparks) {
      spark.x += spark.vx * 0.016;
      spark.y += spark.vy * 0.016;
      spark.vy += 12 * 0.016;
      spark.life -= 0.018;
      if (spark.life <= 0) {
        continue;
      }
      ctx.beginPath();
      ctx.arc(spark.x, spark.y, 2.2 * spark.life, 0, Math.PI * 2);
      ctx.fillStyle = spark.color;
      ctx.shadowBlur = 10;
      ctx.shadowColor = spark.color;
      ctx.globalAlpha = Math.max(0, spark.life);
      ctx.fill();
      ctx.shadowBlur = 0;
      ctx.globalAlpha = 1;
      next.push(spark);
    }
    this.sparks = next;
  }

  private drawSkeleton(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    hands: TrackedHand[],
  ): void {
    for (const hand of hands) {
      ctx.strokeStyle = 'rgba(52, 211, 153, 0.35)';
      ctx.lineWidth = 1.5;
      for (const [a, b] of HAND_CONNECTIONS) {
        const pa = mirrorX(hand.landmarks[a]);
        const pb = mirrorX(hand.landmarks[b]);
        ctx.beginPath();
        ctx.moveTo(pa.x * width, pa.y * height);
        ctx.lineTo(pb.x * width, pb.y * height);
        ctx.stroke();
      }
    }
  }

  private drawLiveTips(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    state: MotionRenderState,
    now: number,
  ): void {
    state.hands.forEach((hand, handIndex) => {
      const fist = isFist(hand, 0.34);
      for (const finger of ['index', 'middle', 'ring', 'pinky'] as const) {
        const tip = mirrorX(hand.tips[finger]);
        const speed = Math.hypot(hand.tipVelocity[finger].x, hand.tipVelocity[finger].y);
        ctx.beginPath();
        ctx.arc(tip.x * width, tip.y * height, fist ? 4 : 6 + Math.min(4, speed), 0, Math.PI * 2);
        ctx.fillStyle = fist
          ? 'rgba(248, 113, 113, 0.85)'
          : colorForStroke(state.colorMode, tip.x, speed, now, handIndex);
        ctx.fill();
      }
    });
  }

  private drawHud(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    state: MotionRenderState,
    drawing: boolean,
  ): void {
    ctx.fillStyle = 'rgba(244,246,255,0.78)';
    ctx.font = '600 13px sans-serif';
    ctx.textAlign = 'left';
    ctx.fillText(state.hint, 18, height - 18);

    if (state.showDebug) {
      ctx.fillText(
        `cam ${state.cameraFps} · hands ${state.handFps} · ${drawing ? 'ink' : 'pause'}`,
        18,
        28,
      );
    }

    if (!drawing && state.hands.length > 0) {
      ctx.textAlign = 'center';
      ctx.fillStyle = 'rgba(252, 165, 165, 0.9)';
      ctx.font = '700 14px sans-serif';
      ctx.fillText('Кулак = пауза рисования', width / 2, 36);
    }
  }
}
