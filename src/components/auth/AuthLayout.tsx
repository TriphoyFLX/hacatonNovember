import type { ReactNode } from 'react';
import styles from './AuthLayout.module.css';

interface AuthLayoutProps {
  title: string;
  subtitle: string;
  children: ReactNode;
  footer: ReactNode;
}

export function AuthLayout({ title, subtitle, children, footer }: AuthLayoutProps) {
  return (
    <div className={styles.screen}>
      <div className={styles.glowA} aria-hidden />
      <div className={styles.glowB} aria-hidden />

      <div className={styles.panel}>
        <div className={styles.brand}>
          <div className={styles.mark} aria-hidden>
            <span />
          </div>
          <div>
            <strong>FEEL AI</strong>
            <p>Ощути мир. Управляй им по-своему.</p>
          </div>
        </div>

        <div className={styles.copy}>
          <h1>{title}</h1>
          <p>{subtitle}</p>
        </div>

        {children}

        <div className={styles.footer}>{footer}</div>
      </div>
    </div>
  );
}
