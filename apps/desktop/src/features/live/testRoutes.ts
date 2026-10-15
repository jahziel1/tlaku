import type { JobPreview } from '@/api/types';

/** Rutas sintéticas para las pruebas de la vista en vivo (no se usa en la app). */

export type Seg = readonly [kind: 0 | 1, x: number, y: number, power: number];

/** Vista previa a partir de movimientos `[kind, x, y, power]`: tiempos a velocidad constante (corte 10 mm/s, vacío 100 mm/s). */
export function previewOf(segs: readonly Seg[], layers: { color: string; mode: string; from: number }[] = [{ color: '#ff0000', mode: 'line', from: 0 }]): JobPreview {
  const moves: number[] = [];
  const times: number[] = [];
  let x = 0;
  let y = 0;
  let t = 0;
  for (const [kind, nx, ny, power] of segs) {
    const d = Math.hypot(nx - x, ny - y);
    t += d / (kind === 0 ? 100 : 10);
    moves.push(kind, nx, ny, power);
    times.push(t);
    x = nx;
    y = ny;
  }
  const secondsOf = (k: number) => {
    const a = layers[k]!.from;
    const b = k + 1 < layers.length ? layers[k + 1]!.from : segs.length;
    return (times[b - 1] ?? 0) - (a > 0 ? times[a - 1]! : 0);
  };
  return {
    layers: layers.map((l, k) => ({ color: l.color, mode: l.mode, seconds: secondsOf(k), summary: '', travel_saved_pct: null, explanations: [] })),
    moves,
    move_times: times,
    total_s: t,
    bounds: [[0, 0], [100, 100]],
    order_note: null,
    warnings: [],
  };
}

/** Cuadrado de `size` mm cortado desde (10, 10): viaje a la esquina y cuatro lados. */
export function squareCut(size = 100): JobPreview {
  return previewOf([
    [0, 10, 10, 0],
    [1, 10 + size, 10, 80],
    [1, 10 + size, 10 + size, 80],
    [1, 10, 10 + size, 80],
    [1, 10, 10, 80],
  ]);
}

/** Relleno en serpentina: `rows` pasadas de `width` mm separadas `pitch` mm, con overscan de 2 mm. */
export function rasterFill(rows: number, width = 50, pitch = 0.1): JobPreview {
  const segs: Seg[] = [[0, -2, 0, 0]];
  for (let r = 0; r < rows; r++) {
    const left = r % 2 === 0;
    const y = r * pitch;
    if (r > 0) segs.push([0, left ? -2 : width + 2, y, 0]);
    segs.push([1, left ? 0 : width, y, 0]); // entrada (overscan)
    segs.push([1, left ? width : 0, y, 60]);
    segs.push([1, left ? width + 2 : -2, y, 0]); // salida (overscan)
  }
  return previewOf(segs, [{ color: '#2e86c1', mode: 'fill', from: 0 }]);
}
