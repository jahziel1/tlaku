import { describe, expect, it } from 'vitest';
import type { Document } from '@/api';
import { sampleDocument } from '@/api/mock/sample';
import { defaultStored } from '@/features/costs/costProfiles';
import type { Remnant } from '@/features/nest/remnants';
import {
  CONFLICT_POLICIES,
  defaultPolicies,
  diffByKey,
  mergeByKey,
  planCosts,
  planHistory,
  planRemnants,
  planTemplates,
  resolveCosts,
  resolveRemnants,
  sanitizeRemnants,
  sanitizeTemplates,
  templatesToWrite,
  touched,
  uniqueLabel,
  validTemplateName,
} from './extras';

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

describe('diffByKey y mergeByKey', () => {
  const key = (x: { id: string }) => x.id;
  const rekey = (x: { id: string; v: number }, taken: ReadonlySet<string>) => ({ ...x, id: uniqueLabel(x.id, taken) });
  const existing = [{ id: 'a', v: 1 }, { id: 'b', v: 2 }];
  const incoming = [{ id: 'a', v: 1 }, { id: 'b', v: 9 }, { id: 'c', v: 3 }];
  const diff = diffByKey(incoming, existing, key);

  it('separa nuevos, en conflicto e iguales', () => {
    expect(diff.added.map(key)).toEqual(['c']);
    expect(diff.conflicts.map(key)).toEqual(['b']);
    expect(diff.unchanged.map(key)).toEqual(['a']);
  });

  it('reemplazar pisa el local y agrega lo nuevo', () => {
    expect(mergeByKey(existing, diff, 'replace', key, rekey)).toEqual([{ id: 'a', v: 1 }, { id: 'b', v: 9 }, { id: 'c', v: 3 }]);
  });

  it('conservar ambos agrega el conflictivo con otro id y deja el local intacto', () => {
    const out = mergeByKey(existing, diff, 'keepBoth', key, rekey);
    expect(out).toHaveLength(4);
    expect(out.find((x) => x.id === 'b')).toEqual({ id: 'b', v: 2 });
    expect(out.find((x) => x.id === 'b (importado)')).toEqual({ id: 'b (importado)', v: 9 });
  });

  it('omitir solo agrega lo nuevo', () => {
    expect(mergeByKey(existing, diff, 'skip', key, rekey).map((x) => [x.id, x.v])).toEqual([['a', 1], ['b', 2], ['c', 3]]);
  });

  it('cuenta lo que toca cada política', () => {
    expect(CONFLICT_POLICIES.map((p) => touched(diff, p))).toEqual([2, 2, 1]);
  });

  it('un nombre ya ocupado recibe un número', () => {
    expect(uniqueLabel('X', new Set(['X (importado)', 'X (importado 2)']))).toBe('X (importado 3)');
    expect(uniqueLabel('X'.repeat(100), new Set(), 20).length).toBeLessThanOrEqual(20);
  });

  it('las políticas por omisión: máquinas y costos reemplazan, lo demás conserva ambos', () => {
    expect(defaultPolicies()).toEqual({ devices: 'replace', materials: 'keepBoth', costs: 'replace', remnants: 'keepBoth', templates: 'keepBoth' });
  });
});

describe('costos', () => {
  const local = () => {
    const c = defaultStored('es', 'es-MX');
    c.machines = { falcon: { hourlyRate: 10, tubePrice: 0, tubeLifeH: 0, watts: 100, accessoryWatts: 0 } };
    c.materials = [{ name: 'MDF 3', mode: 'bbox', material: { sheetPrice: 50, sheetW: 600, sheetH: 400, wastePct: 10, piecesPerSheet: 1 } }];
    return c;
  };
  const bundleCosts = () => ({
    machines: { falcon: { hourlyRate: 99, tubePrice: 0, tubeLifeH: 0, watts: 100, accessoryWatts: 0 }, otra: { hourlyRate: 5, tubePrice: 0, tubeLifeH: 0, watts: 60, accessoryWatts: 0 } },
    sales: { ...local().sales, margin: 0.9 },
    materials: [
      { name: 'MDF 3', mode: 'area', material: { sheetPrice: 80, sheetW: 600, sheetH: 400, wastePct: 10, piecesPerSheet: 1 } },
      { name: 'Acrílico', mode: 'bbox', material: { sheetPrice: 120, sheetW: 600, sheetH: 400, wastePct: 5, piecesPerSheet: 1 } },
    ],
  });

  it('el plan separa nuevos, distintos e iguales', () => {
    const plan = planCosts(bundleCosts(), local(), 'es')!;
    expect(plan.machines.added.map((e) => e.id)).toEqual(['otra']);
    expect(plan.machines.conflicts.map((e) => e.id)).toEqual(['falcon']);
    expect(plan.materials.added.map((p) => p.name)).toEqual(['Acrílico']);
    expect(plan.materials.conflicts.map((p) => p.name)).toEqual(['MDF 3']);
    expect(plan.sales).toBe('different');
  });

  it('sobre lo mismo no hay cambios', () => {
    const same = { machines: local().machines, sales: local().sales, materials: local().materials };
    const plan = planCosts(same, local(), 'es')!;
    expect(plan.machines.conflicts).toHaveLength(0);
    expect(plan.materials.conflicts).toHaveLength(0);
    expect(plan.sales).toBe('same');
  });

  it('reemplazar pisa máquina, perfil y venta; omitir solo agrega lo nuevo', () => {
    const plan = planCosts(bundleCosts(), local(), 'es')!;
    const replaced = resolveCosts(plan, local(), 'replace');
    expect(replaced.machines.falcon!.hourlyRate).toBe(99);
    expect(replaced.machines.otra).toBeTruthy();
    expect(replaced.materials.find((p) => p.name === 'MDF 3')!.mode).toBe('area');
    expect(replaced.sales.margin).toBe(0.9);
    const skipped = resolveCosts(plan, local(), 'skip');
    expect(skipped.machines.falcon!.hourlyRate).toBe(10);
    expect(skipped.machines.otra).toBeTruthy();
    expect(skipped.materials.map((p) => p.name)).toEqual(['MDF 3', 'Acrílico']);
    expect(skipped.materials[0]!.mode).toBe('bbox');
    expect(skipped.sales).toEqual(local().sales);
  });

  it('conservar ambos renombra el perfil de material y conserva los costos locales de la máquina', () => {
    const plan = planCosts(bundleCosts(), local(), 'es')!;
    const both = resolveCosts(plan, local(), 'keepBoth');
    expect(both.materials.map((p) => p.name)).toEqual(['MDF 3', 'Acrílico', 'MDF 3 (importado)']);
    expect(both.machines.falcon!.hourlyRate).toBe(10);
    expect(both.sales).toEqual(local().sales);
  });

  it('descarta con cuenta lo que no es válido y devuelve null si no es una sección', () => {
    expect(planCosts(null, local(), 'es')).toBeNull();
    const plan = planCosts({ machines: {}, materials: [{ name: '' }, 'x'] }, local(), 'es')!;
    expect(plan.invalid).toBe(2);
    expect(plan.sales).toBeNull();
  });
});

describe('retazos', () => {
  it('descarta los inválidos y los de id repetido', () => {
    const { remnants, invalid } = sanitizeRemnants({ remnants: [remnant('a'), remnant('a'), { id: 'x' }, remnant('b', { width_mm: Number.NaN }), 'z', remnant('c')] });
    expect(remnants.map((r) => r.id)).toEqual(['a', 'c']);
    expect(invalid).toBe(4);
  });

  it('plan y resolución por política', () => {
    const local = { version: 1, remnants: [remnant('a'), remnant('b')] };
    const plan = planRemnants({ version: 1, remnants: [remnant('a'), remnant('b', { name: 'Cambiado' }), remnant('c')] }, local)!;
    expect(plan.diff.unchanged.map((r) => r.id)).toEqual(['a']);
    expect(plan.diff.conflicts.map((r) => r.id)).toEqual(['b']);
    expect(plan.diff.added.map((r) => r.id)).toEqual(['c']);
    expect(resolveRemnants(plan, local, 'replace').library.remnants.find((r) => r.id === 'b')!.name).toBe('Cambiado');
    expect(resolveRemnants(plan, local, 'skip').library.remnants.map((r) => r.id)).toEqual(['a', 'b', 'c']);
    const both = resolveRemnants(plan, local, 'keepBoth').library.remnants;
    expect(both.map((r) => r.id)).toEqual(['a', 'b', 'c', 'b-imp']);
    expect(both.find((r) => r.id === 'b')!.name).toBe('Retazo b');
    expect(both.find((r) => r.id === 'b-imp')!.name).toBe('Cambiado (importado)');
  });

  it('respeta el máximo del inventario y dice cuántos dejó fuera', () => {
    const many = Array.from({ length: 499 }, (_, i) => remnant(`r${i}`));
    const local = { version: 1, remnants: many };
    const plan = planRemnants({ version: 1, remnants: [remnant('n1'), remnant('n2'), remnant('n3')] }, local)!;
    const out = resolveRemnants(plan, local, 'replace');
    expect(out.library.remnants).toHaveLength(500);
    expect(out.dropped).toBe(2);
  });
});

describe('plantillas', () => {
  const doc = (name: string): Document => ({ ...sampleDocument(), name }) as Document;

  it('valida el nombre como el motor', () => {
    for (const ok of ['Mi plantilla', 'Taza 90x90', 'ñandú']) expect(validTemplateName(ok), ok).toBe(true);
    for (const bad of ['', '  ', 'a/b', 'a\\b', 'a:b', '.oculta', 'termina.', 'a*b', 'x'.repeat(81), 'con\u0007control']) expect(validTemplateName(bad), bad).toBe(false);
  });

  it('descarta plantillas inválidas, sin documento o de nombre repetido (sin distinguir mayúsculas)', () => {
    const { items, invalid } = sanitizeTemplates({ items: [{ name: 'A', doc: doc('a') }, { name: 'a', doc: doc('a2') }, { name: 'B/x', doc: doc('b') }, { name: 'C', doc: { nada: 1 } }, 5] });
    expect(items.map((i) => i.name)).toEqual(['A']);
    expect(invalid).toBe(4);
  });

  it('compara por nombre y contenido; escribe según la política', () => {
    const local = [{ name: 'A', doc: doc('a') }, { name: 'B', doc: doc('b') }];
    const plan = planTemplates({ items: [{ name: 'a', doc: doc('a') }, { name: 'B', doc: doc('otro') }, { name: 'C', doc: doc('c') }] }, local)!;
    expect(plan.diff.unchanged.map((i) => i.name)).toEqual(['a']);
    expect(plan.diff.conflicts.map((i) => i.name)).toEqual(['B']);
    expect(plan.diff.added.map((i) => i.name)).toEqual(['C']);
    const names = ['A', 'B'];
    expect(templatesToWrite(plan, names, 'replace').map((i) => i.name)).toEqual(['C', 'B']);
    expect(templatesToWrite(plan, names, 'skip').map((i) => i.name)).toEqual(['C']);
    expect(templatesToWrite(plan, names, 'keepBoth').map((i) => i.name)).toEqual(['C', 'B (importado)']);
    // Si «B (importado)» ya existe localmente, el siguiente es «B (importado 2)».
    expect(templatesToWrite(plan, ['A', 'B', 'b (importado)'], 'keepBoth').map((i) => i.name)).toEqual(['C', 'B (importado 2)']);
  });
});

describe('historial', () => {
  it('agrega solo las entradas cuyo id no está y cuenta las inválidas', () => {
    const plan = planHistory({ entries: [{ id: 'a', at: 1 }, { id: 'b', at: 2 }, { id: 'b', at: 2 }, { id: '', at: 1 }, { id: 'c' }, 'x'] }, new Set(['a']))!;
    expect(plan.added.map((e) => e.id)).toEqual(['b']);
    expect(plan.unchanged).toBe(1);
    expect(plan.invalid).toBe(4);
    expect(planHistory(undefined, new Set())).toBeNull();
  });
});
