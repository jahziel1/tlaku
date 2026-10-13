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

/** Lee los colores del lienzo de los tokens CSS vigentes (cambian con el tema). */
export function readCanvasPalette(el: Element = document.documentElement): CanvasPalette {
  const css = getComputedStyle(el);
  const out = {} as CanvasPalette;
  for (const key of Object.keys(VARS) as (keyof CanvasPalette)[]) {
    out[key] = css.getPropertyValue(VARS[key]).trim() || '#808080';
  }
  return out;
}
