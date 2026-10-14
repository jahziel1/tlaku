import { renderToString } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import type { MaterialPreset } from '@/api/types';
import { createMockApi, type MockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import { sampleMaterialLibrary } from '@/api/mock/materials';
import { commands } from '@/commands';
import { importPaths } from '@/features/history/files';
import { loadMaterials } from '@/features/materials/actions';
import { describePick } from '@/features/materials/pick';
import { useMaterials } from '@/features/materials/state';
import { emptyLibrary } from '@/features/materials/library';
import { en, es, setLanguage } from '@/i18n';
import { simple as esSimple } from '@/i18n/es/simple';
import { makeLayer } from '@/lib/layers';
import { bindMachineEvents, selectActiveDevice, useStore } from '@/store';
import {
  baseKnobs,
  chooseAction,
  chooseMaterial,
  choicesFor,
  chooseWatts,
  connectVirtual,
  currentChoice,
  currentMachine,
  fitToBed,
  goToStep,
  lessBurn,
  moreCutPasses,
  openExample,
  placeAt,
  resetToMaterial,
  saveAsMyMaterial,
  setDarkness,
  setNumber,
  startJob,
} from './actions';
import { needsFirstRun, setUiMode, toggleUiMode } from './mode';
import {
  advanceBlock,
  colorGroups,
  cutBurnLess,
  cutMorePasses,
  designBox,
  fitFactor,
  knobChanges,
  knobsOf,
  layerWithAction,
  materialChoices,
  moveToAnchor,
  roleOf,
  type PickContext,
  tuneEngrave,
  validate,
  type Knobs,
} from './model';
import { ChangeList, Swatch, Values, actionBlock } from './ColorCard';
import { SimplePanel } from './SimplePanel';
import { useSimple } from './store';

const RED = '#ff3b4a';
const BLUE = '#2f7bff';
const AMBER = '#ffb020';
const GRAY = '#9aa4b2';
const DEVICE = 'falcon2-22w';

const emptyDoc = () => ({ version: 2 as const, objects: [], layers: [], device_id: DEVICE });
const sampleDoc = () => ({ version: 2 as const, objects: sampleObjects(), layers: sampleLayers(), device_id: DEVICE });
const layer = (color: string) => useStore.getState().doc.layers.find((l) => l.color === color)!;
const mdf = () => currentChoice()!;

const NO_FACTORY = { version: 1 as const, presets: [], restrictions: [] };
/** Contexto de elección con la biblioteca de fábrica vacía (solo los ajustes del usuario) y potencia desconocida. */
const ctx = (user: MaterialPreset[], patch: Partial<PickContext> = {}): PickContext => ({
  user,
  factory: NO_FACTORY,
  deviceId: DEVICE,
  kind: 'diode',
  machineClass: null,
  ...patch,
});
/** Los materiales que se ofrecen ahora mismo, con la biblioteca del usuario y la de fábrica ya leídas. */
const choicesNow = () => {
  const m = useMaterials.getState();
  return choicesFor(m.library.presets, m.factory, DEVICE, currentMachine());
};

let api: MockApi;
let off: () => void;

beforeEach(async () => {
  vi.stubGlobal('localStorage', { getItem: () => null, setItem: () => undefined });
  api = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 100, jobSpeedup: 1e6 });
  setApi(api);
  off = await bindMachineEvents(api);
  const s = useStore.getState();
  await s.loadDevices();
  s.setActiveDevice(DEVICE);
  chooseWatts(null);
  s.replaceDocument(emptyDoc(), null);
  s.setPref('uiMode', 'full');
  s.setPreviewVisible(false);
  useSimple.getState().reset();
  useMaterials.setState({ library: emptyLibrary(), loaded: false, loadError: null });
  await loadMaterials(true);
});

afterEach(async () => {
  if (useStore.getState().connection.connected) await useStore.getState().disconnect();
  off();
  api.dispose();
  vi.unstubAllGlobals();
});

/** Abre el diseño de ejemplo (4 colores: corte, grabado, marcas y foto) y elige MDF 3 mm. */
async function openSampleAndPickMdf() {
  expect(await importPaths(['ejemplos/placa.svg'])).toBeGreaterThan(0);
  const c = choicesNow().find((x) => x.material === 'MDF' && x.thickness === 3)!;
  chooseMaterial(c.key);
  return c;
}

describe('modelo: color y material', () => {
  it('agrupa el diseño por color y reconoce imágenes, textos y trazos abiertos', () => {
    const groups = colorGroups(sampleDoc());
    expect(groups.map((g) => g.color)).toEqual([RED, BLUE, AMBER, GRAY]);
    expect(groups.find((g) => g.color === GRAY)).toMatchObject({ isImage: true, count: 1 });
    expect(groups.find((g) => g.color === RED)).toMatchObject({ isImage: false, texts: 0 });
    // La estrella de marcas trae dos líneas abiertas.
    expect(groups.find((g) => g.color === AMBER)!.openPaths).toBe(2);
    expect(groups.find((g) => g.color === BLUE)!.openPaths).toBe(0);
  });

  it('deduce el papel de un ajuste por su modo, o usa el que declare', () => {
    const [cut, engrave] = sampleMaterialLibrary().presets;
    expect(roleOf(cut!)).toBe('cut');
    expect(roleOf(engrave!)).toBe('engrave');
    expect(roleOf({ ...cut!, role: 'engrave' } as typeof cut & { role: string })).toBe('engrave');
  });

  it('un material sin grabado (o sin corte) lo dice con el motivo', () => {
    const choices = materialChoices(ctx(sampleMaterialLibrary().presets));
    const acrylic = choices.find((c) => c.material === 'Acrílico negro')!;
    expect(acrylic.cut).not.toBeNull();
    expect(acrylic.engrave).toBeNull();
    expect(acrylic.engraveWhy).toBe('simple.why.noEngrave');
    const leather = choices.find((c) => c.material === 'Cuero vegetal')!;
    expect(leather.cut).toBeNull();
    expect(leather.cutWhy).toBe('simple.why.noCut');
    expect(choices.find((c) => c.material === 'MDF')!.label).toBe('MDF 3 mm');
  });

  it('prefiere los ajustes de la máquina activa, luego los de su clase y marca los generales', () => {
    const [cut, engrave] = sampleMaterialLibrary().presets;
    const mine = { ...cut!, id: 'mio', device_id: DEVICE, settings: { ...cut!.settings, speed: 123 } };
    const other = { ...cut!, id: 'otro', device_id: 'otra-maquina', settings: { ...cut!.settings, speed: 999 } };
    const generic = materialChoices(ctx([cut!]))[0]!;
    expect(generic.cut).toMatchObject({ specific: false, match: 'generic' });
    const both = materialChoices(ctx([cut!, mine, other]))[0]!;
    expect(both.cut).toMatchObject({ specific: true, match: 'device' });
    expect(both.cut!.preset.settings.speed).toBe(123);
    // Un ajuste de la clase de la máquina gana al genérico.
    const ofClass = { ...cut!, id: 'clase', machine_class: 'diode_10' as const, settings: { ...cut!.settings, speed: 222 } };
    const byClass = materialChoices(ctx([cut!, ofClass], { machineClass: 'diode_10' }))[0]!;
    expect(byClass.cut).toMatchObject({ match: 'class' });
    expect(byClass.cut!.preset.settings.speed).toBe(222);
    // Solo hay corte de otra máquina: no sirve y se explica (el grabado sí aparece).
    const onlyOther = materialChoices(ctx([engrave!, other]))[0]!;
    expect(onlyOther.cut).toBeNull();
    expect(onlyOther.cutWhy).toBe('simple.why.otherMachine');
    expect(onlyOther.engrave).not.toBeNull();
  });

  it('con la biblioteca del usuario vacía, la de fábrica da materiales a quien empieza', async () => {
    useMaterials.setState({ library: emptyLibrary() });
    const labels = choicesNow().map((c) => c.label);
    expect(labels.length).toBeGreaterThan(5);
    // Los nombres salen traducidos.
    expect(labels.some((l) => l.startsWith('MDF'))).toBe(true);
    expect(labels.some((l) => l.includes('Acrílico negro'))).toBe(true);
    expect(labels.some((l) => l.includes('Contrachapado'))).toBe(true);
    const mdf3 = choicesNow().find((c) => c.material === 'MDF' && c.thickness === 3)!;
    expect(mdf3.cut).not.toBeNull();
    expect(mdf3.cut!.preset.material_key).toBe('mdf');
  });

  it('los nombres de fábrica cambian con el idioma', () => {
    setLanguage('en');
    try {
      const labels = choicesNow().map((c) => c.label);
      expect(labels.some((l) => l.includes('Black acrylic'))).toBe(true);
      expect(labels.some((l) => l.includes('Acrílico'))).toBe(false);
    } finally {
      setLanguage('es');
    }
  });

  it('un material del usuario con el nombre de uno de fábrica se junta con él (un solo MDF 3 mm)', () => {
    expect(choicesNow().filter((c) => c.material === 'MDF' && c.thickness === 3)).toHaveLength(1);
  });

  it('el acrílico transparente con un diodo no se puede cortar y se explica', () => {
    const clear = choicesNow().find((c) => c.key.startsWith('acrylic_clear'))!;
    expect(clear).toBeDefined();
    expect(clear.cut).toBeNull();
    expect(clear.cutWhy).toBe('simple.why.restricted');
    expect(clear.restriction?.warning).toBe('not_cut_with_diode');
    const group = colorGroups(sampleDoc())[0]!;
    expect(actionBlock('cut', group, clear)).toBe('simple.why.restricted');
    const p = validate({ doc: sampleDoc(), device: { bed: [400, 415] }, connected: true, choice: clear, actions: {} }).find((x) => x.id === 'restricted')!;
    expect(p.level).toBe('block');
    expect(String(p.params!.text)).toContain('CO2');
    // Con un láser de CO2 sí se puede cortar.
    const m = useMaterials.getState();
    const co2 = choicesFor(m.library.presets, m.factory, DEVICE, { kind: 'co2', machineClass: 'co2_60' }).find((c) => c.key.startsWith('acrylic_clear'))!;
    expect(co2.cut).not.toBeNull();
    expect(co2.restriction).toBeNull();
  });

  it('con la potencia desconocida usa la clase más baja y lo avisa; con la potencia dicha, no', () => {
    const unknown = choicesNow().find((c) => c.material === 'MDF' && c.thickness === 3)!;
    expect(unknown.cut!.powerUnknown).toBe(true);
    expect(currentMachine()).toEqual({ kind: 'diode', machineClass: null });
    useMaterials.setState({ library: emptyLibrary() });
    const m = useMaterials.getState();
    const known = choicesFor(m.library.presets, m.factory, DEVICE, { kind: 'diode', machineClass: 'diode_20' }).find((c) => c.material === 'MDF' && c.thickness === 3)!;
    expect(known.cut).toMatchObject({ powerUnknown: false, match: 'class', usedClass: 'diode_20' });
  });

  it('Cortar y Grabar copian el ajuste correcto; Ignorar apaga la salida; las imágenes se graban como imagen', () => {
    const choice = materialChoices(ctx(sampleMaterialLibrary().presets)).find((c) => c.material === 'MDF')!;
    const base = makeLayer(RED, 'Corte');
    const cut = layerWithAction(base, 'cut', choice.cut);
    expect(cut).toMatchObject({ mode: 'line', speed: 300, power: 100, passes: 2, output: true, color: RED, name: 'Corte' });
    const engr = layerWithAction(base, 'engrave', choice.engrave);
    expect(engr).toMatchObject({ mode: 'fill', speed: 3000, power: 35 });
    const photo = layerWithAction(makeLayer(GRAY, 'Foto', { mode: 'image' }), 'engrave', choice.engrave);
    expect(photo.mode).toBe('image');
    expect(photo.power).toBe(35);
    expect(layerWithAction(base, 'ignore', null).output).toBe(false);
    // Sin ajuste disponible no se toca la capa.
    expect(layerWithAction(base, 'cut', null)).toBe(base);
  });
});

describe('modelo: ajustes finos', () => {
  const base: Knobs = { speed: 3000, power: 35, passes: 1 };

  it('el centro de la escala claro/oscuro es el valor del material', () => {
    expect(tuneEngrave(base, 0, 6000)).toEqual(base);
  });

  it('oscuro gasta más energía por milímetro y claro menos, siempre dentro de límites', () => {
    const energy = (k: Knobs) => k.power / k.speed;
    const steps = [-3, -2, -1, 0, 1, 2, 3].map((s) => tuneEngrave(base, s, 6000));
    for (let i = 1; i < steps.length; i++) expect(energy(steps[i]!)).toBeGreaterThan(energy(steps[i - 1]!));
    expect(energy(steps[6]!) / energy(base)).toBeCloseTo(1.25 ** 3, 1);
    for (const k of steps) {
      expect(k.power).toBeGreaterThanOrEqual(1);
      expect(k.power).toBeLessThanOrEqual(100);
      expect(k.speed).toBeGreaterThanOrEqual(1);
    }
    // Oscuro sube la potencia o baja la velocidad; claro al revés.
    expect(steps[6]!.power).toBeGreaterThan(base.power);
    expect(steps[0]!.power).toBeLessThan(base.power);
  });

  it('con la potencia ya al 100 %, lo oscuro lo pone la velocidad', () => {
    const full = tuneEngrave({ speed: 300, power: 100, passes: 1 }, 3, 6000);
    expect(full.power).toBe(100);
    expect(full.speed).toBeLessThan(300);
    // La velocidad claro no pasa de la máxima de la máquina.
    expect(tuneEngrave({ speed: 5900, power: 50, passes: 1 }, -3, 6000).speed).toBeLessThanOrEqual(6000);
  });

  it('"No atraviesa" suma una pasada hasta un tope; "Quema mucho" baja potencia y sube velocidad', () => {
    expect(cutMorePasses({ speed: 300, power: 100, passes: 2 }).passes).toBe(3);
    expect(cutMorePasses({ speed: 300, power: 100, passes: 10 }).passes).toBe(10);
    const less = cutBurnLess({ speed: 300, power: 100, passes: 2 }, 6000);
    expect(less.power).toBe(95);
    expect(less.speed).toBe(330);
    expect(less.passes).toBe(2);
    expect(cutBurnLess({ speed: 5900, power: 100, passes: 1 }, 6000).speed).toBe(6000);
  });

  it('lista lo que cambió respecto del material', () => {
    expect(knobChanges(base, base)).toEqual([]);
    expect(knobChanges(base, { speed: 3000, power: 30, passes: 2 })).toEqual([
      { field: 'power', from: 35, to: 30 },
      { field: 'passes', from: 1, to: 2 },
    ]);
  });
});

describe('modelo: validaciones amigables', () => {
  const device = { bed: [400, 415] as [number, number] };
  const input = (doc = sampleDoc(), extra = {}) => ({ doc, device, connected: true, choice: null, actions: {}, ...extra });
  const ids = (p: ReturnType<typeof validate>) => p.map((x) => x.id);

  it('sin máquina conectada ofrece conectar o probar la virtual', () => {
    const p = validate(input(sampleDoc(), { connected: false })).find((x) => x.id === 'noMachine')!;
    expect(p).toMatchObject({ level: 'block', fix: 'connect', key: 'simple.problem.noMachine' });
    expect(ids(validate(input()))).not.toContain('noMachine');
  });

  it('avisa si el diseño se sale de la cama y ofrece reducirlo o dividirlo', () => {
    const doc = sampleDoc();
    doc.objects[0] = { ...doc.objects[0]!, transform: [1, 0, 0, 1, 300, 70] };
    const p = validate(input(doc)).find((x) => x.id === 'outside')!;
    expect(p).toMatchObject({ level: 'block', fix: 'fit' });
    expect(es['simple.problem.outside']).toContain('tramos');
    expect(ids(validate(input()))).not.toContain('outside');
  });

  it('un material sin ajustes para esta máquina sugiere la prueba de materiales', () => {
    const none = { ...materialChoices(ctx(sampleMaterialLibrary().presets))[0]!, cut: null, engrave: null, restriction: null };
    expect(validate(input(sampleDoc(), { choice: none })).find((x) => x.id === 'noSettings')).toMatchObject({ level: 'block', fix: 'test' });
  });

  it('siempre dice que son valores de partida y suma los avisos del ajuste', () => {
    const [cut] = sampleMaterialLibrary().presets;
    const factory = { ...cut!, id: 'f', material_key: 'mdf', machine_class: 'diode_10' as const, role: 'cut' as const, warnings: ['range_edge' as const, 'needs_coating' as const] };
    const choice = materialChoices(ctx([], { factory: { presets: [factory], restrictions: [] }, machineClass: 'diode_10' }))[0]!;
    const found = validate(input(sampleDoc(), { choice, actions: { [RED]: 'cut' } }));
    expect(found.find((x) => x.id === 'startingPoint')).toMatchObject({ level: 'warn', fix: 'test' });
    expect(found.map((x) => x.id)).toEqual(expect.arrayContaining(['warning:range_edge', 'warning:needs_coating']));
    expect(found.find((x) => x.id === 'warning:range_edge')!.params!.text).toBe(es['materials.warn.range_edge']);
    expect(ids(found)).not.toContain('powerUnknown');
    // Aun con ajustes guardados para la máquina, el recordatorio sigue.
    const own = materialChoices(ctx([{ ...cut!, device_id: DEVICE }]))[0]!;
    expect(ids(validate(input(sampleDoc(), { choice: own, actions: { [RED]: 'cut' } })))).toContain('startingPoint');
  });

  it('avisa si no se conoce la potencia o si se usa la clase más cercana', () => {
    const [cut] = sampleMaterialLibrary().presets;
    const factory = { ...cut!, id: 'f', material_key: 'mdf', machine_class: 'diode_10' as const, role: 'cut' as const };
    const unknown = materialChoices(ctx([], { factory: { presets: [factory], restrictions: [] } }))[0]!;
    expect(ids(validate(input(sampleDoc(), { choice: unknown, actions: { [RED]: 'cut' } })))).toContain('powerUnknown');
    const near = materialChoices(ctx([], { factory: { presets: [factory], restrictions: [] }, machineClass: 'diode_40' }))[0]!;
    const p = validate(input(sampleDoc(), { choice: near, actions: { [RED]: 'cut' } })).find((x) => x.id === 'nearest')!;
    expect(p.params).toEqual({ used: es['materials.class.diode_10'] });
  });

  it('avisa de textos sin convertir y de trazos abiertos en capas de corte (solo avisa)', () => {
    const doc = sampleDoc();
    doc.objects.push({
      id: 'txt',
      layer: BLUE,
      transform: [1, 0, 0, 1, 0, 0],
      kind: { type: 'text', text: 'Hola', font: 'x', size: 10, outline: [] } as unknown as (typeof doc.objects)[number]['kind'],
      locked: false,
      hidden: false,
      group: null,
    });
    const found = validate(input(doc, { actions: { [AMBER]: 'cut', [BLUE]: 'engrave' } }));
    expect(found.find((x) => x.id === 'texts')).toMatchObject({ level: 'warn', params: { n: 1 } });
    expect(found.find((x) => x.id === 'openCut')).toMatchObject({ level: 'warn', params: { n: 2 } });
    // Si el color con trazos abiertos se graba o ignora, no avisa de corte.
    expect(ids(validate(input(doc, { actions: { [AMBER]: 'ignore' } })))).not.toContain('openCut');
  });

  it('no hay nada que enviar si todo está ignorado', () => {
    const doc = sampleDoc();
    doc.layers = doc.layers.map((l) => ({ ...l, output: false }));
    expect(validate(input(doc)).find((x) => x.id === 'nothing')?.level).toBe('block');
  });

  it('cada paso dice por qué no se puede avanzar', () => {
    const doc = sampleDoc();
    const g = { doc, choice: null, actions: {}, problems: [], previewReady: false };
    expect(advanceBlock('open', { ...g, doc: emptyDoc() })).toBe('simple.need.design');
    expect(advanceBlock('open', g)).toBeNull();
    expect(advanceBlock('material', g)).toBe('simple.need.material');
    const choice = materialChoices(ctx(sampleMaterialLibrary().presets))[0]!;
    expect(advanceBlock('material', { ...g, choice })).toBe('simple.need.actions');
    const all = { [RED]: 'ignore', [BLUE]: 'ignore', [AMBER]: 'ignore', [GRAY]: 'ignore' } as const;
    expect(advanceBlock('material', { ...g, choice, actions: all })).toBe('simple.need.oneAction');
    expect(advanceBlock('material', { ...g, choice, actions: { ...all, [RED]: 'cut' } })).toBeNull();
    expect(advanceBlock('review', g)).toBe('simple.need.preview');
    expect(advanceBlock('review', { ...g, previewReady: true })).toBeNull();
  });
});

describe('modelo: colocar', () => {
  const bed = [400, 400] as const;
  const box: [[number, number], [number, number]] = [[10, 10], [110, 60]];

  it('el centro deja el diseño en el medio de la cama', () => {
    const [dx, dy] = moveToAnchor(box, bed, 4);
    expect(box[0][0] + dx + 50).toBeCloseTo(200);
    expect(box[0][1] + dy + 25).toBeCloseTo(200);
  });

  it('las esquinas dejan un pequeño margen; arriba es la Y más alta', () => {
    const [dx, dy] = moveToAnchor(box, bed, 0);
    expect(box[0][0] + dx).toBeCloseTo(5);
    expect(box[1][1] + dy).toBeCloseTo(395);
    const [bx, by] = moveToAnchor(box, bed, 8);
    expect(box[1][0] + bx).toBeCloseTo(395);
    expect(box[0][1] + by).toBeCloseTo(5);
  });

  it('calcula cuánto reducir para que quepa, y no agranda', () => {
    expect(fitFactor([[0, 0], [100, 50]], bed)).toBe(1);
    expect(fitFactor([[0, 0], [800, 100]], bed)).toBeCloseTo(390 / 800);
  });
});

describe('flujo completo contra el mock', () => {
  it('abrir, material, acciones, colocar, revisar e iniciar en la máquina virtual', async () => {
    const s = () => useStore.getState();
    // 1. abrir
    expect(await importPaths(['ejemplos/placa.svg'])).toBe(4);
    expect(advanceBlock('open', { doc: s().doc, choice: null, actions: {}, problems: [], previewReady: false })).toBeNull();

    // 2. material y acción por color
    goToStep(1);
    const choice = choicesNow().find((c) => c.material === 'MDF' && c.thickness === 3)!;
    chooseMaterial(choice.key);
    expect(mdf().label).toBe('MDF 3 mm');
    chooseAction(RED, 'cut');
    chooseAction(BLUE, 'engrave');
    chooseAction(AMBER, 'ignore');
    chooseAction(GRAY, 'engrave');
    expect(layer(RED)).toMatchObject({ mode: 'line', speed: 300, power: 100, passes: 2, output: true });
    expect(layer(BLUE)).toMatchObject({ mode: 'fill', speed: 3000, power: 35, output: true });
    expect(layer(AMBER).output).toBe(false);
    expect(layer(GRAY)).toMatchObject({ mode: 'image', power: 35, output: true });

    // Cada decisión es un solo paso de deshacer.
    const before = s().past.length;
    chooseAction(RED, 'ignore');
    expect(s().past.length).toBe(before + 1);
    expect(layer(RED).output).toBe(false);
    s().undo();
    expect(layer(RED)).toMatchObject({ mode: 'line', output: true });
    chooseAction(RED, 'cut');

    // 3. colocar
    goToStep(2);
    expect(s().activeTool).toBe('select');
    expect(s().selectedIds).toHaveLength(s().doc.objects.length);
    expect(placeAt(4)).toBe(true);
    const box = designBox(s().doc)!;
    const [bw, bh] = selectActiveDevice(s()).bed;
    expect((box[0][0] + box[1][0]) / 2).toBeCloseTo(bw / 2, 1);
    expect((box[0][1] + box[1][1]) / 2).toBeCloseTo(bh / 2, 1);

    // 4. revisar
    goToStep(3);
    expect(s().previewVisible).toBe(true);
    await vi.waitFor(() => expect(s().previewStatus).toBe('ready'));
    expect(s().preview!.total_s).toBeGreaterThan(0);
    const data = { doc: s().doc, choice: mdf(), actions: useSimple.getState().actions, problems: [], previewReady: true };
    expect(advanceBlock('review', data)).toBeNull();
    // Sin máquina: se puede revisar, pero no iniciar, y se ofrece conectar.
    const problems = validate({ doc: s().doc, device: selectActiveDevice(s()), connected: false, choice: mdf(), actions: useSimple.getState().actions });
    expect(problems.find((p) => p.id === 'noMachine')?.fix).toBe('connect');

    // 5. iniciar en la máquina virtual
    goToStep(4);
    expect(await connectVirtual()).toBe(true);
    await vi.waitFor(() => expect(s().status).not.toBeNull());
    startJob();
    expect(s().dialog).toEqual({ kind: 'start' });
    await s().confirmStart();
    await vi.waitFor(() => expect(['running', 'finished']).toContain(s().jobState));
    await vi.waitFor(() => expect(s().jobState).toBe('finished'));
  });

  it('el ejemplo de la biblioteca de arte entra al diseño', async () => {
    expect(await openExample()).toBe(true);
    expect(useStore.getState().doc.objects.length).toBeGreaterThan(0);
    expect(colorGroups(useStore.getState().doc).length).toBeGreaterThan(0);
  });

  it('un diseño fuera de la cama se puede reducir para que quepa', async () => {
    await importPaths(['ejemplos/placa.svg']);
    const s = useStore.getState();
    const [bw] = selectActiveDevice(s).bed;
    // Agranda el diseño al doble del ancho de la cama.
    const k = (2 * bw) / (designBox(s.doc)![1][0] - designBox(s.doc)![0][0]);
    s.applyDocChange('history.transform', (doc) => ({ ...doc, objects: doc.objects.map((o) => ({ ...o, transform: [o.transform[0] * k, o.transform[1], o.transform[2], o.transform[3] * k, o.transform[4] * k, o.transform[5] * k] })) }));
    const problems = () => validate({ doc: useStore.getState().doc, device: selectActiveDevice(useStore.getState()), connected: true, choice: null, actions: {} });
    expect(problems().map((p) => p.id)).toContain('outside');
    expect(fitToBed()).toBe(true);
    expect(problems().map((p) => p.id)).not.toContain('outside');
  });
});

describe('ajustes por color contra las capas reales', () => {
  const s = () => useStore.getState();

  it('más claro / más oscuro mueve potencia y velocidad juntas, en un solo paso de deshacer', async () => {
    await openSampleAndPickMdf();
    chooseAction(BLUE, 'engrave');
    expect(baseKnobs(BLUE)).toEqual({ speed: 3000, power: 35, passes: 1 });
    const before = s().past.length;
    setDarkness(BLUE, 3);
    expect(s().past.length).toBe(before + 1);
    expect(layer(BLUE).power).toBeGreaterThan(35);
    expect(layer(BLUE).speed).toBeLessThanOrEqual(3000);
    expect(useSimple.getState().tune[BLUE]).toBe(3);
    const dark = layer(BLUE).power / layer(BLUE).speed;
    setDarkness(BLUE, -3);
    expect(layer(BLUE).power / layer(BLUE).speed).toBeLessThan(dark);
    setDarkness(BLUE, 0);
    expect(layer(BLUE)).toMatchObject({ speed: 3000, power: 35 });
    s().undo();
    expect(layer(BLUE).power / layer(BLUE).speed).toBeLessThan(dark);
  });

  it('los botones de corte cambian pasadas, potencia y velocidad', async () => {
    await openSampleAndPickMdf();
    chooseAction(RED, 'cut');
    moreCutPasses(RED);
    expect(layer(RED).passes).toBe(3);
    lessBurn(RED);
    expect(layer(RED)).toMatchObject({ power: 95, speed: 330, passes: 3 });
    // Volver al material deja los valores originales.
    resetToMaterial(RED);
    expect(layer(RED)).toMatchObject({ power: 100, speed: 300, passes: 2 });
  });

  it('"Ver números" aplica a la capa y vuelve a modo manual la escala', async () => {
    await openSampleAndPickMdf();
    chooseAction(BLUE, 'engrave');
    setNumber(BLUE, 'power', 50);
    expect(layer(BLUE).power).toBe(50);
    expect(useSimple.getState().tune[BLUE]).toBeNull();
    // Se ve igual al pasar al modo completo: son las mismas capas.
    setUiMode('simple');
    setUiMode('full');
    expect(layer(BLUE).power).toBe(50);
  });

  it('guardar como mi material crea un ajuste del usuario para esta máquina', async () => {
    await openSampleAndPickMdf();
    chooseAction(RED, 'cut');
    lessBurn(RED);
    const before = useMaterials.getState().library.presets.length;
    expect(await saveAsMyMaterial(RED)).toBe(true);
    const { presets } = useMaterials.getState().library;
    expect(presets).toHaveLength(before + 1);
    const mine = presets.at(-1)!;
    expect(mine).toMatchObject({ material: 'MDF', thickness: 3, device_id: DEVICE, title: es['simple.mine.cut'], role: 'cut' });
    // Con la potencia desconocida no se inventa la clase.
    expect(mine.machine_class).toBeUndefined();
    expect(mine.settings).toMatchObject({ power: 95, speed: 330, passes: 2 });
    // Quedó guardado en la biblioteca (lo que el motor devolverá la próxima vez).
    expect((await api.loadMaterialLibrary()).presets.at(-1)!.id).toBe(mine.id);
    // Y ahora es el ajuste preferido de MDF 3 mm para esta máquina: ya no hay diferencias que mostrar.
    expect(mdf().cut).toMatchObject({ specific: true, match: 'device' });
    expect(knobChanges(baseKnobs(RED)!, knobsOf(layer(RED)))).toEqual([]);
    expect(s().notices.at(-1)?.text).toContain('MDF');
  });

  it('no guarda nada si el color se ignora o no hay material', async () => {
    await importPaths(['ejemplos/placa.svg']);
    expect(await saveAsMyMaterial(RED)).toBe(false);
    await openSampleAndPickMdf();
    chooseAction(RED, 'ignore');
    expect(await saveAsMyMaterial(RED)).toBe(false);
  });
});

describe('biblioteca de fábrica y potencia del láser', () => {
  it('al entrar al modo sencillo se leen los materiales aunque nunca se abriera el panel', async () => {
    useMaterials.setState({ library: emptyLibrary(), factory: emptyLibrary(), loaded: false, loadError: null });
    expect(useMaterials.getState().factory.presets).toHaveLength(0);
    setUiMode('simple');
    await vi.waitFor(() => expect(useMaterials.getState().factory.presets.length).toBeGreaterThan(50));
    expect(useMaterials.getState().loaded).toBe(true);
    expect(choicesNow().length).toBeGreaterThan(5);
  });

  it('decir los watts recuerda la clase de la máquina y cambia el ajuste elegido', async () => {
    useMaterials.setState({ library: emptyLibrary() });
    await importPaths(['ejemplos/placa.svg']);
    expect(currentMachine().machineClass).toBeNull();
    chooseWatts('diode_20');
    expect(currentMachine()).toEqual({ kind: 'diode', machineClass: 'diode_20' });
    const c = choicesNow().find((x) => x.material === 'MDF' && x.thickness === 3)!;
    expect(c.cut).toMatchObject({ match: 'class', powerUnknown: false, usedClass: 'diode_20' });
    chooseMaterial(c.key);
    chooseAction(RED, 'cut');
    expect(layer(RED)).toMatchObject({ mode: 'line', speed: c.cut!.preset.settings.speed, power: c.cut!.preset.settings.power });
    // "No lo sé" vuelve a la clase más prudente.
    chooseWatts(null);
    expect(currentMachine().machineClass).toBeNull();
  });

  it('guardar como mi material pone el rol y la clase de la máquina', async () => {
    chooseWatts('diode_10');
    await importPaths(['ejemplos/placa.svg']);
    const c = choicesNow().find((x) => x.material === 'MDF' && x.thickness === 3)!;
    chooseMaterial(c.key);
    chooseAction(BLUE, 'engrave');
    setDarkness(BLUE, 2);
    expect(await saveAsMyMaterial(BLUE)).toBe(true);
    const mine = useMaterials.getState().library.presets.at(-1)!;
    expect(mine).toMatchObject({ role: 'engrave', machine_class: 'diode_10', device_id: DEVICE, title: es['simple.mine.engrave'] });
    expect(mine.settings.power).toBe(layer(BLUE).power);
    // Lo guardado gana a lo de fábrica en la próxima elección.
    expect(mdf().engrave).toMatchObject({ match: 'device' });
    expect(mdf().engrave!.preset.id).toBe(mine.id);
  });

  it('cada operación dice de dónde salió su ajuste con el texto de la biblioteca', async () => {
    chooseWatts('diode_10');
    const c = choicesNow().find((x) => x.material === 'MDF' && x.thickness === 3)!;
    const d = describePick(c.cut!.result, 'diode_10');
    expect(d.key).toBe('materials.pick.class');
    expect(es[d.key]).toContain('{class}');
  });
});

describe('modo sencillo y modo completo', () => {
  it('pasar de un modo a otro y volver no toca el documento ni el historial', async () => {
    await openSampleAndPickMdf();
    chooseAction(RED, 'cut');
    goToStep(1);
    const s = () => useStore.getState();
    const { doc, past } = s();
    setUiMode('simple');
    expect(s().prefs.uiMode).toBe('simple');
    expect(s().activeTool).toBe('select');
    setUiMode('full');
    expect(s().prefs.uiMode).toBe('full');
    expect(s().doc).toBe(doc);
    expect(s().past).toBe(past);
    // El paso, el material y las decisiones siguen ahí.
    setUiMode('simple');
    expect(useSimple.getState()).toMatchObject({ step: 1, actions: { [RED]: 'cut' } });
    expect(currentChoice()?.label).toBe('MDF 3 mm');
  });

  it('el comando "simple.toggle" alterna y cambia su etiqueta', async () => {
    const s = () => useStore.getState();
    expect(commands.label('simple.toggle', s())).toBe('simple.cmd.toSimple');
    await commands.run('simple.toggle');
    expect(s().prefs.uiMode).toBe('simple');
    expect(commands.label('simple.toggle', s())).toBe('simple.cmd.toFull');
    expect(commands.checked('simple.toggle', s())).toBe(true);
    toggleUiMode();
    expect(s().prefs.uiMode).toBe('full');
  });

  it('la pregunta de primer arranque solo sale si nunca se eligió y ya se leyeron las preferencias', () => {
    const s = useStore.getState();
    expect(needsFirstRun({ prefs: { ...s.prefs, uiMode: 'ask' }, prefsLoaded: false })).toBe(false);
    expect(needsFirstRun({ prefs: { ...s.prefs, uiMode: 'ask' }, prefsLoaded: true })).toBe(true);
    expect(needsFirstRun({ prefs: { ...s.prefs, uiMode: 'simple' }, prefsLoaded: true })).toBe(false);
    expect(needsFirstRun({ prefs: { ...s.prefs, uiMode: 'full' }, prefsLoaded: true })).toBe(false);
  });

  it('uiMode se sanea y arranca sin elegir', async () => {
    const { sanitizePref, DEFAULT_PREFS } = await import('@/store/prefs');
    expect(DEFAULT_PREFS.uiMode).toBe('ask');
    expect(sanitizePref('uiMode', 'simple')).toBe('simple');
    expect(sanitizePref('uiMode', 'otro')).toBe('ask');
  });
});

describe('interfaz', () => {
  it('el panel dibuja la barra de pasos y la tarjeta del paso actual', async () => {
    await importPaths(['ejemplos/placa.svg']);
    const html = renderToString(<SimplePanel />);
    for (const label of ['Diseño', 'Material', 'Colocar', 'Revisar', 'Iniciar']) expect(html).toContain(label);
    expect(html).toContain('Paso 1: tu diseño');
    expect(html).toContain('Abrir un archivo');
    expect(html).toContain('Siguiente');
    expect(html).toContain('Atrás');
  });

  it('los valores que se usarán se muestran siempre, en texto simple', () => {
    const l = makeLayer(RED, 'Corte', { speed: 300, power: 100, passes: 2 });
    expect(renderToString(<Values layer={l} galvo={false} />)).toContain('Velocidad 300 mm/min · Potencia 100 % · Pasadas 2');
    // En una máquina galvo la velocidad se dice en mm/s.
    expect(renderToString(<Values layer={{ ...l, speed: 6000 }} galvo />)).toContain('Velocidad 100,0 mm/s');
  });

  it('lo que cambió respecto del material se muestra con el antes y el después', () => {
    const html = renderToString(<ChangeList base={{ speed: 300, power: 100, passes: 2 }} cur={{ speed: 330, power: 95, passes: 3 }} galvo={false} />);
    expect(html).toContain('Potencia: 100,0 → 95,0 %');
    expect(html).toContain('Velocidad: 300 mm/min → 330 mm/min');
    expect(html).toContain('Pasadas: 2 → 3');
    expect(renderToString(<ChangeList base={{ speed: 1, power: 1, passes: 1 }} cur={{ speed: 1, power: 1, passes: 1 }} galvo={false} />)).toBe('');
  });

  it('Cortar y Grabar se deshabilitan con el motivo cuando el material o la capa no los admiten', () => {
    const [red, , , gray] = colorGroups(sampleDoc());
    const choices = materialChoices(ctx(sampleMaterialLibrary().presets));
    const leather = choices.find((c) => c.material === 'Cuero vegetal')!;
    const acrylic = choices.find((c) => c.material === 'Acrílico negro')!;
    expect(actionBlock('cut', red!, null)).toBe('simple.why.pickMaterial');
    expect(actionBlock('ignore', red!, null)).toBeNull();
    expect(actionBlock('cut', red!, leather)).toBe('simple.why.noCut');
    expect(actionBlock('engrave', red!, leather)).toBeNull();
    expect(actionBlock('engrave', red!, acrylic)).toBe('simple.why.noEngrave');
    expect(actionBlock('cut', gray!, acrylic)).toBe('simple.why.imageNoCut');
  });

  it('la muestra de un color dibuja sus formas; una imagen usa un ícono', () => {
    const doc = sampleDoc();
    const groups = colorGroups(doc);
    expect(renderToString(<Swatch group={groups[0]!} doc={doc} />)).toContain('<path');
    expect(renderToString(<Swatch group={groups[3]!} doc={doc} />)).not.toContain('<path d="M');
  });

  it('todos los textos del área existen en español e inglés', () => {
    for (const key of Object.keys(esSimple)) {
      expect(key.startsWith('simple.'), key).toBe(true);
      expect((en as Record<string, string>)[key], key).toBeTruthy();
    }
  });
});
