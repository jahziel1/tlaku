import { DEFAULT_JOB_OPTIONS } from '@/store/preview';
import { describe, expect, it } from 'vitest';
import { DEFAULT_TAB_SETTINGS } from '@/lib/layers';
import type { Document, JobOptions, LayerSettings } from '../types';
import { mockComputeJob } from './job';
import { sampleDevices, sampleDocument } from './sample';

const options: JobOptions = { start_from: 'absolute', job_origin: 6, user_origin: null, selection: null, layer_order: 'auto', optimization: DEFAULT_JOB_OPTIONS.optimization, use_selection_origin: false };
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

describe('mockComputeJob con los ajustes de la ola 2', () => {
  const base = () => sampleDocument().layers.find((l) => l.mode === 'line' && l.output)!;
  const rows = (p: ReturnType<typeof mockComputeJob>, color: string) => p.layers.filter((l) => l.color === color);

  it('la perforación abre huecos en el recorrido y lo explica', () => {
    const sin = mockComputeJob(withCut({}), device, options);
    const con = mockComputeJob(withCut({ perforation: { enabled: true, cut_mm: 2, skip_mm: 2 } }), device, options);
    expect(gaps(con.moves)).toBeGreaterThan(gaps(sin.moves));
    expect(rows(con, base().color)[0]!.explanations.join(' ')).toContain('Perforación');
  });

  it('el modo punto parte el trazo en disparos y la rampa baja la potencia en los extremos', () => {
    const dots = mockComputeJob(withCut({ dot_mode: { enabled: true, dwell_ms: 1, spacing_mm: 2 } }), device, options);
    expect(gaps(dots.moves)).toBeGreaterThan(gaps(mockComputeJob(withCut({}), device, options).moves));

    const ramp = mockComputeJob(withCut({ ramp: { enabled: true, length_mm: 3, start_pct: 0, outer_only: false } }), device, options);
    const powers: number[] = [];
    for (let i = 0; i < ramp.moves.length; i += 4) if (ramp.moves[i] === 1) powers.push(ramp.moves[i + 3]!);
    expect(Math.min(...powers)).toBeLessThan(Math.max(...powers));
  });

  it('el modo punto anula la rampa (no hay trazo continuo)', () => {
    const doc = withCut({ dot_mode: { enabled: true, dwell_ms: 1, spacing_mm: 2 }, ramp: { enabled: true, length_mm: 3, start_pct: 0, outer_only: false } });
    const p = mockComputeJob(doc, device, options);
    expect(rows(p, base().color)[0]!.explanations.join(' ')).not.toContain('sube y baja');
  });

  it('el sobrecorte alarga el recorrido de los contornos cerrados', () => {
    const sin = mockComputeJob(withCut({}), device, options);
    const con = mockComputeJob(withCut({ overcut: 5 }), device, options);
    expect(con.moves.length).toBeGreaterThan(sin.moves.length);
  });

  it('las pausas y el corte pasante suman tiempo', () => {
    const sin = mockComputeJob(withCut({}), device, options);
    const con = mockComputeJob(withCut({ start_delay_ms: 200, end_delay_ms: 200, cut_through: { enabled: true, power: 100, ms: 300 } }), device, options);
    expect(con.total_s).toBeGreaterThan(sin.total_s);
  });

  it('cada operación adicional es una fila más del resumen, en orden, con su tiempo', () => {
    const parent = base();
    const sub = { ...parent, mode: 'fill' as const, name: '', sub_layers: [] };
    const doc = withCut({ sub_layers: [sub, { ...sub, mode: 'line' as const }] });
    const sin = mockComputeJob(withCut({}), device, options);
    const con = mockComputeJob(doc, device, options);
    const filas = rows(con, parent.color);
    expect(filas).toHaveLength(3);
    expect(filas[0]!.mode).toBe(parent.mode);
    expect(filas[1]!.mode).toBe('fill');
    expect(filas[1]!.summary).toContain('(+1)');
    expect(filas[2]!.summary).toContain('(+2)');
    expect(con.total_s).toBeGreaterThan(sin.total_s);
    expect(con.moves.length).toBeGreaterThan(sin.moves.length);
  });

  it('una operación de imagen sobre objetos que no son imagen no genera nada ni falla', () => {
    const parent = base();
    const doc = withCut({ sub_layers: [{ ...parent, mode: 'image' as const, sub_layers: [] }] });
    expect(() => mockComputeJob(doc, device, options)).not.toThrow();
    expect(rows(mockComputeJob(doc, device, options), parent.color)).toHaveLength(1);
  });

  it('relleno forma por forma, por regiones, incremento de ángulo y pass-through se estiman sin fallar', () => {
    const doc = sampleDocument();
    const mk = (mode: 'fill' | 'image', patch: Partial<LayerSettings>) => ({
      ...doc,
      layers: doc.layers.map((l) => (l.mode === mode ? { ...l, ...patch } : l)),
    });
    const fill = mockComputeJob(mk('fill', { fill_grouping: 'by_shape', fill_style: 'flood', angle_increment: 15, passes: 2 }), device, options);
    expect(fill.layers.some((l) => l.explanations.join(' ').includes('regiones'))).toBe(true);
    expect(fill.layers.some((l) => l.explanations.join(' ').includes('gira el barrido 15°'))).toBe(true);
    const img = doc.layers.find((l) => l.mode === 'image')!;
    const photo = mockComputeJob(mk('image', { image: { ...img.image, pass_through: true } }), device, options);
    expect(photo.layers.some((l) => l.explanations.join(' ').includes('pass-through'))).toBe(true);
  });
});
