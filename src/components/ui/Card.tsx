import type { HTMLAttributes, ReactNode } from 'react';
import styles from './Card.module.css';

interface CardProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
  padded?: boolean;
  interactive?: boolean;
}

export function Card({
  children,
  padded = true,
  interactive = false,
  className,
  ...props
}: CardProps) {
  const classes = [
    styles.card,
    padded ? styles.padded : undefined,
    interactive ? styles.interactive : undefined,
    className,
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <div className={classes} {...props}>
      {children}
    </div>
  );
}
