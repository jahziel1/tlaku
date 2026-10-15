import type { FontAxisInfo, FontFeatureInfo, FontInstanceInfo, TextAxis, TextFeature } from '@/api/types';
import { t } from '@/i18n';

/**
 * Lógica de las funciones OpenType y los ejes variables de un texto (sin interfaz).
 *
 * Lo que se guarda en el objeto son solo las diferencias con la fuente: una función que queda
 * como la trae la fuente (kerning y ligaduras estándar encendidos, el resto apagadas) no deja
 * rastro en `features`, y un eje en su valor por omisión no deja rastro en `variations`. Así un
 * documento sin ajustes es idéntico al de antes de existir estas opciones.
 */

export type FeatureGroup = 'ligatures' | 'alternates' | 'caps' | 'figures' | 'position';

export const GROUP_ORDER: readonly FeatureGroup[] = ['ligatures', 'alternates', 'caps', 'figures', 'position'];

const GROUPS: Record<string, FeatureGroup> = {
  kern: 'ligatures',
  liga: 'ligatures',
  clig: 'ligatures',
  dlig: 'ligatures',
  calt: 'ligatures',
  salt: 'alternates',
  swsh: 'alternates',
  cswh: 'alternates',
  smcp: 'caps',
  c2sc: 'caps',
  pcap: 'caps',
  c2pc: 'caps',
  case: 'caps',
  lnum: 'figures',
  onum: 'figures',
  tnum: 'figures',
  pnum: 'figures',
  zero: 'figures',
  frac: 'position',
  afrc: 'position',
  sups: 'position',
  subs: 'position',
  ordn: 'position',
};

/** Funciones que no pueden estar encendidas a la vez (al encender una se quitan las otras). */
const EXCLUSIVE: readonly (readonly string[])[] = [
  ['lnum', 'onum'],
  ['tnum', 'pnum'],
  ['smcp', 'pcap'],
  ['c2sc', 'c2pc'],
  ['sups', 'subs'],
];

/** Funciones cuyo valor es el número de la alternativa (1 = la primera). */
export const NUMBERED_FEATURES: readonly string[] = ['salt'];

const KNOWN_LABELS = [
  'liga', 'clig', 'dlig', 'calt', 'kern', 'salt', 'swsh', 'cswh', 'smcp', 'c2sc', 'pcap', 'c2pc',
  'lnum', 'onum', 'tnum', 'pnum', 'frac', 'afrc', 'zero', 'case', 'sups', 'subs', 'ordn',
] as const;

export function featureGroup(tag: string): FeatureGroup {
  if (/^ss\d\d$/.test(tag) || /^cv\d\d$/.test(tag)) return 'alternates';
  return GROUPS[tag] ?? 'position';
}

/** Nombre legible de una función (la etiqueta si no se conoce). */
export function featureLabel(tag: string): string {
  const known = (KNOWN_LABELS as readonly string[]).includes(tag);
  if (known) return t(`text.ot.feature.${tag}` as 'text.ot.feature.liga');
  if (/^ss\d\d$/.test(tag)) return t('text.ot.feature.ss', { n: Number(tag.slice(2)) });
  if (/^cv\d\d$/.test(tag)) return t('text.ot.feature.cv', { n: Number(tag.slice(2)) });
  return tag;
}

export function groupLabel(group: FeatureGroup): string {
  return t(`text.ot.group.${group}` as 'text.ot.group.ligatures');
}

/** Agrupa las funciones que declara la fuente en el orden de `GROUP_ORDER`, cada grupo por etiqueta. */
export function groupFeatures(features: readonly FontFeatureInfo[]): { group: FeatureGroup; items: FontFeatureInfo[] }[] {
  return GROUP_ORDER.map((group) => ({
    group,
    items: features.filter((f) => featureGroup(f.tag) === group).sort((a, b) => a.tag.localeCompare(b.tag)),
  })).filter((g) => g.items.length > 0);
}

/** Valor guardado de la función, o `undefined` si el objeto no la toca. */
export function storedFeature(features: readonly TextFeature[] | undefined, tag: string): number | undefined {
  let value: number | undefined;
  for (const f of features ?? []) if (f.tag === tag) value = f.value;
  return value;
}

/** ¿La función está encendida con lo que hay guardado? (`defaultOn`: lo que hace la fuente sin pedirlo). */
export function featureOn(features: readonly TextFeature[] | undefined, tag: string, defaultOn: boolean): boolean {
  const stored = storedFeature(features, tag);
  return stored === undefined ? defaultOn : stored > 0;
}

/**
 * Enciende o apaga una función y devuelve la lista nueva (`undefined` si ya no queda ninguna
 * diferencia con la fuente). `value` es el número de alternativa de las funciones numeradas.
 */
export function withFeature(
  features: readonly TextFeature[] | undefined,
  tag: string,
  on: boolean,
  defaultOn: boolean,
  value = 1,
): TextFeature[] | undefined {
  const others = on ? (EXCLUSIVE.find((set) => set.includes(tag)) ?? []).filter((x) => x !== tag) : [];
  const next = (features ?? []).filter((f) => f.tag !== tag && !others.includes(f.tag));
  if (on !== defaultOn) next.push({ tag, value: on ? Math.max(1, Math.round(value)) : 0 });
  else if (on && value > 1) next.push({ tag, value: Math.round(value) });
  return next.length > 0 ? next : undefined;
}

/** Cuántas funciones están cambiadas respecto de la fuente (para el resumen del panel). */
export function changedFeatureCount(features: readonly TextFeature[] | undefined): number {
  return new Set((features ?? []).map((f) => f.tag)).size;
}

const AXIS_LABELS = ['wght', 'wdth', 'slnt', 'opsz', 'ital'] as const;

/** Nombre de un eje: el nuestro si es de los comunes, si no el que da la fuente, si no la etiqueta. */
export function axisLabel(axis: Pick<FontAxisInfo, 'tag' | 'name'>): string {
  if ((AXIS_LABELS as readonly string[]).includes(axis.tag)) return t(`text.ot.axis.${axis.tag}` as 'text.ot.axis.wght');
  return axis.name?.trim() || axis.tag;
}

export function axisValue(variations: readonly TextAxis[] | undefined, axis: Pick<FontAxisInfo, 'tag' | 'default'>): number {
  let value = axis.default;
  for (const a of variations ?? []) if (a.tag === axis.tag) value = a.value;
  return value;
}

/** Paso razonable de un eje: enteros en rangos amplios, décimas en rangos cortos (como `slnt`). */
export function axisStep(axis: Pick<FontAxisInfo, 'min' | 'max'>): number {
  return axis.max - axis.min > 20 ? 1 : 0.1;
}

const close = (a: number, b: number) => Math.abs(a - b) < 1e-4;

/** Pone un eje en `value` (acotado al rango) y devuelve la lista nueva; `undefined` si todo queda por omisión. */
export function withAxis(variations: readonly TextAxis[] | undefined, axis: FontAxisInfo, value: number): TextAxis[] | undefined {
  const clamped = Math.min(axis.max, Math.max(axis.min, value));
  const next = (variations ?? []).filter((a) => a.tag !== axis.tag);
  if (!close(clamped, axis.default)) next.push({ tag: axis.tag, value: clamped });
  return next.length > 0 ? next : undefined;
}

/** Posición de los ejes que corresponde a una instancia con nombre (solo los que difieren del valor por omisión). */
export function variationsOfInstance(instance: FontInstanceInfo, axes: readonly FontAxisInfo[]): TextAxis[] | undefined {
  const next = instance.coords.filter((c) => {
    const axis = axes.find((a) => a.tag === c.tag);
    return axis && !close(c.value, axis.default);
  });
  return next.length > 0 ? next : undefined;
}

/** Instancia con nombre que coincide con la posición actual, si hay una. */
export function matchingInstance(
  variations: readonly TextAxis[] | undefined,
  instances: readonly FontInstanceInfo[],
  axes: readonly FontAxisInfo[],
): FontInstanceInfo | undefined {
  return instances.find((inst) =>
    axes.every((axis) => {
      const coord = inst.coords.find((c) => c.tag === axis.tag);
      return close(coord?.value ?? axis.default, axisValue(variations, axis));
    }),
  );
}
