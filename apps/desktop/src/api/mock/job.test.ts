import { describe, expect, it } from 'vitest';
import { DEFAULT_TAB_SETTINGS } from '@/lib/layers';
import type { Document, JobOptions, LayerSettings } from '../types';
import { mockComputeJob } from './job';
import { sampleDevices, sampleDocument } from './sample';

const options: JobOptions = { start_from: 'absolute', job_origin: 6, user_origin: null, selection: null, layer_order: 'auto' };
const device = sampleDevices()[0]!;

/** Documento de ejemplo con `patch` aplicado a la capa de corte (la que tiene más contornos cerrados). */
function withCut(patch: Partial<LayerSettings>): Document {
  const doc = sampleDocument();
  const cut = doc.layers.find((l) => l.mode === 'line' && l.output)!;
  return { ...doc, layers: doc.layers.map((l) => (l === cut ? { ...l, ...patch } : l)) };
}

const gaps = (moves: number[]) => {
  let n = 0;
  for (let i = 0; i < moves.length; i += 4) if (moves[i] === 0) n++;
  return n;
};

describe('mockComputeJob con los campos de la Fase 4', () => {
  it('el documento de ejemplo se estima sin errores', () => {
    const p = mockComputeJob(sampleDocument(), device, options);
    expect(p.total_s).toBeGreaterThan(0);
  });

  it('una capa de un documento viejo (sin tabs, fill_style ni Z) no hace fallar la estimación', () => {
    const doc = sampleDocument();
    const viejo = doc.layers.map((l) => {
      const c: Record<string, unknown> = { ...l };
      for (const k of ['fill_style', 'tabs', 'z_offset', 'z_step']) delete c[k];
      return c as never;
    });
    expect(() => mockComputeJob({ ...doc, layers: viejo }, device, options)).not.toThrow();
  });

  it('los puentes abren huecos en el recorrido de una capa line', () => {
    const sin = mockComputeJob(withCut({}), device, options);
    const con = mockComputeJob(withCut({ tabs: { ...DEFAULT_TAB_SETTINGS, enabled: true, width: 3, count: 4 } }), device, options);
    expect(gaps(con.moves)).toBeGreaterThan(gaps(sin.moves));
  });

  it('con skip_inner las perforaciones no llevan puentes', () => {
    const tabs = { ...DEFAULT_TAB_SETTINGS, enabled: true, width: 3, count: 4 };
    const solo = mockComputeJob(withCut({ tabs: { ...tabs, skip_inner: true } }), device, options);
    const todos = mockComputeJob(withCut({ tabs: { ...tabs, skip_inner: false } }), device, options);
    expect(gaps(todos.moves)).toBeGreaterThan(gaps(solo.moves));
  });

  it('el relleno por contornos genera movimientos distintos al barrido', () => {
    const doc = sampleDocument();
    const fill = doc.layers.find((l) => l.mode === 'fill')!;
    const mk = (fill_style: 'scan' | 'offset') => ({ ...doc, layers: doc.layers.map((l) => (l === fill ? { ...l, fill_style } : l)) });
    const a = mockComputeJob(mk('scan'), device, options);
    const b = mockComputeJob(mk('offset'), device, options);
    expect(b.moves.length).not.toBe(a.moves.length);
    expect(b.layers.find((l) => l.color === fill.color)!.explanations.join(' ')).toContain('Contornos concéntricos');
  });

  it('Z en una capa con máquina sin eje Z avisa; con eje Z no', () => {
    const doc = withCut({ z_offset: 1 });
    expect(mockComputeJob(doc, device, options).warnings.join(' ')).toMatch(/no tiene eje Z/);
    expect(mockComputeJob(doc, { ...device, z_axis: true }, options).warnings.join(' ')).not.toMatch(/eje Z/);
  });
});
