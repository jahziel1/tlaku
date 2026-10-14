import type { CanvasPalette } from './renderer';

const VARS: Record<keyof CanvasPalette, string> = {
  workspace: '--cv-workspace',
  bed: '--cv-bed',
  bedEdge: '--cv-bed-edge',
  gridMinor: '--cv-grid-minor',
  gridMajor: '--cv-grid-major',
  origin: '--cv-origin',
  selection: '--cv-selection',
  selectionFill: '--cv-selection-fill',
  travel: '--cv-travel',
  cut: '--cv-cut',
  engrave: '--cv-engrave',
  overscan: '--cv-overscan',
  head: '--cv-head',
  ink: '--cv-ink',
  guide: '--cv-origin',
  crossing: '--cv-engrave',
  docGuide: '--cv-docguide',
  tab: '--cv-tab',
};

/** Cuánto se multiplica la opacidad de la rejilla según el contraste elegido en Preferencias. */
export const GRID_CONTRAST_FACTOR = { low: 0.55, normal: 1, high: 1.9 } as const;

/** `rgba(r, g, b, a)` con la opacidad multiplicada por `k` (sin pasar de 1); cualquier otro color queda igual. */
export function scaleAlpha(color: string, k: number): string {
  if (k === 1) return color;
  const m = /^rgba\(\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*([\d.]+)\s*\)$/i.exec(color);
  if (!m) return color;
  const a = Math.min(1, Math.max(0, Number(m[4]) * k));
  return `rgba(${m[1]}, ${m[2]}, ${m[3]}, ${Math.round(a * 1000) / 1000})`;
}

/**
 * Lee los colores del lienzo de los tokens CSS vigentes (cambian con el tema). `gridContrast`
 * (Preferencias) hace más o menos visibles las líneas de la rejilla.
 */
export function readCanvasPalette(el: Element = document.documentElement, gridContrast: keyof typeof GRID_CONTRAST_FACTOR = 'normal'): CanvasPalette {
  const css = getComputedStyle(el);
  const out = {} as CanvasPalette;
  for (const key of Object.keys(VARS) as (keyof CanvasPalette)[]) {
    out[key] = css.getPropertyValue(VARS[key]).trim() || '#808080';
  }
  const k = GRID_CONTRAST_FACTOR[gridContrast];
  out.gridMinor = scaleAlpha(out.gridMinor, k);
  out.gridMajor = scaleAlpha(out.gridMajor, k);
  return out;
}
