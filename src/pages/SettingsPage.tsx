import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { Card } from '@/components/ui/Card';
import { useAppInfo } from '@/hooks/useAppInfo';
import { useAppStore } from '@/stores/appStore';
import styles from './SettingsPage.module.css';

export function SettingsPage() {
  const { info, loading } = useAppInfo();
  const userName = useAppStore((state) => state.userName);
  const setUserName = useAppStore((state) => state.setUserName);
  const theme = useAppStore((state) => state.theme);
  const toggleTheme = useAppStore((state) => state.toggleTheme);

  return (
    <div className={styles.page}>
      <div className={styles.header}>
        <h2>Settings</h2>
        <p>Local preferences for the FEEL AI shell. Persistence arrives with SQLite profiles.</p>
      </div>

      <div className={styles.grid}>
        <Card>
          <Badge>Profile</Badge>
          <h3>Display name</h3>
          <label className={styles.field}>
            <span>Name</span>
            <input
              value={userName}
              onChange={(event) => setUserName(event.target.value || 'Creator')}
              maxLength={32}
            />
          </label>
          <Button variant="secondary" onClick={toggleTheme}>
            Switch theme · current {theme}
          </Button>
        </Card>

        <Card>
          <Badge tone="accent">Runtime</Badge>
          <h3>Application info</h3>
          {loading ? (
            <p className={styles.muted}>Reading Electron runtime…</p>
          ) : (
            <dl className={styles.meta}>
              <div>
                <dt>Name</dt>
                <dd>{info.name}</dd>
              </div>
              <div>
                <dt>Version</dt>
                <dd>{info.version}</dd>
              </div>
              <div>
                <dt>Platform</dt>
                <dd>
                  {info.platform} · {info.arch}
                </dd>
              </div>
              <div>
                <dt>Electron</dt>
                <dd>{info.electronVersion}</dd>
              </div>
              <div>
                <dt>Packaged</dt>
                <dd>{info.isPackaged ? 'yes' : 'no'}</dd>
              </div>
            </dl>
          )}
        </Card>
      </div>
    </div>
  );
}
