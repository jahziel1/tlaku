import type { TileStatus } from './session';
import { useTilingUi, type TilingUiState } from './store';
import type { TileSpec } from './types';

/** Un tramo tal como lo dibuja el lienzo: rectángulo del documento, número y estado. */
export interface SceneTile {
  rect: TileSpec['rect'];
  /** Número que se muestra (1 = primero). */
  label: number;
  state: TileStatus | 'preview';
  current: boolean;
}

export interface SceneTiles {
  tiles: SceneTile[];
}

let cacheKey: unknown[] = [];
let cacheValue: SceneTiles | null = null;

/**
 * Los tramos que el lienzo debe dibujar: con el asistente abierto, los del plan (vista previa) o,
 * en plena sesión, los de la sesión con su avance. Memoizado por identidad del estado.
 */
export function selectSceneTiles(s: TilingUiState = useTilingUi.getState()): SceneTiles | null {
  if (!s.open) return null;
  const key = [s.session, s.plan, s.phase];
  if (key.length === cacheKey.length && key.every((v, i) => v === cacheKey[i])) return cacheValue;
  cacheKey = key;
  if (s.session && s.phase !== 'config') {
    const { session } = s;
    cacheValue = {
      tiles: session.specs.map((spec, i) => ({ rect: spec.rect, label: i + 1, state: session.status[i]!, current: i === session.current })),
    };
  } else if (s.plan?.ok) {
    cacheValue = {
      tiles: s.plan.plan.tiles.map((p, i) => ({ rect: p.spec.rect, label: i + 1, state: 'preview', current: false })),
    };
  } else cacheValue = null;
  return cacheValue;
}
