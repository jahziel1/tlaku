/**
 * Engranajes SIMULADOS para el modo mock (`crates/tlaku-parts/src/gears` es el real). Misma
 * matemática del evolvente (radio base `r·cos α`, semiángulo `s/2r + inv α − inv φ`), pero sin
 * filete, sin chavetero ni eje en D (el agujero sale redondo), sin aligeramiento y con pocos
 * puntos por flanco: sirve para probar el diálogo, no para cortar. La verdad es el motor Rust.
 */
import type { PathDto } from '@/bindings/PathDto';

type P = [number, number];
type Params = Record<string, unknown>;
export interface GearPart { key: string; name: string; cuts: PathDto[]; engraves: PathDto[]; width: number; height: number }
export interface GearGenerated { parts: GearPart[]; warnings: string[] }
type Fail = (field: string | null, message: string) => Error;

const num = (p: Params, k: string, d: number) => (typeof p[k] === 'number' ? (p[k] as number) : d);
const str = (p: Params, k: string, d: string) => (typeof p[k] === 'string' ? (p[k] as string) : d);

const inv = (phi: number) => Math.tan(phi) - phi;
const polar = (r: number, a: number): P => [r * Math.cos(a), r * Math.sin(a)];

interface Prof { z: number; m: number; alpha: number; ra: number; rf: number; s: number }

/** Contorno de un engranaje (diente 0 centrado en el ángulo `rot`). */
function outline(g: Prof, rot: number): P[] {
  const r = (g.m * g.z) / 2;
  const rb = r * Math.cos(g.alpha);
  const gamma = g.s / (2 * r);
  const half = (R: number) => gamma + inv(g.alpha) - inv(Math.acos(Math.min(1, rb / Math.max(R, rb))));
  const right: P[] = [];
  const N = 10;
  if (g.rf < rb) right.push(polar(g.rf, -half(rb)));
  const t0 = g.rf > rb ? Math.sqrt(g.rf ** 2 - rb ** 2) / rb : 0;
  const t1 = Math.sqrt(g.ra ** 2 - rb ** 2) / rb;
  for (let i = 0; i <= N; i++) {
    const t = t0 + ((t1 - t0) * i) / N;
    const R = rb * Math.sqrt(1 + t * t);
    right.push(polar(R, -half(R)));
  }
  const pts: P[] = [...right, ...[...right].reverse().map(([x, y]): P => [x, -y])];
  const out: P[] = [];
  for (let k = 0; k < g.z; k++) {
    const a = rot + (2 * Math.PI * k) / g.z;
    const [c, s] = [Math.cos(a), Math.sin(a)];
    for (const [x, y] of pts) out.push([x * c - y * s, x * s + y * c]);
  }
  return out;
}

const circle = (cx: number, cy: number, r: number, n = 48): P[] => Array.from({ length: n }, (_, i) => [cx + r * Math.cos((2 * Math.PI * i) / n), cy + r * Math.sin((2 * Math.PI * i) / n)] as P);
const at = (pts: P[], dx: number, dy: number): PathDto => ({ points: pts.map(([x, y]): P => [x + dx, y + dy]), closed: true });

function rack(zr: number, m: number, alpha: number, c: number, s: number, base: number): P[] {
  const p = Math.PI * m;
  const tan = Math.tan(alpha);
  const ht = s / 2 - m * tan;
  const hr = s / 2 + (m + c) * tan;
  const pts: P[] = [[-p / 2, -(m + c) - base], [(zr - 0.5) * p, -(m + c) - base], [(zr - 0.5) * p, -(m + c)]];
  for (let i = zr - 1; i >= 0; i--) pts.push([i * p + hr, -(m + c)], [i * p + ht, m], [i * p - ht, m], [i * p - hr, -(m + c)]);
  pts.push([-p / 2, -(m + c)]);
  return pts;
}

function bounds(paths: PathDto[]) {
  const xs = paths.flatMap((p) => p.points.map((q) => q[0]));
  const ys = paths.flatMap((p) => p.points.map((q) => q[1]));
  return { x0: Math.min(...xs), y0: Math.min(...ys), x1: Math.max(...xs), y1: Math.max(...ys) };
}

function part(key: string, name: string, cuts: PathDto[]): GearPart {
  const b = bounds(cuts);
  const move = (p: PathDto): PathDto => ({ ...p, points: p.points.map(([x, y]): P => [x - b.x0, y - b.y0]) });
  return { key, name, cuts: cuts.map(move), engraves: [], width: b.x1 - b.x0, height: b.y1 - b.y0 };
}

export function gearsParts(p: Params, fail: Fail): GearGenerated {
  const kind = str(p, 'kind', 'single');
  const m = str(p, 'size_by', 'module') === 'module' ? num(p, 'module_mm', 2) : 25.4 / num(p, 'diametral_pitch', 12);
  const alpha = ({ deg14_5: 14.5, deg20: 20, deg25: 25 }[str(p, 'pressure_angle', 'deg20')] ?? 20) * (Math.PI / 180);
  const c = num(p, 'clearance', 0.25) * m;
  const b = num(p, 'backlash_mm', 0.1);
  const s = (Math.PI * m) / 2 - b / 2;
  const z1 = Math.round(num(p, 'teeth', 20));
  const z2 = Math.round(num(p, 'teeth2', 40));
  const zr = Math.round(num(p, 'rack_teeth', 10));
  const ext = (z: number): Prof => ({ z, m, alpha, ra: m * (z / 2 + 1), rf: m * (z / 2 - 1) - c, s });
  const warnings = ['Modo simulado: sin filete, chavetero, eje en D ni aligeramiento; el agujero sale redondo.'];
  const min = Math.round(2 / Math.sin(alpha) ** 2);
  for (const z of kind === 'rack' ? [] : [z1, ...(kind === 'pair' ? [z2] : [])]) {
    if (z < min) warnings.push(`Con ${z} dientes y ${(alpha * 180 / Math.PI).toFixed(1)}° hay socavado: usa al menos ${min} dientes o sube el ángulo de presión.`);
  }
  const bore = (d: number, cx: number, cy: number): PathDto[] => (d > 0 ? [{ points: circle(cx, cy, d / 2), closed: true }] : []);
  const meshed = str(p, 'layout', 'meshed') === 'meshed';
  const join = (key: string, name: string, sets: { key: string; name: string; cuts: PathDto[] }[]): GearPart[] =>
    meshed ? [part(key, name, sets.flatMap((x) => x.cuts))] : sets.map((x) => part(x.key, x.name, x.cuts));

  if (kind === 'single') {
    return { parts: [part('gear', `Engranaje de ${z1} dientes`, [at(outline(ext(z1), 0), 0, 0), ...bore(num(p, 'bore_mm', 5), 0, 0)])], warnings };
  }
  if (kind === 'pair') {
    const dx = meshed ? (m * (z1 + z2)) / 2 : 0;
    return {
      parts: join('pair', `Par de engranajes de ${z1} y ${z2} dientes`, [
        { key: 'gear_1', name: `Engranaje de ${z1} dientes`, cuts: [at(outline(ext(z1), 0), 0, 0), ...bore(num(p, 'bore_mm', 5), 0, 0)] },
        { key: 'gear_2', name: `Engranaje de ${z2} dientes`, cuts: [at(outline(ext(z2), Math.PI - Math.PI / z2), dx, 0), ...bore(num(p, 'bore2_mm', 5), dx, 0)] },
      ]),
      warnings,
    };
  }
  if (kind === 'ring') {
    if (z2 <= z1) throw fail('teeth2', `La corona debe tener más dientes que el piñón (${z1}).`);
    const r2 = (m * z2) / 2;
    const hole: Prof = { z: z2, m, alpha, ra: r2 + m + c, rf: r2 - m, s: (Math.PI * m) / 2 + b / 2 };
    const dx = meshed ? (m * (z2 - z1)) / 2 : 0;
    return {
      parts: join('ring_pair', `Piñón de ${z1} dientes y corona de ${z2}`, [
        { key: 'pinion', name: `Piñón de ${z1} dientes`, cuts: [at(outline(ext(z1), 0), dx, 0), ...bore(num(p, 'bore_mm', 5), dx, 0)] },
        { key: 'ring', name: `Corona de ${z2} dientes`, cuts: [{ points: circle(0, 0, hole.ra + num(p, 'ring_rim_mm', 6), 120), closed: true }, at(outline(hole, 0), 0, 0)] },
      ]),
      warnings,
    };
  }
  const rk = rack(zr, m, alpha, c, s, num(p, 'rack_base_mm', 8));
  if (kind === 'rack') return { parts: [part('rack', `Cremallera de ${zr} dientes`, [at(rk, 0, 0)])], warnings };
  const r1 = (m * z1) / 2;
  const shiftX = meshed ? -Math.floor(zr / 2) * Math.PI * m : 0;
  return {
    parts: join('rack_pair', `Piñón de ${z1} dientes y cremallera de ${zr}`, [
      { key: 'pinion', name: `Piñón de ${z1} dientes`, cuts: [at(outline(ext(z1), -Math.PI / 2 - Math.PI / z1), 0, 0), ...bore(num(p, 'bore_mm', 5), 0, 0)] },
      { key: 'rack', name: `Cremallera de ${zr} dientes`, cuts: [at(rk, shiftX, meshed ? -r1 : 0)] },
    ]),
    warnings,
  };
}
