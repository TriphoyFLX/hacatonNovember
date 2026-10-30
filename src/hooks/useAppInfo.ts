import { useEffect, useState } from 'react';
import type { AppInfo } from '@shared/ipc/types';

const fallbackInfo: AppInfo = {
  name: 'FEEL AI',
  version: '0.1.0',
  platform: 'darwin',
  arch: 'arm64',
  electronVersion: 'browser',
  chromeVersion: 'browser',
  nodeVersion: 'browser',
  isPackaged: false,
};

export function useAppInfo() {
  const [info, setInfo] = useState<AppInfo>(fallbackInfo);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      if (!window.feelAI) {
        if (!cancelled) {
          setLoading(false);
        }
        return;
      }

      try {
        const next = await window.feelAI.getAppInfo();
        if (!cancelled) {
          setInfo(next);
        }
      } catch {
        if (!cancelled) {
          setInfo(fallbackInfo);
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void load();

    return () => {
      cancelled = true;
    };
  }, []);

  return { info, loading };
}
