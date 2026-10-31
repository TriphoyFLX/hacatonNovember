export type ElementId = 'fire' | 'water' | 'lightning' | 'energy';

export interface ElementDef {
  id: ElementId;
  label: string;
  accent: string;
  hint: string;
}

export const ELEMENTS: ElementDef[] = [
  {
    id: 'fire',
    label: 'Огонь',
    accent: '#fb923c',
    hint: 'Щелчок / флик / щипок — огонь. Расширь глаза — лазер, прищурься — выкл.',
  },
  {
    id: 'water',
    label: 'Вода',
    accent: '#38bdf8',
    hint: 'Расширь глаза — водяные лазеры по взгляду. Прищур гасит.',
  },
  {
    id: 'lightning',
    label: 'Молния',
    accent: '#fde047',
    hint: 'Широко открой глаза — непрерывный разряд. Прищур = стоп.',
  },
  {
    id: 'energy',
    label: 'Энергия',
    accent: '#f472b6',
    hint: 'Расширь глаза — лазер держится. Взгляд целится, прищур гасит.',
  },
];
