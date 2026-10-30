import { useMemo, useState, type FormEvent } from 'react';
import { Link, Navigate, useNavigate } from 'react-router-dom';
import { loginSchema } from '@shared/auth/schemas';
import { AuthLayout } from '@/components/auth/AuthLayout';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { useAuthStore } from '@/stores/authStore';
import styles from './AuthPages.module.css';

export function LoginPage() {
  const navigate = useNavigate();
  const user = useAuthStore((state) => state.user);
  const loading = useAuthStore((state) => state.loading);
  const error = useAuthStore((state) => state.error);
  const login = useAuthStore((state) => state.login);
  const clearError = useAuthStore((state) => state.clearError);

  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});

  const canSubmit = useMemo(() => email.trim().length > 0 && password.length > 0, [email, password]);

  if (user) {
    return <Navigate to="/" replace />;
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    clearError();

    const parsed = loginSchema.safeParse({ email, password });
    if (!parsed.success) {
      const next: Record<string, string> = {};
      for (const issue of parsed.error.issues) {
        const key = String(issue.path[0] ?? 'form');
        if (!next[key]) {
          next[key] = issue.message;
        }
      }
      setFieldErrors(next);
      return;
    }

    setFieldErrors({});
    const ok = await login(parsed.data);
    if (ok) {
      navigate('/', { replace: true });
    }
  }

  return (
    <AuthLayout
      title="Вход"
      subtitle="Войдите в локальный профиль FEEL AI. Данные остаются на этом устройстве."
      footer={
        <>
          Нет аккаунта? <Link to="/register">Создать профиль</Link>
        </>
      }
    >
      <form className={styles.form} onSubmit={(event) => void handleSubmit(event)} noValidate>
        <Input
          label="Email"
          name="email"
          type="email"
          autoComplete="email"
          placeholder="you@example.com"
          value={email}
          onChange={(event) => setEmail(event.target.value)}
          error={fieldErrors.email}
        />
        <Input
          label="Пароль"
          name="password"
          type="password"
          autoComplete="current-password"
          placeholder="••••••••"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          error={fieldErrors.password}
        />

        {error ? <p className={styles.formError}>{error}</p> : null}

        <Button type="submit" size="lg" disabled={loading || !canSubmit}>
          {loading ? 'Входим…' : 'Войти'}
        </Button>
      </form>
    </AuthLayout>
  );
}
