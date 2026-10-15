import { readPref, writePref } from '@/lib/platform';
import type { Language } from '@/i18n';
import {
  clampNum,
  DEFAULT_MACHINE,
  DEFAULT_MATERIAL,
  DEFAULT_SALES,
  LIMITS,
  ROUND_STEPS,
  type MachineCost,
  type MarginBase,
  type MaterialCost,
  type MaterialMode,
  type RoundStep,
  type SalesCost,
  type TimeSource,
} from './costModel';
import { CURRENCIES, defaultCurrency } from './currency';

/**
 * Perfiles de costo. Viven en `localStorage` (clave `tlaku.costs`): son datos de esta computadora,
 * no del proyecto, y nada sale de ella. Hay tres capas:
 * - valores de la máquina, uno por perfil de máquina (se eligen solos con la máquina activa);
 * - perfiles de material con nombre (se guardan, se cargan y se borran a mano);
 * - venta y mano de obra (moneda, kWh, tarifa, margen, comisión...), comunes a todas.
 */

export const COSTS_KEY = 'costs';
/** Tope de perfiles de material y de longitud de un nombre. */
export const MATERIAL_PROFILES_MAX = 100;
export const PROFILE_NAME_MAX = 80;

export interface MaterialProfile {
  name: string;
  material: MaterialCost;
  mode: MaterialMode;
}

export interface CostStored {
  machines: Record<string, MachineCost>;
  sales: SalesCost;
  material: MaterialCost;
  materialName: string | null;
  materials: MaterialProfile[];
  materialMode: MaterialMode;
  timeSource: TimeSource;
  quantity: number;
  piecesInDesign: number;
}

const MODES: readonly MaterialMode[] = ['bbox', 'area', 'sheet'];
const SOURCES: readonly TimeSource[] = ['estimate', 'calibrated', 'lastReal'];
const BASES: readonly MarginBase[] = ['cost', 'price'];

const obj = (v: unknown): Record<string, unknown> => (v && typeof v === 'object' && !Array.isArray(v) ? (v as Record<string, unknown>) : {});
const pick = <T extends string>(v: unknown, list: readonly T[], fallback: T): T => (list.includes(v as T) ? (v as T) : fallback);
/** Entero ≥ 1 (un valor ilegible cae en 1). */
const int = (v: unknown): number => Math.floor(clampNum(v, LIMITS.count.min, LIMITS.count.max));

export function sanitizeMachine(raw: unknown): MachineCost {
  const r = obj(raw);
  return {
    hourlyRate: clampNum(r.hourlyRate, LIMITS.money.min, LIMITS.money.max),
    tubePrice: clampNum(r.tubePrice, LIMITS.money.min, LIMITS.money.max),
    tubeLifeH: clampNum(r.tubeLifeH, LIMITS.hours.min, LIMITS.hours.max),
    watts: clampNum(r.watts, LIMITS.watts.min, LIMITS.watts.max),
    accessoryWatts: clampNum(r.accessoryWatts, LIMITS.watts.min, LIMITS.watts.max),
  };
}

export function sanitizeMaterial(raw: unknown): MaterialCost {
  const r = obj(raw);
  const d = DEFAULT_MATERIAL;
  return {
    sheetPrice: clampNum(r.sheetPrice, LIMITS.money.min, LIMITS.money.max),
    sheetW: 'sheetW' in r ? clampNum(r.sheetW, LIMITS.mm.min, LIMITS.mm.max) : d.sheetW,
    sheetH: 'sheetH' in r ? clampNum(r.sheetH, LIMITS.mm.min, LIMITS.mm.max) : d.sheetH,
    wastePct: 'wastePct' in r ? clampNum(r.wastePct, LIMITS.pct.min, LIMITS.pct.max) : d.wastePct,
    piecesPerSheet: 'piecesPerSheet' in r ? int(r.piecesPerSheet) : d.piecesPerSheet,
  };
}

export function sanitizeSales(raw: unknown, language: Language, systemTag?: string | null): SalesCost {
  const r = obj(raw);
  const d = DEFAULT_SALES;
  const currency = typeof r.currency === 'string' && (CURRENCIES as readonly string[]).includes(r.currency) ? r.currency : defaultCurrency(language, systemTag);
  const step = ROUND_STEPS.includes(r.roundTo as RoundStep) ? (r.roundTo as RoundStep) : d.roundTo;
  return {
    currency,
    kwhPrice: clampNum(r.kwhPrice, LIMITS.money.min, LIMITS.money.max),
    laborRate: clampNum(r.laborRate, LIMITS.money.min, LIMITS.money.max),
    prepMin: clampNum(r.prepMin, LIMITS.minutes.min, LIMITS.minutes.max),
    finishMin: clampNum(r.finishMin, LIMITS.minutes.min, LIMITS.minutes.max),
    consumables: clampNum(r.consumables, LIMITS.money.min, LIMITS.money.max),
    margin: 'margin' in r ? clampNum(r.margin, LIMITS.margin.min, LIMITS.margin.max) : d.margin,
    marginBase: pick(r.marginBase, BASES, d.marginBase),
    commission: clampNum(r.commission, LIMITS.pct.min, LIMITS.pct.max),
    tax: clampNum(r.tax, LIMITS.pct.min, LIMITS.pct.max),
    roundTo: step,
  };
}

function cleanName(v: unknown): string | null {
  if (typeof v !== 'string') return null;
  const name = v.trim().slice(0, PROFILE_NAME_MAX);
  return name === '' ? null : name;
}

/** Valores de fábrica, con la moneda de la región del sistema. */
export function defaultStored(language: Language, systemTag?: string | null): CostStored {
  return {
    machines: {},
    sales: sanitizeSales({}, language, systemTag),
    material: { ...DEFAULT_MATERIAL },
    materialName: null,
    materials: [],
    materialMode: 'bbox',
    timeSource: 'estimate',
    quantity: 1,
    piecesInDesign: 1,
  };
}

/** Datos válidos a partir de cualquier cosa leída del disco (lo dañado se descarta o se arregla). */
export function sanitizeStored(raw: unknown, language: Language, systemTag?: string | null): CostStored {
  const r = obj(raw);
  const base = defaultStored(language, systemTag);
  const machines: Record<string, MachineCost> = {};
  for (const [id, m] of Object.entries(obj(r.machines)).slice(0, 200)) machines[id.slice(0, 200)] = sanitizeMachine(m);
  const seen = new Set<string>();
  const materials: MaterialProfile[] = [];
  if (Array.isArray(r.materials)) {
    for (const item of r.materials) {
      const p = obj(item);
      const name = cleanName(p.name);
      if (!name || seen.has(name) || materials.length >= MATERIAL_PROFILES_MAX) continue;
      seen.add(name);
      materials.push({ name, material: sanitizeMaterial(p.material), mode: pick(p.mode, MODES, 'bbox') });
    }
  }
  const materialName = cleanName(r.materialName);
  return {
    machines,
    sales: 'sales' in r ? sanitizeSales(r.sales, language, systemTag) : base.sales,
    material: 'material' in r ? sanitizeMaterial(r.material) : base.material,
    materialName: materialName && seen.has(materialName) ? materialName : null,
    materials,
    materialMode: pick(r.materialMode, MODES, base.materialMode),
    timeSource: pick(r.timeSource, SOURCES, base.timeSource),
    quantity: int(r.quantity),
    piecesInDesign: int(r.piecesInDesign),
  };
}

export function readCosts(language: Language, systemTag?: string | null): CostStored {
  let parsed: unknown;
  try {
    const text = readPref(COSTS_KEY);
    parsed = text ? JSON.parse(text) : null;
  } catch {
    parsed = null; // JSON dañado: se parte de los valores de fábrica
  }
  return sanitizeStored(parsed, language, systemTag);
}

export function writeCosts(data: CostStored): void {
  writePref(COSTS_KEY, JSON.stringify(data));
}

/** Valores de la máquina `id` (los de fábrica si nunca se editó). */
export function machineFor(data: Pick<CostStored, 'machines'>, id: string): MachineCost {
  return data.machines[id] ?? { ...DEFAULT_MACHINE };
}

/** Guarda (o reemplaza) el perfil de material `name`. Devuelve la lista nueva, sin pasarse del tope. */
export function upsertMaterial(list: readonly MaterialProfile[], profile: MaterialProfile): MaterialProfile[] {
  const name = cleanName(profile.name);
  if (!name) return [...list];
  const next: MaterialProfile = { name, material: sanitizeMaterial(profile.material), mode: profile.mode };
  const i = list.findIndex((p) => p.name === name);
  if (i >= 0) return list.map((p, k) => (k === i ? next : p));
  return list.length >= MATERIAL_PROFILES_MAX ? [...list] : [...list, next];
}

export function removeMaterial(list: readonly MaterialProfile[], name: string): MaterialProfile[] {
  return list.filter((p) => p.name !== name);
}

export { cleanName as cleanProfileName };
