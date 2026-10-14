import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { DesignObject, Document, LayerSettings } from '@/api/types';
import { ALL_COMMANDS, CommandRegistry } from '@/commands';
import { en, es, setLanguage, type I18nKey } from '@/i18n';
import { review as enReview } from '@/i18n/en/review';
import { review as esReview } from '@/i18n/es/review';
import { polylineBez } from '@/lib/bezpath';
import { makeLayer } from '@/lib/layers';
import { selectActiveDevice, useStore } from '@/store';
import { canFix, fixFinding, fixSafe, isSafe, selectFinding } from './fixes';
import { ReviewHost } from './index';
import { ReviewPanel } from './ReviewPanel';
import { SimpleReview, simpleFindings } from './SimpleReview';
import { closeReview, openReview, useReviewUi } from './store';
import { findingSimple, findingTitle, findingWhy } from './text';
import type { Finding, FindingKind } from './types';

const s = () => useStore.getState();
const review = async (doc: Document = s().doc): Promise<Finding[]> => {
  const bed = selectActiveDevice(s()).bed;
  const { getApi } = await import('@/api');
  return getApi().reviewDesign({ doc, bed: [bed[0], bed[1]] });
};
const kinds = (list: readonly Finding[]) => list.map((f) => f.kind).sort();
const find = (list: readonly Finding[], kind: FindingKind): Finding => {
  const f = list.find((x) => x.kind === kind);
  if (!f) throw new Error(`falta el hallazgo ${kind}`);
  return f;
};

const RED = '#ff0000';
const GREEN = '#00ff00';
const BLUE = '#0000ff';
const YELLOW = '#ffff00';
const IMG_HIGH = '#808080';
const IMG_LOW = '#818080';

const base = (id: string, layer: string, kind: DesignObject['kind'], transform: DesignObject['transform'] = [1, 0, 0, 1, 0, 0]): DesignObject => ({
  id, layer, transform, kind, locked: false, hidden: false, group: null,
});
const poly = (id: string, layer: string, pts: [number, number][], closed: boolean) => base(id, layer, { type: 'vector', paths: [polylineBez(pts, closed)] });
const rect = (id: string, layer: string, x0: number, y0: number, x1: number, y1: number) => poly(id, layer, [[x0, y0], [x1, y0], [x1, y1], [x0, y1]], true);
const image = (id: string, layer: string, px: number, mm: number, at: [number, number]) =>
  base(id, layer, { type: 'image', source_path: 'x.png', width_px: px, height_px: px, size_mm: [mm, mm] }, [1, 0, 0, 1, at[0], at[1]]);

/** Un caso de cada tipo; lo de la capa roja cae dentro de la cama salvo `half` y `away`. */
function synthetic(bed: [number, number]): Document {
  const layers: LayerSettings[] = [
    makeLayer(RED, 'Rojo', { mode: 'line', output: true, visible: true }),
    makeLayer(GREEN, 'Verde', { output: false }),
    makeLayer(BLUE, 'Azul', { visible: false }),
    makeLayer(YELLOW, 'Amarillo'),
    makeLayer(IMG_HIGH, 'Foto alta', { mode: 'image', interval: 0.1 }),
    makeLayer(IMG_LOW, 'Foto baja', { mode: 'image', interval: 0.1 }),
  ];
  const text = base('txt', YELLOW, {
    type: 'text', text: 'Hola', font_family: 'Arial', bold: false, italic: false, size_mm: 10, letter_spacing_mm: 0,
    line_spacing: 1.2, align: 'left', on_path: null, outline: [polylineBez([[0, 0], [5, 0], [5, 5]], true)],
  }, [1, 0, 0, 1, 20, 70]);
  const objects: DesignObject[] = [
    poly('gap', RED, [[0, 0], [10, 0], [10, 10], [0, 10], [0, 0.3]], false),
    rect('dup-a', RED, 20, 0, 30, 10),
    rect('dup-b', RED, 20, 0, 30, 10),
    poly('join-a', RED, [[40, 0], [50, 0]], false),
    poly('join-b', RED, [[50.05, 0], [60, 0]], false),
    rect('speck', RED, 70, 0, 70.1, 0.1),
    poly('nodes', RED, Array.from({ length: 11 }, (_, i): [number, number] => [i * 2, 50]), false),
    rect('half', RED, bed[0] - 5, 20, bed[0] + 5, 30),
    rect('away', RED, bed[0] + 100, 20, bed[0] + 110, 30),
    poly('silent', GREEN, [[0, 80], [10, 80], [10, 90]], false),
    rect('hid', BLUE, 40, 80, 50, 90),
    text,
    image('img-high', IMG_HIGH, 3000, 10, [60, 60]),
    image('img-low', IMG_LOW, 100, 30, [10, 60]),
  ];
  return { version: 2, objects, layers, device_id: null };
}

const ALL_KINDS: FindingKind[] = ['near_closed', 'overlap', 'joinable', 'tiny', 'extra_nodes', 'outside_all', 'outside_part', 'text', 'silent_layer', 'hidden_layer', 'image_dpi_high', 'image_dpi_low'];

function load(doc: Document) {
  s().replaceDocument(doc, null);
  s().clearHistory();
}

beforeEach(() => {
  setApi(createMockApi({ latencyMs: 0 }));
  setLanguage('es');
  useReviewUi.setState({ open: false, ignored: [], showIgnored: false, busy: false });
  load({ version: 2, objects: [], layers: [], device_id: null });
});

const bedOf = (): [number, number] => {
  const b = selectActiveDevice(s()).bed;
  return [b[0], b[1]];
};

describe('detección', () => {
  it('un documento sintético da un hallazgo de cada tipo, lo grave primero', async () => {
    load(synthetic(bedOf()));
    const found = await review();
    expect(kinds(found)).toEqual([...ALL_KINDS].sort());
    expect(find(found, 'near_closed').object_ids).toEqual(['gap']);
    expect(find(found, 'overlap').fix_ids).toEqual(expect.arrayContaining(['dup-a', 'dup-b']));
    expect(find(found, 'joinable').object_ids).toEqual(['join-a', 'join-b']);
    expect(find(found, 'tiny').object_ids).toEqual(['speck']);
    expect(find(found, 'extra_nodes').object_ids).toEqual(['nodes']);
    expect(find(found, 'outside_all').object_ids).toEqual(['away']);
    expect(find(found, 'outside_part').object_ids).toEqual(['half']);
    expect(find(found, 'text').object_ids).toEqual(['txt']);
    expect(find(found, 'silent_layer').object_ids).toEqual(['silent']);
    expect(find(found, 'hidden_layer').object_ids).toEqual(['hid']);
    expect(find(found, 'image_dpi_high').object_ids).toEqual(['img-high']);
    expect(find(found, 'image_dpi_low').object_ids).toEqual(['img-low']);
    const firstOther = found.findIndex((f) => f.severity !== 'serious');
    expect(found.slice(firstOther).every((f) => f.severity !== 'serious')).toBe(true);
    expect(new Set(found.map((f) => f.key)).size).toBe(found.length);
  });

  it('un diseño limpio y uno vacío no dan hallazgos', async () => {
    load({ version: 2, objects: [rect('a', RED, 10, 10, 30, 30), rect('b', RED, 40, 10, 60, 30)], layers: [makeLayer(RED, 'Rojo')], device_id: null });
    expect(await review()).toEqual([]);
    load({ version: 2, objects: [], layers: [], device_id: null });
    expect(await review()).toEqual([]);
  });
});

describe('arreglos: cada uno es un solo paso de deshacer', () => {
  const fixable: FindingKind[] = ['near_closed', 'overlap', 'joinable', 'tiny', 'extra_nodes', 'text', 'silent_layer', 'hidden_layer'];

  it.each(fixable)('%s: arregla, el hallazgo desaparece y deshacer lo revierte', async (kind) => {
    load(synthetic(bedOf()));
    const before = s().doc;
    const finding = find(await review(), kind);
    expect(canFix(finding)).toBe(true);
    expect(await fixFinding(finding)).toBe(true);
    expect(s().past).toHaveLength(1);
    expect(s().doc).not.toBe(before);
    const after = await review();
    expect(after.some((f) => f.kind === kind), `${kind} sigue`).toBe(false);
    // Un arreglo no estropea los demás hallazgos de otro tipo.
    expect(after.length).toBeGreaterThanOrEqual(ALL_KINDS.length - 2);
    s().undo();
    expect(s().doc).toBe(before);
    expect(s().past).toHaveLength(0);
  });

  it('cerrar deja el trazo cerrado; unir hace un solo objeto; quitar duplicados deja uno', async () => {
    load(synthetic(bedOf()));
    await fixFinding(find(await review(), 'near_closed'));
    expect(s().doc.objects.some((o) => o.id === 'gap')).toBe(false);
    await fixFinding(find(await review(), 'joinable'));
    expect(s().doc.objects.some((o) => o.id === 'join-a' || o.id === 'join-b')).toBe(false);
    await fixFinding(find(await review(), 'overlap'));
    expect(s().doc.objects.filter((o) => o.id.startsWith('dup-')).length).toBe(1);
    expect(s().past).toHaveLength(3);
  });

  it('borrar fragmentos no toca lo demás y convertir texto lo deja vector', async () => {
    load(synthetic(bedOf()));
    const total = s().doc.objects.length;
    await fixFinding(find(await review(), 'tiny'));
    expect(s().doc.objects).toHaveLength(total - 1);
    await fixFinding(find(await review(), 'text'));
    expect(s().doc.objects.find((o) => o.id === 'txt')?.kind.type).toBe('vector');
  });

  it('activar la salida y mostrar la capa cambian solo esa capa', async () => {
    load(synthetic(bedOf()));
    await fixFinding(find(await review(), 'silent_layer'));
    await fixFinding(find(await review(), 'hidden_layer'));
    const layers = s().doc.layers;
    expect(layers.find((l) => l.color === GREEN)?.output).toBe(true);
    expect(layers.find((l) => l.color === BLUE)?.visible).toBe(true);
    expect(layers.find((l) => l.color === RED)?.output).toBe(true);
    // Dos arreglos seguidos de capa no se funden en un solo paso de deshacer.
    expect(s().past).toHaveLength(2);
  });

  it('«Arreglar todo lo seguro» cierra, une y quita duplicados en un solo paso, y nada más', async () => {
    load(synthetic(bedOf()));
    const before = s().doc;
    const found = await review();
    expect(found.filter(isSafe).map((f) => f.kind).sort()).toEqual(['joinable', 'near_closed', 'overlap']);
    expect(await fixSafe(found)).toBe(true);
    expect(s().past).toHaveLength(1);
    const after = await review();
    for (const kind of ['near_closed', 'overlap', 'joinable'] as const) expect(after.some((f) => f.kind === kind), kind).toBe(false);
    // No borra ni mueve: los fragmentos, lo que está fuera y los textos siguen ahí.
    for (const id of ['speck', 'away', 'half', 'txt', 'silent']) expect(s().doc.objects.some((o) => o.id === id), id).toBe(true);
    for (const kind of ['tiny', 'outside_all', 'outside_part', 'text', 'silent_layer', 'hidden_layer'] as const) expect(after.some((f) => f.kind === kind), kind).toBe(true);
    s().undo();
    expect(s().doc).toBe(before);
  });

  it('sin hallazgos seguros avisa y no cambia nada', async () => {
    load(synthetic(bedOf()));
    const before = s().doc;
    const found = (await review()).filter((f) => !isSafe(f));
    expect(await fixSafe(found)).toBe(false);
    expect(s().doc).toBe(before);
    expect(s().past).toHaveLength(0);
  });

  it('si el diseño cambia mientras se arregla, no aplica nada', async () => {
    load(synthetic(bedOf()));
    const finding = find(await review(), 'near_closed');
    const pending = fixFinding(finding);
    s().deleteObjects(['speck']);
    const edited = s().doc;
    expect(await pending).toBe(false);
    expect(s().doc).toBe(edited);
    expect(s().past).toHaveLength(1);
  });

  it('lo que está fuera de la cama solo se selecciona: no se mueve', async () => {
    load(synthetic(bedOf()));
    const before = s().doc;
    for (const kind of ['outside_all', 'outside_part'] as const) {
      const f = find(await review(), kind);
      expect(canFix(f)).toBe(false);
      selectFinding(f);
      expect(s().selectedIds).toEqual(f.object_ids);
    }
    expect(s().doc).toBe(before);
    expect(s().past).toHaveLength(0);
  });

  it('las imágenes solo avisan', async () => {
    load(synthetic(bedOf()));
    const found = await review();
    expect(canFix(find(found, 'image_dpi_high'))).toBe(false);
    expect(canFix(find(found, 'image_dpi_low'))).toBe(false);
  });
});

describe('textos', () => {
  it('cada tipo tiene título, motivo, texto sencillo y arreglo en español e inglés', () => {
    for (const kind of ALL_KINDS) {
      for (const key of [`review.kind.${kind}.title.one`, `review.kind.${kind}.title.other`, `review.kind.${kind}.why`] as I18nKey[]) {
        expect(es[key], key).toBeTruthy();
        expect(en[key], key).toBeTruthy();
      }
      for (const form of ['one', 'other']) {
        expect(es[`review.simple.${kind}.${form}` as I18nKey], kind).toBeTruthy();
        expect(en[`review.simple.${kind}.${form}` as I18nKey], kind).toBeTruthy();
      }
    }
    for (const kind of ['near_closed', 'overlap', 'joinable', 'tiny', 'extra_nodes', 'text', 'silent_layer', 'hidden_layer']) {
      const fixKeys = ['review.fix.', 'review.history.'].map((prefix) => `${prefix}${kind}`);
      // Los botones del modo sencillo concuerdan en número, salvo los de capa (hablan de una sola).
      fixKeys.push(...(kind.endsWith('_layer') ? [`review.simple.fix.${kind}`] : [`review.simple.fix.${kind}.one`, `review.simple.fix.${kind}.other`]));
      for (const key of fixKeys) {
        expect(es[key as I18nKey], key).toBeTruthy();
        expect(en[key as I18nKey], key).toBeTruthy();
      }
    }
    expect(Object.keys(esReview).sort()).toEqual(Object.keys(enReview).sort());
  });

  it('los textos de cada hallazgo salen completos, sin marcadores sin llenar', async () => {
    load(synthetic(bedOf()));
    const found = await review();
    for (const lang of ['es', 'en'] as const) {
      setLanguage(lang);
      for (const f of found) {
        for (const text of [findingTitle(f, s().doc), findingWhy(f, s().doc), findingSimple(f, s().doc)]) {
          expect(text, `${lang} ${f.kind}`).not.toMatch(/\{\w+\}/);
          expect(text.length).toBeGreaterThan(10);
        }
      }
    }
  });
});

describe('plurales', () => {
  it('con un solo objeto los títulos salen en singular, en español e inglés', async () => {
    load(synthetic(bedOf()));
    const found = await review();
    const away = { ...find(found, 'outside_all'), count: 1 };
    const tiny = { ...find(found, 'tiny'), count: 1 };
    const doc = s().doc;
    setLanguage('es');
    expect(findingTitle(away, doc)).toBe('1 objeto fuera de la cama');
    expect(findingTitle(tiny, doc)).toBe('1 fragmento diminuto');
    expect(findingTitle({ ...tiny, count: 2 }, doc)).toBe('2 fragmentos diminutos');
    expect(findingSimple(away, doc)).toBe('1 cosa está fuera de la cama y no se hará.');
    setLanguage('en');
    expect(findingTitle(away, doc)).toBe('1 object off the bed');
    expect(findingTitle({ ...away, count: 3 }, doc)).toBe('3 objects off the bed');
    setLanguage('es');
  });
});

describe('interfaz', () => {
  it('el comando «Revisar diseño…» existe, pide objetos y abre el panel', async () => {
    const registry = new CommandRegistry(ALL_COMMANDS, () => s());
    expect(registry.availability('review.open', s())).toBe('review.reason.empty');
    load(synthetic(bedOf()));
    expect(registry.availability('review.open', s())).toBe(true);
    await registry.run('review.open');
    expect(useReviewUi.getState().open).toBe(true);
    closeReview();
    expect(useReviewUi.getState().open).toBe(false);
    openReview();
    expect(useReviewUi.getState().open).toBe(true);
  });

  it('el panel se dibuja y dice que está revisando', () => {
    const html = renderToString(<ReviewPanel />);
    expect(html).toContain('Revisar diseño');
    expect(html).toContain('Revisando el diseño');
  });

  it('ReviewHost cerrado no pinta nada', () => {
    expect(renderToString(<ReviewHost />)).toBe('');
    expect(renderToString(<SimpleReview shown={[]} />)).toBe('');
  });

  it('el modo sencillo no repite lo que ya avisa ni las capas apagadas a propósito', async () => {
    load(synthetic(bedOf()));
    const found = await review();
    expect(simpleFindings(found, []).some((f) => f.kind === 'silent_layer')).toBe(false);
    expect(simpleFindings(found, []).some((f) => f.kind === 'text')).toBe(true);
    const dup = simpleFindings(found, ['texts', 'outside']).map((f) => f.kind);
    expect(dup).not.toContain('text');
    expect(dup).not.toContain('outside_all');
    expect(dup).not.toContain('outside_part');
    expect(dup).toContain('near_closed');
  });

  it('«Ignorar» y volver a mostrar', () => {
    const ui = useReviewUi.getState();
    ui.ignore('a');
    ui.ignore('a');
    expect(useReviewUi.getState().ignored).toEqual(['a']);
    ui.restore('a');
    expect(useReviewUi.getState().ignored).toEqual([]);
  });
});
