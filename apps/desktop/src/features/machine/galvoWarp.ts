import type { DeviceProfile } from '@/api/types';

/**
 * Vista previa de la deformación de cilindro y cono (M46): copia de `Projection::apply` de `tlaku-pipeline`
 * (`cylinder.rs`). Modelo propio sin verificar con hardware. Las pruebas de `galvoWarp.test.ts` fijan los mismos
 * números calculados a mano que `hand_computed_reference_points` y compañeras del motor: si una de las dos copias
 * cambia, la otra prueba se rompe.
 *
 * Con la coordenada a lo largo de la curvatura `c` en mm de arco y la del eje `a` en mm, ambas desde el centro de la
 * cama: `theta = c / R`, `h = D ± r (1 - cos theta)` (más para convexo, menos para cóncavo), `s = D / h`,
 * `c' = r sin(theta) s`, `a' = a s`, con `r = R k(a)` el radio local (`k` = 1 sin cono). El ángulo se recorta al
 * horizonte: 90 grados, o `acos(r / (r + D))` en un cilindro convexo.
 */

export type Pt = [number, number];

type Galvo = DeviceProfile['galvo'];

const HORIZON_EPS = 1e-9;

/** Resultado de proyectar un punto: dónde cae en el campo plano y si el espejo lo ve (si no, se recortó al borde). */
export interface Warped {
  point: Pt;
  visible: boolean;
}

/** ¿Está activa alguna deformación con parámetros válidos? */
export function warpActive(g: Galvo): boolean {
  return warpOf(g) !== null;
}

interface Warp {
  cyl: { radius: number; distance: number; concave: boolean; axis: 'x' | 'y' } | null;
  cone: { axis: 'x' | 'y'; dNeg: number; dPos: number; half: number } | null;
}

const positive = (n: number) => Number.isFinite(n) && n > 0;

function warpOf(g: Galvo): Warp | null {
  const c = g.cylinder;
  const k = g.taper;
  const cyl = c.enabled && positive(c.diameter_mm) && positive(c.mirror_distance_mm) ? { radius: c.diameter_mm / 2, distance: c.mirror_distance_mm, concave: c.concave, axis: c.axis } : null;
  const cone = k.enabled && positive(k.top_diameter_mm) && positive(k.bottom_diameter_mm) && positive(k.length_mm)
    ? { axis: k.axis, dNeg: k.top_at_positive ? k.bottom_diameter_mm : k.top_diameter_mm, dPos: k.top_at_positive ? k.top_diameter_mm : k.bottom_diameter_mm, half: k.length_mm / 2 }
    : null;
  if (c.enabled && !cyl) return null;
  if (k.enabled && !cone) return null;
  if (cyl && cone && cyl.axis !== cone.axis) return null;
  return cyl || cone ? { cyl, cone } : null;
}

/** El punto del campo plano que marca el punto `p` del diseño (mm); `null` si no hay deformación válida. */
export function warpPoint(g: Galvo, center: Pt, p: Pt): Warped | null {
  const w = warpOf(g);
  if (!w) return null;
  const axis = w.cyl?.axis ?? w.cone?.axis ?? 'x';
  const [dx, dy] = [p[0] - center[0], p[1] - center[1]];
  const [a, c] = axis === 'x' ? [dx, dy] : [dy, dx];
  const k = w.cone ? ratio(w.cone, a) : 1;
  let a2 = a;
  let c2 = c * k;
  let visible = true;
  if (w.cyl) {
    const { radius: rRef, distance: d, concave } = w.cyl;
    const r = rRef * k;
    const raw = c / rRef;
    const limit = concave ? Math.PI / 2 : Math.min(Math.PI / 2, Math.acos(r / (r + d)));
    const theta = Math.min(limit, Math.max(-limit, raw));
    const drop = r * (1 - Math.cos(theta));
    const h = Math.max(concave ? d - drop : d + drop, d * 1e-3);
    const s = d / h;
    a2 = a * s;
    c2 = r * Math.sin(theta) * s;
    visible = Math.abs(raw - theta) <= HORIZON_EPS;
  }
  const point: Pt = axis === 'x' ? [center[0] + a2, center[1] + c2] : [center[0] + c2, center[1] + a2];
  return { point, visible };
}

function ratio(cone: NonNullable<Warp['cone']>, a: number): number {
  const t = Math.min(1, Math.max(0, (a + cone.half) / (2 * cone.half)));
  return (cone.dNeg + (cone.dPos - cone.dNeg) * t) / Math.max(cone.dNeg, cone.dPos);
}

/** Un tramo de la rejilla deformada: puntos en mm del campo plano y si cada tramo es visible desde el espejo. */
export interface WarpLine {
  points: Pt[];
  /** Todos los puntos del tramo son visibles. */
  visible: boolean;
}

/**
 * La rejilla de referencia deformada: líneas cada `spacing` mm que cubren `[0, bed]`, muestreadas cada `step` mm y
 * cortadas donde pasan del horizonte (los puntos no visibles forman tramos aparte). Vacía si no hay deformación.
 */
export function warpGrid(g: Galvo, bed: Pt, spacing = 10, step = 2): WarpLine[] {
  if (!warpActive(g) || !(spacing > 0) || !(step > 0)) return [];
  const center: Pt = [bed[0] / 2, bed[1] / 2];
  const lines: WarpLine[] = [];
  const trace = (at: (t: number) => Pt, len: number) => {
    let cur: WarpLine | null = null;
    let last: Pt | null = null;
    const n = Math.max(1, Math.ceil(len / step));
    for (let i = 0; i <= n; i++) {
      const w = warpPoint(g, center, at((i / n) * len));
      if (!w) return;
      if (!cur || cur.visible !== w.visible) {
        cur = { points: last ? [last] : [], visible: w.visible };
        lines.push(cur);
      }
      cur.points.push(w.point);
      last = w.point;
    }
  };
  for (let x = 0; x <= bed[0] + 1e-9; x += spacing) trace((t) => [x, t], bed[1]);
  for (let y = 0; y <= bed[1] + 1e-9; y += spacing) trace((t) => [t, y], bed[0]);
  return lines.filter((l) => l.points.length > 1);
}
