import { Link } from 'react-router-dom';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { Card } from '@/components/ui/Card';
import { aiStatusLabel, cameraStatusLabel, modeStatusLabel } from '@/data/labels';
import { modes } from '@/data/modes';
import { useAppStore } from '@/stores/appStore';
import styles from './HomePage.module.css';

const recentEffects = [
  { id: 'neon-trail', label: 'Неоновый след', mode: 'Искусство движения' },
  { id: 'cheek-warp', label: 'Деформация щёк', mode: 'Студия лица' },
  { id: 'air-kick', label: 'Воздушный снэр', mode: 'Воздушные инструменты' },
];

const recommendations = [
  {
    id: 'calibrate',
    title: 'Пройдите первую калибровку',
    detail: 'Настройте чувствительность жестов перед следующей сессией.',
    to: '/ai-personalization',
  },
  {
    id: 'face',
    title: 'Попробуйте Студию лица',
    detail: 'Лучшая точка старта для взаимодействия через камеру.',
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
          <p className={styles.kicker}>С возвращением</p>
          <h2>{userName}, ощути мир по-своему.</h2>
          <p className={styles.lead}>
            FEEL AI превращает камеру в приватный слой взаимодействия — лицо, руки и тело
            управляют визуалом, звуком и адаптивными контролами полностью на устройстве.
          </p>
        </div>

        <Card className={styles.cameraCard} interactive>
          <div className={styles.cameraGlow} aria-hidden />
          <div className={styles.cameraHeader}>
            <Badge tone={cameraStatus === 'active' ? 'success' : 'accent'}>
              Камера · {cameraStatusLabel[cameraStatus]}
            </Badge>
            <Badge tone={aiStatus === 'ready' ? 'success' : 'neutral'}>
              AI · {aiStatusLabel[aiStatus]}
            </Badge>
          </div>
          <h3>Запуск сессии камеры</h3>
          <p>
            Этот контроль готовит общий Camera Manager. Реальный трекинг MediaPipe появится на
            следующем этапе — состояние сессии уже связано с хранилищем приложения.
          </p>
          <div className={styles.cameraActions}>
            <Button size="lg" onClick={handleCameraLaunch}>
              {cameraStatus === 'active' ? 'Остановить сессию' : 'Запустить камеру'}
            </Button>
            <Link to="/ai-personalization" className={styles.inlineLink}>
              Калибровка
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
          <h3>Режимы</h3>
          <p>Пять пространств взаимодействия на одном vision-конвейере.</p>
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
                  <Badge>{modeStatusLabel[mode.status]}</Badge>
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
            <h3>Недавние эффекты</h3>
            <p>Демо-история, пока профили не сохраняются локально.</p>
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
            <h3>Рекомендации</h3>
            <p>Движок персонализации позже заменит эти значения по умолчанию.</p>
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
