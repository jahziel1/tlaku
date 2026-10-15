/// <reference types="node" />
import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { beforeEach, describe, expect, it } from 'vitest';
import { exampleCatalogIndex } from '@/api/mock/communityCatalog';
import type { CatalogEntry, DeviceProfile } from '@/api/types';
import { CATALOG } from '@/features/devices/catalog';
import { setLanguage } from '@/i18n';
import { FALLBACK_DEVICE } from '@/lib/device';
import {
  EMPTY_FILTER,
  catalogModelOf,
  compatOf,
  entryMaterialName,
  entrySummary,
  filterEntries,
  isOnlyIndicative,
  materialOptions,
  mineIsDegraded,
  myMachine,
  presetFromEntry,
  thicknessOptions,
  wattsInName,
  type CommunityFilter,
  type MyMachine,
} from './communityModel';

const ENTRIES = exampleCatalogIndex().entries;
const get = (suffix: string) => ENTRIES.find((e) => e.id.endsWith(`.${suffix}`))!;

const device = (patch: Partial<DeviceProfile> = {}): DeviceProfile => ({ ...FALLBACK_DEVICE, id: 'd1', name: 'Creality Falcon2 22 W', kind: 'grbl', ...patch });
const filter = (patch: Partial<CommunityFilter> = {}): CommunityFilter => ({ ...EMPTY_FILTER, ...patch });
const ids = (rows: { entry: CatalogEntry }[]) => rows.map((r) => r.entry.id.split('.').at(-1));

beforeEach(() => setLanguage('es'));

describe('mi máquina y el catálogo de modelos', () => {
  it('el nombre que pone el asistente (marca y modelo) se reconoce, sin importar mayúsculas ni acentos', () => {
    expect(catalogModelOf('Creality Falcon2 22 W')).toBe('creality-falcon2-22w');
    expect(catalogModelOf('  creality falcon2 22 w ')).toBe('creality-falcon2-22w');
    expect(catalogModelOf('Falcon2 22 W')).toBe('creality-falcon2-22w');
  });

  it('un nombre cualquiera, vacío o ambiguo no se asocia a ningún modelo', () => {
    expect(catalogModelOf('Mi láser del taller')).toBeNull();
    expect(catalogModelOf('')).toBeNull();
    expect(catalogModelOf('   ')).toBeNull();
  });

  it('lee la potencia que escribe el nombre del modelo', () => {
    expect(wattsInName('Falcon2 22 W')).toBe(22);
    expect(wattsInName('Atomstack A20 Pro')).toBeNull();
    expect(wattsInName('CO2 60w')).toBe(60);
    expect(wattsInName('Láser 7,5 W')).toBe(7.5);
  });

  it('myMachine usa la clase elegida y, si no hay, la potencia del nombre', () => {
    const chosen = myMachine(device(), 'diode_10');
    expect(chosen).toMatchObject({ model: 'creality-falcon2-22w', kind: 'diode', machineClass: 'diode_10' });
    const byName = myMachine(device(), null);
    expect(byName.machineClass).toBe('diode_20');
    const unknown = myMachine(device({ name: 'Mi láser' }), null);
    expect(unknown).toMatchObject({ model: null, kind: 'diode', machineClass: null });
    expect(myMachine(device({ kind: 'ruida', name: 'Taller' }), null).kind).toBe('co2');
  });
});

describe('parecido entre mi máquina y la del ajuste', () => {
  const me = (patch: Partial<MyMachine> = {}): MyMachine => ({ name: 'Mi láser', model: null, kind: 'diode', machineClass: 'diode_20', ...patch });

  it('mismo modelo, misma clase, mismo tipo y otro láser', () => {
    const falcon = get('101'); // creality-falcon2-22w, diode_20
    expect(compatOf(falcon, me({ model: 'creality-falcon2-22w' }))).toBe('model');
    expect(compatOf(falcon, me())).toBe('class');
    expect(compatOf(falcon, me({ machineClass: 'diode_10' }))).toBe('kind');
    expect(compatOf(falcon, me({ kind: 'co2', machineClass: 'co2_60' }))).toBe('other');
    expect(compatOf(falcon, me({ kind: null, machineClass: null }))).toBe('other');
  });

  it('el mismo modelo con otra clase no cuenta como mismo modelo (por ejemplo una Falcon2 Pro de 40 W)', () => {
    const falcon = get('101');
    expect(compatOf(falcon, me({ model: 'creality-falcon2-22w', machineClass: 'diode_40' }))).toBe('kind');
    // Si no sé mi clase, el modelo basta.
    expect(compatOf(falcon, me({ model: 'creality-falcon2-22w', machineClass: null }))).toBe('model');
  });

  it('todo menos el mismo modelo es solo orientativo', () => {
    expect(isOnlyIndicative('model')).toBe(false);
    for (const c of ['class', 'kind', 'other'] as const) expect(isOnlyIndicative(c)).toBe(true);
  });
});

describe('filtros', () => {
  const falconMe: MyMachine = { name: 'Creality Falcon2 22 W', model: 'creality-falcon2-22w', kind: 'diode', machineClass: 'diode_20' };

  it('por omisión muestra solo mi modelo y mi clase, los más parecidos primero', () => {
    const rows = filterEntries(ENTRIES, filter(), falconMe);
    // 101 y 102: mismo modelo; 105: misma clase (Atomstack 20 W). El diodo de 10 W, el CO2 y la fibra no.
    expect(ids(rows)).toEqual(['101', '102', '105']);
    expect(rows.map((r) => r.compat)).toEqual(['model', 'model', 'class']);
  });

  it('«mismo tipo de láser» agrega otras potencias; «todas» agrega todo', () => {
    expect(ids(filterEntries(ENTRIES, filter({ scope: 'kind' }), falconMe))).toEqual(['101', '102', '105', '103']);
    expect(ids(filterEntries(ENTRIES, filter({ scope: 'all' }), falconMe))).toHaveLength(ENTRIES.length);
  });

  it('si no sé ni el modelo ni la clase, «mi máquina» se amplía al tipo de láser y se avisa', () => {
    const vague: MyMachine = { name: 'Mi láser', model: null, kind: 'diode', machineClass: null };
    expect(mineIsDegraded('mine', vague)).toBe(true);
    expect(mineIsDegraded('kind', vague)).toBe(false);
    expect(mineIsDegraded('mine', falconMe)).toBe(false);
    expect(ids(filterEntries(ENTRIES, filter(), vague)).sort()).toEqual(['101', '102', '103', '105']);
  });

  it('por material, grosor y operación', () => {
    const all = filter({ scope: 'all' });
    expect(ids(filterEntries(ENTRIES, { ...all, material: 'basswood' }, falconMe))).toEqual(['101', '102']);
    expect(ids(filterEntries(ENTRIES, { ...all, thickness: '3' }, falconMe)).sort()).toEqual(['101', '102', '103']);
    expect(ids(filterEntries(ENTRIES, { ...all, thickness: 'none' }, falconMe)).sort()).toEqual(['104', '106']);
    expect(ids(filterEntries(ENTRIES, { ...all, operation: 'cut' }, falconMe)).sort()).toEqual(['101', '103', '105']);
    expect(ids(filterEntries(ENTRIES, { ...all, operation: 'image' }, falconMe))).toEqual(['106']);
    expect(ids(filterEntries(ENTRIES, { ...all, material: 'basswood', operation: 'fill' }, falconMe))).toEqual(['102']);
  });

  it('solo verificados y búsqueda de texto sin acentos', () => {
    const all = filter({ scope: 'all' });
    expect(ids(filterEntries(ENTRIES, { ...all, verifiedOnly: true }, falconMe))).toEqual(['101', '104']);
    expect(ids(filterEntries(ENTRIES, { ...all, search: 'fieltro' }, falconMe))).toEqual(['105']);
    expect(ids(filterEntries(ENTRIES, { ...all, search: 'ruida' }, falconMe))).toEqual(['104']);
    expect(ids(filterEntries(ENTRIES, { ...all, search: 'ejemplo-ocho' }, falconMe))).toEqual(['106']);
    expect(ids(filterEntries(ENTRIES, { ...all, search: 'zzz' }, falconMe))).toEqual([]);
  });

  it('entre los igual de parecidos van primero los más confirmados', () => {
    const base = get('101');
    const mk = (n: number, verified: number): CatalogEntry => ({ ...base, id: `community.basswood.3.cut.diode_20.${n}`, verified_by: verified, confirmations: [] });
    const rows = filterEntries([mk(1, 0), mk(2, 3), mk(3, 1)], filter({ scope: 'all' }), falconMe);
    expect(ids(rows)).toEqual(['2', '3', '1']);
  });

  it('las opciones de los selectores salen del catálogo', () => {
    const options = materialOptions(ENTRIES);
    expect(options.map((m) => m.key).sort()).toEqual(['basswood', 'felt', 'leather', 'mdf', 'slate']);
    expect(options.map((m) => m.name)).toEqual([...options.map((m) => m.name)].sort((a, b) => a.localeCompare(b)));
    expect(options.find((m) => m.key === 'felt')?.name).toBe('Fieltro'); // material fuera de la tabla de fábrica: su propio nombre
    expect(thicknessOptions(ENTRIES)).toEqual({ values: [2, 3], hasNone: true });
    expect(thicknessOptions([])).toEqual({ values: [], hasNone: false });
  });
});

describe('del catálogo a la biblioteca', () => {
  it('conserva los valores, el origen y la confianza «foro»; no toca la capa por sí solo', () => {
    const e = get('101');
    const p = presetFromEntry(e, 'mat-1', 'Comunidad · @ejemplo-uno');
    expect(p).toMatchObject({
      id: 'mat-1',
      material: entryMaterialName(e),
      thickness: 3,
      notes: 'Comunidad · @ejemplo-uno',
      device_id: null,
      role: 'cut',
      machine_class: 'diode_20',
      source: e.source,
      confidence: 'forum',
    });
    expect(p.settings).toMatchObject({ mode: 'line', speed: 500, power: 100, passes: 2, air: true });
    expect(p.material).not.toBe('basswood'); // ya va traducido
    expect(p.material_key).toBeUndefined();
  });

  it('un relleno trae su intervalo y una imagen su modo; la frecuencia va al galvo', () => {
    const fill = presetFromEntry(get('102'), 'x', '');
    expect(fill.settings).toMatchObject({ mode: 'fill', speed: 6000, power: 40, interval: 0.1 });
    const image = presetFromEntry(get('106'), 'x', '');
    expect(image.settings).toMatchObject({ mode: 'image', interval: 0.05 });
    expect(image.settings.galvo.frequency_khz).toBe(40);
    expect(image.thickness).toBeNull();
  });

  it('el resumen de una línea de la lista', () => {
    expect(entrySummary(get('101'))).toBe('500 mm/min · 100 % · ×2');
    expect(entrySummary(get('102'))).toBe('6000 mm/min · 40 % · 0.1 mm');
    expect(entrySummary(get('106'))).toBe('3000 mm/min · 60 % · 0.05 mm · 40 kHz');
  });
});

describe('aportes reales del repositorio', () => {
  const dir = fileURLToPath(new URL('../../../../../community/materials/', import.meta.url));
  const files = readdirSync(dir).filter((f) => f.endsWith('.json'));

  it('si traen machine_model, es un id del catálogo de máquinas de Tlaku', () => {
    const known = new Set(CATALOG.map((c) => c.id));
    for (const f of files) {
      const entry = JSON.parse(readFileSync(`${dir}${f}`, 'utf8')) as { machine_model?: string };
      if (entry.machine_model !== undefined) expect(known.has(entry.machine_model), `${f}: ${entry.machine_model}`).toBe(true);
    }
  });
});
