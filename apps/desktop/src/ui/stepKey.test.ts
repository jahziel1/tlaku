import { describe, expect, it } from 'vitest';
import { stepFromKey } from './stepKey';

const key = (k: string, mods: { shiftKey?: boolean; altKey?: boolean } = {}) => ({ key: k, shiftKey: false, altKey: false, ...mods });

describe('stepFromKey', () => {
  it('suma y resta el paso, y respeta los límites', () => {
    expect(stepFromKey(key('ArrowUp'), 5, 2)).toBe(7);
    expect(stepFromKey(key('ArrowDown'), 5, 2)).toBe(3);
    expect(stepFromKey(key('ArrowUp'), 9, 5, 0, 10)).toBe(10);
    expect(stepFromKey(key('ArrowDown'), 1, 5, 0, 10)).toBe(0);
  });

  it('Mayús multiplica el paso por 10 y Alt por 0,1', () => {
    expect(stepFromKey(key('ArrowUp', { shiftKey: true }), 0, 1)).toBe(10);
    expect(stepFromKey(key('ArrowUp', { altKey: true }), 0, 1)).toBe(0.1);
  });

  it('devuelve null con otras teclas y redondea 0,1 + 0,2', () => {
    expect(stepFromKey(key('Enter'), 1, 1)).toBeNull();
    expect(stepFromKey(key('ArrowUp'), 0.1, 0.2)).toBe(0.3);
  });
});
