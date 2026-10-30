import { useAppInfo } from '@/hooks/useAppInfo';
import { useAppStore } from '@/stores/appStore';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import styles from './TopBar.module.css';

export function TopBar() {
  const { info } = useAppInfo();
  const userName = useAppStore((state) => state.userName);
  const cameraStatus = useAppStore((state) => state.cameraStatus);
  const aiStatus = useAppStore((state) => state.aiStatus);
  const theme = useAppStore((state) => state.theme);
  const toggleTheme = useAppStore((state) => state.toggleTheme);
  const notificationsOpen = useAppStore((state) => state.notificationsOpen);
  const setNotificationsOpen = useAppStore((state) => state.setNotificationsOpen);

  const cameraTone =
    cameraStatus === 'active' ? 'success' : cameraStatus === 'denied' ? 'warning' : 'neutral';

  return (
    <header className={styles.topbar}>
      <div className={styles.left}>
        <div>
          <p className={styles.eyebrow}>Desktop experience</p>
          <h1 className={styles.title}>Control room</h1>
        </div>
      </div>

      <div className={styles.right}>
        <Badge tone={cameraTone}>Camera · {cameraStatus}</Badge>
        <Badge tone={aiStatus === 'ready' ? 'accent' : 'neutral'}>AI · {aiStatus}</Badge>
        <Badge>v{info.version}</Badge>

        <Button
          variant="ghost"
          size="sm"
          onClick={() => setNotificationsOpen(!notificationsOpen)}
          aria-expanded={notificationsOpen}
        >
          Alerts
        </Button>
        <Button variant="secondary" size="sm" onClick={toggleTheme}>
          Theme · {theme}
        </Button>
        <div className={styles.profile} title="Profile">
          <span className={styles.avatar} aria-hidden>
            {userName.slice(0, 1).toUpperCase()}
          </span>
          <span>{userName}</span>
        </div>
      </div>
    </header>
  );
}
