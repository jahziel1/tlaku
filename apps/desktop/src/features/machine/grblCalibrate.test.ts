import { describe, expect, it } from 'vitest';
import { calibrateGrblSteps } from './grblCalibrate';

describe('calibración de pasos por mm de GRBL', () => {
  it('si el eje recorre menos de lo pedido, hacen falta más pasos por mm', () => {
    const r = calibrateGrblSteps(80, 100, 99);
    expect(r).toMatchObject({ ok: true, current: 80, proposed: 80.808 });
    // Y si recorre de más, menos.
    expect(calibrateGrblSteps(80, 100, 101)).toMatchObject({ ok: true, proposed: 79.208 });
  });

  it('rechaza medidas inválidas, correcciones de más de 25 % y cambios nulos', () => {
    expect(calibrateGrblSteps(80, 0, 99)).toEqual({ ok: false, reason: 'commanded' });
    expect(calibrateGrblSteps(80, 100, Number.NaN)).toEqual({ ok: false, reason: 'measured' });
    expect(calibrateGrblSteps(80, 100, 20000)).toEqual({ ok: false, reason: 'measured' });
    expect(calibrateGrblSteps(0, 100, 99)).toEqual({ ok: false, reason: 'current' });
    expect(calibrateGrblSteps(80, 100, 60)).toMatchObject({ ok: false, reason: 'ratio' });
    expect(calibrateGrblSteps(80, 100, 130)).toMatchObject({ ok: false, reason: 'ratio' });
    expect(calibrateGrblSteps(80, 100, 100)).toMatchObject({ ok: false, reason: 'same' });
  });
});
