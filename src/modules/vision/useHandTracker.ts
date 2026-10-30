import { useEffect, useState } from 'react';
import { handTracker } from './handTracker';
import type { HandTrackerSnapshot } from './types';

const initial: HandTrackerSnapshot = {
  hands: [],
  fps: 0,
  status: 'idle',
  errorMessage: null,
  hasHands: false,
};

export function useHandTracker(video: HTMLVideoElement | null, enabled: boolean) {
  const [snapshot, setSnapshot] = useState<HandTrackerSnapshot>(initial);

  useEffect(() => {
    return handTracker.subscribe(setSnapshot);
  }, []);

  useEffect(() => {
    if (!enabled) {
      handTracker.stop();
      return;
    }

    let cancelled = false;

    void (async () => {
      try {
        await handTracker.init();
        if (!cancelled && video) {
          handTracker.start(video);
        }
      } catch {
        // Snapshot carries the error.
      }
    })();

    return () => {
      cancelled = true;
      handTracker.stop();
    };
  }, [enabled, video]);

  useEffect(() => {
    if (enabled && video && snapshot.status === 'ready') {
      handTracker.start(video);
    }
  }, [enabled, video, snapshot.status]);

  return snapshot;
}
