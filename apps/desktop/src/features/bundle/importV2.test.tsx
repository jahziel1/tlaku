// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { getApi, setApi, type Document, type UserBundle } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { sampleDevices, sampleDocument } from '@/api/mock/sample';
import { sampleMaterialLibrary } from '@/api/mock/materials';
import { defaultStored, readCosts, writeCosts } from '@/features/costs/costProfiles';
import { emptyLibrary } from '@/features/materials/library';
import { useMaterials } from '@/features/materials/state';
import { emptyRemnants, type Remnant } from '@/features/nest/remnants';
import { useRemnants } from '@/features/nest/remnantStore';
import { setLanguage } from '@/i18n';
import { DEFAULT_PREFS, useStore } from '@/store';
import { cleanup, click, mount } from '@/test/dom';
import { applyImport, currentState, saveSafetyBackup } from './apply';
import { BundleDialog } from './BundleDialog';
import { defaultPolicies } from './extras';
import { buildUserBundle, planImport, sectionChanges, defaultSections, type BundleSource } from './model';
import { buildCurrentBundle } from './sources';
import { useBundleUi } from './uiStore';

let api: MockApi;
/** Espera, dejando que React pinte entre intento e intento (dentro de un solo `act` no se pinta nada). */
async function until(cond: () => boolean): Promise<void> {
  for (let i = 0; i < 300 && !cond(); i++) await act(async () => { await new Promise((r) => setTimeout(r, 5)); });
}
/** Espera (con act) a que termine una importación: aparece su aviso de resultado. */
const applied = () => until(() => useStore.getState().notices.length > 0);

const remnant = (id: string, over: Partial<Remnant> = {}): Remnant => ({
  id,
  name: `Retazo ${id}`,
  material: 'Madera',
  thickness_mm: 3,
  width_mm: 100,
  height_mm: 50,
  area_mm2: 5000,
  rect: true,
  paths: [[[0, 0], [100, 0], [100, 50], [0, 50]]],
  created_ms: 1,
  ...over,
});
/** Un documento distinguible por el nombre de su primera capa. */
const doc = (label: string): Document => {
  const d = sampleDocument();
  return { ...d, layers: d.layers.map((l, i) => (i === 0 ? { ...l, name: label } : l)) };
};
const label = (d: Document) => d.layers[0]!.name;
const machine = (hourlyRate: number) => ({ hourlyRate, tubePrice: 0, tubeLifeH: 0, watts: 100, accessoryWatts: 0 });

/** Estado local de «esta computadora»: una plantilla, un retazo, un costo, un perfil de material. */
async function seedLocal() {
  const base = sampleDevices()[0]!;
  await api.saveTemplate({ name: 'Mesa', doc: doc('local') });
  await api.saveRemnants({ library: { version: 1, remnants: [remnant('r1')] } });
  const costs = defaultStored('es', 'es-MX');
  costs.machines = { [base.id]: machine(10) };
  costs.materials = [{ name: 'MDF', mode: 'bbox', material: { sheetPrice: 50, sheetW: 600, sheetH: 400, wastePct: 10, piecesPerSheet: 1 } }];
  writeCosts(costs);
  useMaterials.setState({ library: { version: 1, presets: [sampleMaterialLibrary().presets[0]!] }, loaded: true });
}

/** Lo que trae «la otra computadora»: choca en una máquina, un retazo, una plantilla, un costo y un material. */
function otherBundle(): UserBundle {
  const base = sampleDevices()[0]!;
  const lib = sampleMaterialLibrary();
  const source: BundleSource = {
    devices: [{ ...base, name: 'Cambiada en la otra' }, { ...base, id: 'solo-alla', name: 'Solo allá' }],
    activeDeviceId: base.id,
    library: { version: 1, presets: [{ ...lib.presets[0]!, title: 'Distinto' }, lib.presets[1]!] },
    prefs: { ...DEFAULT_PREFS, snapPx: 25 },
    shortcuts: {},
    costs: {
      ...defaultStored('es', 'es-MX'),
      machines: { [base.id]: machine(99), 'solo-alla': machine(5) },
      materials: [
        { name: 'MDF', mode: 'area', material: { sheetPrice: 80, sheetW: 600, sheetH: 400, wastePct: 10, piecesPerSheet: 1 } },
        { name: 'Acrílico', mode: 'bbox', material: { sheetPrice: 120, sheetW: 600, sheetH: 400, wastePct: 5, piecesPerSheet: 1 } },
      ],
    },
    remnants: { version: 1, remnants: [remnant('r1', { name: 'Cambiado' }), remnant('r2')] },
    templates: [{ name: 'Mesa', doc: doc('otra') }, { name: 'Taza', doc: doc('taza') }],
    history: [{ id: 'h1', at: 10 }, { id: 'h2', at: 20 }],
  };
  return buildUserBundle(source, { sections: { ...defaultSections(), history: true }, app: '0.9.0' });
}

beforeEach(async () => {
  setLanguage('es');
  api = createMockApi({ latencyMs: 0 });
  setApi(api);
  localStorage.clear();
  useMaterials.setState({ library: emptyLibrary(), loaded: false, loadError: null });
  useRemnants.setState({ library: emptyRemnants(), loaded: false, loadError: null });
  useStore.setState({ jobState: 'idle', notices: [] });
  useBundleUi.setState({ mode: null, source: null });
  useStore.getState().replacePrefs({ prefs: { ...DEFAULT_PREFS }, shortcuts: {} }, false);
  await useStore.getState().loadDevices();
  await seedLocal();
});

afterEach(cleanup);

const section = (host: HTMLElement, id: string) => host.querySelector<HTMLElement>(`.bnd-section[data-section="${id}"]`)!;
const checkbox = (host: HTMLElement, id: string) => section(host, id).querySelector<HTMLInputElement>('input[type="checkbox"]')!;
const button = (host: HTMLElement, label: string) => [...host.querySelectorAll('button')].find((b) => b.textContent === label)!;

async function choose(select: HTMLSelectElement, value: string) {
  await act(async () => {
    select.value = value;
    select.dispatchEvent(new Event('change', { bubbles: true }));
  });
}

async function openPreview() {
  await api.exportUserBundle({ path: 'otra.tlakubundle', bundle: otherBundle() });
  const m = await mount(<BundleDialog mode="import" onClose={() => {}} source={{ path: 'otra.tlakubundle', createdMs: 12345 }} />);
  await until(() => !!m.host.querySelector('.bnd-section'));
  return m;
}

describe('versión del formato', () => {
  it('sin secciones nuevas sigue siendo versión 1; con ellas, versión 2', async () => {
    const src = (await currentState()) as unknown as BundleSource;
    expect(buildUserBundle(src, { sections: { devices: true, materials: true, prefs: true } }).version).toBe(1);
    expect(buildUserBundle(src, { sections: { costs: true } }).version).toBe(2);
    expect(buildUserBundle(src, { sections: { history: true } }).version).toBe(2);
  });

  it('un paquete de la versión 1 se lee y se aplica como siempre', async () => {
    const src = (await currentState()) as unknown as BundleSource;
    const v1 = buildUserBundle(src, { sections: { devices: true, materials: true, prefs: true } });
    const plan = planImport(v1, await currentState(v1));
    expect(plan.costs).toBeNull();
    expect(plan.remnants).toBeNull();
    expect(plan.templates).toBeNull();
    expect(plan.history).toBeNull();
  });
});

describe('vista previa de importar', () => {
  it('resume cada categoría: nuevos, distintos y sin cambio', async () => {
    const m = await openPreview();
    const text = (id: string) => section(m.host, id).textContent ?? '';
    expect(text('devices')).toContain('1 nuevas · 1 distintas con el mismo id');
    expect(text('materials')).toContain('1 nuevos · 1 distintos con el mismo id');
    expect(text('costs')).toContain('Costos por máquina: 1 nuevos · 1 distintos · 0 iguales');
    expect(text('costs')).toContain('Perfiles de material: 1 nuevos · 1 distintos · 0 iguales');
    expect(text('remnants')).toContain('1 nuevos · 1 distintos con el mismo id · 0 iguales');
    expect(text('templates')).toContain('1 nuevas · 1 distintas con el mismo nombre · 0 iguales');
    expect(text('history')).toContain('2 entradas nuevas');
    expect(text('prefs')).toMatch(/\d+ preferencias cambiarían/);
    expect(m.host.textContent).toContain('Antes de aplicar se guarda un respaldo del estado actual');
  });

  it('cada categoría con conflictos ofrece reemplazar, conservar ambos u omitir', async () => {
    const m = await openPreview();
    for (const id of ['devices', 'materials', 'costs', 'remnants', 'templates']) {
      const select = section(m.host, id).querySelector('select');
      expect(select, id).toBeTruthy();
      expect([...select!.options].map((o) => o.value)).toEqual(['replace', 'keepBoth', 'skip']);
    }
    // Las que no tienen conflictos de id no ofrecen elegir.
    expect(section(m.host, 'prefs').querySelector('select')).toBeNull();
    expect(section(m.host, 'history').querySelector('select')).toBeNull();
    expect(section(m.host, 'devices').querySelector('select')!.value).toBe('replace');
    expect(section(m.host, 'templates').querySelector('select')!.value).toBe('keepBoth');
  });

  it('la vista previa no escribe nada', async () => {
    await openPreview();
    expect(useStore.getState().devices.find((d) => d.id === 'solo-alla')).toBeUndefined();
    expect(readCosts('es', 'es-MX').machines['solo-alla']).toBeUndefined();
    expect((await api.listTemplates()).map((t) => t.name)).toEqual(['Mesa']);
    expect((await api.loadRemnants()).remnants).toHaveLength(1);
    expect(await api.jobHistoryLoad({ max: null })).toHaveLength(0);
    expect(api.backupSim.safetyCount()).toBe(0);
  });

  it('elegir por categoría: lo desmarcado no se importa y cada conflicto sigue su política', async () => {
    const m = await openPreview();
    await click(checkbox(m.host, 'remnants')); // retazos fuera
    await click(checkbox(m.host, 'history')); // historial fuera
    await choose(section(m.host, 'devices').querySelector('select')!, 'keepBoth');
    await choose(section(m.host, 'templates').querySelector('select')!, 'skip');
    await choose(section(m.host, 'costs').querySelector('select')!, 'replace');
    await choose(section(m.host, 'materials').querySelector('select')!, 'replace');
    await click(button(m.host, 'Aplicar'));
    await applied();

    // Respaldo del estado anterior, antes de escribir.
    expect(api.backupSim.safetyCount()).toBe(1);

    // Máquinas: la local se conserva y la otra entra renombrada; la nueva también.
    const devices = useStore.getState().devices;
    const base = sampleDevices()[0]!;
    expect(devices.find((d) => d.id === base.id)!.name).toBe(base.name);
    expect(devices.find((d) => d.name === 'Cambiada en la otra (importado)')).toBeTruthy();
    expect(devices.find((d) => d.id === 'solo-alla')).toBeTruthy();

    // Materiales: reemplaza el del mismo id y agrega el nuevo.
    const presets = useMaterials.getState().library.presets;
    expect(presets.find((p) => p.id === sampleMaterialLibrary().presets[0]!.id)!.title).toBe('Distinto');
    expect(presets).toHaveLength(2);

    // Costos: reemplaza lo distinto y agrega lo nuevo.
    const costs = readCosts('es', 'es-MX');
    expect(costs.machines[base.id]!.hourlyRate).toBe(99);
    expect(costs.machines['solo-alla']!.hourlyRate).toBe(5);
    expect(costs.materials.map((p) => p.name).sort()).toEqual(['Acrílico', 'MDF']);
    expect(costs.materials.find((p) => p.name === 'MDF')!.mode).toBe('area');

    // Plantillas: omitir deja la local y agrega la nueva.
    expect((await api.listTemplates()).map((t) => t.name).sort()).toEqual(['Mesa', 'Taza']);
    expect(label(await api.loadTemplate({ name: 'Mesa' }))).toBe('local');

    // Retazos e historial no se tocaron (casillas desmarcadas).
    expect((await api.loadRemnants()).remnants.map((r) => r.id)).toEqual(['r1']);
    expect(await api.jobHistoryLoad({ max: null })).toHaveLength(0);

    // Preferencias aplicadas y el respaldo origen anotado como visto.
    expect(useStore.getState().prefs.snapPx).toBe(25);
    expect(useStore.getState().notices.at(-1)?.level).toBe('success');
    expect((await api.backupConfigGet()).last_seen_ms).toBe(12345);
  });

  it('conservar ambos en retazos y plantillas agrega copias renombradas sin tocar lo local', async () => {
    const m = await openPreview();
    await click(button(m.host, 'Aplicar'));
    await applied();
    const remnants = (await api.loadRemnants()).remnants;
    expect(remnants.map((r) => r.id).sort()).toEqual(['r1', 'r1-imp', 'r2']);
    expect(remnants.find((r) => r.id === 'r1')!.name).toBe('Retazo r1');
    expect(remnants.find((r) => r.id === 'r1-imp')!.name).toBe('Cambiado (importado)');
    expect((await api.listTemplates()).map((t) => t.name).sort()).toEqual(['Mesa', 'Mesa (importado)', 'Taza']);
    expect(label(await api.loadTemplate({ name: 'Mesa' }))).toBe('local');
    expect((await api.jobHistoryLoad({ max: null })).map((e) => e.id)).toEqual(['h1', 'h2']);
  });

  it('con todo omitido solo entra lo nuevo', async () => {
    const m = await openPreview();
    for (const id of ['devices', 'materials', 'costs', 'remnants', 'templates']) await choose(section(m.host, id).querySelector('select')!, 'skip');
    await click(button(m.host, 'Aplicar'));
    await applied();
    const base = sampleDevices()[0]!;
    expect(useStore.getState().devices.find((d) => d.id === base.id)!.name).toBe(base.name);
    expect(useStore.getState().devices.find((d) => d.id === 'solo-alla')).toBeTruthy();
    expect(readCosts('es', 'es-MX').machines[base.id]!.hourlyRate).toBe(10);
    expect((await api.loadRemnants()).remnants.map((r) => r.id).sort()).toEqual(['r1', 'r2']);
  });
});

describe('respaldo previo a importar', () => {
  it('guarda el estado actual completo y sin historial', async () => {
    expect(await saveSafetyBackup()).toBe(true);
    expect(api.backupSim.safetyCount()).toBe(1);
    const bundle = await buildCurrentBundle({ ...defaultSections() });
    expect(bundle.history).toBeUndefined();
    expect(bundle.templates?.items.map((i) => i.name)).toEqual(['Mesa']);
    expect(bundle.remnants?.remnants.map((r) => r.id)).toEqual(['r1']);
  });

  it('si no se puede guardar, no se aplica nada', async () => {
    const plan = planImport(otherBundle(), await currentState(otherBundle()));
    setApi({ ...api, backupSafety: () => Promise.reject(new Error('disco lleno')) });
    const result = await applyImport(plan, defaultSections(), 'otra.tlakubundle', defaultPolicies());
    expect(result.blocked).toBe(true);
    expect(useStore.getState().devices.find((d) => d.id === 'solo-alla')).toBeUndefined();
    expect(readCosts('es', 'es-MX').machines['solo-alla']).toBeUndefined();
    expect(useStore.getState().prefs.snapPx).not.toBe(25);
    const last = useStore.getState().notices.at(-1)!;
    expect(last.level).toBe('error');
    expect(last.text).toContain('disco lleno');
  });

  it('con un trabajo en curso tampoco se guarda ni se aplica', async () => {
    const plan = planImport(otherBundle(), await currentState(otherBundle()));
    useStore.setState({ jobState: 'running' });
    const result = await applyImport(plan, defaultSections());
    expect(result.blocked).toBe(true);
    expect(api.backupSim.safetyCount()).toBe(0);
  });
});

describe('conteo de cambios según la política', () => {
  it('omitir descuenta los conflictos; reemplazar y conservar ambos los cuentan', async () => {
    const plan = planImport(otherBundle(), await currentState(otherBundle()));
    const replace = sectionChanges(plan, { ...defaultPolicies(), templates: 'replace', remnants: 'replace' });
    const skip = sectionChanges(plan, { devices: 'skip', materials: 'skip', costs: 'skip', remnants: 'skip', templates: 'skip' });
    expect(replace.templates).toBe(2);
    expect(skip.templates).toBe(1);
    expect(replace.remnants).toBe(2);
    expect(skip.remnants).toBe(1);
    expect(replace.costs).toBeGreaterThan(skip.costs);
    expect(getApi()).toBe(api);
  });
});
