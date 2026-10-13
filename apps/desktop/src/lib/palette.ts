import { t, type I18nKey } from '@/i18n';

/**
 * Paleta de capas propia. Cada color es una capa = una operación del trabajo (no el color final
 * de la pieza). Las muestras se identifican por nombre ("Rojo", "Azul"...), no por número, y
 * hay una capa especial de guías que nunca sale a la máquina.
 */
export interface PaletteEntry {
  /** Identificador estable (clave del nombre, comando de atajo). */
  id: string;
  color: string;
  /** Clave i18n del nombre visible. */
  nameKey: I18nKey;
  /** Capa de guías: referencias y marcas que nunca se envían a la máquina. */
  guide?: boolean;
  /** Atajo opcional para elegir la muestra (`Alt+1`...). */
  shortcut?: string;
}

/** Colores elegidos para distinguirse entre sí sobre fondo oscuro y claro, también con visión de color reducida. */
const COLORS = [
  { id: 'red', color: '#ff3b4a', nameKey: 'layers.color.red' },
  { id: 'blue', color: '#2f7bff', nameKey: 'layers.color.blue' },
  { id: 'green', color: '#14c28a', nameKey: 'layers.color.green' },
  { id: 'amber', color: '#ffb020', nameKey: 'layers.color.amber' },
  { id: 'violet', color: '#b04dff', nameKey: 'layers.color.violet' },
  { id: 'cyan', color: '#00c2e0', nameKey: 'layers.color.cyan' },
  { id: 'orange', color: '#ff7a1a', nameKey: 'layers.color.orange' },
  { id: 'yellow', color: '#e6e600', nameKey: 'layers.color.yellow' },
  { id: 'pink', color: '#ff4fb0', nameKey: 'layers.color.pink' },
  { id: 'lime', color: '#7fd11f', nameKey: 'layers.color.lime' },
  { id: 'indigo', color: '#6a5cff', nameKey: 'layers.color.indigo' },
  { id: 'copper', color: '#c8793c', nameKey: 'layers.color.copper' },
  { id: 'teal', color: '#2fd6c8', nameKey: 'layers.color.teal' },
  { id: 'white', color: '#f2f2f2', nameKey: 'layers.color.white' },
  { id: 'gray', color: '#9aa4b2', nameKey: 'layers.color.gray' },
  { id: 'navy', color: '#1d3d8f', nameKey: 'layers.color.navy' },
] as const satisfies readonly Omit<PaletteEntry, 'guide' | 'shortcut'>[];

/**
 * Color reservado de la capa de guías.
 * TODO(contrato): el motor no distingue capas de guías; se serializa como una capa normal con
 * `output: false`. Confirmar con el motor si conviene un marcador propio.
 */
export const GUIDE_COLOR = '#7e8aa0';

export const GUIDE_ENTRY: PaletteEntry = {
  id: 'guides',
  color: GUIDE_COLOR,
  nameKey: 'layers.color.guides',
  guide: true,
  shortcut: 'Alt+G',
};

/** Atajos `Alt+1`..`Alt+9`, `Alt+0` para los diez primeros colores. */
export const PALETTE_SHORTCUTS = ['Alt+1', 'Alt+2', 'Alt+3', 'Alt+4', 'Alt+5', 'Alt+6', 'Alt+7', 'Alt+8', 'Alt+9', 'Alt+0'] as const;

/** Colores de trabajo seguidos de la capa de guías. */
export const LAYER_PALETTE: readonly PaletteEntry[] = [
  ...COLORS.map((c, i): PaletteEntry => ({ ...c, shortcut: PALETTE_SHORTCUTS[i] })),
  GUIDE_ENTRY,
];

const norm = (color: string) => color.toLowerCase();

/** Entrada de la paleta para un color (sin distinguir mayúsculas), si existe. */
export function paletteEntry(color: string): PaletteEntry | undefined {
  const c = norm(color);
  return LAYER_PALETTE.find((p) => p.color === c);
}

/** ¿Es el color de la capa de guías? */
export function isGuideColor(color: string): boolean {
  return norm(color) === GUIDE_COLOR;
}

/** Nombre legible de una entrada ("Rojo", "Guías"). */
export function paletteLabel(entry: PaletteEntry): string {
  return t(entry.nameKey);
}

/** Nombre legible de un color; si no está en la paleta, el propio `#rrggbb`. */
export function colorName(color: string): string {
  const entry = paletteEntry(color);
  return entry ? paletteLabel(entry) : norm(color);
}

// ---- contraste (WCAG 2.x) ------------------------------------------------------------------

export const INK_DARK = '#020308';
export const INK_LIGHT = '#ffffff';

function channel(v: number): number {
  const s = v / 255;
  return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
}

/** Luminancia relativa (0 = negro, 1 = blanco) de un `#rrggbb`. */
export function relativeLuminance(hex: string): number {
  const n = parseInt(hex.slice(1), 16);
  return 0.2126 * channel((n >> 16) & 255) + 0.7152 * channel((n >> 8) & 255) + 0.0722 * channel(n & 255);
}

/** Relación de contraste entre dos colores `#rrggbb` (1 a 21). */
export function contrastRatio(a: string, b: string): number {
  const la = relativeLuminance(a);
  const lb = relativeLuminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

/**
 * Tinta (casi negra o blanca) con más contraste sobre un fondo `#rrggbb`. Con cualquier fondo
 * alcanza al menos 4,5:1, así que sirve para texto y para indicadores sobre la muestra.
 */
export function readableOn(hex: string): typeof INK_DARK | typeof INK_LIGHT {
  return contrastRatio(hex, INK_DARK) >= contrastRatio(hex, INK_LIGHT) ? INK_DARK : INK_LIGHT;
}
