import { mirrorX } from '@/modules/air-instruments/coords';
import { handOpenness, isFist } from '@/modules/vision/handMetrics';
import { distance } from '@/modules/vision/smoothing';
import {
  FINGER_NAMES,
  HAND_CONNECTIONS,
  type FingerName,
  type Point2D,
  type TrackedHand,
} from '@/modules/vision/types';
import type { ElementId } from './types';

type ParticleKind = 'ember' | 'drop' | 'spark' | 'orb' | 'shard' | 'smoke' | 'foam' | 'star' | 'flare';

interface Fireball {
  x: number;
  y: number;
  vx: number;
  vy: number;
  life: number;
  size: number;
}

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  life: number;
  maxLife: number;
  size: number;
  hue: number;
  kind: ParticleKind;
  attractX?: number;
  attractY?: number;
  spin?: number;
  trail?: boolean;
}

interface Bolt {
  points: Array<{ x: number; y: number }>;
  life: number;
  color: string;
  width: number;
}

interface Shockwave {
  x: number;
  y: number;
  radius: number;
  maxRadius: number;
  life: number;
  hue: number;
  mode: 'crush' | 'bloom' | 'ultimate';
  width: number;
}

interface Ribbon {
  points: Array<{ x: number; y: number; at: number }>;
  hue: number;
  width: number;
}

interface HandPoseState {
  open: number;
  charged: number;
  lastCrushAt: number;
  lastBloomAt: number;
  lastSnapAt: number;
  lastFlickAt: number;
  fist: boolean;
  snapArmed: boolean;
  thumbMiddle: number;
}

export interface MagicRenderState {
  hands: TrackedHand[];
  element: ElementId;
  intensity: number;
  showSkeleton: boolean;
  showDebug: boolean;
  handFps: number;
  cameraFps: number;
  hint: string;
}

const ELEMENT_HUE: Record<ElementId, number> = {
  fire: 28,
  water: 198,
  lightning: 52,
  energy: 320,
};

const FINGER_CAST: FingerName[] = ['thumb', 'index', 'middle', 'ring', 'pinky'];

/**
 * Maxed elemental VFX: crush/bloom/ultimate, finger streams, ribbons, shake.
 */
export class MagicRenderer {
  private particles: Particle[] = [];
  private bolts: Bolt[] = [];
  private waves: Shockwave[] = [];
  private ribbons = new Map<string, Ribbon>();
  private fireballs: Fireball[] = [];
  private flash = 0;
  private chroma = 0;
  private shake = 0;
  private heat = 0;
  private lastComboAt = 0;
  private lastUltimateAt = 0;
  private pendingBlooms: Array<{ at: number; x: number; y: number; power: number }> = [];
  private prevPalm = new Map<number, Point2D>();
  private pose = new Map<number, HandPoseState>();
  private gestureLabel: string | null = null;
  private gestureLabelUntil = 0;
  private time = 0;

  clear(): void {
    this.particles = [];
    this.bolts = [];
    this.waves = [];
    this.fireballs = [];
    this.ribbons.clear();
    this.flash = 0;
    this.chroma = 0;
    this.shake = 0;
    this.heat = 0;
    this.pendingBlooms = [];
    this.prevPalm.clear();
    this.pose.clear();
    this.gestureLabel = null;
  }

  draw(canvas: HTMLCanvasElement, state: MagicRenderState): void {
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

    const now = performance.now();
    this.time = now;
    this.updateFromHands(state, width, height, now);
    this.resolveUltimates(state, now);
    this.stepParticles(state.element);
    this.stepFireballs(state.element);
    this.stepWaves();
    this.pruneRibbons(now);
    this.bolts = this.bolts.filter((bolt) => {
      bolt.life -= 0.035;
      return bolt.life > 0;
    });
    this.flash = Math.max(0, this.flash - 0.028);
    this.chroma = Math.max(0, this.chroma - 0.022);
    this.shake = Math.max(0, this.shake - 0.05);
    this.heat = Math.max(0, this.heat - 0.01);

    const ox = this.shake > 0 ? (Math.random() - 0.5) * this.shake * 14 : 0;
    const oy = this.shake > 0 ? (Math.random() - 0.5) * this.shake * 14 : 0;

    ctx.save();
    ctx.translate(ox, oy);

    ctx.clearRect(-20, -20, width + 40, height + 40);
    this.drawBackdrop(ctx, width, height, state.element);

    if (this.flash > 0) {
      const flashHue = state.element === 'fire' ? 25 : ELEMENT_HUE[state.element];
      ctx.fillStyle = `hsla(${flashHue}, 95%, 72%, ${this.flash * 0.32})`;
      ctx.fillRect(0, 0, width, height);
    }

    this.drawAmbientAura(ctx, width, height, state.element);
    this.drawRibbons(ctx, now);
    this.drawWaves(ctx);
    this.drawBolts(ctx);
    this.drawParticles(ctx);
    this.drawFireballs(ctx);
    this.drawHandOrbs(ctx, width, height, state);

    if (state.showSkeleton) {
      this.drawSkeleton(ctx, width, height, state.hands, state.element);
    }

    ctx.restore();
    this.drawHud(ctx, width, height, state, now);
  }

  private drawBackdrop(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    element: ElementId,
  ): void {
    const hue = ELEMENT_HUE[element];
    ctx.fillStyle = 'rgba(6, 3, 8, 0.3)';
    ctx.fillRect(0, 0, width, height);

    const g1 = ctx.createRadialGradient(
      width * 0.2,
      height * 0.15,
      10,
      width * 0.2,
      height * 0.15,
      width * 0.55,
    );
    g1.addColorStop(0, `hsla(${hue}, 90%, 50%, ${0.1 + this.heat * 0.12})`);
    g1.addColorStop(1, 'rgba(0,0,0,0)');
    ctx.fillStyle = g1;
    ctx.fillRect(0, 0, width, height);

    const g2 = ctx.createRadialGradient(
      width * 0.8,
      height * 0.85,
      10,
      width * 0.8,
      height * 0.85,
      width * 0.5,
    );
    g2.addColorStop(0, `hsla(${(hue + 40) % 360}, 85%, 45%, ${0.08 + this.chroma * 0.1})`);
    g2.addColorStop(1, 'rgba(0,0,0,0)');
    ctx.fillStyle = g2;
    ctx.fillRect(0, 0, width, height);
  }

  private updateFromHands(
    state: MagicRenderState,
    width: number,
    height: number,
    now: number,
  ): void {
    const palms: Array<{
      id: number;
      x: number;
      y: number;
      open: number;
      speed: number;
      charged: number;
      fist: boolean;
    }> = [];

    for (const hand of state.hands) {
      const palm = mirrorX(hand.palm);
      const open = handOpenness(hand);
      const fist = isFist(hand, 0.34);
      const prev = this.prevPalm.get(hand.id);
      const speed = Math.hypot(hand.tipVelocity.wrist.x, hand.tipVelocity.wrist.y);
      const px = palm.x * width;
      const py = palm.y * height;

      const pose = this.pose.get(hand.id) ?? {
        open,
        charged: 0,
        lastCrushAt: 0,
        lastBloomAt: 0,
        lastSnapAt: 0,
        lastFlickAt: 0,
        fist,
        snapArmed: false,
        thumbMiddle: 1,
      };
      const deltaOpen = open - pose.open;

      if (deltaOpen < -0.18 && pose.open > 0.4 && open < 0.48 && now - pose.lastCrushAt > 360) {
        pose.lastCrushAt = now;
        const power = Math.min(1.85, Math.abs(deltaOpen) * 2.8) * state.intensity;
        this.castCrush(state.element, px, py, power);
        this.pulseGesture(crushLabel(state.element), now);
      }

      if (deltaOpen > 0.18 && pose.open < 0.52 && open > 0.45 && now - pose.lastBloomAt > 360) {
        pose.lastBloomAt = now;
        const power =
          Math.min(2.1, deltaOpen * 2.9 + pose.charged * 1.15) * state.intensity;
        this.castBloom(state.element, px, py, power);
        this.pendingBlooms.push({ at: now, x: px, y: py, power });
        this.pulseGesture(
          pose.charged > 0.55 ? bloomChargedLabel(state.element) : bloomLabel(state.element),
          now,
        );
        pose.charged = 0;
      }

      if (fist) {
        pose.charged = Math.min(1, pose.charged + 0.042 * state.intensity);
        this.emitCrushIdle(state.element, px, py, pose.charged, state.intensity);
        this.attractParticles(px, py, 160 + pose.charged * 150, 0.1 + pose.charged * 0.16);
        this.heat = Math.min(1, this.heat + 0.02);
      } else {
        pose.charged = Math.max(0, pose.charged - 0.018);
        const intensity = state.intensity * (0.5 + open * 0.75);
        this.emitIdle(state.element, px, py, intensity, open);
        this.emitFingerStreams(hand, state.element, width, height, intensity, open);
        this.updateRibbons(hand, state.element, width, height, now, open);

        if (prev && speed > 0.95 / Math.max(0.4, state.intensity)) {
          const dx = (palm.x - prev.x) * width;
          const dy = (palm.y - prev.y) * height;
          this.castSwipe(state.element, px, py, dx, dy, speed, intensity);
        }

        // Pinch-ish: thumb near index → focused beam / fire jet.
        const tip = mirrorX(hand.tips.index);
        const thumb = mirrorX(hand.tips.thumb);
        const middle = mirrorX(hand.tips.middle);
        const pinch = Math.hypot(tip.x - thumb.x, tip.y - thumb.y);
        if (pinch < 0.045 && open > 0.25 && open < 0.75) {
          if (state.element === 'fire') {
            this.emitFireJet(
              tip.x * width,
              tip.y * height,
              tip.x - palm.x,
              tip.y - palm.y,
              state.intensity * (0.85 + (1 - pinch / 0.045)),
            );
          } else {
            this.emitBeam(
              state.element,
              tip.x * width,
              tip.y * height,
              tip.x - palm.x,
              tip.y - palm.y,
              state.intensity * (0.7 + (1 - pinch / 0.045)),
            );
          }
        }

        // Snap: thumb+middle touch then whip apart.
        const thumbMiddle = distance(thumb, middle);
        if (thumbMiddle < 0.036 && open > 0.28) {
          pose.snapArmed = true;
        }
        const middleSpeed = Math.hypot(
          hand.tipVelocity.middle.x,
          hand.tipVelocity.middle.y,
        );
        if (
          pose.snapArmed &&
          thumbMiddle > 0.065 &&
          middleSpeed > 1.15 &&
          now - pose.lastSnapAt > 380
        ) {
          pose.snapArmed = false;
          pose.lastSnapAt = now;
          const sx = ((thumb.x + middle.x) / 2) * width;
          const sy = ((thumb.y + middle.y) / 2) * height;
          const power = Math.min(2, 0.85 + middleSpeed / 2.2) * state.intensity;
          if (state.element === 'fire') {
            this.castFireSnap(sx, sy, hand.tipVelocity.middle.x * width, hand.tipVelocity.middle.y * height, power);
            this.pulseGesture('ЩЕЛЧОК · искра судьбы', now);
          } else {
            this.castGenericSnap(state.element, sx, sy, power);
            this.pulseGesture('ЩЕЛЧОК', now);
          }
        }
        if (thumbMiddle > 0.12) {
          pose.snapArmed = false;
        }
        pose.thumbMiddle = thumbMiddle;

        // Flick: fast index while other tips stay closer to palm → fireball.
        if (state.element === 'fire' && now - pose.lastFlickAt > 420) {
          const indexSpeed = Math.hypot(hand.tipVelocity.index.x, hand.tipVelocity.index.y);
          const indexTip = hand.tips.index;
          const curled =
            distance(hand.tips.middle, hand.palm) < distance(indexTip, hand.palm) * 0.92 &&
            distance(hand.tips.ring, hand.palm) < distance(indexTip, hand.palm) * 0.95;
          if (indexSpeed > 1.55 && curled && open > 0.25 && open < 0.85) {
            pose.lastFlickAt = now;
            const dirX = hand.tipVelocity.index.x;
            const dirY = hand.tipVelocity.index.y;
            this.castFireball(
              tip.x * width,
              tip.y * height,
              dirX * width,
              dirY * height,
              Math.min(2, indexSpeed / 1.4) * state.intensity,
            );
            this.pulseGesture('ФЛИК · огненный шар', now);
          }
        }

        // Upward open palm torch column for fire.
        if (state.element === 'fire' && open > 0.62) {
          const up = palm.y - middle.y; // mirrored y: tip above palm ⇒ positive in image? y grows down, so tip above = smaller y ⇒ palm.y - tip.y > 0
          if (up > 0.04) {
            this.emitFireTorch(px, py, open, intensity);
          }
        }
      }

      palms.push({
        id: hand.id,
        x: px,
        y: py,
        open,
        speed,
        charged: pose.charged,
        fist,
      });

      pose.open = open;
      pose.fist = fist;
      this.pose.set(hand.id, pose);
      this.prevPalm.set(hand.id, palm);
    }

    // Two open palms close → arc / combo.
    const openPalms = palms.filter((p) => !p.fist && p.open > 0.5);
    if (openPalms.length >= 2) {
      const [a, b] = openPalms;
      const dist = Math.hypot(a.x - b.x, a.y - b.y);
      const link = Math.min(width, height) * 0.2;
      if (dist < link) {
        this.drawLiveLink = { a, b, element: state.element };
        if (now - this.lastComboAt > 650) {
          this.lastComboAt = now;
          this.castCombo(state.element, (a.x + b.x) / 2, (a.y + b.y) / 2, state.intensity * 1.15);
          this.pulseGesture('РЕЗОНАНС · две ладони', now);
        }
        if (state.element === 'lightning' || state.element === 'energy') {
          this.bolts.push({
            points: jaggedLine(a.x, a.y, b.x, b.y, 8),
            life: 0.45,
            color:
              state.element === 'lightning'
                ? 'rgba(253, 224, 71, 0.95)'
                : 'rgba(244, 114, 182, 0.95)',
            width: 2.4,
          });
        }
        // Soft particle bridge
        for (let i = 0; i < 3; i += 1) {
          const t = Math.random();
          this.particles.push({
            x: a.x + (b.x - a.x) * t,
            y: a.y + (b.y - a.y) * t,
            vx: (Math.random() - 0.5) * 20,
            vy: (Math.random() - 0.5) * 20,
            life: 1,
            maxLife: 0.25,
            size: 3,
            hue: ELEMENT_HUE[state.element],
            kind: 'star',
          });
        }
      } else {
        this.drawLiveLink = null;
      }
    } else {
      this.drawLiveLink = null;
    }

    // Dual fist charge aura between hands.
    const fists = palms.filter((p) => p.fist);
    if (fists.length >= 2) {
      const [a, b] = fists;
      const charge = (a.charged + b.charged) / 2;
      if (charge > 0.35) {
        this.bolts.push({
          points: jaggedLine(a.x, a.y, b.x, b.y, 5),
          life: 0.25 + charge * 0.3,
          color: `hsla(${ELEMENT_HUE[state.element]}, 95%, 70%, 0.7)`,
          width: 1.5 + charge * 2,
        });
        this.heat = Math.min(1, this.heat + 0.03);
      }
    }

    for (const key of [...this.prevPalm.keys()]) {
      if (!state.hands.some((hand) => hand.id === key)) {
        this.prevPalm.delete(key);
        this.pose.delete(key);
      }
    }
  }

  private drawLiveLink: {
    a: { x: number; y: number };
    b: { x: number; y: number };
    element: ElementId;
  } | null = null;

  private resolveUltimates(state: MagicRenderState, now: number): void {
    this.pendingBlooms = this.pendingBlooms.filter((item) => now - item.at < 320);
    if (this.pendingBlooms.length >= 2 && now - this.lastUltimateAt > 1400) {
      const [a, b] = this.pendingBlooms;
      const midX = (a.x + b.x) / 2;
      const midY = (a.y + b.y) / 2;
      const power = Math.min(2.4, (a.power + b.power) * 0.7);
      this.lastUltimateAt = now;
      this.pendingBlooms = [];
      this.castUltimate(state.element, midX, midY, power);
      this.pulseGesture('УЛЬТИМЕЙТ · двойной каст', now);
    }
  }

  private castCrush(element: ElementId, x: number, y: number, power: number): void {
    this.flash = Math.min(1, this.flash + 0.4);
    this.chroma = Math.min(1, this.chroma + 0.65);
    this.shake = Math.min(1, this.shake + 0.45 + power * 0.2);
    this.heat = Math.min(1, this.heat + 0.35);
    const hue = ELEMENT_HUE[element];

    for (let ring = 0; ring < 4; ring += 1) {
      this.waves.push({
        x,
        y,
        radius: 110 + ring * 42 + power * 40,
        maxRadius: 6,
        life: 1,
        hue: hue + ring * 6,
        mode: 'crush',
        width: 5 + power * 2.5,
      });
    }

    this.attractParticles(x, y, 260, 0.42);

    for (let i = 0; i < Math.floor(40 + power * 55); i += 1) {
      const angle = Math.random() * Math.PI * 2;
      const dist = 80 + Math.random() * (130 + power * 90);
      const spiral = 1 + Math.random() * 0.8;
      this.particles.push({
        x: x + Math.cos(angle) * dist,
        y: y + Math.sin(angle) * dist,
        vx: (-Math.cos(angle) * 170 - Math.sin(angle) * 110) * spiral * power,
        vy: (-Math.sin(angle) * 170 + Math.cos(angle) * 110) * spiral * power,
        life: 1,
        maxLife: 0.6 + Math.random() * 0.4,
        size: 3 + Math.random() * 7 * power,
        hue: hue + Math.random() * 35,
        kind: elementKind(element, 'crush'),
        attractX: x,
        attractY: y,
        trail: true,
      });
    }

    if (element === 'lightning') {
      for (let i = 0; i < 8; i += 1) {
        const angle = (i / 8) * Math.PI * 2;
        this.bolts.push({
          points: jaggedLine(x + Math.cos(angle) * 140, y + Math.sin(angle) * 140, x, y, 8),
          life: 0.7,
          color: 'rgba(254, 249, 195, 0.98)',
          width: 2.2,
        });
      }
    }

    if (element === 'fire') {
      for (let i = 0; i < 18; i += 1) {
        this.particles.push({
          x: x + (Math.random() - 0.5) * 40,
          y: y + (Math.random() - 0.5) * 40,
          vx: (Math.random() - 0.5) * 30,
          vy: -20 - Math.random() * 40,
          life: 1,
          maxLife: 1.1,
          size: 10 + Math.random() * 16,
          hue: 15,
          kind: 'smoke',
        });
      }
    }
  }

  private castBloom(element: ElementId, x: number, y: number, power: number): void {
    this.flash = 1;
    this.chroma = Math.min(1, this.chroma + 0.9);
    this.shake = Math.min(1, this.shake + 0.55 + power * 0.25);
    this.heat = Math.min(1, this.heat + 0.5);
    const hue = ELEMENT_HUE[element];

    for (let ring = 0; ring < 5; ring += 1) {
      this.waves.push({
        x,
        y,
        radius: 10 + ring * 8,
        maxRadius: 160 + power * 190 + ring * 48,
        life: 1,
        hue: hue + ring * 10,
        mode: 'bloom',
        width: 3.5 + power * 3.5,
      });
    }

    const count = Math.floor(64 + power * 90);
    for (let i = 0; i < count; i += 1) {
      const angle = (i / count) * Math.PI * 2 + Math.random() * 0.15;
      const vel = 120 + power * 210 + Math.random() * 80;
      this.particles.push({
        x,
        y,
        vx: Math.cos(angle) * vel,
        vy: Math.sin(angle) * vel,
        life: 1,
        maxLife: 0.75 + Math.random() * 0.5,
        size: 4 + Math.random() * 10 * power,
        hue: hue + Math.random() * 45,
        kind: elementKind(element, 'bloom'),
        trail: Math.random() > 0.55,
      });
    }

    if (element === 'lightning' || element === 'energy') {
      for (let i = 0; i < 10; i += 1) {
        const angle = (i / 10) * Math.PI * 2;
        this.bolts.push({
          points: jaggedLine(
            x,
            y,
            x + Math.cos(angle) * (180 + power * 100),
            y + Math.sin(angle) * (180 + power * 100),
            9,
          ),
          life: 0.85,
          color:
            element === 'lightning'
              ? 'rgba(253, 224, 71, 0.98)'
              : 'rgba(244, 114, 182, 0.95)',
          width: 2.6,
        });
      }
    }

    if (element === 'water') {
      for (let i = 0; i < 30; i += 1) {
        this.particles.push({
          x: x + (Math.random() - 0.5) * 30,
          y: y + (Math.random() - 0.5) * 30,
          vx: (Math.random() - 0.5) * 160 * power,
          vy: (Math.random() - 0.5) * 160 * power,
          life: 1,
          maxLife: 0.7,
          size: 3 + Math.random() * 5,
          hue: 190 + Math.random() * 30,
          kind: 'foam',
        });
      }
    }

    if (element === 'fire') {
      for (let i = 0; i < 28; i += 1) {
        this.particles.push({
          x,
          y,
          vx: (Math.random() - 0.5) * 80,
          vy: -100 - Math.random() * 200 * power,
          life: 1,
          maxLife: 1,
          size: 6 + Math.random() * 12,
          hue: 18 + Math.random() * 30,
          kind: 'ember',
          trail: true,
        });
      }
    }
  }

  private castUltimate(element: ElementId, x: number, y: number, power: number): void {
    this.flash = 1;
    this.chroma = 1;
    this.shake = 1;
    this.heat = 1;
    const hue = ELEMENT_HUE[element];

    for (let ring = 0; ring < 6; ring += 1) {
      this.waves.push({
        x,
        y,
        radius: 8 + ring * 6,
        maxRadius: 220 + power * 240 + ring * 55,
        life: 1.1,
        hue: hue + ring * 12,
        mode: 'ultimate',
        width: 5 + power * 4,
      });
    }

    for (let i = 0; i < Math.floor(120 + power * 100); i += 1) {
      const angle = Math.random() * Math.PI * 2;
      const vel = 150 + power * 260 * Math.random();
      this.particles.push({
        x,
        y,
        vx: Math.cos(angle) * vel,
        vy: Math.sin(angle) * vel,
        life: 1,
        maxLife: 1.1,
        size: 5 + Math.random() * 14,
        hue: hue + Math.random() * 60,
        kind: elementKind(element, 'bloom'),
        trail: true,
      });
    }

    for (let i = 0; i < 16; i += 1) {
      const angle = (i / 16) * Math.PI * 2;
      this.bolts.push({
        points: jaggedLine(
          x,
          y,
          x + Math.cos(angle) * (240 + power * 80),
          y + Math.sin(angle) * (240 + power * 80),
          11,
        ),
        life: 1,
        color: `hsla(${hue}, 95%, 75%, 0.98)`,
        width: 3.2,
      });
    }
  }

  private castCombo(element: ElementId, x: number, y: number, intensity: number): void {
    this.castBloom(element, x, y, 1.35 * intensity);
  }

  private emitCrushIdle(
    element: ElementId,
    x: number,
    y: number,
    charged: number,
    intensity: number,
  ): void {
    const n = 1 + Math.floor(charged * 5);
    for (let i = 0; i < n; i += 1) {
      const angle = Math.random() * Math.PI * 2 + this.time / 180;
      const dist = 28 + charged * 85;
      this.particles.push({
        x: x + Math.cos(angle) * dist,
        y: y + Math.sin(angle) * dist,
        vx: -Math.cos(angle) * (50 + charged * 100) + -Math.sin(angle) * 30,
        vy: -Math.sin(angle) * (50 + charged * 100) + Math.cos(angle) * 30,
        life: 1,
        maxLife: 0.4,
        size: 2.5 + charged * 6 * intensity,
        hue: ELEMENT_HUE[element] + Math.random() * 20,
        kind: 'shard',
        attractX: x,
        attractY: y,
        trail: true,
      });
    }

    // Orbiting charge pearls
    if (charged > 0.25) {
      for (let i = 0; i < 3; i += 1) {
        const angle = this.time / 140 + (i / 3) * Math.PI * 2;
        const r = 24 + charged * 40;
        this.particles.push({
          x: x + Math.cos(angle) * r,
          y: y + Math.sin(angle) * r,
          vx: 0,
          vy: 0,
          life: 1,
          maxLife: 0.12,
          size: 3 + charged * 4,
          hue: ELEMENT_HUE[element],
          kind: 'star',
        });
      }
    }
  }

  private emitFingerStreams(
    hand: TrackedHand,
    element: ElementId,
    width: number,
    height: number,
    intensity: number,
    open: number,
  ): void {
    if (open < 0.4) {
      return;
    }
    for (const finger of FINGER_CAST) {
      if (finger === 'thumb' && element !== 'energy') {
        continue;
      }
      const tip = mirrorX(hand.tips[finger]);
      const vel = hand.tipVelocity[finger];
      const speed = Math.hypot(vel.x, vel.y);
      if (speed < 0.25 && Math.random() > 0.4) {
        continue;
      }
      const x = tip.x * width;
      const y = tip.y * height;
      const count = speed > 0.7 ? 2 : 1;
      for (let i = 0; i < count; i += 1) {
        this.particles.push({
          x,
          y,
          vx: vel.x * width * 0.35 + (Math.random() - 0.5) * 25,
          vy: vel.y * height * 0.35 + (Math.random() - 0.5) * 25,
          life: 1,
          maxLife: 0.35 + Math.random() * 0.25,
          size: 2 + intensity * 3,
          hue: ELEMENT_HUE[element] + Math.random() * 25,
          kind: elementKind(element, 'idle'),
          trail: true,
        });
      }
    }
  }

  private updateRibbons(
    hand: TrackedHand,
    element: ElementId,
    width: number,
    height: number,
    now: number,
    open: number,
  ): void {
    if (open < 0.35) {
      return;
    }
    for (const finger of ['index', 'middle', 'pinky'] as FingerName[]) {
      const tip = mirrorX(hand.tips[finger]);
      const key = `${hand.id}:${finger}`;
      const ribbon = this.ribbons.get(key) ?? {
        points: [],
        hue: ELEMENT_HUE[element],
        width: finger === 'index' ? 5 : 3.2,
      };
      ribbon.hue = ELEMENT_HUE[element];
      ribbon.points.push({ x: tip.x * width, y: tip.y * height, at: now });
      ribbon.points = ribbon.points.filter((p) => now - p.at < 280).slice(-22);
      this.ribbons.set(key, ribbon);
    }
  }

  private pruneRibbons(now: number): void {
    for (const [key, ribbon] of this.ribbons) {
      ribbon.points = ribbon.points.filter((p) => now - p.at < 280);
      if (ribbon.points.length === 0) {
        this.ribbons.delete(key);
      }
    }
  }

  private emitBeam(
    element: ElementId,
    x: number,
    y: number,
    dx: number,
    dy: number,
    power: number,
  ): void {
    const len = Math.hypot(dx, dy) || 1;
    const nx = dx / len;
    const ny = dy / len;
    const reach = 90 + power * 110;
    this.bolts.push({
      points: jaggedLine(x, y, x + nx * reach, y + ny * reach, 6),
      life: 0.28,
      color: `hsla(${ELEMENT_HUE[element]}, 95%, 75%, 0.9)`,
      width: 2 + power,
    });
    this.particles.push({
      x: x + nx * reach,
      y: y + ny * reach,
      vx: nx * 40,
      vy: ny * 40,
      life: 1,
      maxLife: 0.25,
      size: 5,
      hue: ELEMENT_HUE[element],
      kind: 'star',
    });
  }

  /** Continuous flamethrower from pinched index+thumb. */
  private emitFireJet(x: number, y: number, dx: number, dy: number, power: number): void {
    const len = Math.hypot(dx, dy) || 1;
    const nx = dx / len;
    const ny = dy / len;
    for (let i = 0; i < 4; i += 1) {
      const spread = (Math.random() - 0.5) * 0.55;
      const cx = nx * Math.cos(spread) - ny * Math.sin(spread);
      const cy = nx * Math.sin(spread) + ny * Math.cos(spread);
      const vel = 140 + power * 180 + Math.random() * 60;
      this.particles.push({
        x: x + (Math.random() - 0.5) * 8,
        y: y + (Math.random() - 0.5) * 8,
        vx: cx * vel,
        vy: cy * vel - 20,
        life: 1,
        maxLife: 0.35 + Math.random() * 0.25,
        size: 5 + Math.random() * 9 * power,
        hue: 18 + Math.random() * 28,
        kind: Math.random() > 0.75 ? 'smoke' : 'ember',
        trail: true,
      });
    }
    this.heat = Math.min(1, this.heat + 0.04);
  }

  /** Finger snap ignition — crack + radial sparks + shock. */
  private castFireSnap(
    x: number,
    y: number,
    vx: number,
    vy: number,
    power: number,
  ): void {
    this.flash = 1;
    this.shake = Math.min(1, this.shake + 0.7);
    this.chroma = Math.min(1, this.chroma + 0.85);
    this.heat = Math.min(1, this.heat + 0.7);

    for (let ring = 0; ring < 3; ring += 1) {
      this.waves.push({
        x,
        y,
        radius: 6 + ring * 4,
        maxRadius: 90 + power * 110 + ring * 35,
        life: 1,
        hue: 22 + ring * 12,
        mode: 'bloom',
        width: 4 + power * 2,
      });
    }

    // Core flash flare
    for (let i = 0; i < 18; i += 1) {
      this.particles.push({
        x,
        y,
        vx: (Math.random() - 0.5) * 40,
        vy: (Math.random() - 0.5) * 40,
        life: 1,
        maxLife: 0.22,
        size: 10 + Math.random() * 18 * power,
        hue: 35 + Math.random() * 25,
        kind: 'flare',
      });
    }

    // Directional spark cone along snap whip
    const len = Math.hypot(vx, vy) || 1;
    const nx = vx / len;
    const ny = vy / len;
    for (let i = 0; i < Math.floor(36 + power * 40); i += 1) {
      const spread = (Math.random() - 0.5) * 1.4;
      const cx = nx * Math.cos(spread) - ny * Math.sin(spread);
      const cy = nx * Math.sin(spread) + ny * Math.cos(spread);
      const speed = 160 + power * 220 * Math.random();
      this.particles.push({
        x,
        y,
        vx: cx * speed,
        vy: cy * speed - 40,
        life: 1,
        maxLife: 0.55 + Math.random() * 0.35,
        size: 3 + Math.random() * 7,
        hue: 15 + Math.random() * 40,
        kind: Math.random() > 0.65 ? 'spark' : 'ember',
        trail: true,
      });
    }

    // Falling cinders
    for (let i = 0; i < 16; i += 1) {
      this.particles.push({
        x: x + (Math.random() - 0.5) * 30,
        y: y + (Math.random() - 0.5) * 20,
        vx: (Math.random() - 0.5) * 60,
        vy: 40 + Math.random() * 80,
        life: 1,
        maxLife: 0.9,
        size: 2 + Math.random() * 4,
        hue: 25,
        kind: 'ember',
      });
    }
  }

  private castGenericSnap(element: ElementId, x: number, y: number, power: number): void {
    this.flash = Math.min(1, this.flash + 0.6);
    this.shake = Math.min(1, this.shake + 0.35);
    for (let i = 0; i < Math.floor(24 + power * 20); i += 1) {
      const angle = Math.random() * Math.PI * 2;
      const vel = 100 + power * 140 * Math.random();
      this.particles.push({
        x,
        y,
        vx: Math.cos(angle) * vel,
        vy: Math.sin(angle) * vel,
        life: 1,
        maxLife: 0.45,
        size: 3 + Math.random() * 5,
        hue: ELEMENT_HUE[element],
        kind: 'spark',
        trail: true,
      });
    }
  }

  /** Index flick fireball projectile. */
  private castFireball(
    x: number,
    y: number,
    vx: number,
    vy: number,
    power: number,
  ): void {
    const len = Math.hypot(vx, vy) || 1;
    const speed = 280 + power * 160;
    this.fireballs.push({
      x,
      y,
      vx: (vx / len) * speed,
      vy: (vy / len) * speed,
      life: 1.15,
      size: 16 + power * 10,
    });
    this.flash = Math.min(1, this.flash + 0.35);
    this.shake = Math.min(1, this.shake + 0.3);
    this.heat = Math.min(1, this.heat + 0.4);

    for (let i = 0; i < 12; i += 1) {
      this.particles.push({
        x,
        y,
        vx: (vx / len) * 40 + (Math.random() - 0.5) * 50,
        vy: (vy / len) * 40 + (Math.random() - 0.5) * 50,
        life: 1,
        maxLife: 0.35,
        size: 5,
        hue: 25,
        kind: 'ember',
      });
    }
  }

  private emitFireTorch(x: number, y: number, open: number, intensity: number): void {
    const n = 2 + Math.floor(open * 3);
    for (let i = 0; i < n; i += 1) {
      this.particles.push({
        x: x + (Math.random() - 0.5) * 16,
        y: y - 8,
        vx: (Math.random() - 0.5) * 25,
        vy: -70 - Math.random() * 110 * intensity * open,
        life: 1,
        maxLife: 0.7 + Math.random() * 0.4,
        size: 5 + Math.random() * 10 * open,
        hue: 20 + Math.random() * 30,
        kind: Math.random() > 0.85 ? 'smoke' : 'ember',
        trail: true,
      });
    }
  }

  private stepFireballs(element: ElementId): void {
    if (element !== 'fire' && this.fireballs.length === 0) {
      this.fireballs = [];
      return;
    }
    const next: Fireball[] = [];
    for (const ball of this.fireballs) {
      ball.x += ball.vx * 0.016;
      ball.y += ball.vy * 0.016;
      ball.vy += 8 * 0.016;
      ball.life -= 0.016;
      ball.size *= 0.995;

      // Exhaust trail
      for (let i = 0; i < 3; i += 1) {
        this.particles.push({
          x: ball.x + (Math.random() - 0.5) * ball.size * 0.4,
          y: ball.y + (Math.random() - 0.5) * ball.size * 0.4,
          vx: -ball.vx * 0.15 + (Math.random() - 0.5) * 40,
          vy: -ball.vy * 0.15 + (Math.random() - 0.5) * 40,
          life: 1,
          maxLife: 0.3,
          size: 4 + Math.random() * 6,
          hue: 18 + Math.random() * 35,
          kind: Math.random() > 0.7 ? 'smoke' : 'ember',
          trail: true,
        });
      }

      if (ball.life <= 0) {
        // Impact burst
        this.castFireSnap(ball.x, ball.y, ball.vx, ball.vy, 1.1);
        this.pulseGesture('ВЗРЫВ · шар', this.time);
        continue;
      }
      next.push(ball);
    }
    this.fireballs = next.slice(-8);
  }

  private drawFireballs(ctx: CanvasRenderingContext2D): void {
    for (const ball of this.fireballs) {
      const g = ctx.createRadialGradient(ball.x, ball.y, 2, ball.x, ball.y, ball.size);
      g.addColorStop(0, 'rgba(255, 250, 220, 0.95)');
      g.addColorStop(0.25, 'rgba(255, 180, 60, 0.9)');
      g.addColorStop(0.6, 'rgba(255, 80, 20, 0.55)');
      g.addColorStop(1, 'rgba(80, 10, 0, 0)');
      ctx.fillStyle = g;
      ctx.shadowBlur = 24;
      ctx.shadowColor = 'rgba(255, 120, 20, 0.9)';
      ctx.beginPath();
      ctx.arc(ball.x, ball.y, ball.size, 0, Math.PI * 2);
      ctx.fill();
      ctx.shadowBlur = 0;
    }
  }

  private emitIdle(
    element: ElementId,
    x: number,
    y: number,
    intensity: number,
    open: number,
  ): void {
    const count = Math.floor(2 + open * 5 * intensity);
    for (let i = 0; i < count; i += 1) {
      if (element === 'fire') {
        this.particles.push({
          x: x + (Math.random() - 0.5) * 22,
          y: y + (Math.random() - 0.5) * 12,
          vx: (Math.random() - 0.5) * 35,
          vy: -50 - Math.random() * 90 * intensity,
          life: 1,
          maxLife: 0.75 + Math.random() * 0.45,
          size: 4 + Math.random() * 9 * intensity,
          hue: ELEMENT_HUE.fire + Math.random() * 22,
          kind: Math.random() > 0.82 ? 'smoke' : 'ember',
          trail: true,
        });
      } else if (element === 'water') {
        this.particles.push({
          x: x + (Math.random() - 0.5) * 26,
          y: y + (Math.random() - 0.5) * 18,
          vx: (Math.random() - 0.5) * 45,
          vy: 25 + Math.random() * 55,
          life: 1,
          maxLife: 0.85 + Math.random() * 0.45,
          size: 3 + Math.random() * 6,
          hue: ELEMENT_HUE.water + Math.random() * 28,
          kind: Math.random() > 0.7 ? 'foam' : 'drop',
        });
      } else if (element === 'lightning') {
        if (Math.random() > 0.4) {
          this.particles.push({
            x,
            y,
            vx: (Math.random() - 0.5) * 150,
            vy: (Math.random() - 0.5) * 150,
            life: 1,
            maxLife: 0.22 + Math.random() * 0.2,
            size: 2 + Math.random() * 3.5,
            hue: ELEMENT_HUE.lightning,
            kind: 'spark',
            trail: true,
          });
        }
      } else {
        const angle = Math.random() * Math.PI * 2;
        const radius = 22 + open * 56;
        this.particles.push({
          x: x + Math.cos(angle) * radius,
          y: y + Math.sin(angle) * radius,
          vx: Math.cos(angle) * 22,
          vy: Math.sin(angle) * 22,
          life: 1,
          maxLife: 0.65 + Math.random() * 0.4,
          size: 3 + Math.random() * 5,
          hue: ELEMENT_HUE.energy + Math.random() * 45,
          kind: Math.random() > 0.7 ? 'star' : 'orb',
          trail: true,
        });
      }
    }
  }

  private castSwipe(
    element: ElementId,
    x: number,
    y: number,
    dx: number,
    dy: number,
    speed: number,
    intensity: number,
  ): void {
    const len = Math.hypot(dx, dy) || 1;
    const nx = dx / len;
    const ny = dy / len;
    const power = Math.min(2, speed / 1.45) * intensity;

    if (element === 'lightning') {
      const tx = x + nx * (140 + power * 180);
      const ty = y + ny * (140 + power * 180);
      this.bolts.push({
        points: jaggedLine(x, y, tx, ty, 10),
        life: 0.8,
        color: 'rgba(254, 249, 195, 0.98)',
        width: 3,
      });
      // Side forks
      for (let f = 0; f < 2; f += 1) {
        const side = f === 0 ? 1 : -1;
        this.bolts.push({
          points: jaggedLine(
            x,
            y,
            tx + -ny * 50 * side,
            ty + nx * 50 * side,
            7,
          ),
          life: 0.5,
          color: 'rgba(253, 224, 71, 0.85)',
          width: 1.6,
        });
      }
      this.flash = Math.min(1, this.flash + 0.5);
      this.shake = Math.min(1, this.shake + 0.25);
      return;
    }

    const burst = Math.floor(12 + power * 18);
    for (let i = 0; i < burst; i += 1) {
      const spread = (Math.random() - 0.5) * 0.85;
      const cx = nx * Math.cos(spread) - ny * Math.sin(spread);
      const cy = nx * Math.sin(spread) + ny * Math.cos(spread);
      const vel = 100 + power * 160 + Math.random() * 50;
      this.particles.push({
        x,
        y,
        vx: cx * vel,
        vy: cy * vel,
        life: 1,
        maxLife: element === 'fire' ? 0.75 : 0.95,
        size: 4 + Math.random() * 8 * power,
        hue: ELEMENT_HUE[element] + Math.random() * 28,
        kind: elementKind(element, 'bloom'),
        trail: true,
      });
    }
  }

  private attractParticles(x: number, y: number, radius: number, strength: number): void {
    for (const p of this.particles) {
      const dx = x - p.x;
      const dy = y - p.y;
      const dist = Math.hypot(dx, dy) || 1;
      if (dist < radius) {
        p.vx += (dx / dist) * strength * 140;
        p.vy += (dy / dist) * strength * 140;
        p.attractX = x;
        p.attractY = y;
      }
    }
  }

  private pulseGesture(label: string, now: number): void {
    this.gestureLabel = label;
    this.gestureLabelUntil = now + 1000;
  }

  private stepWaves(): void {
    const next: Shockwave[] = [];
    for (const wave of this.waves) {
      if (wave.mode === 'crush') {
        wave.radius += (wave.maxRadius - wave.radius) * 0.24;
        wave.life -= 0.04;
      } else {
        wave.radius += (wave.maxRadius - wave.radius) * 0.12 + (wave.mode === 'ultimate' ? 9 : 7);
        wave.life -= wave.mode === 'ultimate' ? 0.022 : 0.028;
      }
      if (wave.life > 0 && wave.radius > 2) {
        next.push(wave);
      }
    }
    this.waves = next;
  }

  private stepParticles(element: ElementId): void {
    const next: Particle[] = [];
    for (const p of this.particles) {
      if (p.attractX !== undefined && p.attractY !== undefined) {
        const dx = p.attractX - p.x;
        const dy = p.attractY - p.y;
        const dist = Math.hypot(dx, dy) || 1;
        p.vx += (dx / dist) * 32;
        p.vy += (dy / dist) * 32;
        p.vx += (-dy / dist) * 14;
        p.vy += (dx / dist) * 14;
      }

      if (p.spin) {
        p.vx += Math.cos(this.time / 80 + p.spin) * 2;
        p.vy += Math.sin(this.time / 80 + p.spin) * 2;
      }

      p.x += p.vx * 0.016;
      p.y += p.vy * 0.016;

      if (p.kind === 'ember') {
        p.vy -= 22 * 0.016;
        p.vx *= 0.985;
      } else if (p.kind === 'smoke') {
        p.vy -= 10 * 0.016;
        p.vx *= 0.99;
        p.size *= 1.01;
      } else if (p.kind === 'drop') {
        p.vy += 60 * 0.016;
        p.vx *= 0.99;
      } else if (p.kind === 'foam') {
        p.vy += 25 * 0.016;
        p.vx *= 0.98;
      } else if (p.kind === 'orb' || p.kind === 'star') {
        p.vx *= 0.975;
        p.vy *= 0.975;
      } else if (p.kind === 'shard') {
        p.vx *= 0.96;
        p.vy *= 0.96;
      } else {
        p.vx *= 0.94;
        p.vy *= 0.94;
      }

      // Soft trail ghost
      if (p.trail && Math.random() > 0.7) {
        next.push({
          ...p,
          vx: p.vx * 0.2,
          vy: p.vy * 0.2,
          life: p.life * 0.6,
          maxLife: p.maxLife * 0.45,
          size: p.size * 0.65,
          trail: false,
          attractX: undefined,
          attractY: undefined,
          hue: p.hue + (element === 'energy' ? 20 : 0),
        });
      }

      p.life -= 0.016 / p.maxLife;
      if (p.life > 0) {
        next.push(p);
      }
    }
    this.particles = next.slice(-1400);
  }

  private drawRibbons(ctx: CanvasRenderingContext2D, now: number): void {
    for (const ribbon of this.ribbons.values()) {
      if (ribbon.points.length < 2) {
        continue;
      }
      ctx.beginPath();
      ctx.moveTo(ribbon.points[0].x, ribbon.points[0].y);
      for (let i = 1; i < ribbon.points.length; i += 1) {
        ctx.lineTo(ribbon.points[i].x, ribbon.points[i].y);
      }
      const alpha = Math.max(0.15, 1 - (now - ribbon.points[0].at) / 280);
      ctx.strokeStyle = `hsla(${ribbon.hue}, 95%, 65%, ${alpha * 0.75})`;
      ctx.lineWidth = ribbon.width;
      ctx.lineCap = 'round';
      ctx.lineJoin = 'round';
      ctx.shadowBlur = 12;
      ctx.shadowColor = `hsla(${ribbon.hue}, 95%, 60%, 0.8)`;
      ctx.stroke();
      ctx.shadowBlur = 0;
    }
  }

  private drawWaves(ctx: CanvasRenderingContext2D): void {
    for (const wave of this.waves) {
      ctx.save();
      const boost = wave.mode === 'ultimate' ? 1 : 0.85;
      ctx.globalAlpha = Math.max(0, wave.life) * boost;
      ctx.strokeStyle = `hsla(${wave.hue}, 95%, ${wave.mode === 'crush' ? 42 : 72}%, 0.95)`;
      ctx.lineWidth = wave.width * Math.max(0.25, wave.life);
      ctx.shadowBlur = wave.mode === 'ultimate' ? 28 : 18;
      ctx.shadowColor = `hsla(${wave.hue}, 95%, 60%, 0.95)`;
      ctx.beginPath();
      ctx.arc(wave.x, wave.y, Math.max(1, wave.radius), 0, Math.PI * 2);
      ctx.stroke();
      if (wave.mode !== 'crush') {
        ctx.strokeStyle = 'rgba(255,255,255,0.55)';
        ctx.lineWidth = 1.3;
        ctx.shadowBlur = 6;
        ctx.stroke();
      }
      ctx.restore();
    }
  }

  private drawParticles(ctx: CanvasRenderingContext2D): void {
    for (const p of this.particles) {
      const alpha = Math.max(0, p.life);
      ctx.globalAlpha = alpha;
      ctx.shadowBlur = p.kind === 'spark' || p.kind === 'star' ? 16 : 10;
      ctx.shadowColor = `hsla(${p.hue}, 95%, 60%, ${alpha})`;

      if (p.kind === 'flare') {
        const g = ctx.createRadialGradient(p.x, p.y, 0, p.x, p.y, p.size);
        g.addColorStop(0, `hsla(${p.hue}, 100%, 92%, ${alpha})`);
        g.addColorStop(0.35, `hsla(${p.hue}, 95%, 60%, ${alpha * 0.7})`);
        g.addColorStop(1, `hsla(${p.hue}, 90%, 40%, 0)`);
        ctx.fillStyle = g;
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.size, 0, Math.PI * 2);
        ctx.fill();
      } else if (p.kind === 'drop') {
        ctx.fillStyle = `hsla(${p.hue}, 85%, 65%, ${alpha})`;
        ctx.beginPath();
        ctx.ellipse(p.x, p.y, p.size * 0.55, p.size, 0, 0, Math.PI * 2);
        ctx.fill();
      } else if (p.kind === 'foam') {
        ctx.fillStyle = `hsla(${p.hue}, 70%, 85%, ${alpha * 0.8})`;
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.size, 0, Math.PI * 2);
        ctx.fill();
      } else if (p.kind === 'smoke') {
        ctx.fillStyle = `hsla(${p.hue}, 30%, 25%, ${alpha * 0.35})`;
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.size, 0, Math.PI * 2);
        ctx.fill();
      } else if (p.kind === 'shard') {
        ctx.fillStyle = `hsla(${p.hue}, 95%, 70%, ${alpha})`;
        ctx.beginPath();
        ctx.moveTo(p.x, p.y - p.size);
        ctx.lineTo(p.x + p.size * 0.65, p.y + p.size * 0.45);
        ctx.lineTo(p.x - p.size * 0.65, p.y + p.size * 0.45);
        ctx.closePath();
        ctx.fill();
      } else if (p.kind === 'star') {
        ctx.fillStyle = `hsla(${p.hue}, 95%, 80%, ${alpha})`;
        drawStar(ctx, p.x, p.y, 4, p.size, p.size * 0.45);
      } else {
        const gradient = ctx.createRadialGradient(p.x, p.y, 0, p.x, p.y, p.size);
        gradient.addColorStop(0, `hsla(${p.hue}, 95%, 78%, ${alpha})`);
        gradient.addColorStop(0.45, `hsla(${p.hue}, 90%, 55%, ${alpha * 0.8})`);
        gradient.addColorStop(1, `hsla(${p.hue}, 90%, 40%, 0)`);
        ctx.fillStyle = gradient;
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.size, 0, Math.PI * 2);
        ctx.fill();
      }
      ctx.shadowBlur = 0;
      ctx.globalAlpha = 1;
    }
  }

  private drawBolts(ctx: CanvasRenderingContext2D): void {
    for (const bolt of this.bolts) {
      if (bolt.points.length < 2) {
        continue;
      }
      ctx.save();
      ctx.globalAlpha = Math.max(0, bolt.life);
      ctx.strokeStyle = bolt.color;
      ctx.lineWidth = bolt.width;
      ctx.shadowBlur = 20;
      ctx.shadowColor = bolt.color;
      ctx.beginPath();
      ctx.moveTo(bolt.points[0].x, bolt.points[0].y);
      for (let i = 1; i < bolt.points.length; i += 1) {
        ctx.lineTo(bolt.points[i].x, bolt.points[i].y);
      }
      ctx.stroke();
      ctx.lineWidth = Math.max(1, bolt.width * 0.35);
      ctx.strokeStyle = 'rgba(255,255,255,0.92)';
      ctx.shadowBlur = 4;
      ctx.stroke();
      ctx.restore();
    }
  }

  private drawHandOrbs(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    state: MagicRenderState,
  ): void {
    if (this.drawLiveLink) {
      const { a, b, element } = this.drawLiveLink;
      const hue = ELEMENT_HUE[element];
      const grad = ctx.createLinearGradient(a.x, a.y, b.x, b.y);
      grad.addColorStop(0, `hsla(${hue}, 95%, 65%, 0.15)`);
      grad.addColorStop(0.5, `hsla(${hue}, 95%, 75%, 0.45)`);
      grad.addColorStop(1, `hsla(${hue}, 95%, 65%, 0.15)`);
      ctx.strokeStyle = grad;
      ctx.lineWidth = 10;
      ctx.beginPath();
      ctx.moveTo(a.x, a.y);
      ctx.lineTo(b.x, b.y);
      ctx.stroke();
    }

    for (const hand of state.hands) {
      const palm = mirrorX(hand.palm);
      const px = palm.x * width;
      const py = palm.y * height;
      const open = handOpenness(hand);
      const hue = ELEMENT_HUE[state.element];
      const pose = this.pose.get(hand.id);

      if (isFist(hand, 0.34)) {
        const charge = pose?.charged ?? 0;
        const radius = 14 + charge * 40;
        const gradient = ctx.createRadialGradient(px, py, 1, px, py, radius);
        gradient.addColorStop(0, `hsla(${hue}, 95%, 88%, 0.98)`);
        gradient.addColorStop(0.35, `hsla(${hue}, 90%, 50%, 0.6)`);
        gradient.addColorStop(1, `hsla(${hue}, 80%, 20%, 0)`);
        ctx.fillStyle = gradient;
        ctx.beginPath();
        ctx.arc(px, py, radius, 0, Math.PI * 2);
        ctx.fill();

        ctx.strokeStyle = `hsla(${hue}, 95%, 75%, ${0.45 + charge * 0.55})`;
        ctx.lineWidth = 3;
        ctx.beginPath();
        ctx.arc(px, py, radius + 8, -Math.PI / 2, -Math.PI / 2 + charge * Math.PI * 2);
        ctx.stroke();

        ctx.strokeStyle = `hsla(${hue}, 90%, 60%, 0.25)`;
        ctx.lineWidth = 1.5;
        ctx.beginPath();
        ctx.arc(px, py, radius + 16 + charge * 10, 0, Math.PI * 2);
        ctx.stroke();
        continue;
      }

      const radius = 18 + open * 44 * state.intensity;
      const gradient = ctx.createRadialGradient(px, py, 2, px, py, radius);
      gradient.addColorStop(0, `hsla(${hue}, 95%, 80%, 0.9)`);
      gradient.addColorStop(0.45, `hsla(${hue}, 90%, 55%, 0.38)`);
      gradient.addColorStop(1, `hsla(${hue}, 90%, 40%, 0)`);
      ctx.fillStyle = gradient;
      ctx.beginPath();
      ctx.arc(px, py, radius, 0, Math.PI * 2);
      ctx.fill();

      for (const finger of FINGER_NAMES) {
        const tip = mirrorX(hand.tips[finger]);
        ctx.beginPath();
        ctx.arc(tip.x * width, tip.y * height, finger === 'index' ? 5.5 : 3.8, 0, Math.PI * 2);
        ctx.fillStyle = `hsla(${hue}, 95%, 72%, 0.9)`;
        ctx.fill();
      }
    }
  }

  private drawAmbientAura(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    element: ElementId,
  ): void {
    const hue = ELEMENT_HUE[element];
    const gradient = ctx.createRadialGradient(
      width * 0.5,
      height * 0.55,
      20,
      width * 0.5,
      height * 0.55,
      Math.max(width, height) * 0.6,
    );
    gradient.addColorStop(0, `hsla(${hue}, 80%, 45%, ${0.1 + this.chroma * 0.14 + this.heat * 0.08})`);
    gradient.addColorStop(1, 'rgba(0,0,0,0)');
    ctx.fillStyle = gradient;
    ctx.fillRect(0, 0, width, height);
  }

  private drawSkeleton(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    hands: TrackedHand[],
    element: ElementId,
  ): void {
    const hue = ELEMENT_HUE[element];
    ctx.strokeStyle = `hsla(${hue}, 80%, 70%, 0.32)`;
    ctx.lineWidth = 1.5;
    for (const hand of hands) {
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

  private drawHud(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    state: MagicRenderState,
    now: number,
  ): void {
    ctx.fillStyle = 'rgba(244,246,255,0.82)';
    ctx.font = '600 13px sans-serif';
    ctx.textAlign = 'left';
    ctx.fillText(state.hint, 18, height - 18);

    if (this.gestureLabel && now < this.gestureLabelUntil) {
      const t = (this.gestureLabelUntil - now) / 1000;
      ctx.textAlign = 'center';
      ctx.font = '800 26px sans-serif';
      ctx.fillStyle = `rgba(255, 248, 235, ${0.3 + t * 0.7})`;
      ctx.shadowBlur = 22;
      ctx.shadowColor = `hsla(${ELEMENT_HUE[state.element]}, 95%, 60%, 0.95)`;
      ctx.fillText(this.gestureLabel, width / 2, 56);
      ctx.shadowBlur = 0;
    }

    if (state.showDebug) {
      ctx.textAlign = 'left';
      ctx.font = '600 12px sans-serif';
      ctx.fillStyle = 'rgba(244,246,255,0.75)';
      ctx.fillText(
        `cam ${state.cameraFps} · hands ${state.handFps} · p ${this.particles.length} · shake ${this.shake.toFixed(2)}`,
        18,
        28,
      );
    }
  }
}

function elementKind(element: ElementId, mode: 'crush' | 'bloom' | 'idle'): ParticleKind {
  if (element === 'fire') {
    return mode === 'crush' ? 'shard' : 'ember';
  }
  if (element === 'water') {
    return mode === 'crush' ? 'foam' : 'drop';
  }
  if (element === 'lightning') {
    return 'spark';
  }
  return mode === 'crush' ? 'shard' : 'orb';
}

function crushLabel(element: ElementId): string {
  switch (element) {
    case 'fire':
      return 'СЖАТИЕ · пепельная сингулярность';
    case 'water':
      return 'СЖАТИЕ · водоворот';
    case 'lightning':
      return 'СЖАТИЕ · грозовой заряд';
    default:
      return 'СЖАТИЕ · сингулярность';
  }
}

function bloomLabel(element: ElementId): string {
  switch (element) {
    case 'fire':
      return 'РАСКРЫТИЕ · огненная нова';
    case 'water':
      return 'РАСКРЫТИЕ · цунами-кольцо';
    case 'lightning':
      return 'РАСКРЫТИЕ · грозовой венец';
    default:
      return 'РАСКРЫТИЕ · супернова';
  }
}

function bloomChargedLabel(element: ElementId): string {
  switch (element) {
    case 'fire':
      return 'ВЗРЫВ · инферно';
    case 'water':
      return 'ВЗРЫВ · катаклизм';
    case 'lightning':
      return 'ВЗРЫВ · небесный разряд';
    default:
      return 'ВЗРЫВ · перегрузка';
  }
}

function jaggedLine(
  x0: number,
  y0: number,
  x1: number,
  y1: number,
  segments: number,
): Array<{ x: number; y: number }> {
  const points = [{ x: x0, y: y0 }];
  for (let i = 1; i < segments; i += 1) {
    const t = i / segments;
    const x = x0 + (x1 - x0) * t;
    const y = y0 + (y1 - y0) * t;
    const offset = (Math.random() - 0.5) * 34;
    const dx = x1 - x0;
    const dy = y1 - y0;
    const len = Math.hypot(dx, dy) || 1;
    points.push({
      x: x + (-dy / len) * offset,
      y: y + (dx / len) * offset,
    });
  }
  points.push({ x: x1, y: y1 });
  return points;
}

function drawStar(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  spikes: number,
  outer: number,
  inner: number,
): void {
  let rot = (Math.PI / 2) * 3;
  const step = Math.PI / spikes;
  ctx.beginPath();
  ctx.moveTo(x, y - outer);
  for (let i = 0; i < spikes; i += 1) {
    ctx.lineTo(x + Math.cos(rot) * outer, y + Math.sin(rot) * outer);
    rot += step;
    ctx.lineTo(x + Math.cos(rot) * inner, y + Math.sin(rot) * inner);
    rot += step;
  }
  ctx.closePath();
  ctx.fill();
}
