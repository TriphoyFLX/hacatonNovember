import { useCallback, useEffect, useRef, useState } from 'react';
import { cameraManager } from './cameraManager';
import type { CameraSnapshot, CameraStartOptions } from './types';
import { useAppStore } from '@/stores/appStore';

const initialSnapshot: CameraSnapshot = {
  stream: null,
  devices: [],
  activeDeviceId: null,
  status: 'idle',
  errorMessage: null,
  fps: 0,
};

interface UseCameraOptions {
  autoStart?: boolean;
  startOptions?: CameraStartOptions;
}

export function useCamera(options: UseCameraOptions = {}) {
  const { autoStart = false, startOptions } = options;
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const startOptionsRef = useRef(startOptions);
  const [snapshot, setSnapshot] = useState<CameraSnapshot>(initialSnapshot);
  const setCameraStatus = useAppStore((state) => state.setCameraStatus);

  startOptionsRef.current = startOptions;

  useEffect(() => {
    return cameraManager.subscribe((next) => {
      setSnapshot(next);
      if (
        next.status === 'idle' ||
        next.status === 'requesting' ||
        next.status === 'active' ||
        next.status === 'denied' ||
        next.status === 'unavailable'
      ) {
        setCameraStatus(next.status);
      } else if (next.status === 'error') {
        setCameraStatus('unavailable');
      }
    });
  }, [setCameraStatus]);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) {
      return;
    }
    cameraManager.attachToVideo(video);
  }, [snapshot.stream]);

  useEffect(() => {
    if (!autoStart) {
      return;
    }

    let cancelled = false;

    void (async () => {
      try {
        await cameraManager.start(startOptionsRef.current);
      } catch {
        if (!cancelled) {
          // Error is reflected in snapshot via manager.
        }
      }
    })();

    return () => {
      cancelled = true;
      cameraManager.stop();
    };
  }, [autoStart]);

  const start = useCallback((opts?: CameraStartOptions) => {
    return cameraManager.start(opts ?? startOptionsRef.current);
  }, []);

  const stop = useCallback(() => {
    cameraManager.stop();
  }, []);

  const switchDevice = useCallback((deviceId: string) => {
    return cameraManager.switchDevice(deviceId);
  }, []);

  const refreshDevices = useCallback(() => {
    return cameraManager.refreshDevices();
  }, []);

  return {
    videoRef,
    ...snapshot,
    start,
    stop,
    switchDevice,
    refreshDevices,
  };
}
