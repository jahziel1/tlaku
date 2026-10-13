import { polylineBez, type BezPath } from '@/lib/bezpath';
import type { LaserApi } from '../api';
import type { CodeKind, CodeResult, CodeSpec, Point } from '../types';

type Lag = <T>(value: () => T) => Promise<T>;

const LINEAR: CodeKind[] = ['code128', 'code39', 'ean13', 'ean8', 'upc_a', 'itf', 'codabar'];

/** Número pseudoaleatorio estable a partir del contenido (no es un código válido: solo la forma). */
function seeded(text: string): () => number {
  let h = 2166136261;
  for (const ch of text) h = Math.imul(h ^ ch.charCodeAt(0), 16777619);
  return () => {
    h = Math.imul(h ^ (h >>> 15), 2246822507);
    h = Math.imul(h ^ (h >>> 13), 3266489909);
    return ((h ^= h >>> 16) >>> 0) / 4294967296;
  };
}

const rect = (x0: number, y0: number, x1: number, y1: number): BezPath =>
  polylineBez([[x0, y0], [x1, y0], [x1, y1], [x0, y1]] as Point[], true);

/**
 * Código aproximado: una rejilla (2D) o barras (1D) pseudoaleatorias con la forma y el tamaño
 * correctos. No se puede escanear; sirve para desarrollar la interfaz sin el motor.
 */
export function mockGenerateCode(spec: CodeSpec): CodeResult {
  if (!spec.data) throw new Error('Escribe el contenido del código');
  if (!(spec.size_mm > 0)) throw new Error('El tamaño del código debe ser mayor que 0 mm');
  if (spec.kind === 'ean13' && !/^\d{12,13}$/.test(spec.data)) throw new Error('EAN-13 necesita 12 dígitos (o 13 con el dígito de control)');
  const rnd = seeded(spec.data + spec.kind);
  const warnings: string[] = [];
  const paths: BezPath[] = [];
  const margin = spec.invert ? spec.margin_mm : 0;
  let modules: number;
  let w = spec.size_mm;
  let h = spec.size_mm;
  if (LINEAR.includes(spec.kind)) {
    modules = Math.max(30, spec.data.length * 11 + 35);
    h = spec.height_mm + (spec.human_readable ? spec.height_mm * 0.4 : 0);
    const m = spec.size_mm / modules;
    let i = 0;
    while (i < modules) {
      const run = 1 + Math.floor(rnd() * 3);
      if (rnd() < 0.5) paths.push(rect(margin + i * m, margin + h - spec.height_mm, margin + Math.min(modules, i + run) * m, margin + h));
      i += run;
    }
  } else {
    modules = spec.kind === 'qr' ? 21 + 4 * Math.floor(spec.data.length / 20) : 18;
    const m = spec.size_mm / modules;
    const finder = (x: number, y: number) => x < 7 && y < 7 || x >= modules - 7 && y < 7 || x < 7 && y >= modules - 7;
    for (let y = 0; y < modules; y++) {
      for (let x = 0; x < modules; x++) {
        if (finder(x, y) ? rnd() < 0.8 : rnd() < 0.5) {
          paths.push(rect(margin + x * m, margin + (modules - 1 - y) * m, margin + (x + 1) * m, margin + (modules - y) * m));
        }
      }
    }
  }
  if (spec.invert) {
    w += 2 * margin;
    h += 2 * margin;
    paths.unshift(rect(0, 0, w, h));
  }
  if (spec.size_mm / modules < 0.25) warnings.push('Cada módulo mide menos de 0,25 mm: es probable que la máquina no pueda grabarlo con nitidez.');
  return { paths, width_mm: w, height_mm: h, modules, warnings };
}

/** Métodos simulados del área `codes`. */
export function mockCodes(lag: Lag): Pick<LaserApi, 'generateCode'> {
  return { generateCode: ({ spec }) => lag(() => mockGenerateCode(spec)) };
}
