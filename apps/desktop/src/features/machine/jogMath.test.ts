import { describe, expect, it } from 'vitest';
import { FALLBACK_DEVICE } from '@/lib/device';
import { clampJogSpeed, continuousDelta, dirFromKey, JOG_DIRS, jogOutsideBed, MIN_CONTINUOUS_MM, modifiedStep, outsideBedMessage, pointOutsideBed, stepDelta } from './jogMath';

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

describe('destino fuera de la cama', () => {
  const falcon = { bed: [400, 415] as [number, number], origin: 'bottom_left' as const };

  it('el caso real: -5 mm en X desde 0,025 sale 4,97 mm de la cama', () => {
    const out = jogOutsideBed(falcon, [0.025, 0.025], [-5, 0]);
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ axis: 'X', max: 400 });
    expect(out[0]!.value).toBeCloseTo(-4.975, 3);
    expect(outsideBedMessage(out)).toBe('El movimiento saldría de la cama: X llegaría a -4,98 mm (límite 0 a 400).');
  });

  it('dentro de la cama, con el margen de 0,5 mm o sin posición conocida no avisa', () => {
    expect(jogOutsideBed(falcon, [0.025, 0.025], [5, 5])).toEqual([]);
    expect(jogOutsideBed(falcon, [0.025, 0.025], [-0.4, 0])).toEqual([]);
    expect(jogOutsideBed(falcon, null, [-500, 0])).toEqual([]);
  });

  it('dice los dos ejes y respeta el origen del perfil', () => {
    expect(jogOutsideBed(falcon, [399, 414], [5, 5]).map((o) => o.axis)).toEqual(['X', 'Y']);
    // Origen arriba a la derecha: la posición de trabajo crece hacia dentro desde esa esquina.
    const tr = { bed: [400, 415] as [number, number], origin: 'top_right' as const };
    expect(jogOutsideBed(tr, [0, 0], [-5, 0]).map((o) => o.axis)).toEqual(['X']);
    expect(jogOutsideBed(tr, [0, 0], [5, 5])).toEqual([]);
  });

  it('un punto del documento se revisa con la misma regla', () => {
    expect(pointOutsideBed(falcon, [200, 416]).map((o) => o.axis)).toEqual(['Y']);
    expect(pointOutsideBed(falcon, [0, 415])).toEqual([]);
  });
});
