import type { Point, SavedPosition, StatusReport } from '@/api/types';

/** Máximo de posiciones guardadas por perfil. */
export const MAX_POSITIONS = 20;

const round3 = (n: number) => Math.round(n * 1000) / 1000;

/**
 * Agrega o reemplaza (mismo nombre) una posición. Devuelve `null` si el nombre está vacío, la
 * posición no es finita o ya hay `MAX_POSITIONS` y la nueva no reemplaza a ninguna.
 */
export function upsertPosition(list: readonly SavedPosition[], name: string, x: number, y: number): SavedPosition[] | null {
  const clean = name.trim();
  if (clean === '' || !Number.isFinite(x) || !Number.isFinite(y)) return null;
  const next: SavedPosition = { name: clean, x: round3(x), y: round3(y) };
  const at = list.findIndex((p) => p.name === clean);
  if (at < 0) return list.length >= MAX_POSITIONS ? null : [...list, next];
  return list.map((p, i) => (i === at ? next : p));
}

/**
 * Las posiciones se guardan en coordenadas de la máquina (no cambian al mover el origen de
 * trabajo); `$J=G90` va en coordenadas de trabajo. La diferencia es el desplazamiento `WCO` que
 * sale del último reporte de estado.
 */
export function workTarget(saved: Pick<SavedPosition, 'x' | 'y'>, status: Pick<StatusReport, 'machine_pos' | 'work_pos'>): Point {
  return [round3(saved.x - (status.machine_pos[0] - status.work_pos[0])), round3(saved.y - (status.machine_pos[1] - status.work_pos[1]))];
}
