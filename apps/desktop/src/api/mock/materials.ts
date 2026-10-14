import type { LaserApi } from '../api';
import { factoryLayer, makeLayer } from '@/lib/layers';
import type { Confidence, DesignObject, Document, LayerMode, LayerSettings, MachineClass, MaterialLibrary, MaterialPreset, MaterialRestriction, MaterialRole, MaterialTestParams, MaterialWarning, TestAxis, TestParam } from '../types';
// Los mismos datos que usa el motor para la biblioteca de fábrica (formato compacto).
import factoryData from '../../../../../crates/tlaku-doc/data/materials-default.json';

type Lag = <T>(value: () => T) => Promise<T>;

const MAX_COUNT = 30;
const MAX_PASSES = 1000;

// ---------------------------------------------------------------------------
// Biblioteca de fábrica: el mismo archivo de datos que el motor
// ---------------------------------------------------------------------------

interface FactoryEntry {
  id: string;
  material: string;
  thickness?: number;
  role: MaterialRole;
  title: string;
  machine_class: MachineClass;
  mode?: LayerMode;
  speed: number;
  power: number;
  min_power?: number;
  passes?: number;
  interval?: number;
  crosshatch?: boolean;
  air?: boolean;
  frequency_khz?: number;
  source: string;
  confidence: Confidence;
  warnings?: MaterialWarning[];
}

interface FactoryData {
  revision: number;
  materials: Record<string, string>;
  titles: Record<string, string>;
  restrictions: MaterialRestriction[];
  presets: FactoryEntry[];
}

/** Un aporte de la comunidad (`community/materials/*.json`, ver `crates/tlaku-doc/src/community.rs`). */
export interface CommunityEntry extends Omit<FactoryEntry, 'min_power' | 'crosshatch' | 'warnings' | 'title'> {
  title?: string;
  material_name?: string;
  laser_watts: number;
  machine: string;
  notes?: string;
  license: string;
  contributor?: string;
}

// Los aportes de la comunidad, ordenados por nombre de archivo como los reúne `crates/tlaku-doc/build.rs`.
const communityFiles = import.meta.glob('../../../../../community/materials/*.json', { eager: true, import: 'default' }) as Record<string, CommunityEntry>;
const communityEntries: CommunityEntry[] = Object.keys(communityFiles).sort().map((k) => communityFiles[k]!);

/** Igual que `community::expand`: el nombre del material sale de la tabla de fábrica o de `material_name`; las notas llevan la máquina. */
function expandCommunity(e: CommunityEntry, data: FactoryData): FactoryEntry & { notes: string; material_name: string } {
  const title = e.title ?? e.role;
  return { ...e, title, notes: e.notes ? `${e.machine.trim()} · ${e.notes.trim()}` : e.machine.trim(), material_name: data.materials[e.material] ?? e.material_name ?? e.material };
}

/** Expande el formato compacto igual que `tlaku_doc::factory_library`: modo por rol, mm/min y valores de la capa normal. Los aportes de la comunidad van al final. */
export function expandFactory(data: FactoryData, community: CommunityEntry[] = []): MaterialLibrary {
  const rows: (FactoryEntry & { notes?: string; material_name?: string })[] = [...data.presets, ...community.map((c) => expandCommunity(c, data))];
  const presets = rows.map((e): MaterialPreset => {
    const title = data.titles[e.title] ?? e.title;
    const settings = factoryLayer('#000000', title, {
      mode: e.mode ?? (e.role === 'engrave' ? 'fill' : 'line'),
      speed: e.speed,
      power: e.power,
      min_power: e.min_power ?? 0,
      passes: e.passes ?? 1,
      air: e.air ?? false,
      crosshatch: e.crosshatch ?? false,
      ...(e.interval === undefined ? {} : { interval: e.interval }),
    });
    if (e.frequency_khz !== undefined) settings.galvo = { ...settings.galvo, frequency_khz: e.frequency_khz };
    return {
      id: e.id,
      material: e.material_name ?? data.materials[e.material] ?? e.material,
      thickness: e.thickness ?? null,
      title,
      notes: e.notes ?? '',
      device_id: null,
      settings,
      role: e.role,
      machine_class: e.machine_class,
      source: e.source,
      confidence: e.confidence,
      material_key: e.material,
      title_key: e.title,
      ...(e.warnings?.length ? { warnings: e.warnings } : {}),
    };
  });
  return { version: 1, presets, revision: data.revision, ...(data.restrictions.length ? { restrictions: data.restrictions } : {}) };
}

/** Biblioteca de fábrica del modo mock: la misma que trae el motor. */
export function factoryMaterialLibrary(): MaterialLibrary {
  return expandFactory(factoryData as unknown as FactoryData, communityEntries);
}

/**
 * Biblioteca del usuario del modo mock: unos materiales propios de ejemplo (la de fábrica va
 * aparte, ver `factoryMaterialLibrary`). Son ajustes de quien los guardó, sin fuente.
 */
export function sampleMaterialLibrary(): MaterialLibrary {
  const preset = (id: string, material: string, thickness: number | null, title: string, notes: string, patch: Partial<LayerSettings>): MaterialPreset => ({
    id,
    material,
    thickness,
    title,
    notes,
    device_id: null,
    settings: makeLayer('#000000', title, patch),
  });
  return {
    version: 1,
    presets: [
      preset('mat-mdf3-corte', 'MDF', 3, 'Corte', 'Dos pasadas con aire; los bordes quedan oscuros.', { speed: 300, power: 100, passes: 2, air: true }),
      preset('mat-mdf3-grabado', 'MDF', 3, 'Grabado', 'Relleno suave, buen contraste.', { mode: 'fill', speed: 3000, power: 35, interval: 0.1 }),
      preset('mat-triplay-corte', 'Contrachapado de abedul', 3, 'Corte', 'Una pasada lenta; limpiar el hollín después.', { speed: 250, power: 100, passes: 1, air: true }),
      preset('mat-acrilico-corte', 'Acrílico negro', 3, 'Corte', 'Pasadas múltiples; el transparente no se corta con diodo.', { speed: 200, power: 100, passes: 4, air: true }),
      preset('mat-cuero-grabado', 'Cuero vegetal', null, 'Grabado', 'Potencia baja para no quemarlo.', { mode: 'fill', speed: 4000, power: 25, interval: 0.12 }),
    ],
  };
}

/** Biblioteca que devuelve "importar" en modo mock (un id coincide con la de ejemplo para probar la mezcla). */
function importedSample(): MaterialLibrary {
  const base = sampleMaterialLibrary();
  const mdf = base.presets[0]!;
  return {
    version: 1,
    presets: [
      { ...mdf, notes: 'Versión del taller vecino: una sola pasada más lenta.', settings: { ...mdf.settings, speed: 150, passes: 1 } },
      {
        id: 'mat-balsa-corte',
        material: 'Balsa',
        thickness: 2,
        title: 'Corte',
        notes: 'Muy ligera; mucha velocidad.',
        device_id: null,
        settings: makeLayer('#000000', 'Corte', { speed: 900, power: 60, air: true }),
      },
    ],
  };
}

// ---------------------------------------------------------------------------
// Prueba de materiales simulada
// ---------------------------------------------------------------------------

function axisValues(axis: TestAxis): number[] {
  return Array.from({ length: axis.count }, (_, i) => {
    const v = axis.count === 1 ? axis.from : axis.from + ((axis.to - axis.from) * i) / (axis.count - 1);
    return axis.param === 'passes' ? Math.max(1, Math.round(v)) : v;
  });
}

function checkAxis(axis: TestAxis, which: string, mode: MaterialTestParams['mode']): void {
  if (!Number.isInteger(axis.count) || axis.count < 1 || axis.count > MAX_COUNT) {
    throw new Error(`Las ${which} deben ser entre 1 y ${MAX_COUNT} (se pidieron ${axis.count}).`);
  }
  if (!Number.isFinite(axis.from) || !Number.isFinite(axis.to)) throw new Error(`El rango de las ${which} contiene valores no válidos.`);
  if (axis.param === 'interval' && mode === 'line') throw new Error(`El intervalo no se usa en modo line; elige otro parámetro para las ${which} o cambia a relleno.`);
  for (const v of [axis.from, axis.to]) {
    if (axis.param === 'power' && (v < 0 || v > 100)) throw new Error(`La potencia de las ${which} debe estar entre 0 y 100 % (se pidió ${v}).`);
    if ((axis.param === 'speed' || axis.param === 'interval') && v <= 0) throw new Error(`Los valores de las ${which} deben ser mayores que 0 (se pidió ${v}).`);
    if (axis.param === 'passes' && Math.round(v) > MAX_PASSES) throw new Error(`Las pasadas de las ${which} no pueden pasar de ${MAX_PASSES} (se pidió ${v}).`);
  }
}

const trim = (v: number, decimals: number) => String(Number(v.toFixed(decimals)));

function shortName(param: TestParam, v: number): string {
  switch (param) {
    case 'speed':
      return `V ${trim(v, 0)}`;
    case 'power':
      return `P ${trim(v, 1)}%`;
    case 'passes':
      return `Pas ${trim(v, 0)}`;
    case 'interval':
      return `Int ${trim(v, 3)}`;
  }
}

/** Color único por celda (tonos repartidos por la razón áurea); nunca es negro. */
function cellColor(n: number): string {
  const h = ((n * 0.618033988749895) % 1) * 360;
  const [s, l] = [[0.9, 0.55], [0.7, 0.45], [0.85, 0.65]][n % 3]!;
  const a = s * Math.min(l, 1 - l);
  const f = (k: number) => {
    const c = l - a * Math.max(-1, Math.min((k + h / 30) % 12 - 3, 9 - ((k + h / 30) % 12), 1));
    return Math.round(c * 255).toString(16).padStart(2, '0');
  };
  return `#${f(0)}${f(8)}${f(4)}`;
}

/**
 * Cuadrícula equivalente a la del motor, sin rótulos: un rectángulo por celda en su propia capa.
 * Las filas van de arriba hacia abajo; `origin` es la esquina inferior izquierda.
 */
export function mockMaterialTest(p: MaterialTestParams): Document {
  if (p.mode === 'image') throw new Error('La prueba de materiales solo admite los modos line y fill.');
  if (p.rows.param === p.cols.param) throw new Error('Las filas y las columnas deben variar parámetros distintos.');
  checkAxis(p.rows, 'filas', p.mode);
  checkAxis(p.cols, 'columnas', p.mode);
  if (!(p.cell[0] > 0 && p.cell[1] > 0)) throw new Error('El tamaño de la celda debe ser mayor que 0 mm.');
  if (!(p.gap >= 0)) throw new Error('La separación entre celdas no puede ser negativa.');

  const rows = axisValues(p.rows);
  const cols = axisValues(p.cols);
  const [cw, ch] = p.cell;
  const objects: DesignObject[] = [];
  const layers: LayerSettings[] = [];
  rows.forEach((rv, i) =>
    cols.forEach((cv, j) => {
      const color = cellColor(i * cols.length + j + 1);
      const x = p.origin[0] + j * (cw + p.gap);
      const y = p.origin[1] + (rows.length - 1 - i) * (ch + p.gap);
      const layer: LayerSettings = {
        ...p.base,
        color,
        name: `${shortName(p.rows.param, rv)} · ${shortName(p.cols.param, cv)}`,
        mode: p.mode,
        output: true,
        visible: true,
        order: null,
      };
      for (const [param, v] of [[p.rows.param, rv], [p.cols.param, cv]] as const) {
        if (param === 'speed') layer.speed = v;
        else if (param === 'power') layer.power = v;
        else if (param === 'passes') layer.passes = v;
        else layer.interval = v;
      }
      layers.push(layer);
      objects.push({
        id: `cell-${i + 1}-${j + 1}`,
        layer: color,
        transform: [1, 0, 0, 1, 0, 0],
        kind: {
          type: 'vector',
          paths: [
            [
              { type: 'move_to', to: [x, y] },
              { type: 'line_to', to: [x + cw, y] },
              { type: 'line_to', to: [x + cw, y + ch] },
              { type: 'line_to', to: [x, y + ch] },
              { type: 'close' },
            ],
          ],
        },
        locked: false,
        hidden: false,
        group: null,
      });
    }),
  );
  return { version: 2, objects, layers, device_id: null };
}

export function mockMaterials(
  lag: Lag,
): Pick<
  LaserApi,
  'loadMaterialLibrary' | 'loadFactoryMaterials' | 'saveMaterialLibrary' | 'importMaterialLibrary' | 'exportMaterialLibrary' | 'materialTest'
> {
  let library: MaterialLibrary = sampleMaterialLibrary();
  return {
    loadMaterialLibrary: () => lag(() => structuredClone(library)),
    loadFactoryMaterials: () => lag(factoryMaterialLibrary),
    saveMaterialLibrary: ({ library: next }) =>
      lag(() => {
        library = structuredClone(next);
      }),
    importMaterialLibrary: () => lag(importedSample),
    exportMaterialLibrary: () => lag(() => undefined),
    materialTest: ({ params }) => lag(() => mockMaterialTest(params)),
  };
}
