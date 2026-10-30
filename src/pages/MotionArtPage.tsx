import { useEffect, useMemo, useRef, useState } from 'react';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { MotionRenderer } from '@/modules/motion-art/motionRenderer';
import { BRUSHES, COLOR_MODES, type ColorMode } from '@/modules/motion-art/types';
import { useCamera } from '@/modules/camera/useCamera';
import { handTracker } from '@/modules/vision/handTracker';
import type { HandTrackerSnapshot } from '@/modules/vision/types';
import { useMotionArtStore } from '@/stores/motionArtStore';
import styles from './MotionArtPage.module.css';

export function MotionArtPage() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const rendererRef = useRef(new MotionRenderer());
  const handsRef = useRef<HandTrackerSnapshot>({
    hands: [],
    fps: 0,
    status: 'idle',
    errorMessage: null,
    hasHands: false,
  });

  const [hud, setHud] = useState({
    trackingLabel: 'загрузка модели',
    handFps: 0,
    hasHands: false,
    modelError: null as string | null,
    handsStatus: 'idle' as HandTrackerSnapshot['status'],
  });

  const brush = useMotionArtStore((s) => s.brush);
  const colorMode = useMotionArtStore((s) => s.colorMode);
  const trailFade = useMotionArtStore((s) => s.trailFade);
  const thickness = useMotionArtStore((s) => s.thickness);
  const showSkeleton = useMotionArtStore((s) => s.showSkeleton);
  const showDebug = useMotionArtStore((s) => s.showDebug);
  const panelOpen = useMotionArtStore((s) => s.panelOpen);
  const setBrush = useMotionArtStore((s) => s.setBrush);
  const setColorMode = useMotionArtStore((s) => s.setColorMode);
  const setTrailFade = useMotionArtStore((s) => s.setTrailFade);
  const setThickness = useMotionArtStore((s) => s.setThickness);
  const setShowSkeleton = useMotionArtStore((s) => s.setShowSkeleton);
  const setShowDebug = useMotionArtStore((s) => s.setShowDebug);
  const setPanelOpen = useMotionArtStore((s) => s.setPanelOpen);

  const settingsRef = useRef({
    brush,
    colorMode,
    trailFade,
    thickness,
    showSkeleton,
    showDebug,
  });
  settingsRef.current = {
    brush,
    colorMode,
    trailFade,
    thickness,
    showSkeleton,
    showDebug,
  };

  const startOptions = useMemo(() => ({ width: 1280, height: 720 }), []);
  const {
    videoRef,
    status: cameraStatus,
    errorMessage: cameraError,
    devices,
    activeDeviceId,
    fps: cameraFps,
    start,
    stop,
    switchDevice,
  } = useCamera({ autoStart: true, startOptions });

  useEffect(() => {
    const unsubscribe = handTracker.subscribe((snapshot) => {
      handsRef.current = snapshot;
      setHud({
        trackingLabel:
          snapshot.status === 'loading'
            ? 'загрузка модели'
            : snapshot.status === 'error'
              ? 'ошибка модели'
              : snapshot.status === 'ready'
                ? 'модель готова'
                : snapshot.hasHands
                  ? `${snapshot.hands.length} рук`
                  : 'покажите руки',
        handFps: snapshot.fps,
        hasHands: snapshot.hasHands,
        modelError: snapshot.status === 'error' ? snapshot.errorMessage : null,
        handsStatus: snapshot.status,
      });
    });

    return () => {
      unsubscribe();
      handTracker.stop();
    };
  }, []);

  useEffect(() => {
    if (cameraStatus !== 'active') {
      handTracker.stop();
      return;
    }

    const video = videoRef.current;
    if (!video) {
      return;
    }

    let cancelled = false;
    void (async () => {
      try {
        await handTracker.start(video);
      } catch {
        if (!cancelled) {
          // HUD shows modelError.
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [cameraStatus, videoRef]);

  useEffect(() => {
    let raf = 0;

    const tick = () => {
      const canvas = canvasRef.current;
      const settings = settingsRef.current;
      const hands = handsRef.current;

      if (canvas && cameraStatus === 'active') {
        rendererRef.current.draw(canvas, {
          hands: hands.hands,
          brush: settings.brush,
          colorMode: settings.colorMode,
          trailFade: settings.trailFade,
          thickness: settings.thickness,
          showSkeleton: settings.showSkeleton,
          showDebug: settings.showDebug,
          handFps: hands.fps,
          cameraFps,
          drawing: true,
          hint: 'Пальцы рисуют свет · кулак = пауза · лента лепит формы ладонью',
        });
      }

      raf = requestAnimationFrame(tick);
    };

    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [cameraStatus, cameraFps]);

  useEffect(() => {
    const renderer = rendererRef.current;
    return () => {
      renderer.clear();
      handTracker.stop();
    };
  }, []);

  const cameraBusy = cameraStatus === 'requesting';
  const modelBusy = hud.handsStatus === 'loading';
  const hardError =
    cameraStatus === 'denied' || cameraStatus === 'error' || cameraStatus === 'unavailable';
  const showBlockingOverlay =
    cameraBusy || hardError || Boolean(hud.modelError) || cameraStatus === 'idle';

  return (
    <div className={styles.page}>
      <div className={styles.stage}>
        <video ref={videoRef} className={styles.video} playsInline muted autoPlay />
        <canvas ref={canvasRef} className={styles.canvas} />

        <div className={styles.topHud}>
          <div className={styles.switcher}>
            {BRUSHES.map((item) => (
              <button
                key={item.id}
                type="button"
                className={brush === item.id ? styles.switchActive : styles.switchBtn}
                onClick={() => setBrush(item.id)}
              >
                {item.label}
              </button>
            ))}
          </div>

          <div className={styles.hudMeta}>
            <Badge tone={hud.hasHands ? 'success' : 'neutral'}>
              Трекинг · {hud.trackingLabel}
            </Badge>
            <Badge tone="accent">Камера · {cameraFps || '—'} fps</Badge>
          </div>
        </div>

        <div className={styles.sidePanel}>
          <button
            type="button"
            className={styles.panelToggle}
            onClick={() => setPanelOpen(!panelOpen)}
          >
            <span>Кисть и цвет</span>
            <span>{panelOpen ? '▾' : '▸'}</span>
          </button>

          {panelOpen ? (
            <div className={styles.panelBody}>
              <div className={styles.field}>
                <label htmlFor="motion-color">Палитра</label>
                <select
                  id="motion-color"
                  value={colorMode}
                  onChange={(event) => setColorMode(event.target.value as ColorMode)}
                >
                  {COLOR_MODES.map((item) => (
                    <option key={item.id} value={item.id}>
                      {item.label}
                    </option>
                  ))}
                </select>
              </div>

              <div className={styles.field}>
                <label htmlFor="motion-thickness">Толщина · {thickness.toFixed(1)}</label>
                <input
                  id="motion-thickness"
                  type="range"
                  min={2}
                  max={14}
                  step={0.5}
                  value={thickness}
                  onChange={(event) => setThickness(Number(event.target.value))}
                />
              </div>

              <div className={styles.field}>
                <label htmlFor="motion-fade">Затухание · {trailFade.toFixed(3)}</label>
                <input
                  id="motion-fade"
                  type="range"
                  min={0.01}
                  max={0.2}
                  step={0.005}
                  value={trailFade}
                  onChange={(event) => setTrailFade(Number(event.target.value))}
                />
              </div>

              {devices.length > 1 ? (
                <div className={styles.field}>
                  <label htmlFor="motion-camera">Камера</label>
                  <select
                    id="motion-camera"
                    value={activeDeviceId ?? ''}
                    onChange={(event) => {
                      void switchDevice(event.target.value);
                    }}
                  >
                    {devices.map((device) => (
                      <option key={device.deviceId} value={device.deviceId}>
                        {device.label || device.deviceId.slice(0, 8)}
                      </option>
                    ))}
                  </select>
                </div>
              ) : null}

              <label className={styles.check}>
                <input
                  type="checkbox"
                  checked={showSkeleton}
                  onChange={(event) => setShowSkeleton(event.target.checked)}
                />
                <span>Скелет рук</span>
              </label>

              <label className={styles.check}>
                <input
                  type="checkbox"
                  checked={showDebug}
                  onChange={(event) => setShowDebug(event.target.checked)}
                />
                <span>Debug FPS</span>
              </label>

              <div className={styles.actions}>
                <Button
                  size="sm"
                  onClick={() => {
                    rendererRef.current.clear();
                  }}
                >
                  Очистить холст
                </Button>
                {cameraStatus === 'active' ? (
                  <Button size="sm" variant="secondary" onClick={stop}>
                    Стоп камера
                  </Button>
                ) : (
                  <Button
                    size="sm"
                    variant="secondary"
                    onClick={() => {
                      void start();
                    }}
                  >
                    Старт камера
                  </Button>
                )}
              </div>
            </div>
          ) : null}
        </div>

        {modelBusy && cameraStatus === 'active' ? (
          <div className={styles.banner}>Загружаем Hand Landmarker…</div>
        ) : null}

        {showBlockingOverlay ? (
          <div className={styles.overlay}>
            {cameraBusy ? <p>Запрашиваем доступ к камере…</p> : null}
            {hardError ? (
              <>
                <p>{cameraError ?? 'Камера недоступна'}</p>
                <Button
                  onClick={() => {
                    void start().catch(() => undefined);
                  }}
                >
                  Повторить
                </Button>
              </>
            ) : null}
            {hud.modelError ? (
              <>
                <p>{hud.modelError}</p>
                <Button
                  onClick={() => {
                    window.location.reload();
                  }}
                >
                  Перезагрузить
                </Button>
              </>
            ) : null}
            {cameraStatus === 'idle' && !hardError && !cameraBusy ? (
              <>
                <p>Камера остановлена</p>
                <Button
                  onClick={() => {
                    void start();
                  }}
                >
                  Включить камеру
                </Button>
              </>
            ) : null}
          </div>
        ) : null}
      </div>
    </div>
  );
}
