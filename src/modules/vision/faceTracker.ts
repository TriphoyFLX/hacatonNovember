import { FaceLandmarker, FilesetResolver, type FaceLandmarkerResult } from '@mediapipe/tasks-vision';
import { EmaPoint } from './smoothing';
import type { Point2D } from './types';

export interface Point3D {
  x: number;
  y: number;
  z: number;
}

export interface TrackedFace {
  leftEye: Point2D;
  rightEye: Point2D;
  nose: Point2D;
  faceCenter: Point2D;
  /** Координаты зрачков в 3D пространстве камеры */
  leftEye3D: Point3D;
  rightEye3D: Point3D;
  /** Единый 3D-вектор взгляда (x, y, z): при взгляде в камеру z близко к 1.0 */
  dir3D: Point3D;
  blinkLeft: number;
  blinkRight: number;
  eyeWide: number;
  eyesClosed: boolean;
  eyesWide: boolean;
  eyesSquinted: boolean;
  laserActive: boolean;
  score: number;
}

export interface FaceTrackerSnapshot {
  face: TrackedFace | null;
  fps: number;
  status: 'idle' | 'loading' | 'ready' | 'running' | 'error';
  errorMessage: string | null;
  hasFace: boolean;
}

type Listener = (snapshot: FaceTrackerSnapshot) => void;

const LEFT_IRIS = 468;
const RIGHT_IRIS = 473;
const LEFT_IRIS_RING = [469, 470, 471, 472] as const;
const RIGHT_IRIS_RING = [474, 475, 476, 477] as const;
const NOSE_TIP = 1;
const LEFT_OUTER = 33;
const LEFT_INNER = 133;
const LEFT_UPPER = 159;
const LEFT_LOWER = 145;
const RIGHT_OUTER = 263;
const RIGHT_INNER = 362;
const RIGHT_UPPER = 386;
const RIGHT_LOWER = 374;

const WIDE_ENTER = 0.88;
const WIDE_HYSTERESIS = 0.78;
const SQUINT_ENTER = 0.50;

function clamp(v: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, v));
}

function eyeAspectRatio(upper: Point3D, lower: Point3D, outer: Point3D, inner: Point3D): number {
  const v = Math.hypot(upper.x - lower.x, upper.y - lower.y);
  const h = Math.hypot(outer.x - inner.x, outer.y - inner.y) || 0.01;
  return v / h;
}

function blendValue(cats: Array<{ categoryName?: string; displayName?: string; score: number }> | undefined, name: string): number {
  if (!cats) return 0;
  return cats.find((i) => i.categoryName === name || i.displayName === name)?.score ?? 0;
}

export class FaceTracker {
  private landmarker: FaceLandmarker | null = null;
  private initPromise: Promise<void> | null = null;
  private raf = 0;
  private video: HTMLVideoElement | null = null;
  private wantRunning = false;
  private running = false;
  private lastVideoTime = -1;
  private listeners = new Set<Listener>();
  private status: FaceTrackerSnapshot['status'] = 'idle';
  private errorMessage: string | null = null;
  private face: TrackedFace | null = null;
  private fps = 0;
  private fpsFrames = 0;
  private fpsLastTs = performance.now();

  private smoothers = new Map<string, EmaPoint>();
  private gazeXSmooth = 0;
  private gazeYSmooth = 0;
  private wideSmooth = 0;
  private laserActive = false;

  subscribe(listener: Listener): () => void {
    this.listeners.add(listener);
    listener(this.getSnapshot());
    return () => this.listeners.delete(listener);
  }

  getSnapshot(): FaceTrackerSnapshot {
    return {
      face: this.face,
      fps: this.fps,
      status: this.status,
      errorMessage: this.errorMessage,
      hasFace: Boolean(this.face),
    };
  }

  async init(): Promise<void> {
    if (this.landmarker) return;
    if (this.initPromise) return this.initPromise;

    this.status = 'loading';
    this.emit();

    this.initPromise = this.createLandmarker()
      .then(() => {
        this.status = this.wantRunning ? 'running' : 'ready';
        this.emit();
        if (this.wantRunning && this.video) this.beginLoop();
      })
      .catch((err) => {
        this.status = 'error';
        this.errorMessage = err instanceof Error ? err.message : 'Ошибка загрузки Mediapipe';
        this.emit();
        throw err;
      })
      .finally(() => {
        this.initPromise = null;
      });

    return this.initPromise;
  }

  async start(video: HTMLVideoElement): Promise<void> {
    this.video = video;
    this.wantRunning = true;
    try {
      await this.init();
    } catch {
      return;
    }
    if (this.landmarker) this.beginLoop();
  }

  stop(): void {
    this.wantRunning = false;
    this.running = false;
    if (this.raf) cancelAnimationFrame(this.raf);
    this.raf = 0;
    this.face = null;
    this.laserActive = false;
    this.status = this.landmarker ? 'ready' : 'idle';
    this.emit();
  }

  dispose(): void {
    this.stop();
    this.landmarker?.close();
    this.landmarker = null;
    this.status = 'idle';
    this.emit();
  }

  private async createLandmarker(): Promise<void> {
    const vision = await FilesetResolver.forVisionTasks('/mediapipe');
    this.landmarker = await FaceLandmarker.createFromOptions(vision, {
      runningMode: 'VIDEO',
      numFaces: 1,
      outputFaceBlendshapes: true,
      minFaceDetectionConfidence: 0.5,
      minTrackingConfidence: 0.5,
      baseOptions: { modelAssetPath: '/models/face_landmarker.task', delegate: 'GPU' },
    });
  }

  private beginLoop(): void {
    if (this.running || !this.landmarker || !this.video || !this.wantRunning) return;
    this.running = true;
    this.status = 'running';
    this.emit();
    this.loop();
  }

  private loop = (): void => {
    if (!this.wantRunning) {
      this.running = false;
      return;
    }
    const now = performance.now();
    const video = this.video;

    if (video && this.landmarker && video.readyState >= 2) {
      if (video.currentTime !== this.lastVideoTime) {
        this.lastVideoTime = video.currentTime;
        this.fpsFrames++;
        try {
          const res = this.landmarker.detectForVideo(video, now);
          this.mapResult(res);
        } catch {
          // ignore transient frame drop
        }
      }
    }

    if (now - this.fpsLastTs >= 1000) {
      this.fps = Math.round((this.fpsFrames * 1000) / (now - this.fpsLastTs));
      this.fpsFrames = 0;
      this.fpsLastTs = now;
    }

    this.emit();
    this.raf = requestAnimationFrame(this.loop);
  };

  private mapResult(result: FaceLandmarkerResult): void {
    const landmarks = result.faceLandmarks[0];
    if (!landmarks || landmarks.length === 0) {
      this.face = null;
      this.laserActive = false;
      return;
    }

    const p3d = (idx: number): Point3D => ({
      x: landmarks[idx].x,
      y: landmarks[idx].y,
      z: landmarks[idx].z || 0,
    });

    const leftIris = p3d(LEFT_IRIS);
    const rightIris = p3d(RIGHT_IRIS);

    const smoothP2 = (key: string, p: Point3D): Point2D => {
      let s = this.smoothers.get(key);
      if (!s) {
        s = new EmaPoint(0.45);
        this.smoothers.set(key, s);
      }
      return s.next({ x: p.x, y: p.y });
    };

    const leftEye = smoothP2('lEye', leftIris);
    const rightEye = smoothP2('rEye', rightIris);
    const nose = smoothP2('nose', p3d(NOSE_TIP));
    const faceCenter = { x: (leftEye.x + rightEye.x) / 2, y: (leftEye.y + rightEye.y) / 2 };

    const cats = result.faceBlendshapes[0]?.categories;
    const blinkLeft = blendValue(cats, 'eyeBlinkLeft');
    const blinkRight = blendValue(cats, 'eyeBlinkRight');
    const blink = (blinkLeft + blinkRight) / 2;
    const eyesClosed = blink > 0.55;

    const earL = eyeAspectRatio(p3d(LEFT_UPPER), p3d(LEFT_LOWER), p3d(LEFT_OUTER), p3d(LEFT_INNER));
    const earR = eyeAspectRatio(p3d(RIGHT_UPPER), p3d(RIGHT_LOWER), p3d(RIGHT_OUTER), p3d(RIGHT_INNER));
    const ear = (earL + earR) / 2;

    const openFromBlink = Math.pow(clamp(1 - blink, 0, 1), 1.6);
    const openFromEar = clamp((ear - 0.28) / 0.18, 0, 1);
    const eyeWideRaw = clamp(openFromBlink * 0.25 + openFromEar * 0.75, 0, 1);
    this.wideSmooth = this.wideSmooth * 0.8 + eyeWideRaw * 0.2;

    const eyesWide = this.wideSmooth >= (this.laserActive ? WIDE_HYSTERESIS : WIDE_ENTER) && blink < 0.15;
    const eyesSquinted = this.wideSmooth <= SQUINT_ENTER || blink > 0.3;

    if (!this.laserActive && eyesWide && !eyesClosed) {
      this.laserActive = true;
    } else if (this.laserActive && eyesSquinted) {
      this.laserActive = false;
    }

    // Рассчитываем смещение взгляда (Yaw, Pitch)
    const blendYawL = blendValue(cats, 'eyeLookInLeft') - blendValue(cats, 'eyeLookOutLeft');
    const blendYawR = blendValue(cats, 'eyeLookOutRight') - blendValue(cats, 'eyeLookInRight');
    const rawYaw = (blendYawL + blendYawR) * 0.5;

    const blendPitchL = blendValue(cats, 'eyeLookUpLeft') - blendValue(cats, 'eyeLookDownLeft');
    const blendPitchR = blendValue(cats, 'eyeLookUpRight') - blendValue(cats, 'eyeLookDownRight');
    const rawPitch = (blendPitchL + blendPitchR) * 0.5;

    // Плавный поворот взгляда
    this.gazeXSmooth = this.gazeXSmooth * 0.6 + rawYaw * 0.4;
    this.gazeYSmooth = this.gazeYSmooth * 0.6 + rawPitch * 0.4;

    // Сферические координаты в 3D:
    // yaw = поворот влево-вправо (-X / +X)
    // pitch = поворот вверх-вниз (-Y / +Y)
    // Когда смотрите прямо в объектив: yaw=0, pitch=0 -> вектор (0, 0, 1) прямо в экран!
    const yawAngle = this.gazeXSmooth * 1.3;
    const pitchAngle = this.gazeYSmooth * 1.1;

    const dx = Math.sin(yawAngle) * Math.cos(pitchAngle);
    const dy = -Math.sin(pitchAngle); // в координатах экрана вверх - это минус Y
    const dz = Math.cos(yawAngle) * Math.cos(pitchAngle); // вперед в камеру!

    const len = Math.hypot(dx, dy, dz) || 1;
    const dir3D: Point3D = { x: dx / len, y: dy / len, z: dz / len };

    this.face = {
      leftEye,
      rightEye,
      nose,
      faceCenter,
      leftEye3D: leftIris,
      rightEye3D: rightIris,
      dir3D,
      blinkLeft,
      blinkRight,
      eyeWide: this.wideSmooth,
      eyesClosed,
      eyesWide,
      eyesSquinted,
      laserActive: this.laserActive,
      score: 1,
    };
  }

  private emit(): void {
    const s = this.getSnapshot();
    for (const l of this.listeners) l(s);
  }
}

export const faceTracker = new FaceTracker();