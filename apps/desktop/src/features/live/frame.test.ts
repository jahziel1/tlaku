import { describe, expect, it } from 'vitest';
import { FALLBACK_DEVICE } from '@/lib/device';
import { docToWork } from '@/lib/device';
import { reportToDoc, rotaryYScale } from './frame';

const dev = (over: Record<string, unknown> = {}) => ({ ...FALLBACK_DEVICE, bed: [400, 300] as [number, number], ...over }) as typeof FALLBACK_DEVICE;

describe('del reporte de la máquina al documento', () => {
  it('origen abajo a la izquierda: es la identidad', () => {
    expect(reportToDoc(dev({ origin: 'bottom_left' }), [12.5, 40])).toEqual([12.5, 40]);
  });

  it('respeta el origen del perfil y es la inversa de docToWork en las cuatro esquinas', () => {
    for (const origin of ['bottom_left', 'bottom_right', 'top_left', 'top_right'] as const) {
      const d = dev({ origin });
      const work = docToWork(d, [100, 50]);
      const back = reportToDoc(d, work)!;
      expect(back[0]).toBeCloseTo(100, 9);
      expect(back[1]).toBeCloseTo(50, 9);
    }
    // Origen arriba a la derecha: el punto de trabajo (10, 20) está a 390 mm del borde izquierdo y a 280 del inferior.
    expect(reportToDoc(dev({ origin: 'top_right' }), [10, 20])).toEqual([390, 280]);
  });

  it('con intercambio X/Y cruza los ejes', () => {
    const d = dev({ swap_xy: true, bed: [400, 300] });
    const work = docToWork(d, [100, 50]);
    expect(work).toEqual([50, 100]);
    expect(reportToDoc(d, work)).toEqual([100, 50]);
  });

  it('con rotativo divide la Y por el factor (mm de la controladora por mm de superficie)', () => {
    const rotary = { enabled: true, kind: 'chuck' as const, mm_per_rotation: 360, roller_diameter: 20, object_diameter: 100, mirror: false };
    const k = 360 / (Math.PI * 100);
    expect(rotaryYScale(dev({ rotary }))).toBeCloseTo(k, 12);
    const d = dev({ rotary, bed: [400, 300], swap_xy: true }); // con rotativo el intercambio no cuenta
    const p = reportToDoc(d, [30, 2 * k * 25])!;
    expect(p[0]).toBeCloseTo(30, 9);
    expect(p[1]).toBeCloseTo(50, 9);
    // Con rodillo manda el diámetro del rodillo.
    expect(rotaryYScale(dev({ rotary: { ...rotary, kind: 'roller' } }))).toBeCloseTo(360 / (Math.PI * 20), 12);
  });

  it('rotativo con datos inválidos o reflejado: no se puede deshacer', () => {
    const rotary = { enabled: true, kind: 'chuck' as const, mm_per_rotation: 0, roller_diameter: 20, object_diameter: 100, mirror: false };
    expect(reportToDoc(dev({ rotary }), [1, 1])).toBeNull();
    expect(reportToDoc(dev({ rotary: { ...rotary, mm_per_rotation: 360, mirror: true } }), [1, 1])).toBeNull();
  });

  it('el galvo no escala la Y aunque tenga rotativo', () => {
    const rotary = { enabled: true, kind: 'chuck' as const, mm_per_rotation: 360, roller_diameter: 20, object_diameter: 100, mirror: false };
    expect(rotaryYScale({ kind: 'galvo', rotary })).toBe(1);
  });
});
