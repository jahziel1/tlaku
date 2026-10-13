import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import { createMockApi } from '@/api/mock';
import { mockMaterialTest, sampleMaterialLibrary } from '@/api/mock/materials';
import { setApi } from '@/api';
import { commands } from '@/commands';
import { materialCommands } from '@/commands/definitions/materials';
import { usePrompts } from '@/features/history/prompt';
import { es } from '@/i18n';
import { materials as dict } from '@/i18n/es/materials';
import { makeLayer } from '@/lib/layers';
import { selectActiveDevice, useStore } from '@/store';
import {
  applyMaterial,
  applyReasonFor,
  deleteMaterial,
  duplicateMaterial,
  importMaterials,
  loadMaterials,
  openTestAsNewDocument,
  persistMaterials,
  savePresetFromActiveLayer,
  updatePreset,
} from './actions';
import { emptyLibrary } from './library';
import { MaterialsPanelView, type PanelViewProps } from './MaterialsPanel';
import { useMaterials } from './state';
import { GridPreview } from './TestDialog';
import { defaultForm } from './testPlan';

const RED = '#ff3b4a';
const BLUE = '#2f7bff';

function reset(api = createMockApi({ latencyMs: 0 })) {
  setApi(api);
  useMaterials.setState({ library: emptyLibrary(), loaded: false, loadError: null, search: '', onlyDevice: false, editor: null, testOpen: false, testForm: null });
  const s = useStore.getState();
  s.replaceDocument({ version: 2, objects: [], layers: [makeLayer(RED, 'Corte', { speed: 280, power: 95 }), makeLayer(BLUE, 'Grabado')], device_id: null }, null);
  s.setActiveLayer(RED);
  return api;
}

const lastNotice = () => useStore.getState().notices.at(-1);

beforeEach(() => {
  reset();
});

describe('textos', () => {
  it('todas las claves del área empiezan con materials.', () => {
    for (const key of Object.keys(dict)) expect(key.startsWith('materials.'), key).toBe(true);
  });
});

describe('mock', () => {
  it('trae ejemplos de MDF, contrachapado, acrílico y cuero', () => {
    const names = sampleMaterialLibrary().presets.map((p) => p.material);
    for (const m of ['MDF', 'Contrachapado de abedul', 'Acrílico negro', 'Cuero vegetal']) expect(names).toContain(m);
    expect(sampleMaterialLibrary().presets.find((p) => p.material === 'MDF')?.thickness).toBe(3);
  });

  it('la prueba simulada es una cuadrícula equivalente sin rótulos', () => {
    const form = defaultForm(null);
    const doc = mockMaterialTest({
      mode: 'line', rows: form.rows, cols: form.cols, cell: [10, 10], gap: 3, base: makeLayer('#000000', ''),
      labels: true, label_speed: 1500, label_power: 15, origin: [5, 5],
    });
    expect(doc.objects).toHaveLength(25);
    expect(doc.layers).toHaveLength(25);
    expect(new Set(doc.layers.map((l) => l.color)).size).toBe(25);
    expect(doc.layers[0]).toMatchObject({ power: 10, speed: 500, name: 'P 10% · V 500' });
    expect(doc.layers[24]).toMatchObject({ power: 100, speed: 3000 });
    // La fila 0 es la de arriba; la celda (0,0) arranca en x = 5.
    const first = doc.objects[0]!.kind;
    expect(first.type === 'vector' && first.paths[0]![0]).toEqual({ type: 'move_to', to: [5, 5 + 4 * 13] });
  });

  it('la prueba simulada valida como el motor', () => {
    const base = defaultForm(null);
    const params = (patch: object) => ({
      mode: 'line' as const, rows: base.rows, cols: base.cols, cell: [10, 10] as [number, number], gap: 3, base: makeLayer('#000000', ''),
      labels: false, label_speed: 1500, label_power: 15, origin: [0, 0] as [number, number], ...patch,
    });
    expect(() => mockMaterialTest(params({ rows: { ...base.rows, count: 31 } }))).toThrow(/filas/);
    expect(() => mockMaterialTest(params({ rows: { ...base.rows, to: 150 } }))).toThrow(/0 y 100/);
    expect(() => mockMaterialTest(params({ cols: { param: 'interval', count: 3, from: 0.1, to: 0.3 } }))).toThrow(/line/);
    expect(() => mockMaterialTest(params({ cell: [0, 5] }))).toThrow(/celda/);
  });
});

describe('acciones de la biblioteca', () => {
  it('carga la biblioteca de ejemplo del mock', async () => {
    await loadMaterials();
    expect(useMaterials.getState().loaded).toBe(true);
    expect(useMaterials.getState().library.presets).toHaveLength(5);
  });

  it('guarda la capa activa como material y lo persiste', async () => {
    await loadMaterials();
    const p = savePresetFromActiveLayer({ material: 'Pino', thickness: 5, title: 'Corte', notes: 'x', device_id: null });
    expect(p).not.toBeNull();
    expect(p!.settings).toMatchObject({ speed: 280, power: 95 });
    await persistMaterials();
    const reloaded = await useMaterials.getState().library;
    expect(reloaded.presets.at(-1)?.material).toBe('Pino');
    expect(lastNotice()?.level).toBe('success');
  });

  it('sin capa activa no guarda nada', async () => {
    await loadMaterials();
    useStore.getState().setActiveLayer(null);
    expect(savePresetFromActiveLayer({ material: 'Pino', thickness: 5, title: 'Corte', notes: '', device_id: null })).toBeNull();
    expect(useMaterials.getState().library.presets).toHaveLength(5);
  });

  it('edita los datos y, si se pide, los ajustes con los de la capa activa', async () => {
    await loadMaterials();
    const id = useMaterials.getState().library.presets[0]!.id;
    updatePreset(id, { material: 'MDF', thickness: 4, title: 'Corte fino', notes: '', device_id: 'x' }, false);
    let p = useMaterials.getState().library.presets.find((x) => x.id === id)!;
    expect(p).toMatchObject({ thickness: 4, title: 'Corte fino', device_id: 'x' });
    expect(p.settings.speed).toBe(300);
    updatePreset(id, { material: 'MDF', thickness: 4, title: 'Corte fino', notes: '', device_id: 'x' }, true);
    p = useMaterials.getState().library.presets.find((x) => x.id === id)!;
    expect(p.settings).toMatchObject({ speed: 280, power: 95 });
  });

  it('duplica con un id nuevo', async () => {
    await loadMaterials();
    const before = useMaterials.getState().library.presets;
    duplicateMaterial(before[0]!.id);
    const after = useMaterials.getState().library.presets;
    expect(after).toHaveLength(before.length + 1);
    expect(new Set(after.map((p) => p.id)).size).toBe(after.length);
    expect(after.at(-1)?.title).toContain(before[0]!.title);
  });

  it('borra solo si se confirma', async () => {
    await loadMaterials();
    const id = useMaterials.getState().library.presets[0]!.id;
    const pending = deleteMaterial(id);
    expect(usePrompts.getState().current?.title).toBe(es['materials.delete.title']);
    usePrompts.getState().answer('cancel');
    expect(await pending).toBe(false);
    expect(useMaterials.getState().library.presets).toHaveLength(5);

    const again = deleteMaterial(id);
    usePrompts.getState().answer('delete');
    expect(await again).toBe(true);
    expect(useMaterials.getState().library.presets.some((p) => p.id === id)).toBe(false);
  });

  it('aplica a la capa activa como un solo paso de deshacer y conserva color, nombre y orden', async () => {
    await loadMaterials();
    const mdf = useMaterials.getState().library.presets.find((p) => p.id === 'mat-mdf3-grabado')!;
    const before = useStore.getState().past.length;
    expect(applyMaterial(mdf)).toBe(true);
    const layer = useStore.getState().doc.layers.find((l) => l.color === RED)!;
    expect(layer).toMatchObject({ name: 'Corte', color: RED, mode: 'fill', speed: 3000, power: 35, interval: 0.1 });
    expect(useStore.getState().past).toHaveLength(before + 1);
    useStore.getState().undo();
    expect(useStore.getState().doc.layers.find((l) => l.color === RED)).toMatchObject({ mode: 'line', speed: 280, power: 95 });
  });

  it('explica por qué no se puede aplicar', async () => {
    await loadMaterials();
    const p = useMaterials.getState().library.presets[0]!;
    expect(applyReasonFor(null, p)).toBe('materials.reason.noLayer');
    useStore.getState().setActiveLayer(null);
    expect(applyMaterial(p)).toBe(false);
    expect(lastNotice()?.level).toBe('warning');
    useStore.getState().setActiveLayer(RED);
    applyMaterial(p);
    expect(applyReasonFor(useStore.getState().doc.layers[0]!, p)).toBe('materials.reason.alreadyApplied');
  });

  it('importar mezcla: el id que choca entra como copia con id nuevo y el resto se agrega', async () => {
    await loadMaterials();
    expect(await importMaterials()).toBe(true);
    const presets = useMaterials.getState().library.presets;
    // 5 de ejemplo + el que chocaba (copia con id nuevo) + la balsa.
    expect(presets).toHaveLength(7);
    expect(new Set(presets.map((p) => p.id)).size).toBe(7);
    expect(presets.filter((p) => p.material === 'MDF' && p.title === 'Corte')).toHaveLength(2);
    expect(presets.some((p) => p.id === 'mat-balsa-corte')).toBe(true);
    // Importar otra vez el mismo archivo no duplica.
    await importMaterials();
    expect(useMaterials.getState().library.presets).toHaveLength(7);
  });
});

describe('prueba como documento nuevo', () => {
  it('reemplaza el documento por la cuadrícula cuando no hay cambios sin guardar', async () => {
    const form = { ...defaultForm(null), rows: { param: 'power' as const, count: 2, from: 20, to: 40 }, cols: { param: 'speed' as const, count: 3, from: 600, to: 1000 } };
    expect(useStore.getState().dirty).toBe(false);
    expect(await openTestAsNewDocument(form)).toBe(true);
    const s = useStore.getState();
    expect(s.doc.objects).toHaveLength(6);
    expect(s.doc.layers).toHaveLength(6);
    expect(s.filePath).toBeNull();
    expect(s.dirty).toBe(false);
  });

  it('con cambios sin guardar pregunta primero y, si se cancela, no toca el documento', async () => {
    useStore.getState().updateLayer(RED, { power: 10 });
    expect(useStore.getState().dirty).toBe(true);
    const doc = useStore.getState().doc;
    const pending = openTestAsNewDocument(defaultForm(null));
    await new Promise((r) => setTimeout(r, 10));
    expect(usePrompts.getState().current).not.toBeNull();
    usePrompts.getState().answer('cancel');
    expect(await pending).toBe(false);
    expect(useStore.getState().doc).toBe(doc);
  });

  it('un error del motor avisa y no toca el documento', async () => {
    const doc = useStore.getState().doc;
    const bad = { ...defaultForm(null), cell: [0, 0] as [number, number] };
    expect(await openTestAsNewDocument(bad)).toBe(false);
    expect(lastNotice()?.level).toBe('error');
    expect(useStore.getState().doc).toBe(doc);
  });
});

describe('comando', () => {
  it('"Prueba de materiales…" está en el menú Herramientas y abre el diálogo', () => {
    expect(materialCommands.map((c) => c.category)).toEqual(['tools']);
    const def = commands.get('tools.materialTest');
    expect(def).toBeDefined();
    void commands.run('tools.materialTest');
    expect(useMaterials.getState().testOpen).toBe(true);
  });
});

describe('panel', () => {
  const device = selectActiveDevice(useStore.getState());
  const test = { enabled: true, tooltip: 'Prueba de materiales…', label: 'Prueba de materiales…', run: () => {} };
  const html = (props: Partial<PanelViewProps> = {}) =>
    renderToString(
      <MaterialsPanelView
        library={sampleMaterialLibrary()}
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

  it('sin materiales muestra el estado vacío tras cargar', () => {
    const h = html({ library: emptyLibrary() });
    expect(h).toContain(es['materials.panel.empty']);
    expect(h).toContain('Guardar capa como material');
    expect(h).toContain(es['materials.reason.emptyLibrary']);
  });

  it('mientras carga lo dice, y si falla ofrece reintentar', () => {
    expect(html({ library: emptyLibrary(), loaded: false })).toContain(es['materials.loading']);
    const h = html({ library: emptyLibrary(), loaded: false, loadError: 'boom' });
    expect(h).toContain('boom');
    expect(h).toContain('Reintentar');
  });

  it('agrupa por material y grosor con sus botones', () => {
    const h = html();
    for (const m of ['MDF', 'Contrachapado de abedul', 'Acrílico negro', 'Cuero vegetal']) expect(h).toContain(m);
    expect(h).toContain('3 mm');
    expect(h).toContain('Sin grosor');
    expect(h).toContain('Aplicar a la capa');
    expect(h).toContain('Duplicar MDF · Corte');
    expect(h).toContain('Borrar MDF · Corte');
    expect(h).toContain('Importar…');
    expect(h).toContain('Exportar…');
    expect(h).toContain('Prueba de materiales…');
  });

  it('la búsqueda filtra y avisa si no hay coincidencias', () => {
    let h = html({ search: 'cuero' });
    expect(h).toContain('Cuero vegetal');
    expect(h).not.toContain('Acrílico negro');
    h = html({ search: 'zzzz' });
    expect(h).toContain(es['materials.noMatches']);
  });

  it('sin capa activa, "Aplicar" y "Guardar" quedan deshabilitados con el motivo', () => {
    const h = html({ activeLayer: null });
    expect(h).toContain('aria-disabled="true"');
    expect(h).toContain(es['materials.reason.noLayer']);
  });

  it('con capa activa, "Guardar" y "Aplicar" están habilitados', () => {
    const h = html();
    expect(h).not.toContain(es['materials.reason.noLayer']);
  });

  it('marca el material que coincide con una capa del documento y no deja aplicarlo otra vez', () => {
    const lib = sampleMaterialLibrary();
    const layer = { ...lib.presets[0]!.settings, color: RED, name: 'Corte MDF' };
    const h = html({ library: lib, layers: [layer], activeLayer: layer });
    expect(h).toContain('En uso: 1');
    expect(h).toContain(es['materials.reason.alreadyApplied']);
  });

  it('el filtro de máquina oculta los ajustes de otras máquinas pero deja los de cualquiera', () => {
    const lib = sampleMaterialLibrary();
    lib.presets[0] = { ...lib.presets[0]!, device_id: 'otra-maquina' };
    const h = html({ library: lib, onlyDevice: true });
    expect(h).not.toContain('Editar MDF · Corte');
    expect(h).toContain('Acrílico negro');
    const all = html({ library: lib });
    expect(all).toContain('Editar MDF · Corte');
    expect(all).toContain('otra-maquina');
  });
});

describe('vista previa de la prueba', () => {
  it('dibuja una celda por combinación, con los valores de los extremos', () => {
    const form = { ...defaultForm(null), rows: { param: 'power' as const, count: 3, from: 20, to: 60 }, cols: { param: 'speed' as const, count: 4, from: 600, to: 1800 } };
    const h = renderToString(<GridPreview form={form} />);
    expect(h.match(/mat-pv__cell/g)).toHaveLength(12);
    for (const v of ['600', '1800', '20', '60']) expect(h).toContain(`>${v}</text>`);
    expect(h).toContain('3 filas por 4 columnas');
  });

  it('con una configuración inválida no dibuja nada en lugar de fallar', () => {
    const form = { ...defaultForm(null), cell: [0, 0] as [number, number], gap: 0 };
    expect(renderToString(<GridPreview form={form} />)).not.toContain('mat-pv__cell');
  });
});
