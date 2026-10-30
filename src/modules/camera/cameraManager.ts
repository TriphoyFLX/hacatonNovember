import type { CameraDeviceInfo, CameraStartOptions, CameraSnapshot } from './types';

type Listener = (snapshot: CameraSnapshot) => void;

function stopTrack(track: MediaStreamTrack): void {
  track.stop();
}

export class CameraManager {
  private stream: MediaStream | null = null;
  private devices: CameraDeviceInfo[] = [];
  private activeDeviceId: string | null = null;
  private status: CameraSnapshot['status'] = 'idle';
  private errorMessage: string | null = null;
  private fps = 0;
  private listeners = new Set<Listener>();
  private fpsFrames = 0;
  private fpsLastTs = 0;
  private fpsRaf: number | null = null;

  subscribe(listener: Listener): () => void {
    this.listeners.add(listener);
    listener(this.getSnapshot());
    return () => {
      this.listeners.delete(listener);
    };
  }

  getSnapshot(): CameraSnapshot {
    return {
      stream: this.stream,
      devices: this.devices,
      activeDeviceId: this.activeDeviceId,
      status: this.status,
      errorMessage: this.errorMessage,
      fps: this.fps,
    };
  }

  async refreshDevices(): Promise<CameraDeviceInfo[]> {
    if (!navigator.mediaDevices?.enumerateDevices) {
      this.devices = [];
      this.emit();
      return [];
    }

    const all = await navigator.mediaDevices.enumerateDevices();
    this.devices = all
      .filter((device) => device.kind === 'videoinput')
      .map((device, index) => ({
        deviceId: device.deviceId,
        label: device.label || `Камера ${index + 1}`,
      }));
    this.emit();
    return this.devices;
  }

  async start(options: CameraStartOptions = {}): Promise<MediaStream> {
    if (!navigator.mediaDevices?.getUserMedia) {
      this.status = 'unavailable';
      this.errorMessage = 'Камера недоступна в этой среде';
      this.emit();
      throw new Error(this.errorMessage);
    }

    this.status = 'requesting';
    this.errorMessage = null;
    this.emit();

    this.stopInternal(false);

    const preferred: MediaStreamConstraints = {
      audio: false,
      video: {
        deviceId: options.deviceId ? { exact: options.deviceId } : undefined,
        width: { ideal: options.width ?? 1280 },
        height: { ideal: options.height ?? 720 },
        facingMode: options.facingMode ?? 'user',
        frameRate: { ideal: 30 },
      },
    };

    const fallback: MediaStreamConstraints = {
      audio: false,
      video: options.deviceId ? { deviceId: { exact: options.deviceId } } : true,
    };

    try {
      let stream: MediaStream;
      try {
        stream = await navigator.mediaDevices.getUserMedia(preferred);
      } catch {
        stream = await navigator.mediaDevices.getUserMedia(fallback);
      }

      this.stream = stream;
      const track = stream.getVideoTracks()[0];
      this.activeDeviceId = track?.getSettings().deviceId ?? options.deviceId ?? null;
      this.status = 'active';
      this.errorMessage = null;
      await this.refreshDevices();
      this.startFpsLoop();
      this.emit();
      return stream;
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Не удалось открыть камеру';
      const denied =
        error instanceof DOMException &&
        (error.name === 'NotAllowedError' || error.name === 'PermissionDeniedError');

      this.status = denied ? 'denied' : 'error';
      this.errorMessage = denied
        ? 'Доступ к камере запрещён. Разрешите камеру в Системных настройках → Конфиденциальность → Камера для FEEL AI / Electron.'
        : message;
      this.emit();
      throw error;
    }
  }

  async switchDevice(deviceId: string): Promise<MediaStream> {
    return this.start({ deviceId });
  }

  stop(): void {
    this.stopInternal(true);
  }

  attachToVideo(video: HTMLVideoElement): void {
    if (!this.stream) {
      video.srcObject = null;
      return;
    }

    video.srcObject = this.stream;
    video.muted = true;
    video.playsInline = true;
    void video.play().catch(() => {
      // Autoplay can fail briefly while the stream is settling; next frame usually recovers.
    });
  }

  private stopInternal(emit: boolean): void {
    if (this.stream) {
      this.stream.getTracks().forEach(stopTrack);
      this.stream = null;
    }
    this.activeDeviceId = null;
    this.status = 'idle';
    this.errorMessage = null;
    this.fps = 0;
    this.stopFpsLoop();
    if (emit) {
      this.emit();
    }
  }

  private startFpsLoop(): void {
    this.stopFpsLoop();
    this.fpsFrames = 0;
    this.fpsLastTs = performance.now();

    const tick = (now: number) => {
      this.fpsFrames += 1;
      const elapsed = now - this.fpsLastTs;
      if (elapsed >= 1000) {
        this.fps = Math.round((this.fpsFrames * 1000) / elapsed);
        this.fpsFrames = 0;
        this.fpsLastTs = now;
        this.emit();
      }
      this.fpsRaf = window.requestAnimationFrame(tick);
    };

    this.fpsRaf = window.requestAnimationFrame(tick);
  }

  private stopFpsLoop(): void {
    if (this.fpsRaf !== null) {
      window.cancelAnimationFrame(this.fpsRaf);
      this.fpsRaf = null;
    }
  }

  private emit(): void {
    const snapshot = this.getSnapshot();
    for (const listener of this.listeners) {
      listener(snapshot);
    }
  }
}

export const cameraManager = new CameraManager();
