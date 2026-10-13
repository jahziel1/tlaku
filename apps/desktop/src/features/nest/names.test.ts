import { describe, expect, it } from 'vitest';
import { selectionUnits } from '@/features/arrange/units';
import { pieceNames } from './names';
import { docOf, rectObject } from './testUtils';

describe('pieceNames', () => {
  it('nombra las piezas por lo que son y no por su id interno', () => {
    const doc = docOf([rectObject('obj-placa', 40, 20), rectObject('obj-base', 10, 10, [60, 0])]);
    const names = pieceNames(selectionUnits(doc, ['obj-placa', 'obj-base']));
    expect(names.get('obj-placa')).toMatch(/40/);
    expect(names.get('obj-placa')).not.toContain('obj-placa');
    expect(names.get('obj-base')).not.toContain('obj-base');
  });

  it('distingue las piezas que se llaman igual', () => {
    const doc = docOf([rectObject('a', 10, 10), rectObject('b', 10, 10, [20, 0])]);
    const names = pieceNames(selectionUnits(doc, ['a', 'b']));
    expect(names.get('a')).not.toBe(names.get('b'));
    expect(names.get('b')).toMatch(/\(2\)$/);
  });
});
