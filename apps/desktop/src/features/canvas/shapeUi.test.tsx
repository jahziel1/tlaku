import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import { useStore } from '@/store';
import { MeasureReadout, MeasureView } from './MeasureReadout';
import { measureShape } from './measure';
import { patchInteraction } from './interaction';
import { ShapeProps, ShapePropsView } from './ShapeProps';
import type { ShapeParams } from './shapeParams';
import { rectObj } from './testUtil';

beforeEach(() => {
  useStore.getState().replaceDocument({ version: 2, objects: [], layers: [], device_id: null }, null);
  useStore.getState().setShapePropsOpen(true);
  useStore.getState().setTool('select');
  patchInteraction({ measure: null });
});

const noop = () => {};

describe('propiedades de forma', () => {
  it('sin selección o con formas que no son paramétricas no se muestra', () => {
    expect(renderToString(<ShapeProps />)).toBe('');
    useStore.getState().addObjects([rectObj('r', 0, 0, 5, 5)]);
    useStore.getState().select(['r']);
    expect(renderToString(<ShapeProps />)).toBe('');
  });

  it('un polígono muestra sus campos', () => {
    const html = renderToString(<ShapePropsView params={{ type: 'polygon', sides: 6, radius: 10, rounding: 1 }} mixed={false} onApply={noop} onClose={noop} onLock={noop} />);
    for (const label of ['Propiedades de forma', 'Polígono', 'Lados', 'Radio', 'Redondeo', 'Bloquear']) expect(html, label).toContain(label);
    expect(html).not.toContain('Algunos objetos');
  });

  it('rectángulo, elipse y estrella tienen cada uno sus campos; avisa si hay objetos no editables', () => {
    const view = (params: ShapeParams, mixed = false) => renderToString(<ShapePropsView params={params} mixed={mixed} onApply={noop} onClose={noop} onLock={noop} />);
    const rect = view({ type: 'rect', w: 10, h: 5, radius: 1 });
    for (const label of ['Rectángulo', 'Ancho', 'Alto', 'Radio de esquinas']) expect(rect, label).toContain(label);
    const ellipse = view({ type: 'ellipse', w: 10, h: 5 });
    for (const label of ['Elipse', 'Ancho', 'Alto']) expect(ellipse, label).toContain(label);
    expect(ellipse).not.toContain('Radio de esquinas');
    const star = view({ type: 'star', points: 5, outer: 8, ratio: 0.4, rounding: 0 }, true);
    for (const label of ['Estrella', 'Puntas', 'Radio externo', 'Radio interno', 'Algunos objetos']) expect(star, label).toContain(label);
    expect(star).toContain('value="40"'); // 0,4 -> 40 %
  });
});

describe('panel de medidas', () => {
  it('solo con la herramienta Medir se monta el panel', () => {
    expect(renderToString(<MeasureReadout />)).toBe('');
  });

  it('sin medición muestra la ayuda', () => {
    expect(renderToString(<MeasureView measure={null} info={null} />)).toContain('Arrastra entre dos puntos');
  });

  it('muestra distancia, ángulo y deltas', () => {
    const html = renderToString(<MeasureView measure={{ a: [0, 0], b: [3, 4], objectId: null }} info={null} />);
    for (const label of ['Distancia', 'Ángulo', 'Δx', 'Δy']) expect(html, label).toContain(label);
    // El separador decimal depende del idioma del sistema.
    expect(html).toMatch(/5[.,]000 mm/);
    expect(html).toMatch(/53[.,]13°/);
  });

  it('con una forma medida muestra nodos, perímetro y área', () => {
    const info = measureShape(rectObj('r', 0, 0, 10, 4))!;
    const html = renderToString(<MeasureView measure={{ a: [0, 0], b: [0, 0], objectId: 'r' }} info={info} />);
    for (const label of ['Nodos', 'Perímetro', 'Área', 'Cerrado']) expect(html, label).toContain(label);
    expect(html).toMatch(/40[.,]000 mm²/);
    expect(html).toMatch(/28[.,]000 mm/);
  });
});
