import { useEffect, useMemo, useRef, useState } from 'react';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { toneEngine } from '@/modules/air-instruments/audio/toneEngine';
import { GestureMapper } from '@/modules/air-instruments/gestures';
import { drumsHint, getDrumZones } from '@/modules/air-instruments/instruments/drums';
import {
  getGuitarChordZones,
  guitarHint,
  guitarTones,
} from '@/modules/air-instruments/instruments/guitar';
import { getPianoZones, pianoHint, pianoPresets } from '@/modules/air-instruments/instruments/piano';
import { INSTRUMENT_ORDER, type GuitarTone, type InstrumentId } from '@/modules/air-instruments/types';
import { StageRenderer } from '@/modules/air-instruments/visuals/stageRenderer';
import { useCamera } from '@/modules/camera/useCamera';
import { handTracker } from '@/modules/vision/handTracker';
import type { HandTrackerSnapshot } from '@/modules/vision/types';
import { useAirInstrumentsStore } from '@/stores/airInstrumentsStore';
import styles from './AirInstrumentsPage.module.css';

const INSTRUMENTS: Array<{ id: InstrumentId; label: string }> = [
  { id: 'piano', label: 'Piano' },
  { id: 'drums', label: 'Drums' },
  { id: 'guitar', label: 'Guitar' },
];

function hintFor(instrument: InstrumentId): string {
  const base =
    instrument === 'piano' ? pianoHint : instrument === 'drums' ? drumsHint : guitarHint;
  return `${base} · Мах двумя руками вверх — смена инструмента`;
}

function cycleInstrument(current: InstrumentId, direction: 'prev' | 'next'): InstrumentId {
  const index = INSTRUMENT_ORDER.indexOf(current);
  const offset = direction === 'next' ? 1 : -1;
  const nextIndex = (index + offset + INSTRUMENT_ORDER.length) % INSTRUMENT_ORDER.length;
  return INSTRUMENT_ORDER[nextIndex] ?? current;
}

export function AirInstrumentsPage() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const rendererRef = useRef(new StageRenderer());
  const gesturesRef = useRef(new GestureMapper());
  const settingsRef = useRef({
    instrument: 'piano' as InstrumentId,
    volume: 0.8,
    sensitivity: 1,
    muted: false,
    showSkeleton: true,
    activeChord: 'Em',
    guitarTone: 'crunch' as GuitarTone,
    hasPlayed: false,
    pianoZones: getPianoZones(),
    drumZones: getDrumZones(),
    chordZones: getGuitarChordZones(),
  });
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
    lastHitLabel: null as string | null,
    activeChord: 'Am',
    modelError: null as string | null,
    handsStatus: 'idle' as HandTrackerSnapshot['status'],
    switchFlash: null as string | null,
  });
  const [audioEnabled, setAudioEnabled] = useState(false);
  const [audioError, setAudioError] = useState<string | null>(null);
  const [audioBusy, setAudioBusy] = useState(false);

  const instrument = useAirInstrumentsStore((s) => s.instrument);
  const volume = useAirInstrumentsStore((s) => s.volume);
  const sensitivity = useAirInstrumentsStore((s) => s.sensitivity);
  const muted = useAirInstrumentsStore((s) => s.muted);
  const showSkeleton = useAirInstrumentsStore((s) => s.showSkeleton);
  const showDebug = useAirInstrumentsStore((s) => s.showDebug);
  const pianoPreset = useAirInstrumentsStore((s) => s.pianoPreset);
  const guitarTone = useAirInstrumentsStore((s) => s.guitarTone);
  const panelOpen = useAirInstrumentsStore((s) => s.panelOpen);
  const hasPlayed = useAirInstrumentsStore((s) => s.hasPlayed);
  const setInstrument = useAirInstrumentsStore((s) => s.setInstrument);
  const setVolume = useAirInstrumentsStore((s) => s.setVolume);
  const setSensitivity = useAirInstrumentsStore((s) => s.setSensitivity);
  const setMuted = useAirInstrumentsStore((s) => s.setMuted);
  const setShowSkeleton = useAirInstrumentsStore((s) => s.setShowSkeleton);
  const setShowDebug = useAirInstrumentsStore((s) => s.setShowDebug);
  const setPianoPreset = useAirInstrumentsStore((s) => s.setPianoPreset);
  const setGuitarTone = useAirInstrumentsStore((s) => s.setGuitarTone);
  const setPanelOpen = useAirInstrumentsStore((s) => s.setPanelOpen);
  const markPlayed = useAirInstrumentsStore((s) => s.markPlayed);
  const setInstrumentRef = useRef(setInstrument);
  setInstrumentRef.current = setInstrument;

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

  settingsRef.current = {
    ...settingsRef.current,
    instrument,
    volume,
    sensitivity,
    muted,
    showSkeleton,
    guitarTone,
    hasPlayed,
  };

  useEffect(() => {
    toneEngine.setVolume(volume);
  }, [volume]);

  useEffect(() => {
    toneEngine.setMuted(muted);
  }, [muted]);

  useEffect(() => {
    if (toneEngine.isReady()) {
      toneEngine.setPianoPreset(pianoPreset);
    }
  }, [pianoPreset]);

  useEffect(() => {
    if (toneEngine.isReady()) {
      toneEngine.setGuitarTone(guitarTone);
    }
  }, [guitarTone]);

  useEffect(() => {
    gesturesRef.current.panic();
    rendererRef.current.reset();
  }, [instrument]);

  useEffect(() => {
    const unsubscribe = handTracker.subscribe((snapshot) => {
      handsRef.current = snapshot;
      setHud((prev) => ({
        ...prev,
        handFps: snapshot.fps,
        hasHands: snapshot.hasHands,
        handsStatus: snapshot.status,
        modelError: snapshot.status === 'error' ? snapshot.errorMessage : null,
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
      }));
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
    let frame = 0;

    const tick = () => {
      const canvas = canvasRef.current;
      const settings = settingsRef.current;
      const hands = handsRef.current;

      if (canvas && cameraStatus === 'active') {
        gesturesRef.current.process(hands.hands, {
          instrument: settings.instrument,
          sensitivity: settings.sensitivity,
          muted: settings.muted,
          pianoZones: settings.pianoZones,
          drumZones: settings.drumZones,
          chordZones: settings.chordZones,
          activeChord: settings.activeChord,
          guitarTone: settings.guitarTone,
        });

        const switches = gesturesRef.current.consumeInstrumentSwitches();
        if (switches.length > 0) {
          const direction = switches[switches.length - 1]?.direction ?? 'next';
          const nextInstrument = cycleInstrument(settings.instrument, direction);
          settings.instrument = nextInstrument;
          setInstrumentRef.current(nextInstrument);
          const label =
            INSTRUMENTS.find((item) => item.id === nextInstrument)?.label ?? nextInstrument;
          setHud((prev) => ({
            ...prev,
            switchFlash: `↑ ${label}`,
            lastHitLabel: label,
          }));
          window.setTimeout(() => {
            setHud((prev) => ({ ...prev, switchFlash: null }));
          }, 900);
        }

        const hits = gesturesRef.current.consumeHits();
        if (hits.length > 0) {
          rendererRef.current.pushHits(hits);
          const label = hits[hits.length - 1]?.label ?? null;
          setHud((prev) => ({ ...prev, lastHitLabel: label }));
          if (!settings.hasPlayed) {
            markPlayed();
          }
        }

        const selectedChord = gesturesRef.current.getSelectedChord();
        settings.activeChord = selectedChord;

        rendererRef.current.draw(canvas, {
          instrument: settings.instrument,
          hands: hands.hands,
          pianoZones: settings.pianoZones,
          drumZones: settings.drumZones,
          chordZones: settings.chordZones,
          activeChord: selectedChord,
          activeNotes: gesturesRef.current.getActiveNoteLabels(),
          hits,
          showSkeleton: settings.showSkeleton,
          audioPeak: toneEngine.getPeak(),
          guitar: gesturesRef.current.getGuitarState(),
          hint: settings.hasPlayed
            ? settings.instrument === 'guitar'
              ? 'Левая — аккорд/кулак mute · Правая — strum/fingerpick'
              : 'Мах двумя руками вверх — смена инструмента'
            : hintFor(settings.instrument),
        });

        frame += 1;
        if (frame % 10 === 0) {
          setHud((prev) =>
            prev.activeChord === selectedChord ? prev : { ...prev, activeChord: selectedChord },
          );
        }
      }

      raf = requestAnimationFrame(tick);
    };

    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [cameraStatus, markPlayed]);

  useEffect(() => {
    const gestures = gesturesRef.current;
    const renderer = rendererRef.current;
    return () => {
      gestures.panic();
      renderer.reset();
      handTracker.stop();
    };
  }, []);

  async function ensureAudio(): Promise<boolean> {
    if (audioBusy) {
      return audioEnabled;
    }
    setAudioBusy(true);
    setAudioError(null);
    try {
      await toneEngine.unlock(true);
      toneEngine.setVolume(Math.max(0.35, volume));
      if (volume < 0.35) {
        setVolume(0.35);
      }
      toneEngine.setMuted(false);
      setMuted(false);
      toneEngine.setPianoPreset(pianoPreset);
      toneEngine.setGuitarTone(guitarTone);
      setAudioEnabled(true);
      return true;
    } catch (error) {
      setAudioEnabled(false);
      setAudioError(error instanceof Error ? error.message : 'Не удалось включить звук');
      return false;
    } finally {
      setAudioBusy(false);
    }
  }

  async function handleInstrumentChange(next: InstrumentId): Promise<void> {
    await ensureAudio();
    setInstrument(next);
  }

  const cameraBusy = cameraStatus === 'requesting';
  const modelBusy = hud.handsStatus === 'loading';
  const hardError = cameraStatus === 'denied' || cameraStatus === 'error' || cameraStatus === 'unavailable';
  const showBlockingOverlay = cameraBusy || hardError || Boolean(hud.modelError) || cameraStatus === 'idle';
  const showAudioGate = cameraStatus === 'active' && !audioEnabled && !showBlockingOverlay;

  return (
    <div className={styles.page}>
      <div
        className={styles.stage}
        onPointerDown={() => {
          if (!audioEnabled) {
            void ensureAudio();
          }
        }}
      >
        <video ref={videoRef} className={styles.video} playsInline muted autoPlay />
        <canvas ref={canvasRef} className={styles.canvas} />

        <div className={styles.topHud}>
          <div className={styles.switcher}>
            {INSTRUMENTS.map((item) => (
              <button
                key={item.id}
                type="button"
                className={instrument === item.id ? styles.switchActive : styles.switchBtn}
                onClick={() => {
                  void handleInstrumentChange(item.id);
                }}
              >
                {item.label}
              </button>
            ))}
          </div>

          <div className={styles.hudMeta}>
            <Badge tone={audioEnabled ? 'success' : 'warning'}>
              Звук · {audioEnabled ? 'вкл' : 'выкл'}
            </Badge>
            <Badge tone={hud.hasHands ? 'success' : 'neutral'}>
              Трекинг · {hud.trackingLabel}
            </Badge>
            {hud.switchFlash ? <Badge tone="accent">Смена · {hud.switchFlash}</Badge> : null}
            {hud.lastHitLabel && !hud.switchFlash ? (
              <Badge tone="accent">Hit · {hud.lastHitLabel}</Badge>
            ) : null}
            {instrument === 'guitar' ? <Badge>Аккорд · {hud.activeChord}</Badge> : null}
            {showDebug ? (
              <>
                <Badge>Cam FPS · {cameraFps || '—'}</Badge>
                <Badge>Hand FPS · {hud.handFps || '—'}</Badge>
              </>
            ) : null}
          </div>
        </div>

        <div className={`${styles.sidePanel} ${panelOpen ? styles.sideOpen : ''}`}>
          <button
            type="button"
            className={styles.panelToggle}
            onClick={() => setPanelOpen(!panelOpen)}
          >
            {panelOpen ? 'Скрыть' : 'Панель'}
          </button>

          {panelOpen ? (
            <div className={styles.panelBody}>
              <h3>Студия</h3>

              <label className={styles.field}>
                <span>Громкость · {Math.round(volume * 100)}%</span>
                <input
                  type="range"
                  min={0}
                  max={1}
                  step={0.01}
                  value={volume}
                  onChange={(event) => {
                    void ensureAudio();
                    setVolume(Number(event.target.value));
                  }}
                />
              </label>

              <label className={styles.field}>
                <span>Чувствительность · {sensitivity.toFixed(1)}</span>
                <input
                  type="range"
                  min={0.5}
                  max={2}
                  step={0.1}
                  value={sensitivity}
                  onChange={(event) => setSensitivity(Number(event.target.value))}
                />
              </label>

              {instrument === 'piano' ? (
                <label className={styles.field}>
                  <span>Пресет тембра</span>
                  <select
                    value={pianoPreset}
                    onChange={(event) => {
                      void ensureAudio();
                      setPianoPreset(event.target.value as typeof pianoPreset);
                    }}
                  >
                    {pianoPresets.map((preset) => (
                      <option key={preset.id} value={preset.id}>
                        {preset.label}
                      </option>
                    ))}
                  </select>
                </label>
              ) : null}

              {instrument === 'guitar' ? (
                <label className={styles.field}>
                  <span>Amp / тон гитары</span>
                  <select
                    value={guitarTone}
                    onChange={(event) => {
                      void ensureAudio();
                      setGuitarTone(event.target.value as GuitarTone);
                    }}
                  >
                    {guitarTones.map((tone) => (
                      <option key={tone.id} value={tone.id}>
                        {tone.label}
                      </option>
                    ))}
                  </select>
                </label>
              ) : null}

              <label className={styles.field}>
                <span>Камера</span>
                <select
                  value={activeDeviceId ?? ''}
                  disabled={devices.length === 0}
                  onChange={(event) => {
                    const value = event.target.value;
                    if (value) {
                      void switchDevice(value);
                    }
                  }}
                >
                  {devices.length === 0 ? <option value="">Нет устройств</option> : null}
                  {devices.map((device) => (
                    <option key={device.deviceId} value={device.deviceId}>
                      {device.label}
                    </option>
                  ))}
                </select>
              </label>

              <label className={styles.check}>
                <input
                  type="checkbox"
                  checked={muted}
                  onChange={(event) => {
                    void ensureAudio();
                    setMuted(event.target.checked);
                  }}
                />
                <span>Mute</span>
              </label>

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
                  disabled={audioBusy}
                  onClick={() => {
                    void ensureAudio();
                  }}
                >
                  {audioBusy ? 'Включаем…' : audioEnabled ? 'Тест звука' : 'Включить звук'}
                </Button>
                <Button
                  size="sm"
                  variant="secondary"
                  onClick={() => {
                    gesturesRef.current.panic();
                    toneEngine.panic();
                  }}
                >
                  Panic
                </Button>
                {cameraStatus === 'active' ? (
                  <Button size="sm" variant="ghost" onClick={stop}>
                    Стоп камера
                  </Button>
                ) : (
                  <Button
                    size="sm"
                    variant="ghost"
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

        {showAudioGate ? (
          <div className={styles.audioGate}>
            <h3>Звук выключен</h3>
            <p>Нажми кнопку ниже — должен пикнуть тест. Без этого жесты молчат.</p>
            {audioError ? <p className={styles.audioError}>{audioError}</p> : null}
            <Button
              size="lg"
              disabled={audioBusy}
              onClick={(event) => {
                event.stopPropagation();
                void ensureAudio();
              }}
            >
              {audioBusy ? 'Включаем…' : 'Включить звук'}
            </Button>
          </div>
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
                    void start().catch(() => undefined);
                  }}
                >
                  Запустить
                </Button>
              </>
            ) : null}
          </div>
        ) : null}
      </div>
    </div>
  );
}
