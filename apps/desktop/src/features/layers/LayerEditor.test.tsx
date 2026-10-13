import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { DEFAULT_TAB_SETTINGS, makeLayer } from '@/lib/layers';
import { LayerEditor } from './LayerEditor';

/** Humo: el editor se dibuja en cada modo y muestra solo los campos que corresponden. */
const html = (patch = {}) => renderToString(<LayerEditor layer={makeLayer('#ff0000', 'Corte', patch)} />);

describe('LayerEditor por modo', () => {
  it('line: kerf y sección de puentes cerrada por defecto', () => {
    const h = html();
    expect(h).toContain('Kerf');
    expect(h).toContain('Dejar puentes sin cortar');
    expect(h).not.toContain('Ancho del puente');
    expect(h).not.toContain('Tamaño de celda');
    expect(h).not.toContain('Relleno</label>');
  });

  it('line con puentes: ancho, cantidad, separación y formas interiores', () => {
    const h = html({ tabs: { ...DEFAULT_TAB_SETTINGS, enabled: true } });
    for (const label of ['Ancho del puente', 'Cantidad', 'Separación', 'Omitir formas interiores']) expect(h).toContain(label);
    expect(h).toContain('4 puentes por forma cerrada');
  });

  it('line con separación fija: la cantidad pasa a ser el mínimo por forma', () => {
    const h = html({ tabs: { ...DEFAULT_TAB_SETTINGS, enabled: true, spacing: 20 } });
    expect(h).toContain('Mínimo por forma');
    expect(h).toContain('Uno cada 20 mm');
  });

  it('fill con barrido: ángulo, trama cruzada, bidireccional y overscan', () => {
    const h = html({ mode: 'fill' });
    for (const label of ['Ángulo', 'Trama cruzada', 'Bidireccional', 'Overscan automático']) expect(h).toContain(label);
    expect(h).not.toContain('Con contornos no se usan');
    expect(h).not.toContain('Dejar puentes sin cortar');
  });

  it('fill con contornos: oculta los ajustes de barrido y dice por qué', () => {
    const h = html({ mode: 'fill', fill_style: 'offset' });
    expect(h).toContain('Con contornos no se usan');
    for (const label of ['Trama cruzada', 'Bidireccional', 'Overscan automático']) expect(h).not.toContain(label);
    expect(h).toContain('Intervalo');
  });

  it('image: muestra el ángulo de barrido y el tramado ofrece los tres modos nuevos', () => {
    const h = html({ mode: 'image' });
    expect(h).toContain('Ángulo de barrido');
    for (const label of ['Semitono', 'Líneas de periódico', 'Boceto']) expect(h).toContain(label);
    expect(h).not.toContain('Tamaño de celda');
    expect(h).not.toContain('Trama cruzada');
  });

  it('image con semitono o líneas de periódico: tamaño de celda y ángulo de rejilla', () => {
    for (const mode of ['halftone', 'newsprint'] as const) {
      const h = html({ mode: 'image', image: { ...makeLayer('#f00', 'x').image, mode } });
      expect(h).toContain('Tamaño de celda');
      expect(h).toContain('Ángulo de la rejilla');
    }
    const sketch = html({ mode: 'image', image: { ...makeLayer('#f00', 'x').image, mode: 'sketch' } });
    expect(sketch).not.toContain('Tamaño de celda');
    expect(sketch).toContain('Solo graba los bordes');
  });

  it('sin eje Z en la máquina no aparecen los campos de Z', () => {
    expect(html()).not.toContain('Desplazamiento Z');
  });

  it('con Z en la capa y sin eje Z en la máquina avisa que se ignora', () => {
    const h = html({ z_offset: 1.5 });
    expect(h).toContain('no tiene eje Z motorizado');
    expect(h).toContain('Poner Z en 0');
    expect(h).not.toContain('Bajada por pasada');
  });

  it('una capa de un documento viejo (sin campos nuevos) se dibuja igual', () => {
    const viejo: Record<string, unknown> = { ...makeLayer('#00f', 'Viejo', { mode: 'fill' }) };
    for (const k of ['fill_style', 'tabs', 'z_offset', 'z_step']) delete viejo[k];
    expect(renderToString(<LayerEditor layer={viejo as never} />)).toContain('Trama cruzada');
  });
});
