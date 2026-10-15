import { describe, expect, it } from 'vitest';
import {
  needsSoftLimits,
  buildBackupText,
  diffBackup,
  parseBackupText,
  parseRawSettings,
  sameValue,
  settingLine,
  validateSettingValue,
  type RawSetting,
} from './MachineSettingsLogic';

const raw = (id: number, value: string): RawSetting => ({ id, raw: value, value: Number(value) });

describe('lectura de $$ con el texto crudo', () => {
  it('conserva el texto de cada valor, ordena y descarta ok, avisos y comentarios', () => {
    const p = parseRawSettings(['$130=400.000', '$0=10', '[MSG:Caution]', '; nota', '$32=1', '  $100 = 80.000  ', 'ok']);
    expect(p.settings.map((s) => [s.id, s.raw, s.value])).toEqual([
      [0, '10', 10],
      [32, '1', 1],
      [100, '80.000', 80],
      [130, '400.000', 400],
    ]);
    expect(p.namedLines).toBe(0);
  });

  it('un valor que no es número queda con value null; si se repite, vale el último', () => {
    const p = parseRawSettings(['$300=mi-laser', '$30=500', '$30=1000']);
    expect(p.settings).toEqual([
      { id: 30, raw: '1000', value: 1000 },
      { id: 300, raw: 'mi-laser', value: null },
    ]);
  });

  it('cuenta las líneas con nombre de FluidNC sin mezclarlas con las numéricas', () => {
    const p = parseRawSettings(['$/axes/x/steps_per_mm=80', '$/axes/y/steps_per_mm=80', '$10=1', 'ok']);
    expect(p.namedLines).toBe(2);
    expect(p.settings).toHaveLength(1);
  });

  it('una respuesta sin ajustes da una lista vacía', () => {
    expect(parseRawSettings(['error:3', 'ok']).settings).toEqual([]);
  });
});

describe('validación de valores', () => {
  const ok = (id: number, input: string, family: 'grbl' | 'grblhal' | 'fluidnc' | 'unknown' = 'grbl') => {
    const v = validateSettingValue(id, input, family);
    return v.ok ? v.text : v.error;
  };

  it('acepta números y normaliza el texto (coma decimal, ceros de más, signo +)', () => {
    expect(ok(130, '400')).toBe('400');
    expect(ok(130, ' 415,5 ')).toBe('415.5');
    expect(ok(100, '80.000')).toBe('80');
    expect(ok(30, '+1000')).toBe('1000');
  });

  it('rechaza vacío, texto, notación científica, infinitos y NaN', () => {
    expect(ok(30, '')).toEqual({ key: 'mset.err.empty' });
    for (const bad of ['abc', '1e3', 'NaN', 'Infinity', '--5', '0x10', '1.2.3', '5 6']) {
      expect(ok(30, bad), bad).toEqual({ key: 'mset.err.notNumber' });
    }
  });

  it('los booleanos solo admiten 0 y 1', () => {
    expect(ok(32, '1')).toBe('1');
    expect(ok(32, '0')).toBe('0');
    expect(ok(32, '2')).toEqual({ key: 'mset.err.bool' });
    expect(ok(20, '0.5')).toEqual({ key: 'mset.err.bool' });
    expect(ok(5, '-1')).toEqual({ key: 'mset.err.bool' });
  });

  it('en grblHAL $32 admite 0 a 2 (modo láser o torno) y $22 una máscara', () => {
    expect(ok(32, '2', 'grblhal')).toBe('2');
    expect(ok(32, '3', 'grblhal')).toEqual({ key: 'mset.err.range', min: 0, max: 2 });
    expect(ok(22, '5', 'grblhal')).toBe('5');
    expect(ok(22, '5', 'grbl')).toEqual({ key: 'mset.err.bool' });
  });

  it('las máscaras de bits son enteros dentro de su rango', () => {
    expect(ok(3, '7')).toBe('7');
    expect(ok(3, '8')).toEqual({ key: 'mset.err.range', min: 0, max: 7 });
    expect(ok(3, '1.5')).toEqual({ key: 'mset.err.integer' });
    expect(ok(3, '-1')).toEqual({ key: 'mset.err.range', min: 0, max: 7 });
  });

  it('pasos, velocidades, aceleraciones y recorridos deben ser positivos', () => {
    for (const id of [100, 110, 120, 130]) {
      expect(ok(id, '0'), String(id)).toEqual({ key: 'mset.err.range', min: 0.001, max: expect.any(Number) });
      expect(ok(id, '-5'), String(id)).toMatchObject({ key: 'mset.err.range' });
    }
    expect(ok(131, '415')).toBe('415');
  });

  it('S máx. debe ser al menos 1; S mín. puede ser 0', () => {
    expect(ok(30, '0')).toMatchObject({ key: 'mset.err.range', min: 1 });
    expect(ok(31, '0')).toBe('0');
  });

  it('un ajuste desconocido acepta cualquier número razonable', () => {
    expect(ok(999, '-3.5')).toBe('-3.5');
    expect(ok(999, '1e12')).toEqual({ key: 'mset.err.notNumber' });
    expect(ok(999, '99999999999')).toMatchObject({ key: 'mset.err.range' });
  });

  it('settingLine arma la línea exacta', () => {
    expect(settingLine(32, '1')).toBe('$32=1');
  });

  it('sameValue compara números sin importar los decimales y texto tal cual', () => {
    expect(sameValue('25.000', '25')).toBe(true);
    expect(sameValue('0.010', '0.01')).toBe(true);
    expect(sameValue('400', '415')).toBe(false);
    expect(sameValue('abc', 'abc')).toBe(true);
    expect(sameValue('abc', '1')).toBe(false);
  });
});

describe('respaldo', () => {
  const settings = [raw(130, '400.000'), raw(30, '1000'), raw(32, '1')];

  it('lleva encabezado, fecha, firmware y una línea $n=valor por ajuste, en orden', () => {
    const text = buildBackupText(settings, 'Grbl 1.1h', new Date('2026-10-13T12:00:00Z'));
    expect(text.split('\n')).toEqual([
      '; Tlaku: respaldo de ajustes GRBL',
      '; fecha: 2026-10-13T12:00:00.000Z',
      '; firmware: Grbl 1.1h',
      '$30=1000',
      '$32=1',
      '$130=400.000',
      '',
    ]);
  });

  it('ida y vuelta: lo exportado se lee igual', () => {
    const parsed = parseBackupText(buildBackupText(settings, 'Grbl 1.1h', new Date()));
    expect(parsed.firmware).toBe('Grbl 1.1h');
    expect(parsed.entries).toEqual([
      { id: 30, raw: '1000' },
      { id: 32, raw: '1' },
      { id: 130, raw: '400.000' },
    ]);
    expect(parsed.ignored).toEqual([]);
  });

  it('salta comentarios, vacías y comentarios al final; ignora lo demás; avisa de repetidos; tolera CRLF y BOM', () => {
    const text = '﻿; Tlaku\r\n\r\n$0=10 ; pulso\r\nG0 X10\r\n$0=12\r\n(otro comentario)\r\n$X\r\n$30 = 500\r\n';
    const p = parseBackupText(text);
    expect(p.entries).toEqual([
      { id: 0, raw: '12' },
      { id: 30, raw: '500' },
    ]);
    expect(p.duplicates).toEqual([0]);
    expect(p.ignored).toEqual(['G0 X10', '(otro comentario)', '$X']);
  });

  it('un texto sin ajustes da cero entradas', () => {
    expect(parseBackupText('; solo comentarios\n').entries).toEqual([]);
  });
});

describe('vista previa de diferencias', () => {
  const now = [raw(30, '1000'), raw(32, '1'), raw(100, '80.000'), raw(130, '400.000')];

  it('muestra solo lo que cambia, con la línea exacta, y ignora los iguales aunque cambie el formato', () => {
    const diff = diffBackup(now, [
      { id: 30, raw: '1000' },
      { id: 32, raw: '0' },
      { id: 100, raw: '80' },
      { id: 130, raw: '410.5' },
    ]);
    expect(diff.same).toBe(2);
    expect(diff.changes).toEqual([
      { id: 32, current: '1', next: '0', line: '$32=0' },
      { id: 130, current: '400.000', next: '410.5', line: '$130=410.5' },
    ]);
    expect(diff.missing).toEqual([]);
    expect(diff.invalid).toEqual([]);
  });

  it('separa los ajustes que la máquina no tiene y los valores inválidos (no se escriben)', () => {
    const diff = diffBackup(now, [
      { id: 33, raw: '5000' },
      { id: 32, raw: 'abc' },
      { id: 130, raw: '-4' },
    ]);
    expect(diff.changes).toEqual([]);
    expect(diff.missing).toEqual([33]);
    expect(diff.invalid.map((i) => [i.id, i.error.key])).toEqual([
      [32, 'mset.err.notNumber'],
      [130, 'mset.err.range'],
    ]);
  });

  it('respaldo idéntico: sin cambios', () => {
    const diff = diffBackup(now, now.map((s) => ({ id: s.id, raw: s.raw })));
    expect(diff.changes).toEqual([]);
    expect(diff.same).toBe(4);
  });

  it('valida con las reglas del firmware conectado', () => {
    const cur = [raw(32, '1')];
    expect(diffBackup(cur, [{ id: 32, raw: '2' }], 'grblhal').changes).toHaveLength(1);
    expect(diffBackup(cur, [{ id: 32, raw: '2' }], 'grbl').invalid).toHaveLength(1);
  });
});

describe('límites por software', () => {
  const settings = (pairs: Record<number, string>) => parseRawSettings(Object.entries(pairs).map(([id, v]) => `$${id}=${v}`)).settings;

  it('se recomiendan con homing ($22=1) y sin límites por software ($20=0)', () => {
    expect(needsSoftLimits(settings({ 20: '0', 22: '1' }))).toBe(true);
  });

  it('no se recomiendan sin homing, con $20=1 o si no se leyeron', () => {
    expect(needsSoftLimits(settings({ 20: '0', 22: '0' }))).toBe(false);
    expect(needsSoftLimits(settings({ 20: '1', 22: '1' }))).toBe(false);
    expect(needsSoftLimits(settings({ 22: '1' }))).toBe(false);
    expect(needsSoftLimits([])).toBe(false);
  });
});
