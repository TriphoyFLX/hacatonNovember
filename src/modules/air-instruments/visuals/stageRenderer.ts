import { HAND_CONNECTIONS, type TrackedHand } from '@/modules/vision/types';
import { mirrorX } from '../coords';
import { GUITAR_STRING_YS, getVoicing } from '../instruments/guitarVoicings';
import type { CircleZone, GuitarPlayState, HitEvent, InstrumentId, ZoneRect } from '../types';

interface RenderState {
  instrument: InstrumentId;
  hands: TrackedHand[];
  pianoZones: ZoneRect[];
  drumZones: CircleZone[];
  chordZones: ZoneRect[];
  activeChord: string;
  activeNotes: string[];
  hits: HitEvent[];
  showSkeleton: boolean;
  audioPeak: number;
  hint: string | null;
  guitar?: GuitarPlayState | null;
}

interface Ripple {
  x: number;
  y: number;
  color: string;
  born: number;
  life: number;
}

export class StageRenderer {
  private ripples: Ripple[] = [];
  private trails = new Map<string, Array<{ x: number; y: number; at: number }>>();

  pushHits(hits: HitEvent[]): void {
    for (const hit of hits) {
      this.ripples.push({
        x: hit.x,
        y: hit.y,
        color: hit.color,
        born: performance.now(),
        life: 420,
      });
    }
  }

  draw(canvas: HTMLCanvasElement, state: RenderState): void {
    const width = canvas.clientWidth;
    const height = canvas.clientHeight;
    if (width === 0 || height === 0) {
      return;
    }
    if (canvas.width !== width || canvas.height !== height) {
      canvas.width = width;
      canvas.height = height;
    }

    const ctx = canvas.getContext('2d');
    if (!ctx) {
      return;
    }

    ctx.clearRect(0, 0, width, height);
    this.drawWash(ctx, width, height, state.audioPeak);
    this.drawZones(ctx, width, height, state);
    this.drawGuitarStrings(ctx, width, height, state);
    this.updateTrails(state.hands);
    this.drawTrails(ctx, width, height);
    if (state.showSkeleton) {
      this.drawSkeleton(ctx, width, height, state.hands);
    }
    this.drawRipples(ctx, width, height);
    if (state.hint) {
      this.drawHint(ctx, width, height, state.hint);
    }
  }

  reset(): void {
    this.ripples = [];
    this.trails.clear();
  }

  private drawWash(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    peak: number,
  ): void {
    const gradient = ctx.createRadialGradient(
      width * 0.5,
      height * 0.45,
      width * 0.08,
      width * 0.5,
      height * 0.45,
      width * 0.7,
    );
    gradient.addColorStop(0, `rgba(56, 189, 248, ${0.04 + peak * 0.12})`);
    gradient.addColorStop(0.55, `rgba(139, 92, 246, ${0.05 + peak * 0.1})`);
    gradient.addColorStop(1, 'rgba(0,0,0,0)');
    ctx.fillStyle = gradient;
    ctx.fillRect(0, 0, width, height);
  }

  private drawZones(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    state: RenderState,
  ): void {
    if (state.instrument === 'piano') {
      for (const zone of state.pianoZones) {
        const active = state.activeNotes.includes(zone.label);
        const x = zone.x * width;
        const y = zone.y * height;
        const w = zone.w * width;
        const h = zone.h * height;
        ctx.fillStyle = active ? `${zone.color}66` : `${zone.color}22`;
        ctx.strokeStyle = active ? `${zone.color}` : `${zone.color}88`;
        ctx.lineWidth = active ? 2.5 : 1.5;
        this.roundRect(ctx, x, y, w, h, 10);
        ctx.fill();
        ctx.stroke();
        if (active) {
          const beam = ctx.createLinearGradient(x + w / 2, y, x + w / 2, 0);
          beam.addColorStop(0, `${zone.color}55`);
          beam.addColorStop(1, `${zone.color}00`);
          ctx.fillStyle = beam;
          ctx.fillRect(x + w * 0.2, 0, w * 0.6, y);
        }
        ctx.fillStyle = 'rgba(244,246,255,0.85)';
        ctx.font = '600 12px sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(zone.label, x + w / 2, y + h - 12);
      }
      return;
    }

    if (state.instrument === 'drums') {
      for (const zone of state.drumZones) {
        const x = zone.x * width;
        const y = zone.y * height;
        const r = zone.r * Math.min(width, height);
        ctx.beginPath();
        ctx.arc(x, y, r, 0, Math.PI * 2);
        ctx.fillStyle = `${zone.color}24`;
        ctx.fill();
        ctx.strokeStyle = `${zone.color}cc`;
        ctx.lineWidth = 2;
        ctx.stroke();
        ctx.fillStyle = 'rgba(244,246,255,0.9)';
        ctx.font = '600 13px sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(zone.label, x, y + 4);
      }
      return;
    }

    for (const zone of state.chordZones) {
      const active = zone.label === state.activeChord;
      const x = zone.x * width;
      const y = zone.y * height;
      const w = zone.w * width;
      const h = zone.h * height;
      ctx.fillStyle = active ? `${zone.color}55` : `${zone.color}20`;
      ctx.strokeStyle = active ? zone.color : `${zone.color}99`;
      ctx.lineWidth = active ? 2.5 : 1.5;
      this.roundRect(ctx, x, y, w, h, 12);
      ctx.fill();
      ctx.stroke();
      ctx.fillStyle = 'rgba(244,246,255,0.92)';
      ctx.font = '700 16px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(zone.label, x + w / 2, y + h / 2 + 5);
    }
  }

  private drawGuitarStrings(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    state: RenderState,
  ): void {
    if (state.instrument !== 'guitar') {
      return;
    }

    const guitar = state.guitar;
    const voicing = getVoicing(state.activeChord || guitar?.chord || 'Em');
    const neckX = width * 0.26;
    const neckW = width * 0.68;
    const neckTop = height * 0.26;
    const neckBottom = height * 0.72;

    // Neck plate
    const neckGrad = ctx.createLinearGradient(neckX, neckTop, neckX, neckBottom);
    neckGrad.addColorStop(0, 'rgba(20, 16, 28, 0.55)');
    neckGrad.addColorStop(1, 'rgba(12, 18, 32, 0.72)');
    ctx.fillStyle = neckGrad;
    this.roundRect(ctx, neckX, neckTop, neckW, neckBottom - neckTop, 18);
    ctx.fill();
    ctx.strokeStyle = `${voicing.color}55`;
    ctx.lineWidth = 1.5;
    ctx.stroke();

    // Frets
    for (let f = 1; f <= 4; f += 1) {
      const x = neckX + (neckW * f) / 5;
      ctx.beginPath();
      ctx.moveTo(x, neckTop + 8);
      ctx.lineTo(x, neckBottom - 8);
      ctx.strokeStyle = 'rgba(255,255,255,0.08)';
      ctx.lineWidth = 1;
      ctx.stroke();
    }

    // Strings with vibration
    GUITAR_STRING_YS.forEach((stringY, index) => {
      const y = height * stringY;
      const energy = guitar?.stringEnergy[index] ?? 0;
      const amp = energy * 7;
      ctx.beginPath();
      ctx.moveTo(neckX + 8, y);
      const segments = 18;
      for (let s = 1; s <= segments; s += 1) {
        const t = s / segments;
        const x = neckX + 8 + (neckW - 16) * t;
        const wobble = Math.sin(t * Math.PI * 6 + performance.now() / 40) * amp * (1 - Math.abs(t - 0.5));
        ctx.lineTo(x, y + wobble);
      }
      ctx.strokeStyle = `rgba(226, 232, 255, ${0.35 + energy * 0.55 + state.audioPeak * 0.2})`;
      ctx.lineWidth = 1.2 + index * 0.25 + energy * 1.5;
      ctx.shadowBlur = 8 + energy * 18;
      ctx.shadowColor = voicing.color;
      ctx.stroke();
      ctx.shadowBlur = 0;
    });

    // Chord badge
    ctx.fillStyle = 'rgba(8,10,18,0.7)';
    this.roundRect(ctx, width * 0.42, height * 0.14, width * 0.28, 46, 14);
    ctx.fill();
    ctx.fillStyle = voicing.color;
    ctx.font = '800 22px sans-serif';
    ctx.textAlign = 'center';
    const mode = guitar?.muted ? 'MUTE' : guitar?.power ? 'POWER' : 'OPEN';
    ctx.fillText(`${voicing.label}  ·  ${mode}`, width * 0.56, height * 0.14 + 30);

    // Finger cursors over the neck (each fingertip = a pluck voice).
    if (guitar) {
      const fingerColors: Record<string, string> = {
        thumb: '#fbbf24',
        index: '#38bdf8',
        middle: '#34d399',
        ring: '#a78bfa',
        pinky: '#f472b6',
      };

      if (guitar.fingers.length > 0) {
        for (const finger of guitar.fingers) {
          if (!finger.active) {
            continue;
          }
          const px = finger.x * width;
          const py = finger.y * height;
          ctx.beginPath();
          ctx.arc(px, py, finger.finger === 'thumb' ? 8 : 6, 0, Math.PI * 2);
          ctx.fillStyle = fingerColors[finger.finger] ?? '#fbbf24';
          ctx.globalAlpha = 0.92;
          ctx.fill();
          ctx.globalAlpha = 1;
          ctx.strokeStyle = 'rgba(255,255,255,0.55)';
          ctx.lineWidth = 1;
          ctx.stroke();
        }
      } else {
        const px = guitar.pickX * width;
        const py = guitar.pickY * height;
        ctx.beginPath();
        ctx.moveTo(px, py - 10);
        ctx.lineTo(px + 9, py + 8);
        ctx.lineTo(px - 9, py + 8);
        ctx.closePath();
        ctx.fillStyle = 'rgba(251, 191, 36, 0.9)';
        ctx.fill();
      }

      if (guitar.direction) {
        ctx.fillStyle = 'rgba(244,246,255,0.85)';
        ctx.font = '700 12px sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(
          guitar.direction === 'down' ? '↓ STRUM' : '↑ STRUM',
          guitar.pickX * width,
          guitar.pickY * height - 16,
        );
      }
    }
  }

  private updateTrails(hands: TrackedHand[]): void {
    const now = performance.now();
    const seen = new Set<string>();

    for (const hand of hands) {
      const points = [
        { key: `${hand.id}:thumb`, point: mirrorX(hand.tips.thumb) },
        { key: `${hand.id}:index`, point: mirrorX(hand.tips.index) },
        { key: `${hand.id}:middle`, point: mirrorX(hand.tips.middle) },
        { key: `${hand.id}:ring`, point: mirrorX(hand.tips.ring) },
        { key: `${hand.id}:pinky`, point: mirrorX(hand.tips.pinky) },
        { key: `${hand.id}:wrist`, point: mirrorX(hand.wrist) },
      ];
      for (const item of points) {
        seen.add(item.key);
        const trail = this.trails.get(item.key) ?? [];
        trail.push({ x: item.point.x, y: item.point.y, at: now });
        this.trails.set(
          item.key,
          trail.filter((entry) => now - entry.at < 280).slice(-18),
        );
      }
    }

    for (const key of [...this.trails.keys()]) {
      if (!seen.has(key)) {
        this.trails.delete(key);
      }
    }
  }

  private drawTrails(ctx: CanvasRenderingContext2D, width: number, height: number): void {
    const now = performance.now();
    for (const trail of this.trails.values()) {
      if (trail.length < 2) {
        continue;
      }
      ctx.beginPath();
      trail.forEach((point, index) => {
        const x = point.x * width;
        const y = point.y * height;
        if (index === 0) {
          ctx.moveTo(x, y);
        } else {
          ctx.lineTo(x, y);
        }
      });
      const age = now - trail[0].at;
      ctx.strokeStyle = `rgba(167, 139, 250, ${Math.max(0.15, 0.7 - age / 400)})`;
      ctx.lineWidth = 2.5;
      ctx.lineCap = 'round';
      ctx.stroke();
    }
  }

  private drawSkeleton(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    hands: TrackedHand[],
  ): void {
    for (const hand of hands) {
      ctx.strokeStyle = 'rgba(56, 189, 248, 0.75)';
      ctx.lineWidth = 2;
      for (const [a, b] of HAND_CONNECTIONS) {
        const pa = hand.landmarks[a];
        const pb = hand.landmarks[b];
        if (!pa || !pb) {
          continue;
        }
        const ma = mirrorX(pa);
        const mb = mirrorX(pb);
        ctx.beginPath();
        ctx.moveTo(ma.x * width, ma.y * height);
        ctx.lineTo(mb.x * width, mb.y * height);
        ctx.stroke();
      }

      for (const point of hand.landmarks) {
        const mirrored = mirrorX(point);
        ctx.beginPath();
        ctx.fillStyle = 'rgba(244, 246, 255, 0.9)';
        ctx.arc(mirrored.x * width, mirrored.y * height, 2.4, 0, Math.PI * 2);
        ctx.fill();
      }
    }
  }

  private drawRipples(ctx: CanvasRenderingContext2D, width: number, height: number): void {
    const now = performance.now();
    this.ripples = this.ripples.filter((ripple) => now - ripple.born < ripple.life);
    for (const ripple of this.ripples) {
      const t = (now - ripple.born) / ripple.life;
      const radius = 12 + t * 70;
      ctx.beginPath();
      ctx.arc(ripple.x * width, ripple.y * height, radius, 0, Math.PI * 2);
      ctx.strokeStyle = ripple.color;
      ctx.globalAlpha = 1 - t;
      ctx.lineWidth = 3;
      ctx.stroke();
      ctx.globalAlpha = 1;
    }
  }

  private drawHint(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    hint: string,
  ): void {
    const boxW = Math.min(width * 0.82, 520);
    ctx.fillStyle = 'rgba(8, 10, 18, 0.55)';
    this.roundRect(ctx, width * 0.5 - boxW / 2, height * 0.07, boxW, 44, 16);
    ctx.fill();
    ctx.fillStyle = 'rgba(244,246,255,0.9)';
    ctx.font = '600 13px sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText(hint, width * 0.5, height * 0.07 + 27);
  }

  private roundRect(
    ctx: CanvasRenderingContext2D,
    x: number,
    y: number,
    w: number,
    h: number,
    r: number,
  ): void {
    const radius = Math.min(r, w / 2, h / 2);
    ctx.beginPath();
    ctx.moveTo(x + radius, y);
    ctx.arcTo(x + w, y, x + w, y + h, radius);
    ctx.arcTo(x + w, y + h, x, y + h, radius);
    ctx.arcTo(x, y + h, x, y, radius);
    ctx.arcTo(x, y, x + w, y, radius);
    ctx.closePath();
  }
}
