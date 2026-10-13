import { describe, expect, it } from 'vitest';
import { falconProSettings, genericGrblSettings } from '@/api/mock/probe';
import { findEntry } from './catalog';
import { compareWithMachine, defaultMachineChoices, numericSettings } from './compare';

const falcon22 = findEntry('creality-falcon2-22w')!;
const rowOf = (c: ReturnType<typeof compareWithMachine>, f: string) => c.rows.find((r) => r.field === f)!;

describe('numericSettings', () => {
  it('convierte "$130" -> 130 e ignora lo que no es número', () => {
    expect(numericSettings({ $30: '1000', $130: '400.000', $x: '1', $5: 'abc' })).toEqual({ 30: 1000, 130: 400 });
    expect(numericSettings(null)).toEqual({});
  });
});

describe('compareWithMachine', () => {
  it('Falcon2 22 W frente a un volcado de Pro: $30 coincide, la cama difiere en Y, la velocidad en X e Y', () => {
    const c = compareWithMachine(falcon22, falconProSettings());
    expect(rowOf(c, 's_max')).toMatchObject({ catalog: 1000, machine: 1000, status: 'match' });
    expect(rowOf(c, 'bed')).toMatchObject({ catalog: [400, 415], machine: [400, 400], status: 'differs' });
    expect(rowOf(c, 'max_rate')).toMatchObject({ catalog: [25000, 25000], machine: [50000, 50000], status: 'differs' });
    expect(rowOf(c, 'accel')).toMatchObject({ catalog: [1000, 1000], machine: [5000, 5000], status: 'differs' });
    expect(c.laserMode).toBe('on');
    expect(c.nothingRead).toBe(false);
  });

  it('modelo sin dato en el catálogo: el de la máquina es el único', () => {
    const c = compareWithMachine(findEntry('sculpfun-s30')!, genericGrblSettings());
    expect(rowOf(c, 'bed')).toMatchObject({ catalog: null, machine: [400, 400], status: 'catalogEmpty' });
    expect(rowOf(c, 'max_rate')).toMatchObject({ catalog: [6000, 6000], machine: [6000, 6000], status: 'match' });
    expect([...defaultMachineChoices(c)].sort()).toEqual(['accel', 'bed', 's_max']);
  });

  it('$30 distinto del s_max del catálogo se marca (GRBL-LPC trae 1.0)', () => {
    const c = compareWithMachine(findEntry('grbl-generic')!, { $30: '1' });
    expect(rowOf(c, 's_max')).toMatchObject({ catalog: 1000, machine: 1, status: 'differs' });
    expect(defaultMachineChoices(c).has('s_max')).toBe(true);
  });

  it('un ajuste que la máquina no contestó queda "unread" y no se propone', () => {
    const c = compareWithMachine(falcon22, { $30: '1000', $32: '1' });
    expect(rowOf(c, 'bed').status).toBe('unread');
    expect(rowOf(c, 'bed').machine).toBeNull();
    expect(defaultMachineChoices(c).size).toBe(0);
  });

  it('solo un eje de la pareja no cuenta como leído', () => {
    const c = compareWithMachine(falcon22, { $130: '400' });
    expect(rowOf(c, 'bed').status).toBe('unread');
  });

  it('$32: 1 activo, 0 desactivado (rojo), otro valor, ausente', () => {
    const mode = (s: Record<string, string> | null) => compareWithMachine(null, s).laserMode;
    expect(mode({ $32: '1' })).toBe('on');
    expect(mode({ $32: '0' })).toBe('off');
    expect(mode({ $32: '2' })).toBe('other');
    expect(mode({ $30: '1000' })).toBe('unknown');
    expect(mode(null)).toBe('unknown');
  });

  it('sin ajustes no hay nada que comparar', () => {
    expect(compareWithMachine(falcon22, null).nothingRead).toBe(true);
    expect(compareWithMachine(falcon22, {}).nothingRead).toBe(true);
  });

  it('tolera diferencias de redondeo (400.000 frente a 400)', () => {
    const c = compareWithMachine(falcon22, { $130: '400.004', $131: '415.000' });
    expect(rowOf(c, 'bed').status).toBe('match');
  });
});
