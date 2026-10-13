import { describe, expect, it } from 'vitest';
import { classify, gcodeWords, needsConfirmation, parseCommand, prepareLine, type Risk } from './commandSafety';

/** Misma tabla que `cmd_asks_for_everything_that_moves_fires_or_changes_settings` del CLI. */
const CLI_TABLE: [string, Risk][] = [
  ['g0 x10 y10', 'moves'],
  ['G1 X5 F1000', 'moves'],
  ['G2 X5 Y5 I1 J0', 'moves'],
  ['G3 X5 Y5 R2', 'moves'],
  ['$J=G91 X10 F500', 'moves'],
  ['M3 S500', 'laser'],
  ['M4 S1', 'laser'],
  ['M3', 'laser'],
  ['$30=1000', 'setting'],
  ['$N0=G54', 'setting'],
  ['$RST=$', 'other'],
  ['$X', 'unlock'],
  ['$H', 'home'],
  ['$I', 'safe'],
  ['$$', 'safe'],
  ['$#', 'safe'],
  ['$G', 'safe'],
  ['$N', 'safe'],
  ['M5', 'safe'],
];

function line(raw: string) {
  const r = parseCommand(raw, 127);
  if (!r.ok || r.cmd.kind !== 'line') throw new Error(`no es una línea: ${raw}`);
  return r.cmd;
}

describe('clasificación de seguridad (paridad con tlaku cmd)', () => {
  it.each(CLI_TABLE)('%s -> %s', (raw, risk) => {
    expect(line(raw).risk).toBe(risk);
    // Solo lo de solo lectura se envía sin preguntar.
    expect(needsConfirmation(line(raw).risk)).toBe(risk !== 'safe');
  });

  it('M3 S0 no enciende el láser pero tampoco es de solo lectura', () => {
    expect(line('M3 S0').risk).toBe('other');
  });

  it('G-code que no mueve ni enciende pide confirmación igualmente', () => {
    for (const raw of ['G90', 'G21', 'G92 X0 Y0', 'G10 L20 P1 X0 Y0', 'M8', '$C', '$SLP', '$32']) {
      expect(needsConfirmation(line(raw).risk), raw).toBe(true);
    }
  });

  it('G38.x (sonda) y G28/G30 cuentan como movimiento', () => {
    expect(classify('G38.2Z-5F50')).toBe('moves');
    expect(classify('G28')).toBe('moves');
    expect(classify('G30')).toBe('moves');
    expect(classify('G53G0X0')).toBe('moves');
  });

  it('M5 con otras palabras ya no es el M5 de solo lectura', () => {
    expect(classify('M5S0')).toBe('other');
  });
});

describe('normalización: se envía exactamente lo que se muestra', () => {
  it('pasa a mayúsculas, quita espacios y comentarios', () => {
    const cmd = line('g1 x5 s0 f100');
    expect(cmd.text).toBe('G1X5S0F100');
    expect(cmd.shown).toBe('G1X5S0F100');
    expect(line('G0 X1 ; mover (rápido) Y2').text).toBe('G0X1');
    expect(line('G0 (a) X1 (b)').text).toBe('G0X1');
  });

  it('los caracteres de tiempo real no se aceptan como línea', () => {
    for (const raw of ['!', '~', 'G0 X1 !', '$$?']) {
      const r = parseCommand(raw, 127);
      expect(r.ok, raw).toBe(false);
      if (!r.ok) expect(r.error.code).toBe('realtimeChar');
    }
    expect(prepareLine('a\u0007b')).toEqual({ ok: false, ch: '\u0007' });
    expect(prepareLine('G0 X1 é')).toEqual({ ok: false, ch: 'é' });
  });

  it('un comentario puede contener caracteres especiales', () => {
    expect(prepareLine('G0 X1 ; ¿qué?')).toEqual({ ok: true, text: 'G0X1' });
    expect(prepareLine('G0 (¿qué? !) X1')).toEqual({ ok: true, text: 'G0X1' });
  });

  it('rechaza lo vacío y lo que no cabe en el búfer', () => {
    expect(parseCommand('   ', 127)).toEqual({ ok: false, error: { code: 'empty' } });
    expect(parseCommand('; solo comentario', 127)).toEqual({ ok: false, error: { code: 'empty' } });
    const long = 'G1' + 'X1'.repeat(70);
    const r = parseCommand(long, 127);
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error).toMatchObject({ code: 'tooLong', max: 127 });
    // justo en el límite: 126 caracteres + salto de línea = 127
    expect(parseCommand('G'.padEnd(126, '1'), 127).ok).toBe(true);
    expect(parseCommand('G'.padEnd(127, '1'), 127).ok).toBe(false);
  });
});

describe('tiempo real por nombre', () => {
  it('status, hold, resume y reset (y ?) son comandos seguros', () => {
    for (const [raw, name] of [['status', 'status'], ['?', 'status'], ['HOLD', 'hold'], [' resume ', 'resume'], ['Reset', 'reset']] as const) {
      const r = parseCommand(raw, 127);
      expect(r.ok).toBe(true);
      if (r.ok) {
        expect(r.cmd).toMatchObject({ kind: 'realtime', name, risk: 'safe' });
        expect(needsConfirmation(r.cmd.risk)).toBe(false);
      }
    }
  });

  it('muestra el byte que se enviará', () => {
    const r = parseCommand('reset', 127);
    expect(r.ok && r.cmd.shown).toBe('reset (byte 0x18)');
  });
});

describe('palabras G-code', () => {
  it('una letra sin número no es una palabra', () => {
    expect(gcodeWords('G0X')).toEqual([['G', 0]]);
    expect(gcodeWords('G1X-5.5F+10')).toEqual([['G', 1], ['X', -5.5], ['F', 10]]);
    expect(gcodeWords('M3S')).toEqual([['M', 3]]);
  });
});
