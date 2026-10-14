import { renderToString } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { expandFactory, factoryMaterialLibrary, sampleMaterialLibrary } from '@/api/mock/materials';
import { en, es, setLanguage } from '@/i18n';
import { makeLayer } from '@/lib/layers';
import { selectActiveDevice, useStore } from '@/store';
import { applyMaterial, chooseMachineClass, duplicateMaterial, loadMaterials, persistMaterials } from './actions';
import { classLabel, confidenceLabel, materialName, titleName, warningText } from './labels';
import { emptyLibrary } from './library';
import { MACHINE_CLASSES, classKind, resolveMachineClass, storedClassFor } from './machineClass';
import { MaterialsPanelView, type PanelViewProps } from './MaterialsPanel';
import { useMaterials } from './state';

const RED = '#ff3b4a';

function reset(api = createMockApi({ latencyMs: 0 })) {
  setApi(api);
  setLanguage('es');
  useMaterials.setState({ library: emptyLibrary(), factory: emptyLibrary(), machineClass: null, onlyClass: true, loaded: false, loadError: null, search: '', onlyDevice: false, editor: null });
  const s = useStore.getState();
  s.replaceDocument({ version: 2, objects: [], layers: [makeLayer(RED, 'Corte')], device_id: null }, null);
  s.setActiveLayer(RED);
  return api;
}

beforeEach(() => reset());
afterEach(() => setLanguage('es'));

describe('biblioteca de fábrica incluida', () => {
  const factory = factoryMaterialLibrary();

  it('todos los ajustes traen fuente (https), confianza, clase y rol', () => {
    expect(factory.presets.length).toBeGreaterThan(50);
    for (const p of factory.presets) {
      expect(p.source, p.id).toMatch(/^https:\/\/[^\s]+\.[^\s]+$/);
      expect(p.confidence, p.id).toMatch(/^(official|guide|forum)$/);
      expect(MACHINE_CLASSES, p.id).toContain(p.machine_class);
      expect(p.role, p.id).toBeDefined();
      expect(p.id.startsWith('factory.'), p.id).toBe(true);
      expect(p.device_id).toBeNull();
    }
    expect(new Set(factory.presets.map((p) => p.id)).size).toBe(factory.presets.length);
  });

  it('el modo, la velocidad y la frecuencia salen del rol y de los datos', () => {
    const get = (id: string) => factory.presets.find((p) => p.id === id)!;
    const cut = get('factory.basswood.3.cut.diode_20');
    expect(cut.settings).toMatchObject({ mode: 'line', speed: 500, power: 100, passes: 1, air: true });
    const eng = get('factory.mdf.x.engrave.co2_60');
    expect(eng.settings).toMatchObject({ mode: 'fill', speed: 72000 });
    expect(eng.settings.interval).toBeCloseTo(25.4 / 300, 3);
    const white = get('factory.stainless_steel.x.mark_white.fiber_20');
    expect(white.settings.galvo.frequency_khz).toBe(35);
    expect(white.settings.crosshatch).toBe(true);
  });

  it('cubre los materiales pedidos para al menos un tipo de láser y no inventa los que no tienen dato', () => {
    const keys = new Set(factory.presets.map((p) => p.material_key));
    for (const k of ['mdf', 'plywood_birch', 'acrylic_black', 'acrylic_clear', 'leather', 'cardboard', 'paper', 'cork', 'slate', 'anodized_aluminum', 'stainless_steel', 'glass']) {
      expect(keys.has(k), k).toBe(true);
    }
    // Sin dato en ninguna fuente: no hay preset (ni de pino, ni de MDF de 6 mm).
    expect(factory.presets.some((p) => p.material.toLowerCase().includes('pino'))).toBe(false);
    expect(factory.presets.some((p) => p.material_key === 'mdf' && p.thickness === 6)).toBe(false);
    // El acrílico transparente no tiene ajustes de diodo.
    const clearKinds = factory.presets.filter((p) => p.material_key === 'acrylic_clear').map((p) => classKind(p.machine_class!));
    expect(clearKinds).not.toContain('diode');
  });

  it('todo dato de fábrica tiene texto en español e inglés', () => {
    for (const lang of ['es', 'en'] as const) {
      setLanguage(lang);
      for (const p of factory.presets) {
        expect(materialName(p), p.id).not.toMatch(/^materials\./);
        expect(materialName(p), p.id).not.toBe(p.material_key);
        expect(titleName(p), p.id).not.toBe(p.title_key);
        for (const w of p.warnings ?? []) expect(warningText(w)).not.toMatch(/^materials\./);
      }
      for (const c of MACHINE_CLASSES) expect(classLabel(c)).not.toMatch(/^materials\./);
    }
    setLanguage('en');
    expect(materialName(factory.presets.find((p) => p.material_key === 'plywood_birch')!)).toBe('Birch plywood');
  });

  it('las claves de i18n nuevas existen en español y en inglés', () => {
    const wanted = ['materials.section.factory', 'materials.section.mine', 'materials.notice.startingPoints', 'materials.pick.nearest', 'materials.warn.range_edge'];
    for (const k of wanted) {
      expect(Object.keys(es)).toContain(k);
      expect(Object.keys(en)).toContain(k);
    }
  });

  it('los aportes de la comunidad entran al final con confianza «foro» y se muestran como «Comunidad (sin verificar)» en es y en', () => {
    const data = { revision: 1, materials: { basswood: 'Madera de tilo' }, titles: { cut: 'Corte', engrave: 'Grabado' }, restrictions: [], presets: [] };
    const entry = {
      id: 'community.felt.x.engrave.diode_10.9', material: 'felt', material_name: 'Fieltro', role: 'engrave' as const, machine_class: 'diode_10' as const, laser_watts: 10,
      machine: 'Mi láser', speed: 3000, power: 40, interval: 0.1, source: 'https://github.com/jahziel1/tlaku/issues/9', confidence: 'forum' as const, license: 'CC0-1.0', notes: 'Buen contraste.',
    };
    const lib = expandFactory(data, [entry]);
    const p = lib.presets.at(-1)!;
    expect(p).toMatchObject({ id: entry.id, material: 'Fieltro', material_key: 'felt', title: 'Grabado', confidence: 'forum', notes: 'Mi láser · Buen contraste.' });
    expect(p.settings).toMatchObject({ mode: 'fill', speed: 3000, power: 40, interval: 0.1 });
    expect(materialName(p)).toBe('Fieltro');
    setLanguage('es');
    expect(confidenceLabel('forum')).toBe('Comunidad (sin verificar)');
    setLanguage('en');
    expect(confidenceLabel('forum')).toBe('Community (unverified)');
  });

  it('expandFactory conserva avisos, restricciones y revisión', () => {
    const lib = expandFactory({
      revision: 7,
      materials: { m: 'Material' },
      titles: { cut: 'Corte' },
      restrictions: [{ material: 'm', laser: 'diode', role: null, warning: 'not_cut_with_diode', source: 'https://x.example', confidence: 'official' }],
      presets: [{ id: 'factory.m', material: 'm', role: 'cut', title: 'cut', machine_class: 'diode_10', speed: 100, power: 50, source: 'https://x.example', confidence: 'guide', warnings: ['range_edge'] }],
    });
    expect(lib.revision).toBe(7);
    expect(lib.presets[0]).toMatchObject({ material: 'Material', title: 'Corte', warnings: ['range_edge'], thickness: null });
    expect(lib.restrictions).toHaveLength(1);
  });
});

describe('carga y uso', () => {
  it('lee las dos bibliotecas por separado y la de fábrica no entra a la del usuario', async () => {
    await loadMaterials();
    expect(useMaterials.getState().factory.presets.length).toBeGreaterThan(50);
    expect(useMaterials.getState().library.presets).toHaveLength(sampleMaterialLibrary().presets.length);
    expect(useMaterials.getState().library.presets.some((p) => p.id.startsWith('factory.'))).toBe(false);
  });

  it('guardar la biblioteca del usuario manda solo lo del usuario', async () => {
    const api = reset();
    const save = vi.spyOn(api, 'saveMaterialLibrary');
    await loadMaterials();
    await persistMaterials();
    const sent = save.mock.calls.at(-1)?.[0].library;
    expect(sent?.presets.every((p) => !p.id.startsWith('factory.'))).toBe(true);
  });

  it('si falla la de fábrica, la del usuario se carga igual y se puede guardar', async () => {
    const base = createMockApi({ latencyMs: 0 });
    reset({ ...base, loadFactoryMaterials: () => Promise.reject(new Error('rota')) });
    await loadMaterials();
    expect(useMaterials.getState().loaded).toBe(true);
    expect(useMaterials.getState().loadError).toBeNull();
    expect(useMaterials.getState().factory.presets).toHaveLength(0);
    expect(useStore.getState().notices.at(-1)?.level).toBe('error');
  });

  it('duplicar uno de fábrica lo copia a Mis materiales, traducido y editable, con su fuente', async () => {
    await loadMaterials();
    const source = useMaterials.getState().factory.presets.find((p) => p.id === 'factory.basswood.3.cut.diode_20')!;
    const before = useMaterials.getState().library.presets.length;
    duplicateMaterial(source.id);
    const mine = useMaterials.getState().library.presets;
    expect(mine).toHaveLength(before + 1);
    const copy = mine.at(-1)!;
    expect(copy.id.startsWith('factory.')).toBe(false);
    expect(copy).toMatchObject({ material: 'Madera de tilo', title: 'Corte', source: source.source, confidence: 'official', machine_class: 'diode_20', role: 'cut' });
    expect(copy.material_key).toBeUndefined();
    expect(copy.title_key).toBeUndefined();
    expect(copy.settings).toEqual(source.settings);
    // La de fábrica sigue igual y no se duplicó ahí.
    expect(useMaterials.getState().factory.presets.find((p) => p.id === source.id)).toEqual(source);
  });

  it('aplica un ajuste de fábrica a la capa activa', async () => {
    await loadMaterials();
    const p = useMaterials.getState().factory.presets.find((x) => x.id === 'factory.basswood.3.cut.diode_20')!;
    expect(applyMaterial(p)).toBe(true);
    expect(useStore.getState().doc.layers[0]).toMatchObject({ color: RED, name: 'Corte', speed: 500, power: 100 });
  });

  it('la clase elegida se recuerda por máquina', () => {
    const device = selectActiveDevice(useStore.getState());
    chooseMachineClass(device.id, 'diode_10');
    expect(useMaterials.getState().machineClass).toBe('diode_10');
    expect(storedClassFor(device.id)).toBe('diode_10');
    expect(resolveMachineClass(device).machineClass).toBe('diode_10');
    chooseMachineClass(device.id, null);
    expect(storedClassFor(device.id)).toBeNull();
  });
});

describe('panel con biblioteca de fábrica', () => {
  const device = selectActiveDevice(useStore.getState());
  const test = { enabled: true, tooltip: 'Prueba de materiales…', label: 'Prueba de materiales…', run: () => {} };
  const html = (props: Partial<PanelViewProps> = {}) =>
    renderToString(
      <MaterialsPanelView
        library={sampleMaterialLibrary()}
        factory={factoryMaterialLibrary()}
        loaded
        loadError={null}
        search=""
        onlyDevice={false}
        devices={[]}
        device={device}
        layers={[makeLayer(RED, 'Corte')]}
        activeLayer={makeLayer(RED, 'Corte')}
        test={test}
        {...props}
      />,
    );

  it('separa "Mis materiales" y "De fábrica" y siempre muestra el aviso de puntos de partida', () => {
    const h = html();
    expect(h).toContain('Mis materiales');
    expect(h).toContain('De fábrica');
    expect(h).toContain('Son puntos de partida: haz una prueba de materiales con tu máquina.');
    expect(h.indexOf('Mis materiales')).toBeLessThan(h.indexOf('De fábrica'));
    // El aviso sigue aunque no haya nada cargado.
    expect(html({ factory: emptyLibrary(), library: emptyLibrary() })).toContain(es['materials.notice.startingPoints']);
  });

  it('cada ajuste de fábrica muestra su fuente con enlace y su confianza', () => {
    const h = html();
    expect(h).toContain('href="https://eu.crealityfalcon.com/');
    expect(h).toContain('rel="noopener noreferrer"');
    expect(h).toContain('Confianza: Oficial');
    expect(h).toContain('Confianza: Guía');
    expect(h).toContain('Copiar enlace');
  });

  it('los de fábrica son de solo lectura: se duplican, no se editan ni se borran', () => {
    const h = html({ library: emptyLibrary() });
    expect(h).toContain('Duplicar Madera de tilo · Corte');
    expect(h).not.toContain('Editar Madera de tilo');
    expect(h).not.toContain('Borrar Madera de tilo');
    // En Mis materiales sí hay editar y borrar.
    const mine = html();
    expect(mine).toContain('Editar MDF · Corte');
    expect(mine).toContain('Borrar MDF · Corte');
  });

  it('con una clase elegida filtra la fábrica por ella y muestra los avisos de los ajustes', () => {
    const all = html({ machineClass: 'diode_10', onlyClass: false });
    const mine = html({ machineClass: 'diode_10', onlyClass: true });
    expect(all).toContain('CO2 100 W');
    expect(mine).not.toContain('CO2 100 W');
    expect(mine).toContain('Diodo 10 W');
    expect(mine).toContain('Solo mi clase (Diodo 10 W)');
    expect(mine).toContain('No cortes ni grabes cuero curtido al cromo');
  });

  it('avisa del acrílico transparente con un diodo y avisa si el ajuste pide más velocidad de la que llega la máquina', () => {
    const h = html();
    expect(h).toContain('El acrílico transparente no se corta con diodo');
    // La Falcon2 de ejemplo llega a 25 000 mm/min: los 72 000 del CO2 de 60 W superan ese límite.
    expect(html({ onlyClass: false, machineClass: null })).toContain('comprueba que tu máquina alcance esa velocidad');
  });

  it('la búsqueda encuentra por el nombre traducido y por la clase', () => {
    expect(html({ search: 'abedul', library: emptyLibrary() })).toContain('Contrachapado de abedul');
    const h = html({ search: 'fibra 50', library: emptyLibrary() });
    expect(h).toContain('Acero inoxidable');
    expect(h).not.toContain('Contrachapado de abedul');
  });

  it('en inglés se traduce todo', () => {
    setLanguage('en');
    const h = html({ library: emptyLibrary() });
    expect(h).toContain('My materials');
    expect(h).toContain('Factory');
    expect(h).toContain('These are starting points: run a material test with your machine.');
    expect(h).toContain('Birch plywood');
    expect(h).toContain('Confidence: Official');
  });
});
