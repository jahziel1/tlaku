import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import type { JobPreview, LayerSettings, MaterialPreset } from '@/api/types';
import { usePrompts } from '@/features/history/prompt';
import { useStore } from '@/store';
import { JobLogDetail } from './JobLogDetail';
import type { JobLogEntry } from './jobLog';
import { jobsCsv, layersCell } from './jobLogCsv';
import { EMPTY_FILTER, distinctMachines, distinctMaterials, filterJobs, type JobFilter } from './jobLogFilter';
import { applyRepeat, changeLines, planRepeat, repeatWithSettings } from './jobLogRepeat';
import { buildThumbnail, materialOfLayer, summarizeLayers } from './jobLogSnapshot';
import { THUMB_MAX_CHARS } from './jobLog';

const day = (y: number, m: number, d: number, h = 12) => new Date(y, m - 1, d, h, 0, 0).getTime();

const job = (over: Partial<JobLogEntry> = {}): JobLogEntry => ({
  id: String(over.at ?? 1),
  at: day(2026, 10, 1),
  machine: 'Falcon2',
  document: 'caja.tlaku',
  estimatedS: 100,
  actualS: 110,
  result: 'finished',
  message: '',
  ...over,
});

const layer = (over: Partial<NonNullable<JobLogEntry['layers']>[number]['settings']> = {}, extra: Partial<NonNullable<JobLogEntry['layers']>[number]> = {}) => ({
  color: '#ff0000',
  name: 'Corte',
  mode: 'line',
  settings: { speed: 420, power: 85, passes: 2, ...over },
  ...extra,
});

describe('filtros del historial', () => {
  const entries = [
    job({ id: 'a', at: day(2026, 9, 1), machine: 'Falcon2', document: 'Acrílico rojo.tlaku', layers: [layer({}, { material: 'MDF · Corte' })], notes: 'borde quemado' }),
    job({ id: 'b', at: day(2026, 9, 20), machine: 'K40', document: 'llavero.tlaku', result: 'error', message: 'Alarma' }),
    job({ id: 'c', at: day(2026, 10, 5), machine: 'Falcon2', document: 'caja.tlaku', result: 'cancelled', tab: 'Pestaña caja' }),
    job({ id: 'd', at: day(2026, 10, 6), machine: 'Falcon2', result: 'error', interrupted: { id: 'ck', percent: 40, line: 4, total: 10 } }),
  ];
  const ids = (f: Partial<JobFilter>) => filterJobs(entries, { ...EMPTY_FILTER, ...f }).map((e) => e.id);

  it('sin filtros devuelve todo, en el mismo orden', () => {
    expect(ids({})).toEqual(['a', 'b', 'c', 'd']);
  });

  it('por máquina, resultado y material', () => {
    expect(ids({ machine: 'K40' })).toEqual(['b']);
    expect(ids({ result: 'error' })).toEqual(['b', 'd']);
    expect(ids({ material: 'MDF · Corte' })).toEqual(['a']);
    expect(ids({ machine: 'Falcon2', result: 'error' })).toEqual(['d']);
  });

  it('«interrumpido» incluye los que dejaron punto de control para continuar', () => {
    expect(ids({ result: 'interrupted' })).toEqual(['d']);
  });

  it('por fechas locales, ambas incluidas', () => {
    expect(ids({ from: '2026-09-20', to: '2026-10-05' })).toEqual(['b', 'c']);
    expect(ids({ from: '2026-10-06' })).toEqual(['d']);
    expect(ids({ to: '2026-09-01' })).toEqual(['a']);
    // Una fecha mal escrita no filtra.
    expect(ids({ from: 'ayer' })).toEqual(['a', 'b', 'c', 'd']);
  });

  it('la búsqueda ignora mayúsculas y acentos y exige todas las palabras', () => {
    expect(ids({ text: 'acrilico' })).toEqual(['a']);
    expect(ids({ text: 'QUEMADO borde' })).toEqual(['a']);
    expect(ids({ text: 'alarma' })).toEqual(['b']);
    expect(ids({ text: 'pestaña caja' })).toEqual(['c']);
    expect(ids({ text: 'acrilico llavero' })).toEqual([]);
  });

  it('las listas de máquinas y materiales no repiten', () => {
    expect(distinctMachines(entries)).toEqual(['Falcon2', 'K40']);
    expect(distinctMaterials(entries)).toEqual(['MDF · Corte']);
  });
});

describe('CSV del historial', () => {
  it('lleva encabezado, una fila por trabajo y protege las celdas que parecen fórmulas', () => {
    const csv = jobsCsv([
      job({ id: 'a', at: day(2026, 10, 1, 9), document: '=SUMA(A1)', notes: 'dijo "ok", luego falló', layers: [layer({ passes: 1 })], material: 'MDF · Corte', areaMm2: 2500.4, lengthMm: 399.6 }),
      job({ id: 'b', estimatedS: null, result: 'interrupted' }),
    ]);
    const lines = csv.trim().split('\r\n');
    expect(lines).toHaveLength(3);
    expect(lines[0]).toContain('Fecha');
    expect(lines[1]).toContain('2026-10-01 09:00:00');
    expect(lines[1]).toContain("'=SUMA(A1)");
    expect(lines[1]).toContain('"dijo ""ok"", luego falló"');
    expect(lines[1]).toContain(',2500,400,');
    expect(lines[2]).toContain('Interrumpido');
  });

  it('resume las capas con velocidad, potencia y pasadas', () => {
    expect(layersCell(job({ layers: [layer(), layer({ passes: 1 }, { color: '#0000ff', name: 'Grabado', mode: 'fill' })] }))).toBe('Corte (line) 420 mm/min 85 % ×2 | Grabado (fill) 420 mm/min 85 %');
  });
});

describe('resumen del trabajo para el historial', () => {
  const layers = sampleLayers();
  const preset = (settings: LayerSettings): MaterialPreset => ({ id: 'p', material: 'MDF', thickness: 3, title: 'Corte', notes: '', device_id: null, settings });

  it('reconoce el material cuando los ajustes de la capa coinciden con los de la biblioteca', () => {
    const cut = layers[0]!;
    expect(materialOfLayer(cut, [preset(cut)])).toBe('MDF · Corte');
    expect(materialOfLayer(cut, [preset({ ...cut, power: 10 })])).toBeNull();
  });

  it('solo cuenta las capas que salieron, con sus ajustes', () => {
    const doc = { layers, objects: sampleObjects() };
    const out = summarizeLayers(doc, null, []);
    expect(out.length).toBeGreaterThan(0);
    expect(out[0]).toMatchObject({ name: 'Corte', mode: 'line', settings: { speed: 420, power: 85, passes: 2, air: true } });
    const hidden = summarizeLayers({ layers: layers.map((l) => ({ ...l, output: false })), objects: doc.objects }, null, []);
    expect(hidden).toEqual([]);
  });

  it('la miniatura es un SVG corto con el recorrido, Y hacia arriba, y es nula si no hay nada que dibujar', () => {
    const moves: number[] = [];
    for (let i = 0; i < 4000; i++) moves.push(0, i, 0, 0, 1, i + 1, 10 + (i % 7), 50);
    const preview = { layers: [], moves, move_times: [], total_s: 1, bounds: [[0, 0], [10, 10]], order_note: null, warnings: [] } as unknown as JobPreview;
    const svg = buildThumbnail(preview, [400, 300])!;
    expect(svg.startsWith('<svg')).toBe(true);
    expect(svg.length).toBeLessThanOrEqual(THUMB_MAX_CHARS);
    expect(svg).toContain('viewBox="0 0 400 300"');
    expect(svg).toContain('matrix(1 0 0 -1 0 300)');
    expect(buildThumbnail(null, [400, 300])).toBeNull();
    expect(buildThumbnail({ ...preview, moves: [0, 1, 1, 0] } as JobPreview, [400, 300])).toBeNull();
  });
});

describe('repetir con estos ajustes', () => {
  const base = sampleLayers();
  const entryWith = (over: Partial<JobLogEntry> = {}) =>
    job({
      layers: [
        { color: base[0]!.color, name: 'Corte', mode: 'line', settings: { speed: 300, power: 90, passes: 3, air: false } },
        { color: '#123456', name: 'Fantasma', mode: 'line', settings: { speed: 100 } },
      ],
      ...over,
    });

  it('planRepeat lista solo lo que cambia y las capas que ya no están', () => {
    const plan = planRepeat(base, entryWith());
    expect(plan.changes.map((c) => c.field)).toEqual(['speed', 'power', 'passes', 'air']);
    expect(plan.changes[0]).toMatchObject({ from: 420, to: 300, name: 'Corte' });
    expect(plan.missing.map((l) => l.name)).toEqual(['Fantasma']);
    expect(changeLines(plan.changes)[0]).toContain('420');
    expect(planRepeat(applyRepeat(base, entryWith()), entryWith()).changes).toEqual([]);
  });

  it('applyRepeat no toca las demás capas ni los demás campos', () => {
    const next = applyRepeat(base, entryWith());
    expect(next[0]).toMatchObject({ speed: 300, power: 90, passes: 3, air: false, name: 'Corte' });
    expect(next[1]).toBe(base[1]);
    expect(next[0]!.interval).toBe(base[0]!.interval);
  });

  describe('con la aplicación', () => {
    const answerNext = async (id: string) => {
      await vi.waitFor(() => expect(usePrompts.getState().current).not.toBeNull());
      const asked = usePrompts.getState().current!;
      usePrompts.getState().answer(id);
      return asked;
    };

    beforeEach(() => {
      setApi(createMockApi({ latencyMs: 0 }));
      const s = useStore.getState();
      s.replaceDocument({ ...s.doc, layers: sampleLayers(), objects: sampleObjects() }, '/proyectos/caja.tlaku');
    });

    it('con el documento abierto, pregunta y aplica con un paso de deshacer; nunca inicia la máquina', async () => {
      const startJob = vi.spyOn(useStore.getState(), 'startJob');
      const request = vi.spyOn(useStore.getState(), 'requestStart');
      const done = repeatWithSettings(entryWith({ documentPath: '/proyectos/caja.tlaku' }));
      const asked = await answerNext('apply');
      expect(asked.items!.some((l) => l.includes('420'))).toBe(true);
      expect(asked.items!.some((l) => l.includes('Fantasma'))).toBe(true);
      expect(await done).toBe(true);
      expect(useStore.getState().doc.layers[0]).toMatchObject({ speed: 300, power: 90, passes: 3 });
      expect(startJob).not.toHaveBeenCalled();
      expect(request).not.toHaveBeenCalled();
      expect(useStore.getState().jobState).not.toBe('running');
      useStore.getState().undo();
      expect(useStore.getState().doc.layers[0]).toMatchObject({ speed: 420, power: 85 });
    });

    it('si el usuario cancela no cambia nada', async () => {
      const done = repeatWithSettings(entryWith({ documentPath: '/proyectos/caja.tlaku' }));
      await answerNext('cancel');
      expect(await done).toBe(false);
      expect(useStore.getState().doc.layers[0]).toMatchObject({ speed: 420, power: 85 });
    });

    it('si el archivo ya no existe ofrece aplicar a la pestaña activa', async () => {
      const done = repeatWithSettings(entryWith({ documentPath: '/borrado/no-existe.tlaku' }));
      const first = await answerNext('active');
      expect(first.title).toContain('documento');
      await answerNext('apply');
      expect(await done).toBe(true);
      expect(useStore.getState().doc.layers[0]).toMatchObject({ speed: 300 });
    });

    it('un trabajo sin capas guardadas avisa y no pregunta', async () => {
      expect(await repeatWithSettings(job())).toBe(false);
      expect(usePrompts.getState().current).toBeNull();
    });

    it('si las capas ya tienen esos ajustes lo dice y no pregunta de nuevo', async () => {
      const same = entryWith({ documentPath: '/proyectos/caja.tlaku', layers: [{ color: base[0]!.color, name: 'Corte', mode: 'line', settings: { speed: 420, power: 85 } }] });
      expect(await repeatWithSettings(same)).toBe(false);
      expect(usePrompts.getState().current).toBeNull();
    });
  });
});

describe('detalle del trabajo', () => {
  it('muestra capas con sus ajustes, notas y las acciones; «Repetir» va deshabilitado sin capas', () => {
    const full = renderToStaticMarkup(<JobLogDetail entry={job({ layers: [layer()], material: 'MDF · Corte', notes: 'ok', areaMm2: 2500, thumb: '<svg viewBox="0 0 1 1"/>' })} onClose={() => undefined} onRepeated={() => undefined} onDeleted={() => undefined} />);
    expect(full).toContain('Repetir con estos ajustes');
    expect(full).toContain('Corte');
    expect(full).toContain('420');
    expect(full).toMatch(/25[.,]0 cm²/);
    expect(full).toContain('data:image/svg+xml');
    const bare = renderToStaticMarkup(<JobLogDetail entry={job()} onClose={() => undefined} onRepeated={() => undefined} onDeleted={() => undefined} />);
    expect(bare).toContain('antes de que el historial guardara');
    expect(bare).toMatch(/<button[^>]*disabled=""[^>]*>[^]*Repetir con estos ajustes/);
  });
});
