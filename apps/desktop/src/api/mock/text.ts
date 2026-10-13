import { flattenBez, polylineBez, type BezPath } from '@/lib/bezpath';
import type { LaserApi } from '../api';
import type { FontFamily, Point, TextObject, TextOutlineResult } from '../types';

type Lag = <T>(value: () => T) => Promise<T>;

/** Familias simuladas, con los estilos que tendría una fuente real. */
const FONTS: FontFamily[] = [
  { family: 'Arial', styles: ['Regular', 'Bold', 'Italic', 'Bold Italic'] },
  { family: 'Bebas Neue', styles: ['Regular'] },
  { family: 'Courier New', styles: ['Regular', 'Bold', 'Italic', 'Bold Italic'] },
  { family: 'DejaVu Sans', styles: ['Regular', 'Bold', 'Oblique', 'Bold Oblique'] },
  { family: 'Georgia', styles: ['Regular', 'Bold', 'Italic', 'Bold Italic'] },
  { family: 'Helvetica Neue', styles: ['Light', 'Regular', 'Medium', 'Bold', 'Italic'] },
  { family: 'Liberation Serif', styles: ['Regular', 'Bold', 'Italic', 'Bold Italic'] },
  { family: 'Times New Roman', styles: ['Regular', 'Bold', 'Italic', 'Bold Italic'] },
];

/** Proporciones de la caja de cada letra, en múltiplos del tamaño. */
const ADVANCE_EM = 0.58;
const GLYPH_W_EM = 0.42;
const GLYPH_H_EM = 0.7;

const hasStyle = (f: FontFamily, re: RegExp) => f.styles.some((s) => re.test(s));

/** Punto y dirección a `dist` mm del inicio de la polilínea (se queda en los extremos). */
function pointAt(points: Point[], dist: number): { p: Point; angle: number } {
  let rest = Math.max(0, dist);
  for (let i = 1; i < points.length; i++) {
    const [a, b] = [points[i - 1]!, points[i]!];
    const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
    if (rest <= len || i === points.length - 1) {
      const k = len > 0 ? Math.min(rest, len) / len : 0;
      return { p: [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k], angle: Math.atan2(b[1] - a[1], b[0] - a[0]) };
    }
    rest -= len;
  }
  return { p: points[0] ?? [0, 0], angle: 0 };
}

/**
 * Contorno aproximado: un rectángulo por letra visible (sin kerning ni curvas), con varias
 * líneas, alineación y espaciado. Sobre un trayecto, cada rectángulo se orienta según la curva.
 * Sirve para desarrollar la interfaz en el navegador; el motor real usa la fuente de verdad.
 */
export function mockTextOutline(text: TextObject, path: BezPath | null): TextOutlineResult {
  if (!(text.size_mm > 0)) throw new Error('El tamaño del texto debe ser mayor que 0 mm');
  if (!(text.line_spacing > 0)) throw new Error('El interlineado debe ser mayor que 0');
  const family = FONTS.find((f) => f.family.toLowerCase() === text.font_family.trim().toLowerCase());
  if (!family) {
    throw new Error(
      text.font_family.trim()
        ? `La fuente '${text.font_family}' no está instalada. Familias parecidas: ${FONTS.slice(0, 3).map((f) => f.family).join(', ')}`
        : 'Falta el nombre de la fuente',
    );
  }
  const warnings: string[] = [];
  if (text.italic && !hasStyle(family, /italic|oblique/i)) {
    warnings.push(`La fuente '${family.family}' no tiene cursiva; se usa la versión recta (no se genera una cursiva sintética)`);
  }
  if (text.bold && !hasStyle(family, /bold|black/i)) {
    warnings.push(`La fuente '${family.family}' no tiene negrita; se usa el peso 400 (no se genera una negrita sintética)`);
  }
  if (text.on_path && !path) throw new Error('El texto está en trayecto pero no se recibió la trayectoria');

  const size = text.size_mm;
  const step = size * ADVANCE_EM + text.letter_spacing_mm;
  const lines = text.text.split(/\r?\n/);
  const flat = path ? flattenBez(path, 0.05)[0] : undefined;
  if (text.on_path && (!flat || flat.points.length < 2)) throw new Error('La trayectoria no tiene longitud');
  let pts = flat ? (flat.closed ? [...flat.points, flat.points[0]!] : flat.points) : [];
  if (text.on_path?.flip) pts = [...pts].reverse();
  const total = pts.reduce((sum, p, i) => (i ? sum + Math.hypot(p[0] - pts[i - 1]![0], p[1] - pts[i - 1]![1]) : 0), 0);

  const paths: BezPath[] = [];
  let missed = 0;
  lines.forEach((line, row) => {
    const chars = [...line];
    const width = chars.length * step - text.letter_spacing_mm;
    const x0 = text.align === 'left' ? 0 : text.align === 'center' ? -width / 2 : -width;
    const baseline = -row * size * text.line_spacing;
    chars.forEach((ch, i) => {
      if (/\s/.test(ch)) return;
      const x = x0 + i * step;
      const corners: Point[] = [[0, 0], [size * GLYPH_W_EM, 0], [size * GLYPH_W_EM, size * GLYPH_H_EM], [0, size * GLYPH_H_EM]];
      if (text.on_path) {
        const along = text.on_path.offset_mm + x + (size * GLYPH_W_EM) / 2;
        if (along < 0 || along > total) return void missed++;
        const { p, angle } = pointAt(pts, along);
        const [c, s] = [Math.cos(angle), Math.sin(angle)];
        const [ox, oy] = [-(size * GLYPH_W_EM) / 2, baseline];
        paths.push(polylineBez(corners.map(([cx, cy]) => [p[0] + (cx + ox) * c - (cy + oy) * s, p[1] + (cx + ox) * s + (cy + oy) * c]), true));
      } else {
        paths.push(polylineBez(corners.map(([cx, cy]) => [x + cx, baseline + cy]), true));
      }
    });
  });
  if (missed > 0) warnings.push(`El texto no cabe en la trayectoria: ${missed} caracteres quedaron fuera`);
  return { paths, warnings };
}

/** Métodos simulados del área `text`: fuentes plausibles y un contorno aproximado. */
export function mockText(lag: Lag): Pick<LaserApi, 'listFonts' | 'textOutline'> {
  return {
    listFonts: () => lag(() => structuredClone(FONTS)),
    textOutline: ({ text, path }) => lag(() => mockTextOutline(text, path)),
  };
}
