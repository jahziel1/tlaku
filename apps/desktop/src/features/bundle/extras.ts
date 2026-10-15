import type { CostsBundle, Document, HistoryBundle, TemplatesBundle } from '@/api';
import { sanitizeStored, type CostStored, type MaterialProfile } from '@/features/costs/costProfiles';
import type { MachineCost } from '@/features/costs/costModel';
import { sameValue } from '@/features/materials/library';
import { MAX_REMNANTS, MAX_REMNANT_NAME, REMNANT_LIBRARY_VERSION, type Remnant, type RemnantLibrary } from '@/features/nest/remnants';
import { t, type Language } from '@/i18n';

/**
 * Secciones de la versión 2 del paquete `.tlakubundle` (respaldo a carpeta): perfiles de costos,
 * retazos, plantillas e historial de trabajos, más lo común de los conflictos por id. Lógica pura
 * (sin API ni store): armar la sección, leerla con cuidado, calcular qué cambiaría y qué quedaría
 * según la política elegida. Aplicar de verdad es de `apply.ts`.
 */

/** Qué hacer cuando un elemento del respaldo tiene el id (o nombre) de uno local y es distinto. */
export type ConflictPolicy = 'replace' | 'keepBoth' | 'skip';
export const CONFLICT_POLICIES: readonly ConflictPolicy[] = ['replace', 'keepBoth', 'skip'];

/** Secciones con elementos que pueden chocar por id. */
export type ConflictSection = 'devices' | 'materials' | 'costs' | 'remnants' | 'templates';
export const CONFLICT_SECTIONS: readonly ConflictSection[] = ['devices', 'materials', 'costs', 'remnants', 'templates'];
export type PolicyChoice = Record<ConflictSection, ConflictPolicy>;

/** Por omisión: las máquinas y los costos se reemplazan (es lo que se espera al restaurar); lo demás conserva ambos. */
export const defaultPolicies = (): PolicyChoice => ({ devices: 'replace', materials: 'keepBoth', costs: 'replace', remnants: 'keepBoth', templates: 'keepBoth' });

const isObject = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v);

// ---------------------------------------------------------------------------
// Comparar y combinar por clave
// ---------------------------------------------------------------------------

export interface Diff<T> {
  /** Elementos cuya clave no existe todavía. */
  added: T[];
  /** Elementos con la clave de uno local pero contenido distinto. */
  conflicts: T[];
  /** Idénticos a los locales. */
  unchanged: T[];
}

/** Separa `incoming` en nuevos, en conflicto e iguales a `existing`. */
export function diffByKey<T>(incoming: readonly T[], existing: readonly T[], keyOf: (x: T) => string, same: (a: T, b: T) => boolean = sameValue): Diff<T> {
  const byKey = new Map(existing.map((x) => [keyOf(x), x]));
  const out: Diff<T> = { added: [], conflicts: [], unchanged: [] };
  for (const item of incoming) {
    const found = byKey.get(keyOf(item));
    if (!found) out.added.push(item);
    else if (same(found, item)) out.unchanged.push(item);
    else out.conflicts.push(item);
  }
  return out;
}

/** Nombre distinto de los de `taken`: «Nombre (importado)», «Nombre (importado 2)»... */
export function uniqueLabel(base: string, taken: ReadonlySet<string>, maxLen = Infinity, fold: (s: string) => string = (s) => s): string {
  const suffix = t('bundle.rename.suffix');
  const make = (n: number) => {
    const tail = n === 1 ? ` (${suffix})` : ` (${suffix} ${n})`;
    return `${base.slice(0, Math.max(1, maxLen - tail.length))}${tail}`;
  };
  let n = 1;
  while (taken.has(fold(make(n)))) n++;
  return make(n);
}

/**
 * Lista resultante de aplicar `diff` sobre `existing` con la política: los nuevos entran siempre;
 * los conflictivos reemplazan al local, entran como copia (`rekey`) o se omiten.
 */
export function mergeByKey<T>(existing: readonly T[], diff: Diff<T>, policy: ConflictPolicy, keyOf: (x: T) => string, rekey: (item: T, takenKeys: ReadonlySet<string>) => T): T[] {
  const out = [...existing, ...diff.added];
  const taken = new Set(out.map(keyOf));
  if (policy === 'replace') {
    const replacement = new Map(diff.conflicts.map((c) => [keyOf(c), c]));
    return out.map((x) => replacement.get(keyOf(x)) ?? x);
  }
  if (policy === 'keepBoth') {
    for (const c of diff.conflicts) {
      const copy = rekey(c, taken);
      taken.add(keyOf(copy));
      out.push(copy);
    }
  }
  return out;
}

/** Cuántos elementos tocaría la política: los nuevos y, según ella, los conflictivos. */
export const touched = (diff: Diff<unknown>, policy: ConflictPolicy): number => diff.added.length + (policy === 'skip' ? 0 : diff.conflicts.length);

// ---------------------------------------------------------------------------
// Costos
// ---------------------------------------------------------------------------

export interface MachineCostEntry {
  id: string;
  cost: MachineCost;
}

export function buildCostsSection(stored: Pick<CostStored, 'machines' | 'sales' | 'materials'>): CostsBundle {
  return { machines: structuredClone(stored.machines), sales: structuredClone(stored.sales), materials: structuredClone([...stored.materials]) };
}

export interface CostsPlan {
  machines: Diff<MachineCostEntry>;
  materials: Diff<MaterialProfile>;
  /** `null` = el paquete no trae venta y mano de obra. */
  sales: 'same' | 'different' | null;
  /** Lo que el paquete trae, ya saneado. */
  incoming: Pick<CostStored, 'machines' | 'sales' | 'materials'>;
  invalid: number;
}

const machineEntries = (machines: Record<string, MachineCost>): MachineCostEntry[] => Object.entries(machines).map(([id, cost]) => ({ id, cost }));

export function planCosts(raw: unknown, current: CostStored, language: Language): CostsPlan | null {
  if (!isObject(raw)) return null;
  const clean = sanitizeStored(raw, language);
  const rawMachines = isObject(raw.machines) ? Object.keys(raw.machines).length : 0;
  const rawMaterials = Array.isArray(raw.materials) ? raw.materials.length : 0;
  const invalid = Math.max(0, rawMachines - Object.keys(clean.machines).length) + Math.max(0, rawMaterials - clean.materials.length);
  return {
    machines: diffByKey(machineEntries(clean.machines), machineEntries(current.machines), (e) => e.id),
    materials: diffByKey(clean.materials, current.materials, (p) => p.name),
    sales: isObject(raw.sales) ? (sameValue(clean.sales, current.sales) ? 'same' : 'different') : null,
    incoming: { machines: clean.machines, sales: clean.sales, materials: clean.materials },
    invalid,
  };
}

/** Valores de costos que quedarían. Solo «reemplazar» pisa lo local; los costos por máquina no tienen «conservar ambos» (llevan el id de la máquina). */
export function resolveCosts(plan: CostsPlan, current: CostStored, policy: ConflictPolicy): CostStored {
  const machines = { ...current.machines };
  for (const e of plan.machines.added) machines[e.id] = e.cost;
  if (policy === 'replace') for (const e of plan.machines.conflicts) machines[e.id] = e.cost;
  const materials = mergeByKey(current.materials, plan.materials, policy, (p) => p.name, (p, taken) => ({ ...p, name: uniqueLabel(p.name, taken, 80) }));
  const sales = policy === 'replace' && plan.sales === 'different' ? plan.incoming.sales : current.sales;
  return { ...current, machines, materials, sales };
}

export function costsChanges(plan: CostsPlan, policy: ConflictPolicy): number {
  const machines = plan.machines.added.length + (policy === 'replace' ? plan.machines.conflicts.length : 0);
  return machines + touched(plan.materials, policy) + (policy === 'replace' && plan.sales === 'different' ? 1 : 0);
}

// ---------------------------------------------------------------------------
// Retazos
// ---------------------------------------------------------------------------

const finite = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);

/** Retazos válidos del paquete (forma de `edit/remnants.rs`) y cuántos se descartaron. */
export function sanitizeRemnants(raw: unknown): { remnants: Remnant[]; invalid: number } {
  const list = isObject(raw) && Array.isArray(raw.remnants) ? (raw.remnants as unknown[]) : [];
  const seen = new Set<string>();
  const remnants: Remnant[] = [];
  let invalid = 0;
  for (const r of list) {
    const ok =
      isObject(r) &&
      typeof r.id === 'string' &&
      r.id.trim() !== '' &&
      !seen.has(r.id) &&
      typeof r.name === 'string' &&
      typeof r.material === 'string' &&
      finite(r.thickness_mm) &&
      finite(r.width_mm) &&
      finite(r.height_mm) &&
      finite(r.area_mm2) &&
      typeof r.rect === 'boolean' &&
      finite(r.created_ms) &&
      Array.isArray(r.paths) &&
      r.paths.every((ring) => Array.isArray(ring) && ring.every((p) => Array.isArray(p) && p.length === 2 && finite(p[0]) && finite(p[1])));
    if (!ok) {
      invalid++;
      continue;
    }
    seen.add(r.id as string);
    remnants.push(structuredClone(r) as unknown as Remnant);
  }
  return { remnants, invalid };
}

export interface RemnantsPlan {
  diff: Diff<Remnant>;
  invalid: number;
}

export function planRemnants(raw: unknown, current: RemnantLibrary): RemnantsPlan | null {
  if (!isObject(raw)) return null;
  const { remnants, invalid } = sanitizeRemnants(raw);
  return { diff: diffByKey(remnants, current.remnants, (r) => r.id), invalid };
}

const freshRemnantId = (id: string, taken: ReadonlySet<string>): string => {
  let n = 1;
  let next = `${id}-imp`;
  while (taken.has(next)) next = `${id}-imp${++n}`;
  return next;
};

/** Inventario que quedaría y cuántos retazos no caben en el tope de la biblioteca. */
export function resolveRemnants(plan: RemnantsPlan, current: RemnantLibrary, policy: ConflictPolicy): { library: RemnantLibrary; dropped: number } {
  const merged = mergeByKey(
    current.remnants,
    plan.diff,
    policy,
    (r) => r.id,
    // El nombre de un retazo es solo una etiqueta (puede repetirse): basta marcar la copia.
    (r, taken) => ({ ...r, id: freshRemnantId(r.id, taken), name: uniqueLabel(r.name, new Set(), MAX_REMNANT_NAME) }),
  );
  const kept = merged.slice(0, MAX_REMNANTS);
  return { library: { version: REMNANT_LIBRARY_VERSION, remnants: kept }, dropped: merged.length - kept.length };
}

export const remnantsChanges = (plan: RemnantsPlan, policy: ConflictPolicy): number => touched(plan.diff, policy);

// ---------------------------------------------------------------------------
// Plantillas
// ---------------------------------------------------------------------------

export interface TemplateItem {
  name: string;
  doc: Document;
}

export const MAX_TEMPLATE_NAME = 80;

/** Igual que `check_name` de `templates.rs`: sin separadores ni caracteres reservados, sin punto inicial o final. */
export function validTemplateName(name: string): boolean {
  const n = name.trim();
  // eslint-disable-next-line no-control-regex
  return n !== '' && n.length <= MAX_TEMPLATE_NAME && !/[\u0000-\u001f\u007f/\\:*?"<>|]/.test(n) && !n.startsWith('.') && !n.endsWith('.');
}

const looksLikeDoc = (d: unknown): d is Document => isObject(d) && Array.isArray(d.objects) && Array.isArray(d.layers);

export function buildTemplatesSection(items: readonly TemplateItem[]): TemplatesBundle {
  return { items: items.map((i) => ({ name: i.name, doc: structuredClone(i.doc) })) };
}

export function sanitizeTemplates(raw: unknown): { items: TemplateItem[]; invalid: number } {
  const list = isObject(raw) && Array.isArray(raw.items) ? (raw.items as unknown[]) : [];
  const seen = new Set<string>();
  const items: TemplateItem[] = [];
  let invalid = 0;
  for (const it of list) {
    if (!isObject(it) || typeof it.name !== 'string' || !validTemplateName(it.name) || !looksLikeDoc(it.doc) || seen.has(it.name.trim().toLowerCase())) {
      invalid++;
      continue;
    }
    const name = it.name.trim();
    seen.add(name.toLowerCase());
    items.push({ name, doc: it.doc });
  }
  return { items, invalid };
}

export interface TemplatesPlan {
  diff: Diff<TemplateItem>;
  invalid: number;
}

/** `existing`: las plantillas locales cuyo nombre aparece en el paquete (las demás no importan para comparar). */
export function planTemplates(raw: unknown, existing: readonly TemplateItem[]): TemplatesPlan | null {
  if (!isObject(raw)) return null;
  const { items, invalid } = sanitizeTemplates(raw);
  return { diff: diffByKey(items, existing, (i) => i.name.toLowerCase(), (a, b) => sameValue(a.doc, b.doc)), invalid };
}

/** Plantillas que hay que guardar con la política (las demás quedan como están). `localNames`: todos los nombres locales. */
export function templatesToWrite(plan: TemplatesPlan, localNames: readonly string[], policy: ConflictPolicy): TemplateItem[] {
  const out = [...plan.diff.added];
  if (policy === 'replace') out.push(...plan.diff.conflicts);
  if (policy === 'keepBoth') {
    const taken = new Set([...localNames, ...plan.diff.added.map((a) => a.name)].map((n) => n.toLowerCase()));
    for (const c of plan.diff.conflicts) {
      const name = uniqueLabel(c.name, taken, MAX_TEMPLATE_NAME, (s) => s.toLowerCase());
      taken.add(name.toLowerCase());
      out.push({ ...c, name });
    }
  }
  return out;
}

export const templatesChanges = (plan: TemplatesPlan, policy: ConflictPolicy): number => touched(plan.diff, policy);

// ---------------------------------------------------------------------------
// Historial de trabajos
// ---------------------------------------------------------------------------

type HistoryEntry = Record<string, unknown>;

export function buildHistorySection(entries: readonly HistoryEntry[]): HistoryBundle {
  return { entries: structuredClone([...entries]) };
}

export interface HistoryPlan {
  /** Entradas cuyo id todavía no está en el registro. */
  added: HistoryEntry[];
  /** Ya estaban (mismo id). */
  unchanged: number;
  invalid: number;
}

export function planHistory(raw: unknown, localIds: ReadonlySet<string>): HistoryPlan | null {
  if (!isObject(raw)) return null;
  const list = Array.isArray(raw.entries) ? (raw.entries as unknown[]) : [];
  const seen = new Set<string>();
  const added: HistoryEntry[] = [];
  let unchanged = 0;
  let invalid = 0;
  for (const e of list) {
    if (!isObject(e) || typeof e.id !== 'string' || e.id === '' || typeof e.at !== 'number' || seen.has(e.id)) {
      invalid++;
      continue;
    }
    seen.add(e.id);
    if (localIds.has(e.id)) unchanged++;
    else added.push(e);
  }
  return { added, unchanged, invalid };
}
