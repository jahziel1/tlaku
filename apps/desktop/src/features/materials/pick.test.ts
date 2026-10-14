import { describe, expect, it } from 'vitest';
import { factoryMaterialLibrary } from '@/api/mock/materials';
import type { MachineClass, MaterialLibrary, MaterialPreset } from '@/api/types';
import { makeLayer } from '@/lib/layers';
import { classDistance, machineClassOf, nearestClass } from './machineClass';
import { describePick, pickPreset, restrictionFor, type PickQuery } from './pick';
import { setLanguage } from '@/i18n';

const base = (patch: Partial<MaterialPreset>): MaterialPreset => ({
  id: 'x',
  material: 'MDF',
  thickness: 3,
  title: 'Corte',
  notes: '',
  device_id: null,
  settings: makeLayer('#000000', 'Corte', { speed: 100 }),
  ...patch,
});

const query = (patch: Partial<PickQuery> = {}): PickQuery => ({
  material: 'mdf',
  thickness: 3,
  role: 'cut',
  deviceId: 'mi-laser',
  kind: 'diode',
  machineClass: 'diode_10',
  ...patch,
});

const lib = (...presets: MaterialPreset[]): MaterialLibrary => ({ version: 1, presets });
const noFactory = lib();

describe('clase de máquina', () => {
  it('la clase más cercana respeta el tipo de láser y, en un empate, gana la de menos potencia', () => {
    expect(nearestClass('diode', 22)).toBe('diode_20');
    expect(nearestClass('diode', 12)).toBe('diode_10');
    expect(nearestClass('diode', 15)).toBe('diode_10');
    expect(nearestClass('diode', 100)).toBe('diode_40');
    expect(nearestClass('co2', 50)).toBe('co2_40');
    expect(nearestClass('co2', 130)).toBe('co2_100');
    expect(nearestClass('fiber', 60)).toBe('fiber_50');
    expect(classDistance('diode_5', 'diode_20')).toBe(15);
  });

  it('deduce el tipo de láser del perfil y la potencia solo si se conoce', () => {
    const grbl = { kind: 'grbl' as const, galvo: { source: 'fiber_q' as const, watts: 20 } };
    expect(machineClassOf(grbl)).toEqual({ kind: 'diode', machineClass: null });
    expect(machineClassOf(grbl, 22)).toEqual({ kind: 'diode', machineClass: 'diode_20' });
    expect(machineClassOf({ ...grbl, kind: 'ruida' }, 60)).toEqual({ kind: 'co2', machineClass: 'co2_60' });
    expect(machineClassOf({ ...grbl, kind: 'lhy_micro' })).toEqual({ kind: 'co2', machineClass: null });
    // El galvo trae su potencia y su tipo en el perfil.
    expect(machineClassOf({ kind: 'galvo', galvo: { source: 'fiber_mopa', watts: 30 } })).toEqual({ kind: 'fiber', machineClass: 'fiber_30' });
    expect(machineClassOf({ kind: 'galvo', galvo: { source: 'co2', watts: 40 } })).toEqual({ kind: 'co2', machineClass: 'co2_40' });
    expect(machineClassOf({ kind: 'galvo', galvo: { source: 'uv', watts: 5 } })).toEqual({ kind: null, machineClass: null });
  });
});

describe('elección del ajuste', () => {
  it('primero el de la máquina, luego el de su clase, luego uno genérico y al final la clase más cercana', () => {
    const device = base({ id: 'device', device_id: 'mi-laser', machine_class: 'diode_20' });
    const sameClass = base({ id: 'class', machine_class: 'diode_10' });
    const generic = base({ id: 'generic' });
    const other = base({ id: 'other', machine_class: 'diode_40' });
    const pick = (user: MaterialPreset[], q = query()) => pickPreset(user, noFactory, q);

    let r = pick([other, generic, sameClass, device]);
    expect([r.preset?.id, r.match, r.mustTest]).toEqual(['device', 'device', false]);
    r = pick([other, generic, sameClass]);
    expect([r.preset?.id, r.match, r.mustTest]).toEqual(['class', 'class', false]);
    r = pick([other, generic]);
    expect([r.preset?.id, r.match, r.mustTest]).toEqual(['generic', 'generic', true]);
    r = pick([other]);
    expect([r.preset?.id, r.match, r.mustTest, r.usedClass]).toEqual(['other', 'nearest', true, 'diode_40']);
    r = pick([]);
    expect([r.preset, r.match]).toEqual([null, 'none']);
  });

  it('la clase más cercana por potencia nunca cruza de un tipo de láser a otro', () => {
    const co2 = base({ id: 'co2', machine_class: 'co2_40' });
    expect(pickPreset([co2], noFactory, query()).match).toBe('none');
    const d5 = base({ id: 'd5', machine_class: 'diode_5' });
    const d40 = base({ id: 'd40', machine_class: 'diode_40' });
    // Para un diodo de 10 W: el de 5 W está a 5 W y el de 40 W a 30 W.
    expect(pickPreset([d40, d5], noFactory, query()).preset?.id).toBe('d5');
    // En un empate (15 W entre 5 y 20... 10 a 5 y 10 a 20) gana el de menos potencia.
    const d20 = base({ id: 'd20', machine_class: 'diode_20' });
    expect(pickPreset([d20, d5], noFactory, query()).preset?.id).toBe('d5');
  });

  it('no mezcla otros materiales, otras operaciones ni otros grosores', () => {
    const user = [
      base({ id: 'mdf6', thickness: 6, machine_class: 'diode_10' }),
      base({ id: 'grabado', machine_class: 'diode_10', settings: makeLayer('#000000', 'G', { mode: 'fill' }) }),
      base({ id: 'acrilico', material: 'Acrílico', machine_class: 'diode_10' }),
    ];
    expect(pickPreset(user, noFactory, query()).preset).toBeNull();
    // Con rol declarado, el rol manda sobre el modo.
    const cut = base({ id: 'cut', machine_class: 'diode_10', role: 'cut', settings: makeLayer('#000000', 'G', { mode: 'fill' }) });
    expect(pickPreset([cut], noFactory, query()).preset?.id).toBe('cut');
    expect(pickPreset([cut], noFactory, query({ role: 'engrave' })).preset).toBeNull();
  });

  it('un ajuste sin grosor vale para cualquiera, pero el grosor exacto gana', () => {
    const any = base({ id: 'any', thickness: null, machine_class: 'diode_10' });
    const exact = base({ id: 'exact', machine_class: 'diode_10' });
    expect(pickPreset([any, exact], noFactory, query()).preset?.id).toBe('exact');
    expect(pickPreset([any], noFactory, query({ thickness: 4 })).preset?.id).toBe('any');
  });

  it('entre dos del mismo nivel gana el del usuario y luego la fuente más confiable', () => {
    const mine = base({ id: 'mine', machine_class: 'diode_10' });
    const official = base({ id: 'official', machine_class: 'diode_10', confidence: 'official' });
    const guide = base({ id: 'guide', machine_class: 'diode_10', confidence: 'guide' });
    expect(pickPreset([mine], lib(official), query()).preset?.id).toBe('mine');
    expect(pickPreset([], lib(guide, official), query()).preset?.id).toBe('official');
  });

  it('sin saber la potencia parte de la clase más baja del tipo y avisa', () => {
    const d5 = base({ id: 'd5', machine_class: 'diode_5' });
    const d20 = base({ id: 'd20', machine_class: 'diode_20' });
    const r = pickPreset([d20, d5], noFactory, query({ machineClass: null }));
    expect([r.preset?.id, r.match, r.powerUnknown, r.mustTest]).toEqual(['d5', 'class', true, true]);
    expect(describePick(r, null).key).toBe('materials.pick.powerUnknown');
  });

  it('sin tipo de láser conocido solo valen los ajustes genéricos', () => {
    const d10 = base({ id: 'd10', machine_class: 'diode_10' });
    const generic = base({ id: 'generic' });
    expect(pickPreset([d10], noFactory, query({ kind: null, machineClass: null })).preset).toBeNull();
    expect(pickPreset([d10, generic], noFactory, query({ kind: null, machineClass: null })).preset?.id).toBe('generic');
  });

  it('el rol que falta se deduce del modo de la capa', () => {
    const legacy = base({ id: 'legacy', machine_class: 'diode_10', settings: makeLayer('#000000', 'G', { mode: 'fill' }) });
    expect(pickPreset([legacy], noFactory, query({ role: 'engrave' })).preset?.id).toBe('legacy');
  });
});

describe('con la biblioteca de fábrica', () => {
  const factory = factoryMaterialLibrary();
  const pick = (patch: Partial<PickQuery>, user: MaterialPreset[] = []) => pickPreset(user, factory, query({ deviceId: null, ...patch }));

  it('MDF de 3 mm con un diodo de 20 W trae su clase, con fuente y aviso de rango', () => {
    const r = pick({ machineClass: 'diode_20' });
    expect(r.match).toBe('class');
    expect(r.preset?.machine_class).toBe('diode_20');
    expect(r.preset?.source).toMatch(/^https:\/\//);
    expect(r.warnings).toContain('range_edge');
    expect(r.mustTest).toBe(false);
  });

  it('MDF con un diodo de 5 W usa la clase más cercana y avisa que hay que probar', () => {
    const r = pick({ machineClass: 'diode_5' });
    expect(r.match).toBe('nearest');
    expect(r.usedClass).toBe('diode_10');
    expect(r.mustTest).toBe(true);
    expect(describePick(r, 'diode_5').key).toBe('materials.pick.nearest');
  });

  it('CO2 de 100 W elige su clase y la de 70 W (sin ajuste propio) la más cercana', () => {
    expect(pick({ kind: 'co2', machineClass: 'co2_100' }).usedClass).toBe('co2_100');
    // El material con ajuste para co2_60 y co2_80: una clase "co2_70" se resuelve a co2_60 (empate, gana la baja).
    expect(nearestClass('co2', 70)).toBe('co2_60');
    expect(pick({ kind: 'co2', machineClass: nearestClass('co2', 70) }).usedClass).toBe('co2_60');
  });

  it('el acrílico transparente no tiene ajuste de diodo y se explica; con CO2 sí hay', () => {
    const diode = pick({ material: 'acrylic_clear', machineClass: 'diode_20' });
    expect(diode.preset).toBeNull();
    expect(diode.restriction?.warning).toBe('not_cut_with_diode');
    expect(describePick(diode, 'diode_20').key).toBe('materials.pick.restricted');
    const co2 = pick({ material: 'acrylic_clear', kind: 'co2', machineClass: 'co2_60' });
    expect(co2.preset?.machine_class).toBe('co2_60');
    expect(restrictionFor(factory, 'acrylic_clear', 'co2')).toBeNull();
  });

  it('el acero inoxidable solo se marca con fibra o con aviso de recubrimiento', () => {
    const fiber = pick({ material: 'stainless_steel', role: 'engrave', thickness: null, kind: 'fiber', machineClass: 'fiber_20' });
    expect(fiber.match).toBe('class');
    expect(fiber.preset?.settings.galvo.frequency_khz).toBeGreaterThan(0);
    const diode = pick({ material: 'stainless_steel', role: 'engrave', thickness: null, machineClass: 'diode_10' });
    expect(diode.warnings).toContain('needs_coating');
    expect(pick({ material: 'stainless_steel', role: 'engrave', thickness: null, kind: 'co2', machineClass: 'co2_60' }).preset).toBeNull();
  });

  it('un ajuste del usuario para la misma clase gana al de fábrica', () => {
    const mine = base({ id: 'mine', material: 'MDF', machine_class: 'diode_20', settings: makeLayer('#000000', 'Mío', { speed: 111 }) });
    expect(pick({ machineClass: 'diode_20' }, [mine]).preset?.id).toBe('mine');
  });

  it('describePick cubre el caso del ajuste de la máquina y de la clase', () => {
    setLanguage('es');
    const device = base({ id: 'device', device_id: 'mi-laser' });
    const r = pickPreset([device], factory, query());
    expect(describePick(r, 'diode_10').key).toBe('materials.pick.device');
    const c: MachineClass = 'diode_10';
    expect(describePick(pick({ machineClass: c }), c).key).toBe('materials.pick.class');
  });
});
