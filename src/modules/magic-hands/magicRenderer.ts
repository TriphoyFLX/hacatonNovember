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
import type { Point3D, TrackedFace } from '@/modules/vision/faceTracker';
import type { ElementId } from './types';

type ParticleKind = 'ember' | 'drop' | 'spark' | 'orb';

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
  trail?: boolean;
}

interface Shockwave {
  x: number;
  y: number;
  radius: number;
  maxRadius: number;
  life: number;
  hue: number;
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
  face: TrackedFace | null;
  element: ElementId;
  intensity: number;
  showSkeleton: boolean;
  showDebug: boolean;
  handFps: number;
  cameraFps: number;
  hint: string;
}

const ELEMENT_HUE: Record<ElementId, number> = {
  fire: 24,
  water: 198,
  lightning: 50,
  energy: 315,
};

const FINGER_CAST: FingerName[] = ['thumb', 'index', 'middle', 'ring', 'pinky'];

export class MagicRenderer {
  private particles: Particle[] = [];
  private waves: Shockwave[] = [];
  private ribbons = new Map<string, Ribbon>();
  private fireballs: Fireball[] = [];
  private prevPalm = new Map<number, Point2D>();
  private pose = new Map<number, HandPoseState>();

  private flash = 0;
  private shake = 0;
  private heat = 0;
  private time = 0;
  private laserWasActive = false;
  private lastComboAt = 0;
  private gestureLabel: string | null = null;
  private gestureLabelUntil = 0;

  clear(): void {
    this.particles = [];
    this.waves = [];
    this.ribbons.clear();
    this.fireballs = [];
    this.prevPalm.clear();
    this.pose.clear();
    this.flash = 0;
    this.shake = 0;
    this.heat = 0;
    this.laserWasActive = false;
  }

  draw(canvas: HTMLCanvasElement, state: MagicRenderState): void {
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

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

    this.updateHands(state, width, height, now);
    this.stepParticles();
    this.stepFireballs();
    this.stepWaves();
    this.pruneRibbons(now);

    this.flash = Math.max(0, this.flash - 0.04);
    this.shake = Math.max(0, this.shake - 0.05);
    this.heat = Math.max(0, this.heat - (state.face?.laserActive ? 0.005 : 0.02));

    const ox = this.shake > 0 ? (Math.random() - 0.5) * this.shake * 12 : 0;
    const oy = this.shake > 0 ? (Math.random() - 0.5) * this.shake * 12 : 0;

    ctx.save();
    ctx.translate(ox, oy);

    ctx.clearRect(-20, -20, width + 40, height + 40);
    this.drawDarkAtmosphere(ctx, width, height, state.element);

    if (this.flash > 0) {
      ctx.fillStyle = `rgba(255, 255, 255, ${this.flash * 0.3})`;
      ctx.fillRect(0, 0, width, height);
    }

    // Руки и эффекты окружения
    this.drawRibbons(ctx, now);
    this.drawWaves(ctx);
    this.drawParticles(ctx);
    this.drawFireballs(ctx);
    this.drawHandAuras(ctx, width, height, state);

    // 3D-ЛАЗЕРЫ ИЗ ГЛАЗ
    this.draw3DLaserSystem(ctx, width, height, state, now);

    if (state.showSkeleton) {
      this.drawSkeleton(ctx, width, height, state.hands, state.element);
    }

    ctx.restore();
    this.drawHud(ctx, width, height, state, now);
  }

  // ==========================================
  // НАСТОЯЩИЙ 3D-ЛАЗЕР С ПЕРСПЕКТИВОЙ В КАМЕРУ
  // ==========================================
  private draw3DLaserSystem(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    state: MagicRenderState,
    now: number
  ): void {
    const face = state.face;
    if (!face) {
      this.laserWasActive = false;
      return;
    }

    const hue = ELEMENT_HUE[state.element];

    if (face.laserActive) {
      if (!this.laserWasActive) {
        this.flash = 0.8;
        this.shake = 0.6;
        this.pulseGesture('3D ЛАЗЕР · ОГОНЬ', now);
        this.laserWasActive = true;
      }

      this.heat = Math.min(1, this.heat + 0.03);
      this.shake = Math.max(this.shake, 0.15 * state.intensity);

      // Виртуальная камера: фокусное расстояние
      const focalLength = width * 0.85;
      const faceZ = 650; // миллиметры до лица

      // Зеркалим X координату лица
      const eyeL = mirrorX(face.leftEye);
      const eyeR = mirrorX(face.rightEye);

      // Начальные 3D координаты глаз в системе камеры (центр экрана = 0, 0)
      const eyeL_3D = {
        x: (eyeL.x - 0.5) * width,
        y: (eyeL.y - 0.5) * height,
        z: faceZ + (face.leftEye3D.z || 0) * 100,
      };
      const eyeR_3D = {
        x: (eyeR.x - 0.5) * width,
        y: (eyeR.y - 0.5) * height,
        z: faceZ + (face.rightEye3D.z || 0) * 100,
      };

      // 3D-вектор направления взгляда (инвертируем X из-за зеркала)
      const dirX = -face.dir3D.x;
      const dirY = face.dir3D.y;
      const dirZ = face.dir3D.z; // > 0 означает полет вперед прямо в камеру

      ctx.save();
      ctx.lineCap = 'round';
      ctx.lineJoin = 'round';

      for (const eye3D of [eyeL_3D, eyeR_3D]) {
        // Строим перспективный 3D луч из N срезов
        const steps = 24;
        const beamSteps: Array<{ sx: number; sy: number; r: number; z: number }> = [];

        // Длина луча в пространстве
        const maxDist = 900;
        for (let i = 0; i <= steps; i++) {
          const t = (i / steps) * maxDist;

          // Текущая точка в 3D:
          // Если dirZ > 0, точка летит вперед к камере (Z уменьшается)
          const px = eye3D.x + dirX * t;
          const py = eye3D.y + dirY * t;
          const pz = eye3D.z - dirZ * t;

          // Отсекаем если луч врезался в стекло камеры
          if (pz < 90) break;

          // Перспективная проекция 3D -> 2D Canvas:
          const scale = focalLength / pz;
          const sx = width / 2 + px * scale;
          const sy = height / 2 + py * scale;

          // Толщина луча в 3D увеличивается по мере приближения к зрителю!
          const worldRadius = (4 + state.intensity * 3) * (1 + (t / maxDist) * 1.5);
          const screenRadius = Math.max(2, worldRadius * scale * 0.35);

          beamSteps.push({ sx, sy, r: screenRadius, z: pz });
        }

        if (beamSteps.length < 2) continue;

        const start = beamSteps[0];
        const tip = beamSteps[beamSteps.length - 1];

        // 1. ВНЕШНЕЕ СВЕЧЕНИЕ (Рассеивание в воздухе / Atmospheric Rayleigh)
        ctx.globalCompositeOperation = 'screen';
        ctx.beginPath();
        ctx.moveTo(start.sx, start.sy);
        for (let j = 1; j < beamSteps.length; j++) ctx.lineTo(beamSteps[j].sx, beamSteps[j].sy);
        ctx.strokeStyle = `hsla(${hue}, 100%, 60%, 0.22)`;
        ctx.lineWidth = tip.r * 2.8;
        ctx.shadowBlur = 35;
        ctx.shadowColor = `hsla(${hue}, 100%, 50%, 0.85)`;
        ctx.stroke();

        // 2. ПЛАЗМЕННЫЙ СТЕРЖЕНЬ (Ядро)
        ctx.globalCompositeOperation = 'lighter';
        ctx.beginPath();
        ctx.moveTo(start.sx, start.sy);
        for (let j = 1; j < beamSteps.length; j++) ctx.lineTo(beamSteps[j].sx, beamSteps[j].sy);
        ctx.strokeStyle = `hsla(${hue + 20}, 100%, 75%, 0.75)`;
        ctx.lineWidth = tip.r * 1.1;
        ctx.shadowBlur = 15;
        ctx.shadowColor = `hsla(${hue}, 100%, 60%, 1)`;
        ctx.stroke();

        // 3. БЕЛАЯ НИТЬ НАКАЛА ВНУТРИ
        ctx.beginPath();
        ctx.moveTo(start.sx, start.sy);
        for (let j = 1; j < beamSteps.length; j++) ctx.lineTo(beamSteps[j].sx, beamSteps[j].sy);
        ctx.strokeStyle = '#ffffff';
        ctx.lineWidth = Math.max(2.5, tip.r * 0.3);
        ctx.shadowBlur = 8;
        ctx.shadowColor = '#ffffff';
        ctx.stroke();

        // 4. ЭФФЕКТ «ВЗГЛЯДА ПРЯМО В КАМЕРУ» (Lens Flare & Frontal Glare)
        // Если вектор направлен сильно на зрителя (dirZ > 0.8), луч стреляет в объектив
        if (dirZ > 0.75) {
          const directFactor = (dirZ - 0.75) / 0.25; // от 0 до 1
          const flareR = tip.r * 4.5 * directFactor;

          const lensGlow = ctx.createRadialGradient(tip.sx, tip.sy, 0, tip.sx, tip.sy, flareR);
          lensGlow.addColorStop(0, '#ffffff');
          lensGlow.addColorStop(0.3, `hsla(${hue + 25}, 100%, 70%, 0.9)`);
          lensGlow.addColorStop(1, 'rgba(0,0,0,0)');

          ctx.fillStyle = lensGlow;
          ctx.beginPath();
          ctx.arc(tip.sx, tip.sy, flareR, 0, Math.PI * 2);
          ctx.fill();

          // Горизонтальный анаморфный блик в объективе
          const streakW = 90 * directFactor * state.intensity;
          const streakGrad = ctx.createLinearGradient(tip.sx - streakW, tip.sy, tip.sx + streakW, tip.sy);
          streakGrad.addColorStop(0, 'rgba(255,255,255,0)');
          streakGrad.addColorStop(0.5, 'rgba(255,255,255,0.95)');
          streakGrad.addColorStop(1, 'rgba(255,255,255,0)');
          ctx.fillStyle = streakGrad;
          ctx.fillRect(tip.sx - streakW, tip.sy - 2, streakW * 2, 4);
        }

        // Микро-вспышка у зрачка
        const eyeOriginGlow = ctx.createRadialGradient(start.sx, start.sy, 0, start.sx, start.sy, 16);
        eyeOriginGlow.addColorStop(0, '#ffffff');
        eyeOriginGlow.addColorStop(0.4, `hsla(${hue}, 100%, 70%, 0.8)`);
        eyeOriginGlow.addColorStop(1, 'rgba(0,0,0,0)');
        ctx.fillStyle = eyeOriginGlow;
        ctx.beginPath();
        ctx.arc(start.sx, start.sy, 16, 0, Math.PI * 2);
        ctx.fill();
      }

      ctx.restore();
    } else {
      if (this.laserWasActive) {
        this.laserWasActive = false;
        this.pulseGesture('ПРИЩУР', now);
      }
      // Точки глаз в покое
      const eyeL = mirrorX(face.leftEye);
      const eyeR = mirrorX(face.rightEye);
      ctx.fillStyle = `hsla(${hue}, 90%, 75%, 0.7)`;
      ctx.beginPath();
      ctx.arc(eyeL.x * width, eyeL.y * height, 2.5, 0, Math.PI * 2);
      ctx.arc(eyeR.x * width, eyeR.y * height, 2.5, 0, Math.PI * 2);
      ctx.fill();
    }
  }

  // ==========================================
  // ПОЛНАЯ СИСТЕМА ЭФФЕКТОВ РУК (100% ВОССТАНОВЛЕНА)
  // ==========================================
  private updateHands(state: MagicRenderState, width: number, height: number, now: number): void {
    const palms: Array<{ id: number; x: number; y: number; open: number; fist: boolean }> = [];

    for (const hand of state.hands) {
      const palm = mirrorX(hand.palm);
      const px = palm.x * width;
      const py = palm.y * height;
      const open = handOpenness(hand);
      const fist = isFist(hand, 0.34);

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

      // 1. Сжатие в кулак (Сингулярность)
      if (deltaOpen < -0.18 && pose.open > 0.4 && open < 0.48 && now - pose.lastCrushAt > 350) {
        pose.lastCrushAt = now;
        this.castCrush(state.element, px, py, state.intensity);
        this.pulseGesture('СЖАТИЕ', now);
      }

      // 2. Раскрытие ладони (Супернова)
      if (deltaOpen > 0.18 && pose.open < 0.52 && open > 0.45 && now - pose.lastBloomAt > 350) {
        pose.lastBloomAt = now;
        this.castBloom(state.element, px, py, state.intensity * (1 + pose.charged));
        this.pulseGesture('РАСКРЫТИЕ', now);
        pose.charged = 0;
      }

      if (fist) {
        pose.charged = Math.min(1, pose.charged + 0.03 * state.intensity);
        this.attractParticles(px, py, 140, 0.2 + pose.charged * 0.2);
        this.emitCrushCharge(state.element, px, py, pose.charged);
      } else {
        pose.charged = Math.max(0, pose.charged - 0.02);
        const effIntensity = state.intensity * (0.6 + open * 0.6);

        this.emitFingerStreams(hand, state.element, width, height, effIntensity);
        this.updateRibbons(hand, state.element, width, height, now, open);
        this.emitHandIdle(state.element, px, py, effIntensity);

        // Щипок (Pinch) -> Направленная струя
        const tip = mirrorX(hand.tips.index);
        const thumb = mirrorX(hand.tips.thumb);
        const middle = mirrorX(hand.tips.middle);
        const pinch = Math.hypot(tip.x - thumb.x, tip.y - thumb.y);

        if (pinch < 0.045 && open > 0.25 && open < 0.75) {
          this.emitPinchJet(
            state.element,
            tip.x * width,
            tip.y * height,
            tip.x - palm.x,
            tip.y - palm.y,
            state.intensity
          );
        }

        // Щелчок пальцами (Snap)
        const thumbMiddle = distance(thumb, middle);
        if (thumbMiddle < 0.038 && open > 0.28) pose.snapArmed = true;

        const middleSpeed = Math.hypot(hand.tipVelocity.middle.x, hand.tipVelocity.middle.y);
        if (pose.snapArmed && thumbMiddle > 0.065 && middleSpeed > 1.1 && now - pose.lastSnapAt > 380) {
          pose.snapArmed = false;
          pose.lastSnapAt = now;
          const sx = ((thumb.x + middle.x) / 2) * width;
          const sy = ((thumb.y + middle.y) / 2) * height;
          this.castSnap(state.element, sx, sy, state.intensity);
          this.pulseGesture('ЩЕЛЧОК', now);
        }
        if (thumbMiddle > 0.12) pose.snapArmed = false;
        pose.thumbMiddle = thumbMiddle;

        // Флик указательным пальцем -> Файрбол
        if (state.element === 'fire' && now - pose.lastFlickAt > 420) {
          const indexSpeed = Math.hypot(hand.tipVelocity.index.x, hand.tipVelocity.index.y);
          if (indexSpeed > 1.6 && open < 0.8) {
            pose.lastFlickAt = now;
            this.fireballs.push({
              x: tip.x * width,
              y: tip.y * height,
              vx: hand.tipVelocity.index.x * width * 0.9,
              vy: hand.tipVelocity.index.y * height * 0.9,
              life: 1.0,
              size: 16 * state.intensity,
            });
            this.pulseGesture('ФАЙРБОЛ', now);
          }
        }
      }

      palms.push({ id: hand.id, x: px, y: py, open, fist });
      pose.open = open;
      pose.fist = fist;
      this.pose.set(hand.id, pose);
      this.prevPalm.set(hand.id, palm);
    }

    // Резонанс между двумя руками
    const openPalms = palms.filter((p) => !p.fist && p.open > 0.5);
    if (openPalms.length >= 2) {
      const [a, b] = openPalms;
      const d = Math.hypot(a.x - b.x, a.y - b.y);
      if (d < Math.min(width, height) * 0.22 && now - this.lastComboAt > 600) {
        this.lastComboAt = now;
        this.castBloom(state.element, (a.x + b.x) / 2, (a.y + b.y) / 2, 1.4 * state.intensity);
        this.pulseGesture('РЕЗОНАНС', now);
      }
    }
  }

  private emitFingerStreams(hand: TrackedHand, element: ElementId, w: number, h: number, intensity: number): void {
    for (const finger of FINGER_CAST) {
      const tip = mirrorX(hand.tips[finger]);
      const vel = hand.tipVelocity[finger];
      const spd = Math.hypot(vel.x, vel.y);
      if (spd < 0.25 && Math.random() > 0.35) continue;

      this.particles.push({
        x: tip.x * w,
        y: tip.y * h,
        vx: vel.x * w * 0.3 + (Math.random() - 0.5) * 15,
        vy: vel.y * h * 0.3 + (Math.random() - 0.5) * 15,
        life: 1,
        maxLife: 0.35,
        size: 2.2 * intensity,
        hue: ELEMENT_HUE[element] + Math.random() * 15,
        kind: element === 'water' ? 'drop' : 'spark',
        trail: true,
      });
    }
  }

  private updateRibbons(hand: TrackedHand, element: ElementId, w: number, h: number, now: number, open: number): void {
    if (open < 0.35) return;
    for (const finger of ['index', 'pinky'] as FingerName[]) {
      const tip = mirrorX(hand.tips[finger]);
      const key = `${hand.id}:${finger}`;
      const ribbon = this.ribbons.get(key) ?? { points: [], hue: ELEMENT_HUE[element], width: 3.5 };
      ribbon.points.push({ x: tip.x * w, y: tip.y * h, at: now });
      ribbon.points = ribbon.points.filter((p) => now - p.at < 220).slice(-16);
      this.ribbons.set(key, ribbon);
    }
  }

  private pruneRibbons(now: number): void {
    for (const [k, r] of this.ribbons) {
      r.points = r.points.filter((p) => now - p.at < 220);
      if (r.points.length === 0) this.ribbons.delete(k);
    }
  }

  private emitHandIdle(element: ElementId, x: number, y: number, intensity: number): void {
    if (Math.random() > 0.6) return;
    this.particles.push({
      x: x + (Math.random() - 0.5) * 24,
      y: y + (Math.random() - 0.5) * 24,
      vx: (Math.random() - 0.5) * 20,
      vy: -20 - Math.random() * 30 * intensity,
      life: 1,
      maxLife: 0.5,
      size: 2.5 + Math.random() * 3,
      hue: ELEMENT_HUE[element] + Math.random() * 15,
      kind: element === 'water' ? 'drop' : 'ember',
    });
  }

  private emitPinchJet(element: ElementId, x: number, y: number, dx: number, dy: number, intensity: number): void {
    const len = Math.hypot(dx, dy) || 1;
    for (let i = 0; i < 2; i++) {
      this.particles.push({
        x,
        y,
        vx: (dx / len) * (120 + Math.random() * 80) + (Math.random() - 0.5) * 30,
        vy: (dy / len) * (120 + Math.random() * 80) + (Math.random() - 0.5) * 30,
        life: 1,
        maxLife: 0.3,
        size: 3 + Math.random() * 4 * intensity,
        hue: ELEMENT_HUE[element] + Math.random() * 20,
        kind: 'spark',
        trail: true,
      });
    }
  }

  private emitCrushCharge(element: ElementId, x: number, y: number, charge: number): void {
    const angle = Math.random() * Math.PI * 2;
    const dist = 30 + (1 - charge) * 45;
    this.particles.push({
      x: x + Math.cos(angle) * dist,
      y: y + Math.sin(angle) * dist,
      vx: -Math.cos(angle) * 70,
      vy: -Math.sin(angle) * 70,
      life: 1,
      maxLife: 0.28,
      size: 2 + charge * 3,
      hue: ELEMENT_HUE[element] + 10,
      kind: 'spark',
    });
  }

  private castCrush(element: ElementId, x: number, y: number, power: number): void {
    this.shake = 0.4 * power;
    this.waves.push({ x, y, radius: 90, maxRadius: 4, life: 1, hue: ELEMENT_HUE[element], width: 4 });
  }

  private castBloom(element: ElementId, x: number, y: number, power: number): void {
    this.shake = 0.5 * power;
    this.flash = 0.5;
    this.waves.push({ x, y, radius: 8, maxRadius: 130 * power, life: 1, hue: ELEMENT_HUE[element], width: 4 });
  }

  private castSnap(element: ElementId, x: number, y: number, power: number): void {
    this.shake = 0.45 * power;
    this.waves.push({ x, y, radius: 4, maxRadius: 70 * power, life: 1, hue: ELEMENT_HUE[element], width: 3.5 });
  }

  private attractParticles(x: number, y: number, radius: number, strength: number): void {
    for (const p of this.particles) {
      const dx = x - p.x;
      const dy = y - p.y;
      const d = Math.hypot(dx, dy) || 1;
      if (d < radius) {
        p.vx += (dx / d) * strength * 90;
        p.vy += (dy / d) * strength * 90;
      }
    }
  }

  private stepParticles(): void {
    const next: Particle[] = [];
    for (const p of this.particles) {
      p.x += p.vx * 0.016;
      p.y += p.vy * 0.016;
      p.vx *= 0.96;
      p.vy *= 0.96;
      p.life -= 0.016 / p.maxLife;
      if (p.life > 0) next.push(p);
    }
    this.particles = next.slice(-600);
  }

  private stepFireballs(): void {
    const next: Fireball[] = [];
    for (const b of this.fireballs) {
      b.x += b.vx * 0.016;
      b.y += b.vy * 0.016;
      b.life -= 0.02;
      if (b.life > 0) next.push(b);
      else this.castSnap('fire', b.x, b.y, 1.2);
    }
    this.fireballs = next;
  }

  private stepWaves(): void {
    const next: Shockwave[] = [];
    for (const w of this.waves) {
      w.radius += (w.maxRadius - w.radius) * 0.16 + 2;
      w.life -= 0.035;
      if (w.life > 0) next.push(w);
    }
    this.waves = next;
  }

  private drawDarkAtmosphere(ctx: CanvasRenderingContext2D, w: number, h: number, el: ElementId): void {
    ctx.fillStyle = 'rgba(6, 4, 10, 0.45)';
    ctx.fillRect(0, 0, w, h);
  }

  private drawHandAuras(ctx: CanvasRenderingContext2D, w: number, h: number, state: MagicRenderState): void {
    const hue = ELEMENT_HUE[state.element];
    for (const hand of state.hands) {
      const palm = mirrorX(hand.palm);
      const px = palm.x * w;
      const py = palm.y * h;
      const open = handOpenness(hand);
      const pose = this.pose.get(hand.id);

      ctx.save();
      ctx.globalCompositeOperation = 'screen';
      if (isFist(hand, 0.34)) {
        const ch = pose?.charged ?? 0;
        const r = 14 + ch * 28;
        const g = ctx.createRadialGradient(px, py, 1, px, py, r);
        g.addColorStop(0, '#ffffff');
        g.addColorStop(0.4, `hsla(${hue}, 95%, 65%, 0.8)`);
        g.addColorStop(1, 'rgba(0,0,0,0)');
        ctx.fillStyle = g;
        ctx.beginPath();
        ctx.arc(px, py, r, 0, Math.PI * 2);
        ctx.fill();
      } else {
        const r = (16 + open * 26) * state.intensity;
        const g = ctx.createRadialGradient(px, py, 1, px, py, r);
        g.addColorStop(0, `hsla(${hue}, 90%, 75%, 0.5)`);
        g.addColorStop(1, 'rgba(0,0,0,0)');
        ctx.fillStyle = g;
        ctx.beginPath();
        ctx.arc(px, py, r, 0, Math.PI * 2);
        ctx.fill();
      }
      ctx.restore();
    }
  }

  private drawRibbons(ctx: CanvasRenderingContext2D, now: number): void {
    ctx.save();
    ctx.lineCap = 'round';
    for (const r of this.ribbons.values()) {
      if (r.points.length < 2) continue;
      ctx.beginPath();
      ctx.moveTo(r.points[0].x, r.points[0].y);
      for (let i = 1; i < r.points.length; i++) ctx.lineTo(r.points[i].x, r.points[i].y);
      const a = Math.max(0.1, 1 - (now - r.points[0].at) / 220);
      ctx.strokeStyle = `hsla(${r.hue}, 95%, 65%, ${a * 0.7})`;
      ctx.lineWidth = r.width;
      ctx.stroke();
    }
    ctx.restore();
  }

  private drawParticles(ctx: CanvasRenderingContext2D): void {
    ctx.save();
    ctx.globalCompositeOperation = 'lighter';
    for (const p of this.particles) {
      const a = Math.max(0, p.life);
      ctx.fillStyle = `hsla(${p.hue}, 95%, 70%, ${a})`;
      ctx.beginPath();
      ctx.arc(p.x, p.y, p.size * a, 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.restore();
  }

  private drawFireballs(ctx: CanvasRenderingContext2D): void {
    ctx.save();
    ctx.globalCompositeOperation = 'lighter';
    for (const b of this.fireballs) {
      const g = ctx.createRadialGradient(b.x, b.y, 1, b.x, b.y, b.size);
      g.addColorStop(0, '#ffffff');
      g.addColorStop(0.3, 'rgba(255, 170, 40, 0.9)');
      g.addColorStop(1, 'rgba(255, 60, 0, 0)');
      ctx.fillStyle = g;
      ctx.beginPath();
      ctx.arc(b.x, b.y, b.size, 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.restore();
  }

  private drawWaves(ctx: CanvasRenderingContext2D): void {
    ctx.save();
    for (const w of this.waves) {
      ctx.strokeStyle = `hsla(${w.hue}, 95%, 70%, ${w.life * 0.8})`;
      ctx.lineWidth = w.width * w.life;
      ctx.beginPath();
      ctx.arc(w.x, w.y, Math.max(1, w.radius), 0, Math.PI * 2);
      ctx.stroke();
    }
    ctx.restore();
  }

  private drawSkeleton(ctx: CanvasRenderingContext2D, w: number, h: number, hands: TrackedHand[], el: ElementId): void {
    ctx.strokeStyle = `hsla(${ELEMENT_HUE[el]}, 80%, 70%, 0.35)`;
    ctx.lineWidth = 1.2;
    for (const hand of hands) {
      for (const [a, b] of HAND_CONNECTIONS) {
        const pa = mirrorX(hand.landmarks[a]);
        const pb = mirrorX(hand.landmarks[b]);
        ctx.beginPath();
        ctx.moveTo(pa.x * w, pa.y * h);
        ctx.lineTo(pb.x * w, pb.y * h);
        ctx.stroke();
      }
    }
  }

  private drawHud(ctx: CanvasRenderingContext2D, w: number, h: number, state: MagicRenderState, now: number): void {
    ctx.fillStyle = 'rgba(240, 245, 255, 0.75)';
    ctx.font = '600 13px sans-serif';
    ctx.textAlign = 'left';
    ctx.fillText(state.hint, 18, h - 18);

    if (this.gestureLabel && now < this.gestureLabelUntil) {
      const t = (this.gestureLabelUntil - now) / 800;
      ctx.textAlign = 'center';
      ctx.font = '700 22px sans-serif';
      ctx.fillStyle = `rgba(255, 255, 255, ${t})`;
      ctx.fillText(this.gestureLabel, w / 2, 60);
    }
  }

  private pulseGesture(label: string, now: number): void {
    this.gestureLabel = label;
    this.gestureLabelUntil = now + 800;
  }
}