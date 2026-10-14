import { describe, expect, it } from 'vitest';
import { TIP_H, tipYClearOf, toolBarBox, type Box } from './tipPlacement';

/** Barra centrada arriba, como `.tool-bar`: de y=16 a y=56 y de x=300 a x=700 en un lienzo de 1000 px. */
const BAR: Box = { left: 300, top: 16, right: 700, bottom: 56 };

describe('cartel junto al puntero y barra de la herramienta', () => {
  it('sin barra, o lejos de ella, no se mueve', () => {
    expect(tipYClearOf(100, 30, 1000, null)).toBe(30);
    expect(tipYClearOf(100, 30, 1000, BAR)).toBe(30); // a la izquierda de la barra
    expect(tipYClearOf(400, 200, 1000, BAR)).toBe(200); // bajo la barra
  });

  it('si la barra lo taparía, baja justo debajo de ella', () => {
    // Esquina cerca del borde de arriba: el cartel caería en y = 24, dentro de la barra.
    expect(tipYClearOf(416, 24, 1000, BAR)).toBe(BAR.bottom + 6);
    // Cartel que empieza encima de la barra pero se mete en ella por su alto.
    expect(tipYClearOf(416, BAR.top - TIP_H + 4, 1000, BAR)).toBe(BAR.bottom + 6);
  });

  it('con el cartel volteado a la izquierda (no cabe a la derecha) mide el lado correcto', () => {
    // x = 950: el cartel se voltea y ocupa de 718 a 918, fuera de la barra (que termina en 700).
    expect(tipYClearOf(950, 24, 1000, BAR)).toBe(24);
    // x = 850 también se voltea (850 + 200 > 1000), pero ocupa de 618 a 818: choca con la barra.
    expect(tipYClearOf(850, 24, 1000, BAR)).toBe(BAR.bottom + 6);
  });

  it('toolBarBox tolera un lienzo sin DOM', () => {
    expect(toolBarBox({} as HTMLElement)).toBeNull();
  });
});
