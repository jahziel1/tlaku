import { describe, expect, it } from 'vitest';
import type { MaterialLibrary, MaterialPreset } from '@/api/types';
import { makeLayer } from '@/lib/layers';
import {
  applyPresetChange,
  applyPresetToLayer,
  duplicatePreset,
  filterPresets,
  formProblem,
  groupPresets,
  layerMatchesPreset,
  matchingLayers,
  mergeLibraries,
  presetFromLayer,
  presetPatch,
  settingsSummary,
  thicknessLabel,
  uniqueId,
} from './library';

const preset = (id: string, material: string, thickness: number | null, title: string, patch = {}, extra: Partial<MaterialPreset> = {}): MaterialPreset => ({
  id, material, thickness, title, notes: '', device_id: null, settings: makeLayer('#123456', title, patch), ...extra,
});

const lib = (...presets: MaterialPreset[]): MaterialLibrary => ({ version: 1, presets });

describe('agrupar', () => {
  const presets = [
    preset('1', 'MDF', 6, 'Corte'),
    preset('2', 'acrílico negro', 3, 'Grabado'),
    preset('3', 'MDF', 3, 'Corte'),
    preset('4', 'mdf', 3, 'Grabado'),
    preset('5', 'MDF', null, 'Marcado'),
    preset('6', 'Acrílico Negro', 3, 'Corte'),
  ];

  it('une materiales sin distinguir mayúsculas ni acentos y los ordena', () => {
    const groups = groupPresets(presets);
    expect(groups.map((g) => g.material)).toEqual(['acrílico negro', 'MDF']);
  });

  it('ordena los grosores de menor a mayor con "sin grosor" al final y los ajustes por título', () => {
    const mdf = groupPresets(presets).find((g) => g.material === 'MDF')!;
    expect(mdf.thicknesses.map((t) => t.thickness)).toEqual([3, 6, null]);
    expect(mdf.thicknesses[0]!.presets.map((p) => p.title)).toEqual(['Corte', 'Grabado']);
  });

  it('una biblioteca vacía no tiene grupos', () => {
    expect(groupPresets([])).toEqual([]);
  });

  it('da etiqueta de grosor con coma decimal', () => {
    expect(thicknessLabel(3)).toBe('3 mm');
    expect(thicknessLabel(0.5)).toBe('0,5 mm');
    expect(thicknessLabel(null)).toBe('');
  });
});

describe('filtrar', () => {
  const presets = [
    preset('1', 'MDF', 3, 'Corte', {}, { notes: 'Con aire', device_id: 'a' }),
    preset('2', 'Acrílico', 5, 'Grabado', {}, { device_id: 'b' }),
    preset('3', 'Cuero', null, 'Marcado', {}, { device_id: null }),
  ];
  const ids = (f: Parameters<typeof filterPresets>[1]) => filterPresets(presets, f).map((p) => p.id);

  it('sin criterios deja todo', () => {
    expect(ids({ search: '', deviceId: null })).toEqual(['1', '2', '3']);
    expect(ids({ search: '   ', deviceId: null })).toEqual(['1', '2', '3']);
  });

  it('busca en material, operación, notas y grosor sin importar acentos ni mayúsculas', () => {
    expect(ids({ search: 'acrilico', deviceId: null })).toEqual(['2']);
    expect(ids({ search: 'CORTE', deviceId: null })).toEqual(['1']);
    expect(ids({ search: 'aire', deviceId: null })).toEqual(['1']);
    expect(ids({ search: '5 mm', deviceId: null })).toEqual(['2']);
  });

  it('todas las palabras deben coincidir', () => {
    expect(ids({ search: 'mdf corte', deviceId: null })).toEqual(['1']);
    expect(ids({ search: 'mdf grabado', deviceId: null })).toEqual([]);
  });

  it('"solo esta máquina" conserva los de la máquina y los de cualquiera', () => {
    expect(ids({ search: '', deviceId: 'a' })).toEqual(['1', '3']);
    expect(ids({ search: '', deviceId: 'b' })).toEqual(['2', '3']);
    expect(ids({ search: 'cuero', deviceId: 'a' })).toEqual(['3']);
  });
});

describe('aplicar un ajuste a una capa', () => {
  const settings = {
    mode: 'fill' as const, speed: 4000, power: 33, passes: 2, air: true, interval: 0.15, angle: 30, kerf: 0.1, z_offset: 2,
    tabs: { enabled: true, width: 1, count: 2, spacing: 0, skip_inner: false },
  };
  const p = preset('1', 'MDF', 3, 'Grabado', settings);

  it('copia todos los campos menos color, nombre, salida, visibilidad y orden', () => {
    const patch = presetPatch(p);
    for (const k of ['color', 'name', 'output', 'visible', 'order']) expect(patch).not.toHaveProperty(k);
    expect(patch).toMatchObject(settings);
    expect(Object.keys(patch)).toContain('fill_style');
  });

  it('conserva la identidad de la capa y cambia el resto', () => {
    const layer = makeLayer('#ff0000', 'Mi capa', { output: false, visible: false, order: 3 });
    const next = applyPresetToLayer(layer, p);
    expect(next).toMatchObject({ color: '#ff0000', name: 'Mi capa', output: false, visible: false, order: 3 });
    expect(next).toMatchObject(settings);
    expect(layerMatchesPreset(next, p)).toBe(true);
    expect(layerMatchesPreset(layer, p)).toBe(false);
  });

  it('aplicar dos veces devuelve la misma capa (sin cambios)', () => {
    const once = applyPresetToLayer(makeLayer('#ff0000', 'A'), p);
    expect(applyPresetToLayer(once, p)).toBe(once);
  });

  it('un ajuste viejo sin los campos nuevos se completa con los de fábrica', () => {
    const old = preset('2', 'X', null, 'Viejo');
    const { z_offset: _z, tabs: _t, ...rest } = old.settings;
    void _z;
    void _t;
    const patch = presetPatch({ settings: rest as never });
    expect(patch.z_offset).toBe(0);
    expect(patch.tabs.enabled).toBe(false);
  });

  it('el cambio de documento toca solo la capa pedida y conserva el documento si no hay cambios', () => {
    const a = makeLayer('#ff0000', 'A');
    const b = makeLayer('#0000ff', 'B');
    const doc = { layers: [a, b] };
    const out = applyPresetChange(doc, '#FF0000', p);
    expect(out.layers[0]).toMatchObject(settings);
    expect(out.layers[1]).toBe(b);
    expect(applyPresetChange(out, '#ff0000', p)).toBe(out);
    expect(applyPresetChange(doc, '#00ff00', p)).toBe(doc);
  });

  it('encuentra las capas que coinciden con el material', () => {
    const match = applyPresetToLayer(makeLayer('#ff0000', 'A'), p);
    const other = makeLayer('#0000ff', 'B');
    expect(matchingLayers([match, other], p)).toEqual([match]);
  });
});

describe('guardar y duplicar', () => {
  it('crea un ajuste desde la capa con los datos recortados', () => {
    const layer = makeLayer('#ff0000', 'Corte', { speed: 300, power: 90 });
    const p = presetFromLayer(layer, { material: ' MDF ', thickness: 3, title: ' Corte ', notes: ' a ', device_id: 'x' }, 'id1');
    expect(p).toMatchObject({ id: 'id1', material: 'MDF', thickness: 3, title: 'Corte', notes: 'a', device_id: 'x' });
    expect(p.settings).toMatchObject({ speed: 300, power: 90 });
    expect(p.settings).not.toBe(layer);
  });

  it('valida el formulario', () => {
    const ok = { material: 'MDF', thickness: 3, title: 'Corte', notes: '', device_id: null };
    expect(formProblem(ok)).toBeNull();
    expect(formProblem({ ...ok, thickness: null })).toBeNull();
    expect(formProblem({ ...ok, material: '  ' })).toBe('materials.form.needMaterial');
    expect(formProblem({ ...ok, title: '' })).toBe('materials.form.needTitle');
    expect(formProblem({ ...ok, thickness: 0 })).toBe('materials.form.badThickness');
    expect(formProblem({ ...ok, thickness: Number.NaN })).toBe('materials.form.badThickness');
  });

  it('un id nuevo nunca está tomado', () => {
    const taken = new Set<string>();
    const rand = () => 0.5;
    const first = uniqueId(taken, rand);
    taken.add(first);
    // Con el mismo azar y reloj el candidato choca; el bucle sigue hasta dar uno libre.
    let n = 0;
    const next = uniqueId(taken, () => (n++ < 3 ? 0.5 : 0.25));
    expect(taken.has(next)).toBe(false);
  });

  it('duplica con id nuevo y sin compartir objetos', () => {
    const p = preset('1', 'MDF', 3, 'Corte');
    const copy = duplicatePreset(p, 'nuevo', '(copia)');
    expect(copy).toMatchObject({ id: 'nuevo', title: 'Corte (copia)', material: 'MDF' });
    expect(copy.settings).not.toBe(p.settings);
    expect(p.id).toBe('1');
  });

  it('resume los ajustes', () => {
    expect(settingsSummary(makeLayer('#000', 'x', { speed: 1000, power: 60, passes: 2 }))).toBe('1000 mm/min · 60 % · ×2');
    expect(settingsSummary(makeLayer('#000', 'x', { mode: 'fill', speed: 3000, power: 30, interval: 0.1 }))).toBe('3000 mm/min · 30 % · 0.1 mm');
  });
});

describe('mezclar al importar', () => {
  const rand = () => 0.123;

  it('agrega los ajustes con ids nuevos tal cual', () => {
    const r = mergeLibraries(lib(preset('a', 'MDF', 3, 'Corte')), lib(preset('b', 'Balsa', 2, 'Corte')), rand);
    expect(r.library.presets.map((p) => p.id)).toEqual(['a', 'b']);
    expect(r).toMatchObject({ added: 1, renamed: 0, skipped: 0 });
  });

  it('si un id choca con contenido distinto, duplica con id nuevo y no pisa el existente', () => {
    const mine = preset('a', 'MDF', 3, 'Corte', { speed: 300 });
    const theirs = preset('a', 'MDF', 3, 'Corte', { speed: 150 });
    const r = mergeLibraries(lib(mine), lib(theirs), rand);
    expect(r.library.presets).toHaveLength(2);
    expect(r.library.presets[0]).toBe(mine);
    const copy = r.library.presets[1]!;
    expect(copy.id).not.toBe('a');
    expect(copy.settings.speed).toBe(150);
    expect(r).toMatchObject({ added: 1, renamed: 1, skipped: 0 });
  });

  it('un ajuste idéntico no se duplica: importar dos veces el mismo archivo no cambia nada', () => {
    const file = lib(preset('a', 'MDF', 3, 'Corte'), preset('b', 'Balsa', 2, 'Corte'));
    const once = mergeLibraries(lib(), file, rand);
    const twice = mergeLibraries(once.library, file, rand);
    expect(twice.library.presets).toHaveLength(2);
    expect(twice).toMatchObject({ added: 0, renamed: 0, skipped: 2 });
  });

  it('un ajuste idéntico a uno que ya está con otro id tampoco se agrega', () => {
    const mine = preset('a', 'MDF', 3, 'Corte');
    const r = mergeLibraries(lib(mine), lib({ ...mine, id: 'otro' }), rand);
    expect(r.library.presets).toHaveLength(1);
    expect(r.skipped).toBe(1);
  });

  it('dos choques del mismo archivo reciben ids distintos', () => {
    const mine = lib(preset('a', 'MDF', 3, 'Corte'), preset('b', 'MDF', 6, 'Corte'));
    const theirs = lib(preset('a', 'MDF', 3, 'Otra', { speed: 5 }), preset('b', 'MDF', 6, 'Otra', { speed: 6 }));
    const r = mergeLibraries(mine, theirs, rand);
    const ids = r.library.presets.map((p) => p.id);
    expect(new Set(ids).size).toBe(4);
    expect(r.renamed).toBe(2);
  });

  it('el archivo importado no se modifica', () => {
    const theirs = lib(preset('a', 'MDF', 3, 'Otra', { speed: 5 }));
    const before = structuredClone(theirs);
    mergeLibraries(lib(preset('a', 'MDF', 3, 'Corte')), theirs, rand);
    expect(theirs).toEqual(before);
  });
});
