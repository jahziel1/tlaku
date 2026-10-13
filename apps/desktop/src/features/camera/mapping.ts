/**
 * De qué píxel de la imagen corregida a qué punto de la cama (mm de documento), según cómo esté
 * montada la cámara: fija sobre la cama (alineación de 4 puntos o marcas, con la corrección por
 * grosor del material) o en el cabezal (calibración del cabezal y posición actual del láser).
 */
import type { AppState } from '@/store/types';
import { selectHeadDoc } from '@/store/machine';
import { applyH, type Mat3, type Vec2 } from './geometry';
import { headPointMm, headTileToBed } from './head';
import { toBedAt } from './height';
import { correctedSpace } from './session';

export type PxToMm = (p: Vec2) => Vec2;

export type MappingProblem = 'noSpace' | 'noAlignment' | 'stale' | 'noHeadCalibration' | 'staleHead' | 'noHeadPosition';

export type Mapping =
  | {
      ok: true;
      kind: 'lid' | 'head';
      /** Píxel de la imagen corregida (espacio de la calibración) -> mm. */
      map: PxToMm;
      /** La misma transformación como matriz. */
      matrix: Mat3;
      /** Tamaño del espacio de píxeles en que valen `map` y `matrix`. */
      space: [number, number];
      /** Posición del láser al medir (solo con la cámara en el cabezal). */
      laser: Vec2 | null;
    }
  | { ok: false; reason: MappingProblem };

type Source = Pick<AppState, 'cameraSetup' | 'cameraStreamSize' | 'connection' | 'status' | 'devices' | 'activeDeviceId'>;

/** Transformación vigente de la cámara con la imagen que se ve ahora. `space` sale de la sesión. */
export function currentMapping(s: Source, space: [number, number] | null = correctedSpace()): Mapping {
  if (!space) return { ok: false, reason: 'noSpace' };
  const { alignment, prefs, head } = s.cameraSetup;
  if (head?.enabled) {
    const cal = head.calibration;
    if (!cal) return { ok: false, reason: 'noHeadCalibration' };
    if (cal.image_size[0] !== space[0] || cal.image_size[1] !== space[1]) return { ok: false, reason: 'staleHead' };
    const laser = selectHeadDoc(s);
    if (!laser) return { ok: false, reason: 'noHeadPosition' };
    return { ok: true, kind: 'head', map: (p) => headPointMm(cal, laser, p), matrix: headTileToBed(cal, laser), space, laser };
  }
  if (!alignment) return { ok: false, reason: 'noAlignment' };
  if (alignment.image_size[0] !== space[0] || alignment.image_size[1] !== space[1]) return { ok: false, reason: 'stale' };
  const matrix = toBedAt(alignment, prefs.material_height_mm ?? 0);
  return { ok: true, kind: 'lid', map: (p) => applyH(matrix, p), matrix, space, laser: null };
}

/** Texto de i18n para cada problema. */
export const MAPPING_PROBLEM_KEY = {
  noSpace: 'camera.mapping.noSpace',
  noAlignment: 'camera.mapping.noAlignment',
  stale: 'camera.mapping.stale',
  noHeadCalibration: 'camera.mapping.noHeadCalibration',
  staleHead: 'camera.mapping.staleHead',
  noHeadPosition: 'camera.mapping.noHeadPosition',
} as const;

/** Matriz de una foto de `frame` píxeles cuando la matriz vale para un espacio de `space` píxeles. */
export function matrixForFrame(m: Mat3, space: readonly [number, number], frame: readonly [number, number]): Mat3 {
  const kx = space[0] / frame[0];
  const ky = space[1] / frame[1];
  return [
    [m[0][0] * kx, m[0][1] * ky, m[0][2]],
    [m[1][0] * kx, m[1][1] * ky, m[1][2]],
    [m[2][0] * kx, m[2][1] * ky, m[2][2]],
  ];
}
