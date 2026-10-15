import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { selectionUnits } from '@/features/arrange/units';
import { DEFAULT_NEST_OPTIONS, buildNestRequest, materialCandidates, resolveContainer, sanitizeCopies, totalInstances, MAX_NEST_INSTANCES, MAX_NEST_SHEETS } from './request';
import { docOf, rectObject } from './testUtils';

const a = rectObject('a', 10, 5);
const b = rectObject('b', 4, 4, [20, 0]);
const doc = docOf([a, b]);
const units = selectionUnits(doc, ['a', 'b']);
const bed = { type: 'rect', width: 400, height: 415 } as const;

describe('buildNestRequest', () => {
  it('arma la petición con los valores por defecto del diálogo', () => {
    const req = buildNestRequest({ units, copies: { a: 3 }, container: bed, options: DEFAULT_NEST_OPTIONS });
    expect(req.pieces.map((p) => [p.id, p.copies])).toEqual([['a', 3], ['b', 1]]);
    expect(req.pieces[0]!.objects).toEqual([a]);
    expect(req.container).toEqual(bed);
    expect(req).toMatchObject({
      spacing: 1,
      margin: 0,
      rotation: { mode: 'quarter' },
      allow_mirror: false,
      use_holes: false,
      gravity: 'gravity',
      time_limit_s: 20,
      seed: 1,
    });
  });

  it('sanea los rangos del contrato', () => {
    const req = buildNestRequest({
      units,
      copies: { a: 0, b: 99999 },
      container: bed,
      options: { ...DEFAULT_NEST_OPTIONS, spacing: -2, margin: Number.NaN, timeLimitS: 9999, seed: -5.7, stepDeg: 0, rotationMode: 'step' },
    });
    expect(req.spacing).toBe(0);
    expect(req.margin).toBe(0);
    expect(req.time_limit_s).toBe(600);
    expect(req.seed).toBe(0);
    expect(req.rotation).toEqual({ mode: 'step', step_deg: 0.5 });
    expect(req.pieces.map((p) => p.copies)).toEqual([1, MAX_NEST_INSTANCES]);
    expect(buildNestRequest({ units, copies: {}, container: bed, options: { ...DEFAULT_NEST_OPTIONS, timeLimitS: 0.2, seed: 7.9 } })).toMatchObject({ time_limit_s: 1, seed: 7 });
  });

  it('un grupo completo es una sola pieza con todos sus objetos', () => {
    const g1 = rectObject('g1', 5, 5, [0, 0], { group: 'grupo-1' });
    const g2 = rectObject('g2', 5, 5, [8, 0], { group: 'grupo-1' });
    const u = selectionUnits(docOf([g1, g2, a]), ['g1', 'a']);
    const req = buildNestRequest({ units: u, copies: {}, container: bed, options: DEFAULT_NEST_OPTIONS });
    expect(req.pieces.map((p) => [p.id, p.objects.map((o) => o.id)])).toEqual([['g1', ['g1', 'g2']], ['a', ['a']]]);
  });

  it('el contenedor puede ser un objeto material', () => {
    const material = rectObject('m', 100, 80);
    const container = resolveContainer('object', [400, 415], material);
    expect(container).toEqual({ type: 'object', object: material });
    expect(resolveContainer('bed', [400, 415], material)).toEqual(bed.type === 'rect' ? { type: 'rect', width: 400, height: 415 } : null);
    expect(resolveContainer('object', [400, 415], null)).toBeNull();
  });
});

describe('copias y material', () => {
  it('cuenta el total de instancias y limita las copias', () => {
    expect(totalInstances(units, {})).toBe(2);
    expect(totalInstances(units, { a: 5, b: 2.4 })).toBe(7);
    expect(sanitizeCopies(-3)).toBe(1);
    expect(sanitizeCopies(Number.NaN)).toBe(1);
  });

  it('los candidatos a material son vectores cerrados que no son piezas ni están ocultos', () => {
    const open = { ...rectObject('open', 5, 5), kind: { type: 'vector' as const, paths: [[{ type: 'move_to' as const, to: [0, 0] as [number, number] }, { type: 'line_to' as const, to: [5, 0] as [number, number] }, { type: 'line_to' as const, to: [5, 5] as [number, number] }]] } };
    const d = docOf([a, b, rectObject('m1', 50, 50), rectObject('m2', 50, 50, [0, 0], { hidden: true }), open]);
    expect(materialCandidates(d, new Set(['a', 'b'])).map((o) => o.id)).toEqual(['m1']);
  });
});

describe('tope de piezas', () => {
  it('coincide con el del motor (MAX_INSTANCES en tlaku-nest)', () => {
    const rust = readFileSync(new URL('../../../../../crates/tlaku-nest/src/lib.rs', import.meta.url), 'utf8');
    const match = /pub const MAX_INSTANCES: usize = (\d[\d_]*);/.exec(rust);
    expect(match).not.toBeNull();
    expect(Number(match![1]!.replace(/_/g, ''))).toBe(MAX_NEST_INSTANCES);
  });
});

describe('giros por pieza, hojas y retazos en la petición', () => {
  const remnant = { id: 'r1', paths: [[[0, 0], [50, 0], [50, 20], [0, 20]] as [number, number][]] };
  const stored = { ...remnant, name: 'R', material: 'Triplay', thickness_mm: 3, width_mm: 50, height_mm: 20, area_mm2: 1000, rect: true, created_ms: 1 };

  it('cada pieza lleva sus giros; las que no se tocan siguen las opciones', () => {
    const req = buildNestRequest({ units, copies: {}, container: bed, options: DEFAULT_NEST_OPTIONS, rotations: { a: 'grain', b: 'none' } });
    expect(req.pieces.map((p) => [p.id, p.rotation])).toEqual([['a', 'grain'], ['b', 'none']]);
    expect(buildNestRequest({ units, copies: {}, container: bed, options: DEFAULT_NEST_OPTIONS }).pieces.map((p) => p.rotation)).toEqual(['inherit', 'inherit']);
  });

  it('por omisión es una sola hoja, sin retazos, y siempre pide el sobrante', () => {
    const req = buildNestRequest({ units, copies: {}, container: bed, options: DEFAULT_NEST_OPTIONS });
    expect(req).toMatchObject({ max_sheets: 1, remnants: [], leftover_min_mm: 20 });
  });

  it('las hojas nuevas y los retazos suman el tope de hojas usadas', () => {
    const o = { ...DEFAULT_NEST_OPTIONS, multiSheet: true, maxSheets: 4, useRemnants: true };
    const req = buildNestRequest({ units, copies: {}, container: bed, options: o, remnants: [stored, { ...stored, id: 'r2' }] });
    expect(req.max_sheets).toBe(6);
    expect(req.remnants).toEqual([remnant, { ...remnant, id: 'r2' }]);
    const off = buildNestRequest({ units, copies: {}, container: bed, options: { ...o, useRemnants: false }, remnants: [stored] });
    expect(off).toMatchObject({ max_sheets: 4, remnants: [] });
    const single = buildNestRequest({ units, copies: {}, container: bed, options: { ...o, multiSheet: false }, remnants: [stored] });
    expect(single.max_sheets).toBe(2);
  });

  it('sanea el máximo de hojas', () => {
    const o = { ...DEFAULT_NEST_OPTIONS, multiSheet: true };
    expect(buildNestRequest({ units, copies: {}, container: bed, options: { ...o, maxSheets: 9999 } }).max_sheets).toBe(MAX_NEST_SHEETS);
    expect(buildNestRequest({ units, copies: {}, container: bed, options: { ...o, maxSheets: -3 } }).max_sheets).toBe(1);
    expect(buildNestRequest({ units, copies: {}, container: bed, options: { ...o, maxSheets: Number.NaN } }).max_sheets).toBe(1);
  });

  it('el tope de hojas coincide con el del motor (MAX_SHEETS en edit/nest.rs)', () => {
    const rust = readFileSync(new URL('../../../src-tauri/src/edit/nest.rs', import.meta.url), 'utf8');
    const match = /pub const MAX_SHEETS: u32 = (\d+);/.exec(rust);
    expect(match).not.toBeNull();
    expect(Number(match![1])).toBe(MAX_NEST_SHEETS);
  });
});
