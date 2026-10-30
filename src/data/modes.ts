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
    title: 'Face Studio',
    shortTitle: 'Face',
    description: 'Warp, masks, and expression-driven effects on your face in real time.',
    path: '/face-studio',
    accent: '#8b5cf6',
    status: 'planned',
  },
  {
    id: 'air-instruments',
    title: 'Air Instruments',
    shortTitle: 'Instruments',
    description: 'Play piano, drums, and guitar through hand tracking and generative audio.',
    path: '/air-instruments',
    accent: '#38bdf8',
    status: 'planned',
  },
  {
    id: 'magic-hands',
    title: 'Magic Hands',
    shortTitle: 'Magic',
    description: 'Shape fire, water, lightning, and energy fields with both hands.',
    path: '/magic-hands',
    accent: '#f472b6',
    status: 'planned',
  },
  {
    id: 'motion-art',
    title: 'Motion Art',
    shortTitle: 'Art',
    description: 'Paint light trails and sculpt abstract forms with body motion.',
    path: '/motion-art',
    accent: '#34d399',
    status: 'planned',
  },
  {
    id: 'ai-personalization',
    title: 'AI Personalization',
    shortTitle: 'AI',
    description: 'Calibrate sensitivity, learn custom gestures, and adapt the whole app to you.',
    path: '/ai-personalization',
    accent: '#a78bfa',
    status: 'planned',
  },
];
