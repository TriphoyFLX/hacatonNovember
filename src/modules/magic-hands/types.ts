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
    hint: 'Щелчок → искра-взрыв. Флик указательным → шар. Щипок → огнемёт. Ладонь вверх → факел.',
  },
  {
    id: 'water',
    label: 'Вода',
    accent: '#38bdf8',
    hint: 'Сжми → водоворот. Пальцы льют струи. Раскрой → цунами-кольцо.',
  },
  {
    id: 'lightning',
    label: 'Молния',
    accent: '#fde047',
    hint: 'Сжми → заряд. Щипок → луч. Раскрой → грозовой венец.',
  },
  {
    id: 'energy',
    label: 'Энергия',
    accent: '#f472b6',
    hint: 'Сжми → сингулярность. Два кулака → линк. Двойной каст → ультимейт.',
  },
];
