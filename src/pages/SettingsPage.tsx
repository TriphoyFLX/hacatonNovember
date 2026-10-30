import { useEffect, useState, type FormEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { Card } from '@/components/ui/Card';
import { Input } from '@/components/ui/Input';
import { themeLabel } from '@/data/labels';
import { useAppInfo } from '@/hooks/useAppInfo';
import { useAppStore } from '@/stores/appStore';
import { useAuthStore } from '@/stores/authStore';
import styles from './SettingsPage.module.css';

export function SettingsPage() {
  const navigate = useNavigate();
  const { info, loading: infoLoading } = useAppInfo();
  const theme = useAppStore((state) => state.theme);
  const toggleTheme = useAppStore((state) => state.toggleTheme);
  const user = useAuthStore((state) => state.user);
  const authLoading = useAuthStore((state) => state.loading);
  const error = useAuthStore((state) => state.error);
  const updateProfile = useAuthStore((state) => state.updateProfile);
  const logout = useAuthStore((state) => state.logout);
  const clearError = useAuthStore((state) => state.clearError);

  const [displayName, setDisplayName] = useState(user?.displayName ?? '');
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    setDisplayName(user?.displayName ?? '');
  }, [user?.displayName]);

  async function handleSave(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    clearError();
    setSaved(false);
    const ok = await updateProfile({ displayName });
    if (ok) {
      setSaved(true);
    }
  }

  async function handleLogout() {
    await logout();
    navigate('/login', { replace: true });
  }

  return (
    <div className={styles.page}>
      <div className={styles.header}>
        <h2>Настройки</h2>
        <p>Профиль и локальные предпочтения FEEL AI. Аккаунт хранится только на этом устройстве.</p>
      </div>

      <div className={styles.grid}>
        <Card>
          <Badge>Профиль</Badge>
          <h3>Данные аккаунта</h3>
          <p className={styles.email}>{user?.email}</p>
          <form className={styles.form} onSubmit={(event) => void handleSave(event)}>
            <Input
              label="Отображаемое имя"
              name="displayName"
              value={displayName}
              onChange={(event) => setDisplayName(event.target.value)}
              maxLength={32}
            />
            {error ? <p className={styles.error}>{error}</p> : null}
            {saved ? <p className={styles.success}>Профиль сохранён</p> : null}
            <div className={styles.actions}>
              <Button type="submit" disabled={authLoading}>
                {authLoading ? 'Сохраняем…' : 'Сохранить'}
              </Button>
              <Button variant="secondary" onClick={toggleTheme} type="button">
                Тема · {themeLabel[theme]}
              </Button>
              <Button variant="ghost" type="button" onClick={() => void handleLogout()}>
                Выйти
              </Button>
            </div>
          </form>
        </Card>

        <Card>
          <Badge tone="accent">Среда выполнения</Badge>
          <h3>Информация о приложении</h3>
          {infoLoading ? (
            <p className={styles.muted}>Читаем данные Electron…</p>
          ) : (
            <dl className={styles.meta}>
              <div>
                <dt>Название</dt>
                <dd>{info.name}</dd>
              </div>
              <div>
                <dt>Версия</dt>
                <dd>{info.version}</dd>
              </div>
              <div>
                <dt>Платформа</dt>
                <dd>
                  {info.platform} · {info.arch}
                </dd>
              </div>
              <div>
                <dt>Electron</dt>
                <dd>{info.electronVersion}</dd>
              </div>
              <div>
                <dt>Сборка</dt>
                <dd>{info.isPackaged ? 'да' : 'нет'}</dd>
              </div>
            </dl>
          )}
        </Card>
      </div>
    </div>
  );
}
