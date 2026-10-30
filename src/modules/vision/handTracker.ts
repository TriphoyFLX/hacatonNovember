import { FilesetResolver, HandLandmarker, type HandLandmarkerResult } from '@mediapipe/tasks-vision';
import { EmaPoint, velocity } from './smoothing';
import type { HandTrackerSnapshot, Point2D, TrackedHand } from './types';

type Listener = (snapshot: HandTrackerSnapshot) => void;

interface TipHistory {
  thumb: Point2D | null;
  index: Point2D | null;
  middle: Point2D | null;
  ring: Point2D | null;
  pinky: Point2D | null;
  wrist: Point2D | null;
  ts: number;
}

function average(points: Point2D[]): Point2D {
  const sum = points.reduce(
    (acc, point) => ({ x: acc.x + point.x, y: acc.y + point.y }),
    { x: 0, y: 0 },
  );
  return { x: sum.x / points.length, y: sum.y / points.length };
}

export class HandTracker {
  private landmarker: HandLandmarker | null = null;
  private initPromise: Promise<void> | null = null;
  private raf = 0;
  private video: HTMLVideoElement | null = null;
  private wantRunning = false;
  private running = false;
  private lastVideoTime = -1;
  private listeners = new Set<Listener>();
  private status: HandTrackerSnapshot['status'] = 'idle';
  private errorMessage: string | null = null;
  private hands: TrackedHand[] = [];
  private fps = 0;
  private fpsFrames = 0;
  private fpsLastTs = performance.now();
  private smoothers = new Map<string, EmaPoint>();
  private tipHistory = new Map<number, TipHistory>();
  private frameSkip = 0;

  subscribe(listener: Listener): () => void {
    this.listeners.add(listener);
    listener(this.getSnapshot());
    return () => this.listeners.delete(listener);
  }

  getSnapshot(): HandTrackerSnapshot {
    return {
      hands: this.hands,
      fps: this.fps,
      status: this.status,
      errorMessage: this.errorMessage,
      hasHands: this.hands.length > 0,
    };
  }

  async init(): Promise<void> {
    if (this.landmarker) {
      if (this.status !== 'running') {
        this.status = 'ready';
        this.emit();
      }
      return;
    }

    if (this.initPromise) {
      return this.initPromise;
    }

    this.status = 'loading';
    this.errorMessage = null;
    this.emit();

    this.initPromise = this.createLandmarker()
      .then(() => {
        this.status = this.wantRunning ? 'running' : 'ready';
        this.errorMessage = null;
        this.emit();
        if (this.wantRunning && this.video) {
          this.beginLoop();
        }
      })
      .catch((error: unknown) => {
        this.status = 'error';
        this.errorMessage =
          error instanceof Error ? error.message : 'Не удалось загрузить Hand Landmarker';
        this.emit();
        throw error;
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

    if (!this.landmarker) {
      return;
    }

    this.beginLoop();
  }

  stop(): void {
    this.wantRunning = false;
    this.running = false;
    if (this.raf) {
      cancelAnimationFrame(this.raf);
      this.raf = 0;
    }
    this.hands = [];
    this.tipHistory.clear();
    this.smoothers.clear();
    this.lastVideoTime = -1;
    if (this.status === 'running') {
      this.status = this.landmarker ? 'ready' : 'idle';
    }
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
    const base = {
      runningMode: 'VIDEO' as const,
      numHands: 2,
      minHandDetectionConfidence: 0.5,
      minHandPresenceConfidence: 0.5,
      minTrackingConfidence: 0.5,
    };

    try {
      this.landmarker = await HandLandmarker.createFromOptions(vision, {
        ...base,
        baseOptions: {
          modelAssetPath: '/models/hand_landmarker.task',
          delegate: 'GPU',
        },
      });
    } catch {
      this.landmarker = await HandLandmarker.createFromOptions(vision, {
        ...base,
        baseOptions: {
          modelAssetPath: '/models/hand_landmarker.task',
          delegate: 'CPU',
        },
      });
    }
  }

  private beginLoop(): void {
    if (this.running || !this.landmarker || !this.video || !this.wantRunning) {
      return;
    }
    this.running = true;
    this.status = 'running';
    this.emit();
    this.loop();
  }

  private loop = (): void => {
    if (!this.running || !this.wantRunning) {
      return;
    }

    const video = this.video;
    const landmarker = this.landmarker;

    if (video && landmarker && video.readyState >= 2) {
      const now = performance.now();
      const shouldDetect =
        video.currentTime !== this.lastVideoTime &&
        (this.fps === 0 || this.fps >= 18 || this.frameSkip % 2 === 0);
      this.frameSkip += 1;

      if (shouldDetect) {
        this.lastVideoTime = video.currentTime;
        try {
          const result = landmarker.detectForVideo(video, now);
          this.hands = this.mapResult(result, now);
          this.fpsFrames += 1;
        } catch {
          // Keep previous frame on transient inference errors.
        }
      }

      if (now - this.fpsLastTs >= 1000) {
        this.fps = Math.round((this.fpsFrames * 1000) / (now - this.fpsLastTs));
        this.fpsFrames = 0;
        this.fpsLastTs = now;
      }

      this.emit();
    }

    this.raf = requestAnimationFrame(this.loop);
  };

  private mapResult(result: HandLandmarkerResult, now: number): TrackedHand[] {
    const hands: TrackedHand[] = [];

    result.landmarks.forEach((landmarks, index) => {
      const handedness = result.handednesses[index]?.[0];
      const label = (handedness?.categoryName as TrackedHand['handedness']) ?? 'Unknown';
      const score = handedness?.score ?? 0;
      const points = landmarks.map((landmark, landmarkIndex) =>
        this.smooth(`${index}:${landmarkIndex}`, { x: landmark.x, y: landmark.y }),
      );

      const tips = {
        thumb: points[4],
        index: points[8],
        middle: points[12],
        ring: points[16],
        pinky: points[20],
      };
      const wrist = points[0];
      const palm = average([points[0], points[5], points[9], points[13], points[17]]);

      const prev = this.tipHistory.get(index);
      const dt = prev ? now - prev.ts : 16;
      const tipVelocity = {
        thumb: velocity(prev?.thumb ?? null, tips.thumb, dt),
        index: velocity(prev?.index ?? null, tips.index, dt),
        middle: velocity(prev?.middle ?? null, tips.middle, dt),
        ring: velocity(prev?.ring ?? null, tips.ring, dt),
        pinky: velocity(prev?.pinky ?? null, tips.pinky, dt),
        wrist: velocity(prev?.wrist ?? null, wrist, dt),
      };

      this.tipHistory.set(index, {
        thumb: tips.thumb,
        index: tips.index,
        middle: tips.middle,
        ring: tips.ring,
        pinky: tips.pinky,
        wrist,
        ts: now,
      });

      hands.push({
        id: index,
        handedness: label,
        score,
        landmarks: points,
        tips,
        wrist,
        palm,
        tipVelocity,
      });
    });

    for (const key of [...this.tipHistory.keys()]) {
      if (!hands.some((hand) => hand.id === key)) {
        this.tipHistory.delete(key);
      }
    }

    return hands;
  }

  private smooth(key: string, point: Point2D): Point2D {
    let smoother = this.smoothers.get(key);
    if (!smoother) {
      smoother = new EmaPoint(0.45);
      this.smoothers.set(key, smoother);
    }
    return smoother.next(point);
  }

  private emit(): void {
    const snapshot = this.getSnapshot();
    for (const listener of this.listeners) {
      listener(snapshot);
    }
  }
}

export const handTracker = new HandTracker();
