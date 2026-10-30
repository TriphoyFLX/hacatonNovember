import { Link } from 'react-router-dom';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { Card } from '@/components/ui/Card';
import { modes } from '@/data/modes';
import { useAppStore } from '@/stores/appStore';
import styles from './HomePage.module.css';

const recentEffects = [
  { id: 'neon-trail', label: 'Neon Trail', mode: 'Motion Art' },
  { id: 'cheek-warp', label: 'Cheek Warp', mode: 'Face Studio' },
  { id: 'air-kick', label: 'Air Kick Snare', mode: 'Air Instruments' },
];

const recommendations = [
  {
    id: 'calibrate',
    title: 'Run first calibration',
    detail: 'Tune gesture sensitivity before the next session.',
    to: '/ai-personalization',
  },
  {
    id: 'face',
    title: 'Try Face Studio',
    detail: 'Best starting point for camera-driven interaction.',
    to: '/face-studio',
  },
];

export function HomePage() {
  const userName = useAppStore((state) => state.userName);
  const cameraStatus = useAppStore((state) => state.cameraStatus);
  const setCameraStatus = useAppStore((state) => state.setCameraStatus);
  const aiStatus = useAppStore((state) => state.aiStatus);

  function handleCameraLaunch() {
    if (cameraStatus === 'active') {
      setCameraStatus('idle');
      return;
    }

    setCameraStatus('requesting');
    window.setTimeout(() => {
      setCameraStatus('active');
    }, 650);
  }

  return (
    <div className={styles.page}>
      <section className={styles.hero}>
        <div className={styles.heroCopy}>
          <p className={styles.kicker}>Welcome back</p>
          <h2>{userName}, feel the world your way.</h2>
          <p className={styles.lead}>
            FEEL AI turns your camera into a private interaction layer — face, hands, and body
            drive visuals, sound, and adaptive controls entirely on device.
          </p>
        </div>

        <Card className={styles.cameraCard} interactive>
          <div className={styles.cameraGlow} aria-hidden />
          <div className={styles.cameraHeader}>
            <Badge tone={cameraStatus === 'active' ? 'success' : 'accent'}>
              Camera {cameraStatus}
            </Badge>
            <Badge tone={aiStatus === 'ready' ? 'success' : 'neutral'}>AI {aiStatus}</Badge>
          </div>
          <h3>Launch camera session</h3>
          <p>
            This control prepares the shared Camera Manager. Real MediaPipe tracking arrives in
            the next stage — the session state is already wired into the app store.
          </p>
          <div className={styles.cameraActions}>
            <Button size="lg" onClick={handleCameraLaunch}>
              {cameraStatus === 'active' ? 'Stop session' : 'Start camera'}
            </Button>
            <Link to="/ai-personalization" className={styles.inlineLink}>
              Calibrate
            </Link>
          </div>
          <div className={styles.pulseRow} aria-hidden>
            <span />
            <span />
            <span />
          </div>
        </Card>
      </section>

      <section className={styles.section}>
        <div className={styles.sectionHead}>
          <h3>Modes</h3>
          <p>Five interaction spaces sharing one vision pipeline.</p>
        </div>
        <div className={styles.modeGrid}>
          {modes.map((mode) => (
            <Link key={mode.id} to={mode.path} className={styles.modeLink}>
              <Card interactive className={styles.modeCard}>
                <div
                  className={styles.modeAccent}
                  style={{ background: mode.accent }}
                  aria-hidden
                />
                <div className={styles.modeMeta}>
                  <Badge>{mode.status}</Badge>
                  <h4>{mode.title}</h4>
                  <p>{mode.description}</p>
                </div>
              </Card>
            </Link>
          ))}
        </div>
      </section>

      <section className={styles.split}>
        <Card>
          <div className={styles.sectionHead}>
            <h3>Recent effects</h3>
            <p>Demo history until profiles persist locally.</p>
          </div>
          <ul className={styles.list}>
            {recentEffects.map((item) => (
              <li key={item.id}>
                <strong>{item.label}</strong>
                <span>{item.mode}</span>
              </li>
            ))}
          </ul>
        </Card>

        <Card>
          <div className={styles.sectionHead}>
            <h3>Recommendations</h3>
            <p>Personalization engine will replace these defaults.</p>
          </div>
          <div className={styles.recoList}>
            {recommendations.map((item) => (
              <Link key={item.id} to={item.to} className={styles.recoItem}>
                <strong>{item.title}</strong>
                <span>{item.detail}</span>
              </Link>
            ))}
          </div>
        </Card>
      </section>
    </div>
  );
}
