/**
 * Paleta de capas propia. Cada color es una capa = una operación del trabajo
 * (no el color final de la pieza). Los colores se eligieron para distinguirse
 * entre sí sobre fondo oscuro y claro, también con visión de color reducida.
 */
export interface PaletteEntry {
  /** Número visible en la muestra (00, 01, ...). */
  index: number;
  color: string;
  /** Capa de herramienta: guías, límites; nunca se envía a la máquina. */
  tool?: boolean;
}

const COLORS = [
  '#ff3b4a', // 00
  '#2f7bff', // 01
  '#14c28a', // 02
  '#ffb020', // 03
  '#b04dff', // 04
  '#00c2e0', // 05
  '#ff7a1a', // 06
  '#e6e600', // 07
  '#ff4fb0', // 08
  '#7fd11f', // 09
  '#6a5cff', // 10
  '#c8793c', // 11
  '#2fd6c8', // 12
  '#f2f2f2', // 13
  '#9aa4b2', // 14
  '#1d3d8f', // 15
] as const;

// TODO(contrato): las capas de herramienta (T1/T2) no están en docs/app-api.md; confirmar cómo se serializan.
const TOOL_COLORS = ['#7e8aa0', '#a38b6d'] as const;

export const LAYER_PALETTE: readonly PaletteEntry[] = [
  ...COLORS.map((color, index) => ({ index, color })),
  ...TOOL_COLORS.map((color, i) => ({ index: COLORS.length + i, color, tool: true })),
];

export function paletteLabel(entry: PaletteEntry): string {
  return entry.tool ? `T${entry.index - COLORS.length + 1}` : String(entry.index).padStart(2, '0');
}

/** Color de texto legible (negro o blanco) sobre un fondo `#rrggbb`. */
export function readableOn(hex: string): '#0b0c10' | '#ffffff' {
  const n = parseInt(hex.slice(1), 16);
  const r = (n >> 16) & 255;
  const g = (n >> 8) & 255;
  const b = n & 255;
  const lum = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
  return lum > 0.55 ? '#0b0c10' : '#ffffff';
}
