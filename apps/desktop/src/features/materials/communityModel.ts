/**
 * Lógica pura del navegador de ajustes de la comunidad (sin interfaz ni API): qué tan parecida es
 * mi máquina a la de un ajuste, filtros, orden y conversión de un ajuste del catálogo a uno de la
 * biblioteca local.
 *
 * Todo lo que dice el catálogo sobre la máquina es **declarado**, no demostrado: aquí «misma máquina»
 * significa «quien lo aportó escribió el mismo modelo del catálogo de Tlaku», y por eso la interfaz
 * trata hasta la coincidencia exacta como un punto de partida que se prueba en un retazo.
 */
import type { CatalogEntry, CatalogOperation, DeviceProfile, LaserKind, MachineClass, MaterialPreset } from '@/api/types';
import { CATALOG } from '@/features/devices/catalog';
import { factoryLayer } from '@/lib/layers';
import { classKind, laserKindOf, nearestClass } from './machineClass';
import { materialName, titleName } from './labels';
import { fold } from './library';

// ---------------------------------------------------------------------------
// Mi máquina
// ---------------------------------------------------------------------------

export interface MyMachine {
  /** Nombre del perfil activo. */
  name: string;
  /** `id` del modelo del catálogo de máquinas de Tlaku al que corresponde el nombre del perfil, o `null`. */
  model: string | null;
  kind: LaserKind | null;
  /** Clase (tipo y potencia) elegida por la persona o deducida del perfil o del modelo; `null` = no se sabe. */
  machineClass: MachineClass | null;
}

/** Nombre con el que el asistente de máquinas pone el perfil de un modelo del catálogo. */
const modelName = (e: (typeof CATALOG)[number]) => (e.generic ? e.model : `${e.brand} ${e.model}`);

/**
 * `id` del modelo del catálogo cuyo nombre es el del perfil (sin mayúsculas ni acentos). Un perfil
 * renombrado a mano o de un modelo que no está en el catálogo da `null`: entonces solo se compara la clase.
 */
export function catalogModelOf(name: string, entries: typeof CATALOG = CATALOG): string | null {
  const wanted = fold(name);
  if (wanted === '') return null;
  const found = entries.filter((e) => !e.generic && (fold(modelName(e)) === wanted || fold(e.model) === wanted));
  return found.length === 1 ? found[0]!.id : null;
}

/** Potencia escrita en el nombre de un modelo («Falcon2 22 W» da 22), o `null`. */
export function wattsInName(name: string): number | null {
  const m = /(\d+(?:[.,]\d+)?)\s*w\b/i.exec(name);
  if (!m) return null;
  const w = Number(m[1]!.replace(',', '.'));
  return Number.isFinite(w) && w > 0 ? w : null;
}

/**
 * Lo que se sabe de la máquina activa. `chosenClass` es la clase que la persona eligió (o que se dedujo
 * del perfil); sin ella se intenta con la potencia que dice el nombre del modelo.
 */
export function myMachine(device: Pick<DeviceProfile, 'name' | 'kind' | 'galvo'>, chosenClass: MachineClass | null): MyMachine {
  const kind = laserKindOf(device);
  const model = catalogModelOf(device.name);
  let machineClass = chosenClass;
  if (machineClass === null && kind !== null) {
    const watts = wattsInName(device.name);
    if (watts !== null) machineClass = nearestClass(kind, watts);
  }
  return { name: device.name, model, kind, machineClass };
}

// ---------------------------------------------------------------------------
// Parecido entre mi máquina y la del ajuste
// ---------------------------------------------------------------------------

/**
 * Qué tanto se parece mi máquina a la del ajuste (declarada por quien lo aportó):
 * `model` mismo modelo del catálogo (y la misma clase si la mía se conoce), `class` misma clase,
 * `kind` mismo tipo de láser con otra potencia, `other` otro tipo de láser o no se sabe el mío.
 */
export type Compat = 'model' | 'class' | 'kind' | 'other';

const RANK: Record<Compat, number> = { model: 0, class: 1, kind: 2, other: 3 };

export function compatOf(entry: Pick<CatalogEntry, 'machine_model' | 'machine_class'>, me: MyMachine): Compat {
  const sameClass = me.machineClass !== null && entry.machine_class === me.machineClass;
  if (me.model !== null && entry.machine_model === me.model && (me.machineClass === null || sameClass)) return 'model';
  if (sameClass) return 'class';
  if (me.kind !== null && classKind(entry.machine_class) === me.kind) return 'kind';
  return 'other';
}

/** ¿Hay que advertir que el ajuste es de otra máquina o potencia? Todo menos el mismo modelo. */
export const isOnlyIndicative = (c: Compat): boolean => c !== 'model';

// ---------------------------------------------------------------------------
// Nombres
// ---------------------------------------------------------------------------

export const entryMaterialName = (e: Pick<CatalogEntry, 'material' | 'material_name'>): string =>
  materialName({ material: e.material_name ?? e.material, material_key: e.material });

export const entryTitle = (e: Pick<CatalogEntry, 'role' | 'title'>): string => titleName({ title: e.title ?? e.role, title_key: e.title ?? e.role });

/** Resumen corto de los valores: «500 mm/min · 100 % · ×2 · 0,1 mm». */
export function entrySummary(e: Pick<CatalogEntry, 'speed' | 'power' | 'passes' | 'mode' | 'interval' | 'frequency_khz'>): string {
  const parts = [`${e.speed} mm/min`, `${e.power} %`];
  if (e.passes !== undefined && e.passes > 1) parts.push(`×${e.passes}`);
  if (e.mode !== 'line' && e.interval !== undefined) parts.push(`${e.interval} mm`);
  if (e.frequency_khz !== undefined) parts.push(`${e.frequency_khz} kHz`);
  return parts.join(' · ');
}

// ---------------------------------------------------------------------------
// Filtros y orden
// ---------------------------------------------------------------------------

/** `mine` = mismo modelo o misma clase de máquina; `kind` = mismo tipo de láser; `all` = todo el catálogo. */
export type MachineScope = 'mine' | 'kind' | 'all';

export interface CommunityFilter {
  scope: MachineScope;
  /** Clave del material; `''` = todos. */
  material: string;
  /** `''` = todos, `'none'` = sin grosor, o el grosor en mm como texto. */
  thickness: string;
  operation: CatalogOperation | '';
  search: string;
  verifiedOnly: boolean;
}

export const EMPTY_FILTER: CommunityFilter = { scope: 'mine', material: '', thickness: '', operation: '', search: '', verifiedOnly: false };

export interface Row {
  entry: CatalogEntry;
  compat: Compat;
}

/** Con «mi máquina» y sin saber ni el modelo ni la clase, no hay con qué comparar: se amplía al tipo de láser. */
export function mineIsDegraded(scope: MachineScope, me: MyMachine): boolean {
  return scope === 'mine' && me.model === null && me.machineClass === null;
}

const allowedBy = (scope: MachineScope, c: Compat): boolean => (scope === 'all' ? true : scope === 'kind' ? RANK[c] <= RANK.kind : RANK[c] <= RANK.class);

/** Texto donde busca la caja de búsqueda. */
function haystack(e: CatalogEntry): string {
  return fold([entryMaterialName(e), e.material, entryTitle(e), e.machine, e.notes ?? '', e.contributor ?? '', e.thickness ?? ''].join(' '));
}

const sameThickness = (a: number | undefined, b: number) => a !== undefined && Math.abs(a - b) < 1e-9;

/** Ajustes que cumplen los filtros, los más parecidos a mi máquina y los más confirmados primero. */
export function filterEntries(entries: readonly CatalogEntry[], filter: CommunityFilter, me: MyMachine): Row[] {
  const scope: MachineScope = mineIsDegraded(filter.scope, me) ? 'kind' : filter.scope;
  const words = fold(filter.search).split(/\s+/).filter(Boolean);
  const rows: Row[] = [];
  for (const entry of entries) {
    const compat = compatOf(entry, me);
    if (!allowedBy(scope, compat)) continue;
    if (filter.material !== '' && entry.material !== filter.material) continue;
    if (filter.thickness === 'none' ? entry.thickness !== undefined : filter.thickness !== '' && !sameThickness(entry.thickness, Number(filter.thickness))) continue;
    if (filter.operation !== '' && entry.operation !== filter.operation) continue;
    if (filter.verifiedOnly && entry.verified_by === 0) continue;
    if (words.length > 0) {
      const hay = haystack(entry);
      if (!words.every((w) => hay.includes(w))) continue;
    }
    rows.push({ entry, compat });
  }
  return rows.sort(
    (a, b) =>
      RANK[a.compat] - RANK[b.compat] ||
      b.entry.verified_by - a.entry.verified_by ||
      entryMaterialName(a.entry).localeCompare(entryMaterialName(b.entry)) ||
      (a.entry.thickness ?? Infinity) - (b.entry.thickness ?? Infinity) ||
      a.entry.id.localeCompare(b.entry.id),
  );
}

/** Materiales que hay en el catálogo (para el selector), por nombre. */
export function materialOptions(entries: readonly CatalogEntry[]): { key: string; name: string }[] {
  const seen = new Map<string, string>();
  for (const e of entries) if (!seen.has(e.material)) seen.set(e.material, entryMaterialName(e));
  return [...seen].map(([key, name]) => ({ key, name })).sort((a, b) => a.name.localeCompare(b.name));
}

/** Grosores que hay en el catálogo, de menor a mayor, y si hay ajustes sin grosor. */
export function thicknessOptions(entries: readonly CatalogEntry[]): { values: number[]; hasNone: boolean } {
  const values = [...new Set(entries.flatMap((e) => (e.thickness === undefined ? [] : [e.thickness])))].sort((a, b) => a - b);
  return { values, hasNone: entries.some((e) => e.thickness === undefined) };
}

// ---------------------------------------------------------------------------
// Del catálogo a la biblioteca
// ---------------------------------------------------------------------------

/**
 * Ajuste de la biblioteca del usuario a partir de uno del catálogo. Lleva su origen: la dirección del
 * issue (`source`), la confianza «foro» (comunidad, sin verificar), el rol y la clase de la máquina. Los
 * nombres ya van traducidos (es un ajuste propio y editable). `notes` lo compone quien llama (texto de la interfaz).
 */
export function presetFromEntry(e: CatalogEntry, id: string, notes: string): MaterialPreset {
  const title = entryTitle(e);
  const settings = factoryLayer('#000000', title, {
    mode: e.mode,
    speed: e.speed,
    power: e.power,
    passes: e.passes ?? 1,
    air: e.air,
    ...(e.interval === undefined ? {} : { interval: e.interval }),
  });
  if (e.frequency_khz !== undefined) settings.galvo = { ...settings.galvo, frequency_khz: e.frequency_khz };
  return {
    id,
    material: entryMaterialName(e),
    thickness: e.thickness ?? null,
    title,
    notes,
    device_id: null,
    settings,
    role: e.role,
    machine_class: e.machine_class,
    source: e.source,
    confidence: 'forum',
  };
}
