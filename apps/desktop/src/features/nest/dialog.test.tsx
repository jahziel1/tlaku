import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { selectActiveDevice, useStore } from '@/store';
import { NestDialogHost } from './index';
import { NestDialog } from './NestDialog';
import { NestPreview } from './NestPreview';
import { flatPolylines, pathData, placedPieces, unplacedSummary } from './preview';
import { rectObject } from './testUtils';

describe('diálogo de nesting', () => {
  it('cerrado no pinta nada', () => {
    expect(renderToString(<NestDialogHost />)).toBe('');
  });

  it('abierto sin selección muestra los bloques y el motivo de no poder empezar', () => {
    const html = renderToString(<NestDialog />);
    expect(html).toContain('Acomodar piezas (nesting)');
    expect(html).toContain('Copias de todas');
    const [w, h] = selectActiveDevice(useStore.getState()).bed;
    expect(html).toContain(`Cama (${w} × ${h} mm)`);
    expect(html).toContain('Separación entre piezas');
    expect(html).toContain('Empezar');
    expect(html).toContain('Selecciona las piezas que quieres acomodar.');
    expect(html).toContain('aria-disabled="true"');
  });
});

describe('vista previa', () => {
  const a = rectObject('a', 10, 5, [1, 2]);
  const flat = new Map([['a', flatPolylines([a])]]);

  it('aplica el transform del acomodo a cada contorno y calcula la caja', () => {
    expect(pathData(flat.get('a')!, [1, 0, 0, 1, 10, 0])).toBe('M11 2L21 2L21 7L11 7Z');
    const { pieces, bounds } = placedPieces(flat, [
      { piece: 'a', copy: 0, rotation_deg: 0, mirror: false, transform: [1, 0, 0, 1, 10, 0] },
      { piece: 'a', copy: 1, rotation_deg: 0, mirror: false, transform: [1, 0, 0, 1, 0, 30] },
      { piece: 'zzz', copy: 0, rotation_deg: 0, mirror: false, transform: [1, 0, 0, 1, 0, 0] },
    ]);
    expect(pieces.map((p) => [p.key, p.copy])).toEqual([['a#0', 0], ['a#1', 1]]);
    expect(bounds).toEqual([[1, 2], [21, 37]]);
  });

  it('dibuja el contenedor, las piezas y distingue las copias', () => {
    const html = renderToString(
      <NestPreview
        container={{ type: 'rect', width: 100, height: 60 }}
        flat={flat}
        placements={[
          { piece: 'a', copy: 0, rotation_deg: 0, mirror: false, transform: [1, 0, 0, 1, 0, 0] },
          { piece: 'a', copy: 1, rotation_deg: 0, mirror: false, transform: [1, 0, 0, 1, 20, 0] },
        ]}
      />,
    );
    expect(html).toContain('ne-bed');
    expect(html).toContain('scale(1 -1)');
    expect(html.match(/ne-piece"/g)).toHaveLength(1);
    expect(html.match(/ne-piece--copy/g)).toHaveLength(1);
    expect(html).not.toContain('ne-piece--ghost');
  });

  it('sin acomodo muestra las piezas donde están y con el material dibuja su contorno', () => {
    const html = renderToString(<NestPreview container={{ type: 'object', object: rectObject('m', 50, 40) }} flat={flat} placements={null} />);
    expect(html).toContain('ne-material');
    expect(html).toContain('ne-piece--ghost');
  });

  it('agrupa las que no cupieron por pieza', () => {
    expect(unplacedSummary([{ piece: 'a' }, { piece: 'b' }, { piece: 'a' }])).toEqual([{ piece: 'a', count: 2 }, { piece: 'b', count: 1 }]);
  });
});
