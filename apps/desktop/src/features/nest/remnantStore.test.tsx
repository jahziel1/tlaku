import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { useStore } from '@/store';
import { NestDialogHost } from './index';
import { addLeftover, loadRemnants, removeRemnants, renameRemnant, retagRemnant, useRemnants } from './remnantStore';
import { emptyRemnants, type Remnant } from './remnants';
import { RemnantsDialogView } from './RemnantsDialog';
import { leftoverSize, SheetsResult, SheetsSection } from './SheetsPanels';
import { DEFAULT_NEST_OPTIONS } from './request';
import type { NestSheet, NestStatus } from './types';

const ring = (w: number, h: number): [number, number][] => [[10, 10], [10 + w, 10], [10 + w, 10 + h], [10, 10 + h]];

function reset(api = createMockApi({ latencyMs: 0 })) {
  setApi(api);
  useRemnants.setState({ library: emptyRemnants(), loaded: false, loadError: null, panelOpen: false });
  return api;
}

const flush = () => new Promise((r) => setTimeout(r, 0));

describe('inventario de retazos (estado)', () => {
  beforeEach(() => void reset());

  it('guarda un sobrante, lo recuerda al recargar y lo borra', async () => {
    await loadRemnants();
    expect(useRemnants.getState()).toMatchObject({ loaded: true, loadError: null });
    const r = addLeftover([ring(200, 100)], { material: 'Triplay', thicknessMm: 3 })!;
    expect(r).toMatchObject({ material: 'Triplay', thickness_mm: 3, width_mm: 200, height_mm: 100, rect: true });
    expect(r.paths[0]![0]).toEqual([0, 0]);
    await flush();
    // otra «sesión»: se vuelve a leer lo guardado
    useRemnants.setState({ library: emptyRemnants(), loaded: false });
    await loadRemnants();
    expect(useRemnants.getState().library.remnants.map((x) => x.id)).toEqual([r.id]);
    await removeRemnants([r.id]);
    useRemnants.setState({ library: emptyRemnants(), loaded: false });
    await loadRemnants();
    expect(useRemnants.getState().library.remnants).toEqual([]);
  });

  it('renombra y cambia el material y el grosor; un nombre vacío no cambia nada', async () => {
    await loadRemnants();
    const r = addLeftover([ring(50, 50)], { material: 'MDF', thicknessMm: 3 })!;
    await renameRemnant(r.id, '  Esquina buena ');
    expect(useRemnants.getState().library.remnants[0]!.name).toBe('Esquina buena');
    await renameRemnant(r.id, '   ');
    expect(useRemnants.getState().library.remnants[0]!.name).toBe('Esquina buena');
    await retagRemnant(r.id, ' Acrílico ', -4);
    expect(useRemnants.getState().library.remnants[0]).toMatchObject({ material: 'Acrílico', thickness_mm: 0 });
  });

  it('un contorno inválido no se guarda', async () => {
    await loadRemnants();
    expect(addLeftover([[[0, 0], [1, 1]]], { material: 'MDF', thicknessMm: 3 })).toBeNull();
    expect(useRemnants.getState().library.remnants).toEqual([]);
  });

  it('si no se pudo leer el inventario avisa y no guarda nada para no pisar el archivo', async () => {
    const api = createMockApi({ latencyMs: 0 });
    const save = vi.spyOn(api, 'saveRemnants');
    vi.spyOn(api, 'loadRemnants').mockRejectedValue(new Error('archivo dañado'));
    reset(api);
    useStore.setState({ notices: [] });
    await loadRemnants();
    expect(useRemnants.getState()).toMatchObject({ loaded: false, loadError: 'archivo dañado' });
    expect(useStore.getState().notices.some((n) => n.level === 'error' && n.text.includes('archivo dañado'))).toBe(true);
    addLeftover([ring(50, 50)], { material: 'MDF', thicknessMm: 3 });
    await flush();
    expect(save).not.toHaveBeenCalled();
  });

  it('un error al guardar se avisa', async () => {
    const api = createMockApi({ latencyMs: 0 });
    vi.spyOn(api, 'saveRemnants').mockRejectedValue(new Error('disco lleno'));
    reset(api);
    useStore.setState({ notices: [] });
    await loadRemnants();
    addLeftover([ring(50, 50)], { material: 'MDF', thicknessMm: 3 });
    await flush();
    await flush();
    expect(useStore.getState().notices.some((n) => n.level === 'error' && n.text.includes('disco lleno'))).toBe(true);
  });
});

const stored = (id: string, over: Partial<Remnant> = {}): Remnant => ({
  id,
  name: `Retazo ${id}`,
  material: 'Triplay',
  thickness_mm: 3,
  width_mm: 200,
  height_mm: 100,
  area_mm2: 20000,
  rect: true,
  paths: [[[0, 0], [200, 0], [200, 100], [0, 100]]],
  created_ms: 1,
  ...over,
});

describe('inventario de retazos (diálogo)', () => {
  beforeEach(() => void reset());

  const view = (over: Partial<Parameters<typeof RemnantsDialogView>[0]> = {}) =>
    renderToString(<RemnantsDialogView library={emptyRemnants()} loaded loadError={null} onClose={() => undefined} {...over} />);

  it('cerrado no se monta; abierto sin retazos muestra el vacío y mientras lee, el aviso', () => {
    expect(renderToString(<NestDialogHost />)).toBe('');
    const html = view();
    expect(html).toContain('Retazos guardados');
    expect(html).toContain('Aún no hay retazos');
    expect(view({ loaded: false })).toContain('Leyendo los retazos');
    expect(view({ loaded: false, loadError: 'archivo dañado' })).toContain('archivo dañado');
  });

  it('lista cada retazo con su material, grosor y medidas, y distingue una forma irregular', () => {
    const html = view({
      library: {
        ...emptyRemnants(),
        remnants: [stored('a'), stored('b', { name: 'Forma L', rect: false, area_mm2: 14000, material: 'MDF', thickness_mm: 6 })],
      },
    });
    expect(html).toContain('Retazo a');
    expect(html).toContain('200 × 100 mm');
    expect(html).toContain('Forma L');
    expect(html).toContain('Forma irregular');
    expect(html).toContain('140 cm²');
    expect(html).toContain('Retazos guardados: 2');
    expect(html).toContain('Borrar todos');
  });
});

describe('hojas y retazos en el diálogo de acomodo', () => {
  const sheet = (index: number, over: Partial<NestSheet> = {}): NestSheet => ({
    index,
    source: 'container',
    remnant_id: null,
    bounds: [0, 0, 100, 60],
    utilization: 84.26,
    placed: 5,
    area_mm2: 6000,
    placed_area_mm2: 5000,
    leftovers: [],
    ...over,
  });
  const status = (sheets: NestSheet[], running = false): NestStatus => ({
    running,
    elapsed_ms: 10,
    evaluations: 1,
    utilization: 80,
    placed: 5,
    total: 5,
    placements: [],
    sheets,
    unplaced: [],
    timed_out: false,
    error: null,
  });

  it('mide el sobrante por la caja de su contorno exterior', () => {
    expect(leftoverSize([[[10, 10], [430, 10], [430, 190], [10, 190]]])).toBe('420×180');
  });

  it('resume cada hoja con su aprovechamiento, su origen y el sobrante que se puede guardar', () => {
    const left: [number, number][][] = [[[0, 0], [420, 0], [420, 180], [0, 180]]];
    const html = renderToString(
      <SheetsResult
        status={status([sheet(0, { source: 'remnant', remnant_id: 'r1', leftovers: [left] }), sheet(1, { placed: 1, leftovers: [left, left] })])}
        remnantName={(id) => (id === 'r1' ? 'Esquina' : id)}
        saved={new Set([1])}
        onSave={() => undefined}
        disabled={false}
      />,
    );
    expect(html).toContain('Hoja 1');
    expect(html).toContain('retazo «Esquina»');
    expect(html).toContain('84,3 %');
    expect(html).toContain('5 piezas');
    expect(html).toContain('1 pieza');
    expect(html).toContain('Sobra 1 retazo: 420×180 mm');
    expect(html).toContain('Guardar el sobrante como retazo');
    expect(html).toContain('Sobran 2 retazos');
    expect(html).toContain('Guardado en retazos');
  });

  it('mientras corre no ofrece guardar y sin sobrante lo dice', () => {
    const running = renderToString(<SheetsResult status={status([sheet(0, { leftovers: [[[[0, 0], [30, 0], [30, 30]]]] })], true)} remnantName={(i) => i} saved={new Set()} onSave={() => undefined} disabled />);
    expect(running).not.toContain('Guardar el sobrante');
    const none = renderToString(<SheetsResult status={status([sheet(0)])} remnantName={(i) => i} saved={new Set()} onSave={() => undefined} disabled={false} />);
    expect(none).toContain('No sobra un retazo aprovechable');
    expect(renderToString(<SheetsResult status={status([])} remnantName={(i) => i} saved={new Set()} onSave={() => undefined} disabled={false} />)).toBe('');
  });

  it('la sección de hojas explica qué retazos coinciden', () => {
    const base = { options: { ...DEFAULT_NEST_OPTIONS, useRemnants: true }, patch: () => undefined, materials: ['Triplay'] };
    expect(renderToString(<SheetsSection {...base} matching={[]} />)).toContain('Escribe el material');
    const named = { ...base, options: { ...base.options, material: 'Triplay' } };
    expect(renderToString(<SheetsSection {...named} matching={[]} />)).toContain('No hay retazos guardados de ese material y grosor');
    expect(renderToString(<SheetsSection {...named} matching={[stored('a')]} />)).toContain('Coincide 1 retazo guardado');
    expect(renderToString(<SheetsSection {...named} matching={[stored('a'), stored('b')]} />)).toContain('Coinciden 2 retazos guardados');
    const multi = renderToString(<SheetsSection {...named} options={{ ...named.options, multiSheet: true }} matching={[]} />);
    expect(multi).toContain('Máximo de hojas nuevas');
    expect(renderToString(<SheetsSection {...named} matching={[]} />)).not.toContain('Máximo de hojas nuevas');
  });
});
