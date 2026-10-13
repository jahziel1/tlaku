import { describe, expect, it } from 'vitest';
import type { Document, TraceResult } from '@/api/types';
import { roundedRectBez } from '@/lib/bezpath';
import { applyBatchToDoc, entrySize, layoutOutcomes, runBatch, type BatchEntry, type BatchOutcome } from './batch';
import { DEFAULT_TRACE_SETTINGS } from './settings';

const entry = (name: string, w = 96, h = 96): BatchEntry => ({ path: `/x/${name}.png`, name, width_px: w, height_px: h });
const shape = [roundedRectBez(0, 0, 10, 10)];
const outcome = (name: string, w = 30, h = 20): BatchOutcome => ({ path: name, name, size: [w, h], paths: shape, warnings: [] });

describe('vectorizar varios archivos', () => {
  it('el tamaño sale de los píxeles y la resolución', () => {
    expect(entrySize(entry('a', 96, 48), 96)).toEqual([25.4, 12.7]);
    expect(entrySize(entry('a', 254, 254), 254)[0]).toBeCloseTo(25.4);
  });

  it('usa los mismos ajustes en todos, avisa del avance y sigue tras un fallo', async () => {
    const seen: string[] = [];
    const progress: number[] = [];
    const r = await runBatch({
      entries: [entry('a'), entry('b'), entry('c')],
      settings: { ...DEFAULT_TRACE_SETTINGS, threshold: 0.3 },
      dpi: 96,
      trace: async ({ path, settings }): Promise<TraceResult> => {
        seen.push(`${path}@${settings.threshold}`);
        if (path.includes('/b.')) throw new Error('no se pudo leer');
        return { paths: shape, warnings: [] };
      },
      onProgress: (p) => progress.push(p.done),
      message: (e) => (e instanceof Error ? e.message : String(e)),
    });
    expect(seen).toEqual(['/x/a.png@0.3', '/x/b.png@0.3', '/x/c.png@0.3']);
    expect(r.outcomes.map((o) => o.name)).toEqual(['a', 'c']);
    expect(r.failures).toEqual([{ path: '/x/b.png', name: 'b', error: 'no se pudo leer' }]);
    expect(progress).toEqual([0, 1, 2, 3]);
    expect(r.cancelled).toBe(false);
  });

  it('un resultado sin formas cuenta como fallo', async () => {
    const r = await runBatch({
      entries: [entry('a')],
      settings: DEFAULT_TRACE_SETTINGS,
      dpi: 96,
      trace: async () => ({ paths: [], warnings: ['sin zonas que trazar'] }),
    });
    expect(r.outcomes).toHaveLength(0);
    expect(r.failures[0]!.error).toBe('sin zonas que trazar');
  });

  it('se puede cancelar entre archivos', async () => {
    let n = 0;
    const r = await runBatch({
      entries: [entry('a'), entry('b'), entry('c')],
      settings: DEFAULT_TRACE_SETTINGS,
      dpi: 96,
      trace: async () => ({ paths: shape, warnings: [] }),
      onProgress: () => n++,
      cancelled: () => n >= 2,
    });
    expect(r.cancelled).toBe(true);
    expect(r.outcomes.map((o) => o.name)).toEqual(['a', 'b']);
  });

  it('coloca los resultados en filas y pasa a otra al llegar al ancho', () => {
    const objs = layoutOutcomes([outcome('a'), outcome('b'), outcome('c')], '#000000', new Set(), [0, 0], 10, 80);
    expect(objs.map((o) => [o.transform[4], o.transform[5]])).toEqual([
      [0, 0],
      [40, 0],
      [0, 30], // 30 + 10 > 80 con la tercera: nueva fila, 20 de alto + 10 de separación
    ]);
    expect(new Set(objs.map((o) => o.id)).size).toBe(3);
  });

  it('agrega todo al documento en un solo cambio y crea la capa si falta', () => {
    const doc = { version: 2, objects: [], layers: [], device_id: null } as unknown as Document;
    const r = applyBatchToDoc(doc, [outcome('a'), outcome('b')], '#ff0000');
    expect(r.doc.objects).toHaveLength(2);
    expect(r.doc.layers.map((l) => l.color)).toEqual(['#ff0000']);
    expect(applyBatchToDoc(doc, [], '#ff0000').doc).toBe(doc);
  });
});
