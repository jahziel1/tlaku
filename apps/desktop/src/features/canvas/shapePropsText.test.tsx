import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { ShapePropsView } from './ShapeProps';

const noop = () => {};

describe('ventana de propiedades: texto', () => {
  it('un texto muestra tamaño y espaciados en la misma ventana', () => {
    const html = renderToString(
      <ShapePropsView
        params={null}
        mixed={false}
        text={{ font_family: 'Inter', size_mm: 12, letter_spacing_mm: 0.5, line_spacing: 1.2 }}
        onText={noop}
        onApply={noop}
        onClose={noop}
        onLock={noop}
      />,
    );
    expect(html).toContain('Inter');
    expect(html).toContain('Tamaño');
    expect(html).toContain('Interlineado');
    expect(html).toContain('panel de texto');
  });

  it('sin texto no aparece la sección', () => {
    const html = renderToString(<ShapePropsView params={null} mixed={false} onApply={noop} onClose={noop} onLock={noop} />);
    expect(html).not.toContain('Interlineado');
  });
});
