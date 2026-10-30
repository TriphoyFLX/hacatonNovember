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
        <Badge tone="accent">Каркас режима</Badge>
        <h2>{title}</h2>
        <p>{description}</p>
        <p className={styles.note}>
          Маршрут, навигация и макет уже работают. Интерактивное распознавание и эффекты для
          этого режима будут реализованы на следующем этапе — здесь нет фиктивной логики.
        </p>
        <Link to="/" className={styles.back}>
          На главную
        </Link>
      </Card>
    </div>
  );
}
