import { describe, expect, it } from 'vitest';
import { macroProblem, MAX_MACRO_LINES, planMacro, replyOk } from './macros';
import { MAX_POSITIONS, upsertPosition, workTarget } from './positions';

const plan = (gcode: string, confirm = false, rx = 127) => planMacro({ gcode, confirm }, rx);

describe('planMacro', () => {
  it('normaliza, omite líneas vacías y comentarios y toma el riesgo más alto', () => {
    const p = plan('; hola\n\n$i\n g1 x5 y5 f100 ; mover\n(nota)\nM3 S100');
    expect(p.ok).toBe(true);
    if (!p.ok) return;
    expect(p.lines.map((l) => l.text)).toEqual(['$I', 'G1X5Y5F100', 'M3S100']);
    expect(p.lines[1]!.raw).toBe('g1 x5 y5 f100');
    expect(p.risk).toBe('laser');
    expect(p.confirm).toBe(true);
  });

  it('solo lectura sin marcar confirmar: sin confirmación; marcada: con ella', () => {
    const quiet = plan('$I\n$$\nM5');
    expect(quiet).toMatchObject({ ok: true, risk: 'safe', confirm: false });
    expect(plan('$I', true)).toMatchObject({ ok: true, confirm: true });
  });

  it('$RST se marca destructivo y siempre pide confirmación', () => {
    const p = plan('$RST=$');
    expect(p).toMatchObject({ ok: true, confirm: true });
    if (p.ok) expect(p.lines[0]!.destructive).toBe(true);
  });

  it('rechaza vacío, tiempo real, caracteres de tiempo real, líneas largas y demasiadas líneas', () => {
    expect(plan('  \n; solo comentario')).toEqual({ ok: false, error: { code: 'empty' } });
    expect(plan('$I\nreset')).toMatchObject({ ok: false, error: { code: 'realtime', line: 2 } });
    expect(plan('G0 X1\nG1 X?')).toMatchObject({ ok: false, error: { code: 'line', line: 2 } });
    expect(plan('G1 X1 Y1 F1000', false, 5)).toMatchObject({ ok: false, error: { code: 'line', line: 1 } });
    expect(plan(Array.from({ length: MAX_MACRO_LINES + 1 }, () => '$G').join('\n'))).toMatchObject({ ok: false, error: { code: 'tooMany' } });
  });

  it('macroProblem: nombre vacío o G-code inválido', () => {
    expect(macroProblem({ name: ' ', gcode: '$I', confirm: false }, 127)).toBe('name');
    expect(macroProblem({ name: 'a', gcode: '', confirm: false }, 127)).toBe('gcode');
    expect(macroProblem({ name: 'a', gcode: '$I', confirm: false }, 127)).toBeNull();
  });

  it('replyOk solo acepta una respuesta que termina en ok', () => {
    expect(replyOk(['[MSG:x]', 'ok'])).toBe(true);
    expect(replyOk(['error:9: bloqueado'])).toBe(false);
    expect(replyOk([])).toBe(false);
  });
});

describe('posiciones guardadas', () => {
  it('agrega, reemplaza por nombre, redondea y respeta el máximo', () => {
    const a = upsertPosition([], ' Foco ', 1.23456, 2)!;
    expect(a).toEqual([{ name: 'Foco', x: 1.235, y: 2 }]);
    expect(upsertPosition(a, 'Foco', 5, 6)).toEqual([{ name: 'Foco', x: 5, y: 6 }]);
    expect(upsertPosition(a, '', 1, 1)).toBeNull();
    expect(upsertPosition(a, 'x', Number.NaN, 1)).toBeNull();
    const full = Array.from({ length: MAX_POSITIONS }, (_, i) => ({ name: `p${i}`, x: i, y: i }));
    expect(upsertPosition(full, 'nueva', 1, 1)).toBeNull();
    expect(upsertPosition(full, 'p3', 9, 9)).toHaveLength(MAX_POSITIONS);
  });

  it('pasa de coordenadas de máquina a las de trabajo con el WCO del estado', () => {
    expect(workTarget({ x: 100, y: 50 }, { machine_pos: [110, 60], work_pos: [10, 10] })).toEqual([0, 0]);
    expect(workTarget({ x: 20, y: 20 }, { machine_pos: [5, 5], work_pos: [5, 5] })).toEqual([20, 20]);
  });
});
