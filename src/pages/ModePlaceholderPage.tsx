import { Link } from 'react-router-dom';
import { Badge } from '@/components/ui/Badge';
import { Card } from '@/components/ui/Card';
import styles from './ModePlaceholderPage.module.css';

interface ModePlaceholderPageProps {
  title: string;
  description: string;
  accent: string;
}

export function ModePlaceholderPage({ title, description, accent }: ModePlaceholderPageProps) {
  return (
    <div className={styles.page}>
      <Card className={styles.card}>
        <div className={styles.accent} style={{ background: accent }} aria-hidden />
        <Badge tone="accent">Mode scaffold</Badge>
        <h2>{title}</h2>
        <p>{description}</p>
        <p className={styles.note}>
          Route, navigation, and layout are live. Interactive vision and effects for this mode
          will be implemented in a later stage — no fake runtime is wired here.
        </p>
        <Link to="/" className={styles.back}>
          Back to Home
        </Link>
      </Card>
    </div>
  );
}
