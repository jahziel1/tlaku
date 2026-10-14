import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import type { DesignObject, LayerSettings } from '@/api/types';
import { makeLayer } from '@/lib/layers';
import { formatDpi, imageInfo } from './imageProps';
import { ShapePropsView } from './ShapeProps';

const noop = () => {};

const TEXT = { text: 'Hola', font_family: 'Inter', size_mm: 12, letter_spacing_mm: 0.5, line_spacing: 1.2, bold: false, italic: false };

describe('ventana de propiedades: texto', () => {
  it('un texto muestra contenido, fuente, tamaño y espaciados en la misma ventana', () => {
    const html = renderToString(
      <ShapePropsView
        params={null}
        mixed={false}
        text={TEXT}
        fonts={{ families: [], loading: false, fallback: 'Inter' }}
        onText={noop}
        onApply={noop}
        onClose={noop}
        onLock={noop}
      />,
    );
    expect(html).toContain('Hola');
    expect(html).toContain('<textarea');
    expect(html).toContain('Inter');
    expect(html).toContain('Fuente');
    expect(html).toContain('Tamaño');
    expect(html).toContain('Interlineado');
    expect(html).toContain('panel de texto');
  });

  it('sin texto no aparece la sección', () => {
    const html = renderToString(<ShapePropsView params={null} mixed={false} onApply={noop} onClose={noop} onLock={noop} />);
    expect(html).not.toContain('Interlineado');
    expect(html).not.toContain('<textarea');
  });
});

function image(transform: DesignObject['transform'] = [1, 0, 0, 1, 0, 0]): DesignObject {
  return {
    id: 'img',
    layer: '#0000ff',
    transform,
    kind: { type: 'image', source_path: '/tmp/a.png', width_px: 508, height_px: 254, size_mm: [50.8, 25.4] },
    locked: false,
    hidden: false,
    group: null,
  };
}

describe('ventana de propiedades: imagen', () => {
  const layer = (patch: Partial<LayerSettings> = {}) => makeLayer('#0000ff', 'Foto', { mode: 'image', interval: 0.1, ...patch });

  it('calcula el tamaño en el diseño y los ppp efectivos', () => {
    const info = imageInfo(image(), layer())!;
    expect(info).toMatchObject({ widthPx: 508, heightPx: 254, widthMm: 50.8, heightMm: 25.4 });
    expect(info.dpiX).toBeCloseTo(254);
    expect(info.dpiY).toBeCloseTo(254);
    expect(info.layerDpi).toBeCloseTo(254);
    expect(info.dither).toBe('stucki');
  });

  it('la escala del objeto cambia los ppp efectivos', () => {
    const info = imageInfo(image([2, 0, 0, 2, 0, 0]), layer())!;
    expect(info.widthMm).toBeCloseTo(101.6);
    expect(info.dpiX).toBeCloseTo(127);
    // Un giro no cambia el tamaño.
    const turned = imageInfo(image([0, 1, -1, 0, 0, 0]), layer())!;
    expect(turned.widthMm).toBeCloseTo(50.8);
    expect(turned.dpiX).toBeCloseTo(254);
  });

  it('una capa que no es de imagen no tiene tramado ni resolución', () => {
    const info = imageInfo(image(), layer({ mode: 'fill' }))!;
    expect(info.dither).toBeNull();
    expect(info.layerDpi).toBeNull();
    expect(info.layerMode).toBe('fill');
    expect(imageInfo({ ...image(), kind: { type: 'vector', paths: [] } }, layer())).toBeNull();
  });

  it('formatea los ppp', () => {
    expect(formatDpi(254.4)).toBe('254');
    expect(formatDpi(72.04)).toBe('72');
    expect(formatDpi(72.46)).toBe('72.5');
    expect(formatDpi(0)).toBe('–');
  });

  it('la ventana muestra píxeles, ppp, modo y los accesos a ajustes y vista previa', () => {
    const info = imageInfo(image(), layer())!;
    const html = renderToString(<ShapePropsView params={null} mixed={false} image={info} onImageSettings={noop} onImagePreview={noop} onApply={noop} onClose={noop} onLock={noop} />);
    expect(html).toContain('508 × 254 px');
    expect(html).toContain('254');
    expect(html).toContain('Stucki');
    expect(html).toContain('Ajustes de imagen');
    expect(html).toContain('Vista previa de la imagen');
    expect(html).not.toContain('no está en modo Imagen');
  });

  it('una imagen en capa de relleno avisa y no deja abrir la vista previa', () => {
    const info = imageInfo(image(), layer({ mode: 'fill' }))!;
    const html = renderToString(<ShapePropsView params={null} mixed={false} image={info} onImageSettings={noop} onImagePreview={noop} onApply={noop} onClose={noop} onLock={noop} />);
    expect(html).toContain('no está en modo Imagen');
    expect(html).toMatch(/<button[^>]*disabled[^>]*>.*Vista previa/s);
  });
});
