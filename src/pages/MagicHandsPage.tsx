import { useEffect, useMemo, useRef, useState } from 'react';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { useCamera } from '@/modules/camera/useCamera';
import { MagicRenderer } from '@/modules/magic-hands/magicRenderer';
import { ELEMENTS } from '@/modules/magic-hands/types';
import { handTracker } from '@/modules/vision/handTracker';
import type { HandTrackerSnapshot } from '@/modules/vision/types';
import { useMagicHandsStore } from '@/stores/magicHandsStore';
import styles from './MagicHandsPage.module.css';

export function MagicHandsPage() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const rendererRef = useRef(new MagicRenderer());
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

  const element = useMagicHandsStore((s) => s.element);
  const intensity = useMagicHandsStore((s) => s.intensity);
  const showSkeleton = useMagicHandsStore((s) => s.showSkeleton);
  const showDebug = useMagicHandsStore((s) => s.showDebug);
  const panelOpen = useMagicHandsStore((s) => s.panelOpen);
  const setElement = useMagicHandsStore((s) => s.setElement);
  const setIntensity = useMagicHandsStore((s) => s.setIntensity);
  const setShowSkeleton = useMagicHandsStore((s) => s.setShowSkeleton);
  const setShowDebug = useMagicHandsStore((s) => s.setShowDebug);
  const setPanelOpen = useMagicHandsStore((s) => s.setPanelOpen);

  const settingsRef = useRef({ element, intensity, showSkeleton, showDebug });
  settingsRef.current = { element, intensity, showSkeleton, showDebug };

  const activeElement = ELEMENTS.find((item) => item.id === element) ?? ELEMENTS[0];

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
      const hint =
        ELEMENTS.find((item) => item.id === settings.element)?.hint ??
        'Управляйте стихией ладонями';

      if (canvas && cameraStatus === 'active') {
        rendererRef.current.draw(canvas, {
          hands: hands.hands,
          element: settings.element,
          intensity: settings.intensity,
          showSkeleton: settings.showSkeleton,
          showDebug: settings.showDebug,
          handFps: hands.fps,
          cameraFps,
          hint:
            settings.element === 'fire'
              ? `${hint} · Сжатие/раскрытие тоже работают`
              : `${hint} · Щипок = луч · Две ладони = резонанс · Двойной каст = ультимейт`,
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
            {ELEMENTS.map((item) => (
              <button
                key={item.id}
                type="button"
                className={element === item.id ? styles.switchActive : styles.switchBtn}
                onClick={() => setElement(item.id)}
              >
                {item.label}
              </button>
            ))}
          </div>

          <div className={styles.hudMeta}>
            <Badge tone={hud.hasHands ? 'success' : 'neutral'}>
              Трекинг · {hud.trackingLabel}
            </Badge>
            <Badge tone="accent">{activeElement.label}</Badge>
          </div>
        </div>

        <div className={styles.sidePanel}>
          <button
            type="button"
            className={styles.panelToggle}
            onClick={() => setPanelOpen(!panelOpen)}
          >
            <span>Сила магии</span>
            <span>{panelOpen ? '▾' : '▸'}</span>
          </button>

          {panelOpen ? (
            <div className={styles.panelBody}>
              <p className={styles.hint}>{activeElement.hint}</p>

              <div className={styles.field}>
                <label htmlFor="magic-intensity">Интенсивность · {intensity.toFixed(2)}</label>
                <input
                  id="magic-intensity"
                  type="range"
                  min={0.4}
                  max={1.6}
                  step={0.05}
                  value={intensity}
                  onChange={(event) => setIntensity(Number(event.target.value))}
                />
              </div>

              {devices.length > 1 ? (
                <div className={styles.field}>
                  <label htmlFor="magic-camera">Камера</label>
                  <select
                    id="magic-camera"
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
                  Сбросить эффекты
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
