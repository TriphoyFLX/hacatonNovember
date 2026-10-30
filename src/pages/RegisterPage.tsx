import { useMemo, useState, type FormEvent } from 'react';
import { Link, Navigate, useNavigate } from 'react-router-dom';
import { registerSchema } from '@shared/auth/schemas';
import { AuthLayout } from '@/components/auth/AuthLayout';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { useAuthStore } from '@/stores/authStore';
import styles from './AuthPages.module.css';

export function RegisterPage() {
  const navigate = useNavigate();
  const user = useAuthStore((state) => state.user);
  const loading = useAuthStore((state) => state.loading);
  const error = useAuthStore((state) => state.error);
  const register = useAuthStore((state) => state.register);
  const clearError = useAuthStore((state) => state.clearError);

  const [displayName, setDisplayName] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});

  const canSubmit = useMemo(
    () =>
      displayName.trim().length > 1 &&
      email.trim().length > 0 &&
      password.length >= 8 &&
      confirmPassword.length >= 8,
    [confirmPassword, displayName, email, password],
  );

  if (user) {
    return <Navigate to="/" replace />;
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    clearError();

    const parsed = registerSchema.safeParse({
      displayName,
      email,
      password,
      confirmPassword,
    });

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
    const ok = await register(parsed.data);
    if (ok) {
      navigate('/', { replace: true });
    }
  }

  return (
    <AuthLayout
      title="Регистрация"
      subtitle="Создайте локальный профиль. Пароль хранится в виде хеша только на вашем устройстве."
      footer={
        <>
          Уже есть аккаунт? <Link to="/login">Войти</Link>
        </>
      }
    >
      <form className={styles.form} onSubmit={(event) => void handleSubmit(event)} noValidate>
        <Input
          label="Имя"
          name="displayName"
          autoComplete="nickname"
          placeholder="Как к вам обращаться"
          value={displayName}
          onChange={(event) => setDisplayName(event.target.value)}
          error={fieldErrors.displayName}
        />
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
          autoComplete="new-password"
          placeholder="Минимум 8 символов"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          error={fieldErrors.password}
        />
        <Input
          label="Повтор пароля"
          name="confirmPassword"
          type="password"
          autoComplete="new-password"
          placeholder="Повторите пароль"
          value={confirmPassword}
          onChange={(event) => setConfirmPassword(event.target.value)}
          error={fieldErrors.confirmPassword}
        />

        {error ? <p className={styles.formError}>{error}</p> : null}

        <Button type="submit" size="lg" disabled={loading || !canSubmit}>
          {loading ? 'Создаём…' : 'Создать профиль'}
        </Button>
      </form>
    </AuthLayout>
  );
}
