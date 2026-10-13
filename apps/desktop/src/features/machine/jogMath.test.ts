import { describe, expect, it } from 'vitest';
import { FALLBACK_DEVICE } from '@/lib/device';
import { clampJogSpeed, continuousDelta, dirFromKey, JOG_DIRS, MIN_CONTINUOUS_MM, modifiedStep, stepDelta } from './jogMath';

const dev = { bed: [400, 300] as [number, number], origin: 'bottom_left' as const };

describe('jog por pasos', () => {
  it('Y hacia arriba en pantalla es +Y con origen abajo a la izquierda', () => {
    expect(stepDelta(dev, JOG_DIRS.up, 10)).toEqual([0, 10]);
    expect(stepDelta(dev, JOG_DIRS.downLeft, 1)).toEqual([-1, -1]);
  });

  it('con origen arriba a la derecha se invierten los dos ejes del G-code', () => {
    const d = { origin: 'top_right' as const };
    expect(stepDelta(d, JOG_DIRS.right, 5)).toEqual([-5, 0]);
    expect(stepDelta(d, JOG_DIRS.up, 5)).toEqual([0, -5]);
  });

  it('Mayús x10, Alt /10, ambos a la vez no cambian el paso', () => {
    expect(modifiedStep(1, { shift: true })).toBe(10);
    expect(modifiedStep(1, { alt: true })).toBe(0.1);
    expect(modifiedStep(0.1, { alt: true })).toBe(0.01);
    expect(modifiedStep(10, { shift: true, alt: true })).toBe(10);
    expect(modifiedStep(10, {})).toBe(10);
  });

  it('solo las flechas mueven con el teclado', () => {
    expect(dirFromKey('ArrowLeft')).toEqual({ x: -1, y: 0 });
    expect(dirFromKey('a')).toBeNull();
  });
});

describe('jog continuo', () => {
  it('llega hasta el borde de la cama en esa dirección', () => {
    expect(continuousDelta(dev, JOG_DIRS.right, [100, 50])).toEqual([300, 0]);
    expect(continuousDelta(dev, JOG_DIRS.left, [100, 50])).toEqual([-100, 0]);
    expect(continuousDelta(dev, JOG_DIRS.up, [100, 50])).toEqual([0, 250]);
  });

  it('en diagonal recorre lo mismo en los dos ejes hasta el primer borde', () => {
    expect(continuousDelta(dev, JOG_DIRS.upRight, [350, 50])).toEqual([50, 50]);
    expect(continuousDelta(dev, JOG_DIRS.downLeft, [20, 200])).toEqual([-20, -20]);
  });

  it('no se envía si no se conoce la posición o ya está en el borde', () => {
    expect(continuousDelta(dev, JOG_DIRS.right, null)).toBeNull();
    expect(continuousDelta(dev, JOG_DIRS.right, [400 - MIN_CONTINUOUS_MM / 2, 10])).toBeNull();
    expect(continuousDelta(dev, JOG_DIRS.left, [0, 10])).toBeNull();
  });

  it('respeta el origen del perfil: la posición se mide en el plano del documento', () => {
    const d = { bed: [400, 300] as [number, number], origin: 'top_left' as const };
    // Cabezal a 20 mm de arriba (Y de documento 280): subir deja 20 mm; en G-code es -Y.
    expect(continuousDelta(d, JOG_DIRS.up, [50, 20])).toEqual([0, -20]);
  });
});

describe('velocidad', () => {
  it('se limita a la menor velocidad máxima y a valores válidos', () => {
    const d = { max_rate: [20000, 9000] as [number, number] };
    expect(clampJogSpeed(100000, d)).toBe(9000);
    expect(clampJogSpeed(1500.4, d)).toBe(1500);
    expect(clampJogSpeed(-5, d)).toBe(3000);
    expect(clampJogSpeed(NaN, FALLBACK_DEVICE)).toBe(3000);
  });
});
