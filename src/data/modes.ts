export type ModeId =
  | 'face-studio'
  | 'air-instruments'
  | 'magic-hands'
  | 'motion-art'
  | 'ai-personalization';

export interface ModeDefinition {
  id: ModeId;
  title: string;
  shortTitle: string;
  description: string;
  path: string;
  accent: string;
  status: 'planned' | 'ready';
}

export const modes: ModeDefinition[] = [
  {
    id: 'face-studio',
    title: 'Студия лица',
    shortTitle: 'Лицо',
    description:
      'Деформация лица, маски и эффекты по мимике в реальном времени.',
    path: '/face-studio',
    accent: '#8b5cf6',
    status: 'planned',
  },
  {
    id: 'air-instruments',
    title: 'Воздушные инструменты',
    shortTitle: 'Музыка',
    description:
      'Пианино, барабаны и гитара через трекинг рук и генеративный звук.',
    path: '/air-instruments',
    accent: '#38bdf8',
    status: 'planned',
  },
  {
    id: 'magic-hands',
    title: 'Волшебные руки',
    shortTitle: 'Магия',
    description: 'Управляйте огнём, водой, молниями и энергией обеими руками.',
    path: '/magic-hands',
    accent: '#f472b6',
    status: 'planned',
  },
  {
    id: 'motion-art',
    title: 'Искусство движения',
    shortTitle: 'Арт',
    description: 'Рисуйте световые следы и создавайте 3D-формы движениями тела.',
    path: '/motion-art',
    accent: '#34d399',
    status: 'planned',
  },
  {
    id: 'ai-personalization',
    title: 'AI-персонализация',
    shortTitle: 'AI',
    description:
      'Калибровка чувствительности, свои жесты и адаптация всего приложения под вас.',
    path: '/ai-personalization',
    accent: '#a78bfa',
    status: 'planned',
  },
];
