import type { DeviceKind } from '@/bindings/DeviceKind';
import type { ProbeFamily, ProbeResult } from '@/api/api';
import { CATALOG, type CatalogEntry } from './catalog';
import { locale } from '@/i18n';

/** Lo que se sabe de la máquina detectada, listo para ordenar el catálogo. */
export interface Detection {
  family: ProbeFamily;
  kind: DeviceKind;
  /** Saludo e información de `$I`/`M115` en minúsculas. */
  text: string;
}

export function detectionFrom(result: ProbeResult | null): Detection | null {
  if (!result) return null;
  return { family: result.family, kind: result.suggested_kind, text: `${result.banner}\n${result.info.join('\n')}`.toLowerCase() };
}

/** Tipos de entrada del catálogo que pueden ser la máquina de una familia detectada. */
const KINDS_BY_FAMILY: Record<Exclude<ProbeFamily, 'unknown'>, DeviceKind[]> = {
  grbl: ['grbl', 'grbl_m3', 'custom_gcode'],
  grbl_hal: ['grbl_hal'],
  fluid_nc: ['fluid_nc'],
  grbl_lpc: ['grbl_lpc'],
  marlin: ['marlin'],
  smoothie: ['smoothie'],
};

/** Minúsculas sin acentos ni signos: "Falcon-2" y "falcon 2" se buscan igual. */
export function fold(text: string): string {
  return text
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, ' ')
    .trim();
}

export interface RankedEntry {
  entry: CatalogEntry;
  score: number;
  /** El tipo de firmware cuadra con el detectado (o no se detectó nada). */
  compatible: boolean;
  /** El saludo o `$I` contiene un fragmento propio de esta máquina. */
  byBanner: boolean;
}

function isCompatible(entry: CatalogEntry, d: Detection | null): boolean {
  if (!d || d.family === 'unknown') return true;
  const kinds = new Set<DeviceKind>([entry.kind, ...(entry.alsoKinds ?? [])]);
  return KINDS_BY_FAMILY[d.family].some((k) => kinds.has(k));
}

/**
 * Filtra y ordena el catálogo según lo detectado y lo que se escribió en la búsqueda.
 *
 * - Cada palabra de la búsqueda debe aparecer en la marca, el modelo o sus alias.
 * - Con una máquina detectada, primero las que reconocen su saludo, luego la entrada genérica de
 *   su familia y después los modelos del mismo tipo de firmware. Las de otro firmware solo salen
 *   con `includeIncompatible`.
 * - Sin detección, orden alfabético por marca y modelo, con las genéricas al final.
 */
export function rankCatalog(opts: {
  query: string;
  detection: Detection | null;
  includeIncompatible?: boolean;
  entries?: readonly CatalogEntry[];
}): RankedEntry[] {
  const { query, detection, includeIncompatible = false, entries = CATALOG } = opts;
  const words = fold(query).split(' ').filter(Boolean);
  const out: RankedEntry[] = [];
  for (const entry of entries) {
    const hay = fold([entry.brand, entry.model, ...(entry.aliases ?? [])].join(' '));
    if (!words.every((w) => hay.includes(w))) continue;
    const compatible = isCompatible(entry, detection);
    if (!compatible && !includeIncompatible) continue;
    const byBanner = !!detection && !!entry.detect?.value.some((p) => detection.text.includes(p));
    let score = 0;
    if (byBanner) score += 100;
    if (detection && compatible && detection.family !== 'unknown') score += entry.kind === detection.kind ? 20 : 10;
    // La familia genérica de lo detectado va justo después de los modelos que se reconocieron.
    if (entry.generic && detection && compatible && entry.kind === detection.kind) score += 40;
    if (words.length > 0 && fold(entry.model).startsWith(words[0]!)) score += 5;
    out.push({ entry, score, compatible, byBanner });
  }
  return out.sort(
    (a, b) =>
      b.score - a.score ||
      Number(a.entry.generic ?? false) - Number(b.entry.generic ?? false) ||
      a.entry.brand.localeCompare(b.entry.brand, locale()) ||
      a.entry.model.localeCompare(b.entry.model, locale(), { numeric: true }),
  );
}
