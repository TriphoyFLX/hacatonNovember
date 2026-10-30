import { NavLink } from 'react-router-dom';
import { modes } from '@/data/modes';
import styles from './Sidebar.module.css';

const navItems = [
  { to: '/', label: 'Главная', icon: '◇' },
  ...modes.map((mode) => ({
    to: mode.path,
    label: mode.title,
    icon: mode.shortTitle.slice(0, 1),
  })),
  { to: '/settings', label: 'Настройки', icon: '⚙' },
];

export function Sidebar() {
  return (
    <aside className={styles.sidebar}>
      <div className={styles.brand}>
        <div className={styles.mark} aria-hidden>
          <span />
        </div>
        <div className={styles.brandText}>
          <strong>FEEL AI</strong>
          <span>Ощути мир</span>
        </div>
      </div>

      <nav className={styles.nav} aria-label="Основная навигация">
        {navItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === '/'}
            className={({ isActive }) =>
              [styles.link, isActive ? styles.active : undefined].filter(Boolean).join(' ')
            }
          >
            <span className={styles.icon} aria-hidden>
              {item.icon}
            </span>
            <span className={styles.label}>{item.label}</span>
          </NavLink>
        ))}
      </nav>

      <div className={styles.footer}>
        <p>Локальное распознавание. Приватность по умолчанию.</p>
      </div>
    </aside>
  );
}
