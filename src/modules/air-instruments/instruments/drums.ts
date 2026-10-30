import { createDrumZones } from '../zones';

export const drumsHint = 'Бейте ладонью по зонам — сила движения меняет громкость';

export function getDrumZones() {
  return createDrumZones();
}
