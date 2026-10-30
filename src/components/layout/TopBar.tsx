import { useNavigate } from 'react-router-dom';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { aiStatusLabel, cameraStatusLabel, themeLabel } from '@/data/labels';
import { useAppInfo } from '@/hooks/useAppInfo';
import { useAppStore } from '@/stores/appStore';
import { useAuthStore } from '@/stores/authStore';
import styles from './TopBar.module.css';

export function TopBar() {
  const navigate = useNavigate();
  const { info } = useAppInfo();
  const cameraStatus = useAppStore((state) => state.cameraStatus);
  const aiStatus = useAppStore((state) => state.aiStatus);
  const theme = useAppStore((state) => state.theme);
  const toggleTheme = useAppStore((state) => state.toggleTheme);
  const notificationsOpen = useAppStore((state) => state.notificationsOpen);
  const setNotificationsOpen = useAppStore((state) => state.setNotificationsOpen);
  const user = useAuthStore((state) => state.user);
  const logout = useAuthStore((state) => state.logout);
  const loading = useAuthStore((state) => state.loading);

  const cameraTone =
    cameraStatus === 'active' ? 'success' : cameraStatus === 'denied' ? 'warning' : 'neutral';

  async function handleLogout() {
    await logout();
    navigate('/login', { replace: true });
  }

  return (
    <header className={styles.topbar}>
      <div className={styles.left}>
        <div>
          <p className={styles.eyebrow}>Настольное приложение</p>
          <h1 className={styles.title}>Центр управления</h1>
        </div>
      </div>

      <div className={styles.right}>
        <Badge tone={cameraTone}>Камера · {cameraStatusLabel[cameraStatus]}</Badge>
        <Badge tone={aiStatus === 'ready' ? 'accent' : 'neutral'}>
          AI · {aiStatusLabel[aiStatus]}
        </Badge>
        <Badge>v{info.version}</Badge>

        <Button
          variant="ghost"
          size="sm"
          onClick={() => setNotificationsOpen(!notificationsOpen)}
          aria-expanded={notificationsOpen}
        >
          Уведомления
        </Button>
        <Button variant="secondary" size="sm" onClick={toggleTheme}>
          Тема · {themeLabel[theme]}
        </Button>
        <div className={styles.profile} title={user?.email ?? 'Профиль'}>
          <span className={styles.avatar} aria-hidden>
            {(user?.avatar ?? user?.displayName?.slice(0, 1) ?? '?').toUpperCase()}
          </span>
          <span>{user?.displayName ?? 'Гость'}</span>
        </div>
        <Button variant="ghost" size="sm" disabled={loading} onClick={() => void handleLogout()}>
          Выйти
        </Button>
      </div>
    </header>
  );
}
