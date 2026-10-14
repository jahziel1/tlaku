import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { setLanguage } from '@/i18n';
import { FALLBACK_DEVICE } from '@/lib/device';
import { Calib9ResultView } from './GalvoCalibration9';
import { fitNinePoints, IDENTITY_FAMILY, simulateMeasures9, type Fit9Result } from './galvoCalib9';

const current = structuredClone(FALLBACK_DEVICE.galvo.correction);

describe('resultado del asistente de nueve puntos', () => {
  const lens = { ...IDENTITY_FAMILY, scale: [1.02, 1] as [number, number] };
  const fit = fitNinePoints(110, current, simulateMeasures9(110, current, lens), true) as Fit9Result;

  it('muestra los siete parámetros, antes y después, y el error por cruz', () => {
    const h = renderToString(<Calib9ResultView fit={fit} current={current} />);
    for (const label of ['Escala X', 'Escala Y', 'Giro (°)', 'Trapecio X', 'Trapecio Y']) expect(h, label).toContain(label);
    // 1 / 1,02 = 0,98039 (cálculo a mano).
    expect(h).toContain('0,98039');
    expect(h).toContain('Error máximo medido: 0,770 mm');
    expect(h).toContain('Error por cruz');
    expect(h).not.toContain('role="alert"');
  });

  it('en inglés', () => {
    setLanguage('en');
    const h = renderToString(<Calib9ResultView fit={fit} current={current} />);
    expect(h).toContain('Largest measured error');
    expect(h).toContain('Trapezoid X');
    setLanguage('es');
  });
});
