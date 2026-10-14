import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import { createMockApi } from '@/api/mock';
import type { Document, ImageObject, LayerSettings } from '@/api/types';
import { setLanguage } from '@/i18n';
import { factoryLayer, newLayerFor } from '@/lib/layers';
import { useStore } from '@/store';
import { ImagePresets } from './ImagePresets';
import { ImagePreviewButton, ImagePreviewPanel } from './ImagePreviewPanel';
import {
  applyValues, cleanName, FACTORY_PRESETS, MAX_NAME_LENGTH, MAX_USER_PRESETS, matchPreset, NEUTRAL, parseUserPresets, PRESET_KEYS, sameValues, validValues, valuesOf,
  withoutPreset, withPreset, type UserPreset,
} from './presets';
import { usePreviewUi } from './previewStore';
import { clampCenter, imagesOfLayer, panCenter, pickImage, PX_PER_MM, pxPerMm, viewRegion, VIEW_H, VIEW_W } from './view';

const SIZE: [number, number] = [200, 100];

function imageLayer(): LayerSettings {
  return newLayerFor('#0000ff');
}

function docWithImages(ids: string[], layer = '#0000ff'): Document {
  const kind = (): ImageObject => ({ source_path: '/tmp/x.png', width_px: 100, height_px: 50, size_mm: SIZE, mask: [] });
  return {
    version: 2,
    device_id: null,
    layers: [factoryLayer(layer, 'Imagen')],
    objects: ids.map((id) => ({ id, layer, transform: [1, 0, 0, 1, 0, 0], kind: { type: 'image' as const, ...kind() }, locked: false, hidden: false, group: null })),
  };
}

/** `renderToString` lee el estado inicial de los stores: se copia el de ahora mientras se pinta. */
function ssr(node: React.ReactElement): string {
  const stores = [useStore, usePreviewUi] as unknown as { getInitialState: () => Record<string, unknown>; getState: () => Record<string, unknown> }[];
  const saved = stores.map((s) => ({ ...s.getInitialState() }));
  stores.forEach((s) => Object.assign(s.getInitialState(), s.getState()));
  try {
    return renderToString(node);
  } finally {
    stores.forEach((s, i) => Object.assign(s.getInitialState(), saved[i]));
  }
}

beforeEach(() => {
  setLanguage('es');
  usePreviewUi.setState({ open: false, imageId: null, zoom: 'fit', center: null });
});

describe('presets de fábrica', () => {
  it('todos están dentro de los límites del motor y son distintos entre sí', () => {
    for (const p of FACTORY_PRESETS) expect(validValues(p.values), p.id).toBe(true);
    const ids = new Set(FACTORY_PRESETS.map((p) => p.id));
    expect(ids.size).toBe(FACTORY_PRESETS.length);
    for (const a of FACTORY_PRESETS) for (const b of FACTORY_PRESETS) if (a !== b) expect(sameValues(a.values, b.values), `${a.id}/${b.id}`).toBe(false);
  });

  it('incluyen «Foto en madera» y «Logo de alto contraste» y tienen texto en los dos idiomas', () => {
    const names = FACTORY_PRESETS.map((p) => p.name);
    expect(names).toContain('layers.preset.photoWood');
    expect(names).toContain('layers.preset.logo');
    expect(FACTORY_PRESETS[0]!.values).toEqual(NEUTRAL);
  });

  it('aplicar un preset cambia solo esos seis valores', () => {
    const base = imageLayer().image;
    const photo = FACTORY_PRESETS.find((p) => p.id === 'photo-wood')!;
    const out = applyValues({ ...base, mode: 'stucki', threshold: 0.3, invert: true }, photo.values);
    expect(out).toMatchObject({ ...photo.values, mode: 'stucki', threshold: 0.3, invert: true });
    expect(valuesOf(out)).toEqual(photo.values);
    expect(PRESET_KEYS).toHaveLength(6);
  });

  it('reconoce cuál está activo, de fábrica o propio, o ninguno', () => {
    const mine: UserPreset = { id: 'u1', name: 'Mío', values: { ...NEUTRAL, contrast: 0.33 } };
    expect(matchPreset(NEUTRAL, [])).toBe('neutral');
    expect(matchPreset(FACTORY_PRESETS[1]!.values, [mine])).toBe(FACTORY_PRESETS[1]!.id);
    expect(matchPreset(mine.values, [mine])).toBe('u1');
    expect(matchPreset({ ...NEUTRAL, contrast: 0.5 }, [mine])).toBeNull();
  });
});

describe('presets propios', () => {
  let n = 0;
  const id = () => `id${++n}`;
  const values = { ...NEUTRAL, brightness: 0.2 };

  it('el nombre se limpia y uno vacío no sirve', () => {
    expect(cleanName('  Foto   mía  ')).toBe('Foto mía');
    expect(cleanName('   ')).toBeNull();
    expect(cleanName('x'.repeat(100))).toHaveLength(MAX_NAME_LENGTH);
  });

  it('guardar con un nombre repetido reemplaza el preset, conservando su id', () => {
    const one = withPreset([], 'Playa', values, id)!;
    expect(one).toHaveLength(1);
    const two = withPreset(one, 'playa', { ...values, contrast: 0.1 }, id)!;
    expect(two).toHaveLength(1);
    expect(two[0]).toMatchObject({ id: one[0]!.id, name: 'playa', values: { contrast: 0.1 } });
    expect(withPreset(one, 'Otra', values, id)).toHaveLength(2);
  });

  it('rechaza nombres vacíos, valores inválidos y listas llenas', () => {
    expect(withPreset([], ' ', values, id)).toBeNull();
    expect(withPreset([], 'a', { ...values, gamma: 0 }, id)).toBeNull();
    expect(withPreset([], 'a', { ...values, brightness: 2 }, id)).toBeNull();
    expect(withPreset([], 'a', { ...values, denoise: 17 }, id)).toBeNull();
    const full = Array.from({ length: MAX_USER_PRESETS }, (_, i) => ({ id: `p${i}`, name: `n${i}`, values }));
    expect(withPreset(full, 'nuevo', values, id)).toBeNull();
    // Reemplazar uno existente sí se puede con la lista llena.
    expect(withPreset(full, 'n3', { ...values, contrast: 0.2 }, id)).toHaveLength(MAX_USER_PRESETS);
  });

  it('borra por id', () => {
    const list = withPreset(withPreset([], 'a', values, id)!, 'b', values, id)!;
    expect(withoutPreset(list, list[0]!.id).map((p) => p.name)).toEqual(['b']);
  });

  it('lo guardado se lee de vuelta y lo malformado se descarta', () => {
    const list = withPreset([], 'Playa', values, id)!;
    expect(parseUserPresets(JSON.stringify(list))).toEqual(list);
    expect(parseUserPresets(null)).toEqual([]);
    expect(parseUserPresets('no es json')).toEqual([]);
    expect(parseUserPresets('{"a":1}')).toEqual([]);
    const bad = [{ id: 'x', name: 'ok', values }, { id: 'y', name: '', values }, { id: 'z', name: 'g', values: { ...values, gamma: -1 } }, { id: 'w', name: 'q', values: { brightness: 0 } }, 7, null];
    expect(parseUserPresets(JSON.stringify(bad)).map((p) => p.id)).toEqual(['x']);
  });
});

describe('vista: zoom, región y desplazamiento', () => {
  it('entera no pide región; a escala real pide lo que cabe en el visor', () => {
    expect(viewRegion('fit', [100, 50], SIZE)).toBeNull();
    const r = viewRegion(1, [100, 50], SIZE)!;
    expect(r[2] - r[0]).toBeCloseTo(VIEW_W / PX_PER_MM);
    expect(r[3] - r[1]).toBeCloseTo(VIEW_H / PX_PER_MM);
    // Centrada en el centro de la imagen.
    expect((r[0] + r[2]) / 2).toBeCloseTo(100);
  });

  it('con zoom 4 la región es cuatro veces más pequeña por eje', () => {
    const a = viewRegion(1, [100, 50], SIZE)!;
    const b = viewRegion(4, [100, 50], SIZE)!;
    expect((a[2] - a[0]) / (b[2] - b[0])).toBeCloseTo(4);
  });

  it('el centro no deja que el visor se salga de la imagen y una imagen pequeña queda centrada', () => {
    expect(clampCenter([0, 0], 4, SIZE)).toEqual([VIEW_W / pxPerMm(4, SIZE) / 2, VIEW_H / pxPerMm(4, SIZE) / 2]);
    const r = viewRegion(4, [500, 500], SIZE)!;
    expect(r[2]).toBeCloseTo(200);
    expect(r[3]).toBeCloseTo(100);
    // Imagen de 20 x 10 mm a tamaño real: cabe entera en el visor, así que se ve toda.
    expect(viewRegion(1, [3, 3], [20, 10])).toEqual([0, 0, 20, 10]);
  });

  it('arrastrar a la derecha muestra lo de la izquierda; hacia abajo, lo de arriba', () => {
    const c = panCenter([100, 50], 20, 20, 1, SIZE);
    expect(c[0]).toBeLessThan(100);
    expect(c[1]).toBeGreaterThan(50);
    expect(c[0]).toBeCloseTo(100 - 20 / PX_PER_MM);
  });

  it('«entera» ajusta la imagen al visor', () => {
    const k = pxPerMm('fit', SIZE);
    expect(SIZE[0] * k).toBeLessThanOrEqual(VIEW_W + 1e-9);
    expect(SIZE[1] * k).toBeLessThanOrEqual(VIEW_H + 1e-9);
  });
});

describe('imágenes de la capa', () => {
  it('busca las de la capa y elige la pedida, la seleccionada o la primera', () => {
    const doc = docWithImages(['a', 'b', 'c']);
    const imgs = imagesOfLayer(doc, '#0000ff');
    expect(imgs.map((i) => i.id)).toEqual(['a', 'b', 'c']);
    expect(imagesOfLayer(doc, '#ff0000')).toEqual([]);
    expect(pickImage(imgs, 'b', [])?.id).toBe('b');
    expect(pickImage(imgs, null, ['c'])?.id).toBe('c');
    expect(pickImage(imgs, 'zzz', [])?.id).toBe('a');
    expect(pickImage([], null, [])).toBeNull();
  });
});

describe('interfaz', () => {
  it('el selector de presets lista los de fábrica y marca el que coincide', () => {
    useStore.setState({ doc: docWithImages(['a']) });
    const layer = imageLayer();
    const html = ssr(<ImagePresets layer={layer} target={{ color: layer.color, sub: null }} disabled={false} />);
    expect(html).toContain('Preset de imagen');
    expect(html).toContain('Foto en madera');
    expect(html).toContain('Logo de alto contraste');
    expect(html).toContain('Guardar como…');
    // Una capa nueva no tiene ajustes: «Sin cambios» queda elegido y no hay «Personalizado».
    expect(html).not.toContain('Personalizado');
    const tweaked = { ...layer, image: { ...layer.image, contrast: 0.37 } };
    expect(ssr(<ImagePresets layer={tweaked} target={{ color: layer.color, sub: null }} disabled={false} />)).toContain('Personalizado');
  });

  it('en inglés se traduce', () => {
    setLanguage('en');
    const layer = imageLayer();
    const html = ssr(<ImagePresets layer={layer} target={{ color: layer.color, sub: null }} disabled={false} />);
    expect(html).toContain('Image preset');
    expect(html).toContain('Photo on wood');
  });

  it('el botón de vista previa se desactiva si la capa no tiene imágenes', () => {
    const layer = imageLayer();
    useStore.setState({ doc: docWithImages([], layer.color) });
    expect(ssr(<ImagePreviewButton layer={layer} />)).toMatch(/<button[^>]*disabled=""[^>]*>.*Vista previa de la imagen/);
    useStore.setState({ doc: docWithImages(['a'], layer.color) });
    expect(ssr(<ImagePreviewButton layer={layer} />)).not.toContain('disabled=""');
  });

  it('el panel muestra el título, el zoom y el aviso de que no hay imágenes si faltan', () => {
    const layer = imageLayer();
    useStore.setState({ doc: docWithImages(['a'], layer.color), selectedIds: [] });
    const html = ssr(<ImagePreviewPanel layer={layer} />);
    for (const s of ['Vista previa de la imagen', 'Entera', 'Tamaño real', '200 %', '400 %']) expect(html).toContain(s);
    useStore.setState({ doc: docWithImages([], layer.color) });
    expect(ssr(<ImagePreviewPanel layer={layer} />)).toContain('Esta capa no tiene imágenes que ver.');
  });
});

describe('maqueta de la vista previa', () => {
  const api = createMockApi({ latencyMs: 0 });
  const image: ImageObject = { source_path: '/tmp/x.png', width_px: 100, height_px: 50, size_mm: SIZE, mask: [] };

  it('la imagen entera sale más gruesa si la rejilla real no cabe, y de cerca sale exacta', async () => {
    const layer = { ...imageLayer(), interval: 0.1 };
    const full = await api.previewImage({ image, layer, region_mm: null, max_px: 400 });
    expect(full.exact).toBe(false);
    expect(full.width_px).toBe(400);
    expect(full.covered_mm).toEqual([0, 0, 200, 100]);
    const zoom = await api.previewImage({ image, layer, region_mm: [10, 10, 60, 40], max_px: 400 });
    expect(zoom.exact).toBe(true);
    expect(zoom.interval_mm).toBeCloseTo(0.1);
    expect(zoom.width_px).toBe(400); // 50 mm a 0,1 mm = 500 puntos, topados en max_px
    expect(zoom.covered_mm).toEqual([10, 10, 60, 40]);
    expect(zoom.png_base64.length).toBeGreaterThan(10);
  });
});
