// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { DesignObject, TextObject } from '@/api/types';
import { setLanguage } from '@/i18n';
import { useLayerUi } from '@/features/layers/layerUi';
import { usePreviewUi } from '@/features/images/previewStore';
import { useStore } from '@/store';
import { cleanup, click, mount } from '@/test/dom';
import { ShapeProps } from './ShapeProps';

const st = () => useStore.getState();

const textKind: TextObject = {
  text: 'Hola',
  font_family: 'Arial',
  bold: false,
  italic: false,
  size_mm: 10,
  letter_spacing_mm: 0,
  line_spacing: 1,
  align: 'left',
  outline: [],
} as unknown as TextObject;

function textObject(): DesignObject {
  return { id: 't', layer: '#ff0000', transform: [1, 0, 0, 1, 0, 0], kind: { type: 'text', ...textKind }, locked: false, hidden: false, group: null } as DesignObject;
}

function imageObject(): DesignObject {
  return {
    id: 'img',
    layer: '#0000ff',
    transform: [1, 0, 0, 1, 0, 0],
    kind: { type: 'image', source_path: '/tmp/a.png', width_px: 300, height_px: 200, size_mm: [30, 20] },
    locked: false,
    hidden: false,
    group: null,
  };
}

beforeEach(() => {
  setLanguage('es');
  st().replaceDocument({ version: 2, objects: [], layers: [], device_id: null }, null);
  st().setShapePropsOpen(true);
});

afterEach(async () => {
  await cleanup();
  usePreviewUi.getState().set({ open: false, imageId: null });
  useLayerUi.getState().closeEditor();
  st().setShapePropsOpen(false);
});

describe('ventana de propiedades de un texto', () => {
  it('escribir el contenido cambia el texto en un solo paso de deshacer', async () => {
    st().addObjects([textObject()]);
    st().select(['t']);
    await mount(<ShapeProps />);
    const area = document.querySelector<HTMLTextAreaElement>('textarea')!;
    expect(area.value).toBe('Hola');
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')!.set!;
    for (const v of ['Hola m', 'Hola mu', 'Hola mundo']) {
      await act(async () => {
        setter.call(area, v);
        area.dispatchEvent(new Event('input', { bubbles: true }));
      });
    }
    const content = () => {
      const k = st().doc.objects[0]!.kind;
      return k.type === 'text' ? k.text : null;
    };
    expect(content()).toBe('Hola mundo');
    st().undo();
    expect(content()).toBe('Hola');
  });
});

describe('ventana de propiedades de una imagen', () => {
  beforeEach(() => {
    st().addObjects([imageObject()]);
    st().ensureLayer('#0000ff');
    st().updateLayer('#0000ff', { mode: 'image' });
    st().select(['img']);
  });

  it('muestra píxeles y ppp efectivos', async () => {
    await mount(<ShapeProps />);
    const text = document.body.textContent ?? '';
    expect(text).toContain('300 × 200 px');
    expect(text).toContain('254');
  });

  it('«Ajustes de imagen» abre el editor de la capa de la imagen', async () => {
    await mount(<ShapeProps />);
    const btn = [...document.querySelectorAll('button')].find((b) => b.textContent?.includes('Ajustes de imagen'))!;
    await click(btn);
    expect(useLayerUi.getState().editing).toBe('#0000ff');
    expect(st().activeLayer).toBe('#0000ff');
  });

  it('«Vista previa» abre el panel de vista previa de esa imagen', async () => {
    await mount(<ShapeProps />);
    const btn = [...document.querySelectorAll('button')].find((b) => b.textContent?.includes('Vista previa'))!;
    await click(btn);
    expect(usePreviewUi.getState()).toMatchObject({ open: true, imageId: 'img' });
  });
});
