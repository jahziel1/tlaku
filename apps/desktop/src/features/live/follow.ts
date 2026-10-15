import { MoveClass } from '@/features/preview/model';
import { isLaserMove, pointAt, posAtTime, type RouteShape } from './route';
import type { LiveTracker, TrackerMode } from './tracker';

/**
 * Lo que se dibuja en cada cuadro, calculado a partir de la ruta y del seguidor (`LiveTracker`).
 * Sin DOM ni store: lo usan igual la ventana principal (`LiveService.frame`) y la ventana aparte
 * (`remoteSource.ts`), que recibe solo la ruta y los reportes.
 */
export interface LiveFrame {
  /** Tiempo de trabajo dibujado (s) y su fracción del total. */
  t: number;
  fraction: number;
  /** Cabezal (mm del documento). */
  x: number;
  y: number;
  /** Movimiento de la ruta donde está y fracción recorrida de él. */
  move: number;
  frac: number;
  emitting: boolean;
  mode: TrackerMode;
}

/** ¿El láser (o el husillo, en máquinas de fresado) está emitiendo en el instante `t` de la ruta? */
export function emittingAt(route: RouteShape, spindle: number, t: number): boolean {
  const k = posAtTime(route, t).move;
  const laser = isLaserMove(route, k);
  const travel = route.model.cls[k] === MoveClass.Travel;
  return laser || (spindle > 0 && !travel && route.model.cls[k] !== MoveClass.Overscan);
}

/** El cuadro en el instante `wallMs` (ms del reloj); `null` si todavía no llegó ningún reporte. */
export function frameAt(route: RouteShape, tracker: LiveTracker, spindle: number, wallMs: number): LiveFrame | null {
  if (!tracker.started) return null;
  const t = tracker.step(wallMs / 1000);
  const pos = posAtTime(route, t);
  const [x, y] = pointAt(route, pos.move, pos.frac);
  return {
    t,
    fraction: route.total > 0 ? t / route.total : 0,
    x,
    y,
    move: pos.move,
    frac: pos.frac,
    emitting: tracker.mode === 'running' && emittingAt(route, spindle, t),
    mode: tracker.mode,
  };
}
