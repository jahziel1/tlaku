import { describe, expect, it } from 'vitest';
import { ConsoleHistory } from './consoleHistory';
import { commonPrefix, completions, hintRisk, COMMAND_CATALOG } from './completions';

describe('ConsoleHistory', () => {
  it('flecha arriba recorre del más reciente al más antiguo y abajo vuelve al borrador', () => {
    const h = new ConsoleHistory();
    h.push('$$');
    h.push('$I');
    h.push('G0 X1');
    expect(h.prev('G1')).toBe('G0 X1');
    expect(h.prev('')).toBe('$I');
    expect(h.prev('')).toBe('$$');
    expect(h.prev('')).toBe('$$'); // tope
    expect(h.next('')).toBe('$I');
    expect(h.next('')).toBe('G0 X1');
    expect(h.next('')).toBe('G1'); // el borrador original
    expect(h.next('x')).toBe('x'); // ya no hay más abajo
  });

  it('un repetido sube en vez de duplicarse y se ignoran los vacíos', () => {
    const h = new ConsoleHistory();
    h.push('A');
    h.push('B');
    h.push('  A  ');
    h.push('   ');
    expect(h.items()).toEqual(['A', 'B']);
  });

  it('respeta el límite', () => {
    const h = new ConsoleHistory(3);
    for (const c of ['1', '2', '3', '4']) h.push(c);
    expect(h.items()).toEqual(['4', '3', '2']);
  });

  it('enviar o editar reinicia el recorrido', () => {
    const h = new ConsoleHistory();
    h.push('A');
    h.push('B');
    h.prev('x');
    h.reset();
    expect(h.prev('y')).toBe('B');
    h.push('C');
    expect(h.prev('')).toBe('C');
  });

  it('sin historial no cambia lo escrito', () => {
    const h = new ConsoleHistory();
    expect(h.prev('abc')).toBe('abc');
    expect(h.next('abc')).toBe('abc');
  });

  it('parte de un historial guardado', () => {
    expect(new ConsoleHistory(2, ['a', 'b', 'c']).items()).toEqual(['a', 'b']);
  });
});

describe('autocompletado', () => {
  it('filtra por prefijo sin distinguir mayúsculas ni espacios', () => {
    expect(completions('$')).not.toHaveLength(0);
    expect(completions('g9').map((c) => c.text)).toEqual(['G90', 'G91', 'G92 X0 Y0']);
    expect(completions('g 9 0').map((c) => c.text)).toEqual([]);
    expect(completions('')).toEqual([]);
  });

  it('no sugiere lo que ya está escrito completo', () => {
    expect(completions('$$')).toEqual([]);
    expect(completions('$x')).toEqual([]);
  });

  it('el prefijo común sirve para completar con Tab', () => {
    expect(commonPrefix(['G90', 'G91', 'G92 X0 Y0'])).toBe('G9');
    expect(commonPrefix(['status'])).toBe('status');
    expect(commonPrefix([])).toBe('');
  });

  it('cada sugerencia declara el mismo riesgo que se aplicará al enviarla', () => {
    const risk = Object.fromEntries(COMMAND_CATALOG.map((c) => [c.text, hintRisk(c)]));
    expect(risk['$$']).toBe('safe');
    expect(risk['status']).toBe('safe');
    expect(risk['M5']).toBe('safe');
    expect(risk['$X']).toBe('unlock');
    expect(risk['$H']).toBe('home');
    expect(risk['M3 S100']).toBe('laser');
    expect(risk['G0 X0 Y0']).toBe('moves');
    expect(risk['$32=1']).toBe('setting');
  });
});
