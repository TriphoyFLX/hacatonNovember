import { Navigate, Outlet, useLocation } from 'react-router-dom';
import { useAuthStore } from '@/stores/authStore';
import styles from './RequireAuth.module.css';

export function RequireAuth() {
  const location = useLocation();
  const user = useAuthStore((state) => state.user);
  const bootstrapped = useAuthStore((state) => state.bootstrapped);

  if (!bootstrapped) {
    return (
      <div className={styles.boot}>
        <div className={styles.card}>
          <div className={styles.spinner} aria-hidden />
          <p>Проверяем сессию…</p>
        </div>
      </div>
    );
  }

  if (!user) {
    return <Navigate to="/login" replace state={{ from: location.pathname }} />;
  }

  return <Outlet />;
}
