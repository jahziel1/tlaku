import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import { SHAPE_DIALOGS, ShapeOpsHost } from './ShapeOpsDialog';
import { useShapeOps, type ShapeDialogOp } from './shapeOpsUi';

/** Humo: los diálogos de forma se dibujan sin errores (el render en servidor ve el estado sin selección). */
describe('diálogos de forma', () => {
  beforeEach(() => useShapeOps.setState({ dialog: null }));

  it('sin diálogo abierto no pinta nada', () => {
    expect(renderToString(<ShapeOpsHost />)).toBe('');
  });

  const titles: [ShapeDialogOp, string][] = [
    ['break_apart', 'Separar subtrayectos'],
    ['optimize', 'Optimizar formas'],
    ['fillet', 'Redondear esquinas'],
    ['cut_shapes', 'Cortar formas'],
    ['resize_slots', 'Redimensionar ranuras'],
  ];
  for (const [op, title] of titles) {
    it(`${op}: título y motivo de que no hay selección`, () => {
      const Dialog = SHAPE_DIALOGS[op];
      const html = renderToString(<Dialog />);
      expect(html).toContain(title);
      expect(html).toContain('Selecciona uno o más objetos.');
    });
  }
});
