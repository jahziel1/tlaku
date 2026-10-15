// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { getApi, setApi } from '@/api';
import type { LaserApi } from '@/api';
import { createMockApi } from '@/api/mock';
import schema from '@/api/mock/partsSchema.json';
import { commands } from '@/commands';
import { setLanguage, t, type I18nKey } from '@/i18n';
import { groupMembers } from '@/lib/groups';
import { objectBounds } from '@/lib/geometry';
import { selectActiveDevice, useStore } from '@/store';
import { audit, cleanup, click, describeViolations, mount } from '@/test/dom';
import { PartsBar } from './PartsBar';
import { PartsDialog } from './PartsDialog';
import { resetPartsUi, usePartsUi } from './partsStore';

let original: LaserApi;

const dialog = () => document.querySelector<HTMLElement>('[role="dialog"]')!;
const wait = (ms: number) => act(async () => void (await new Promise((r) => setTimeout(r, ms))));
/** Deja pasar la pausa de la vista previa y su respuesta. */
const settle = () => wait(260);
const buttons = () => [...dialog().querySelectorAll('button')];
const button = (text: string) => buttons().find((b) => b.textContent?.trim() === text)!;
const byLabel = (text: string): HTMLInputElement | HTMLSelectElement => {
  const label = [...dialog().querySelectorAll('label')].find((l) => l.textContent === text);
  if (!label) throw new Error(`No hay un campo «${text}»`);
  return dialog().querySelector<HTMLInputElement>(`#${CSS.escape(label.htmlFor)}`)!;
};

async function type(label: string, value: string) {
  const input = byLabel(label) as HTMLInputElement;
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(input, value);
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await act(async () => {
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  });
  await settle();
}

async function choose(label: string, value: string) {
  const select = byLabel(label) as HTMLSelectElement;
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value')!.set!.call(select, value);
    select.dispatchEvent(new Event('change', { bubbles: true }));
  });
  await settle();
}

const open = async () => {
  await mount(
    <>
      <PartsDialog />
      <PartsBar />
    </>,
  );
  await act(async () => commands.run('parts.open'));
  await settle();
};

const doc = () => useStore.getState().doc;

beforeEach(() => {
  setLanguage('es');
  original = getApi();
  setApi(createMockApi({ latencyMs: 0 }));
  resetPartsUi();
  act(() => useStore.getState().newDocument());
});

afterEach(async () => {
  await cleanup();
  act(() => usePartsUi.getState().closeDialog());
  setApi(original);
});

describe('diálogo «Generadores de piezas»', () => {
  it('el comando de Herramientas lo abre con la lista de generadores y la vista previa', async () => {
    await open();
    expect(dialog().textContent).toContain('Generadores de piezas');
    const names = [...dialog().querySelectorAll('.parts__gen-name')].map((n) => n.textContent);
    expect(names).toEqual(['Caja con dientes', 'Bandeja y cajón', 'Bisagra flexible', 'Prueba de encaje']);
    const svg = dialog().querySelector('[data-testid="parts-preview"]');
    expect(svg).not.toBeNull();
    expect(svg!.querySelectorAll('[data-part]')).toHaveLength(6);
    expect(dialog().textContent).toContain('6 piezas');
  });

  it('la vista previa sigue a los parámetros en vivo', async () => {
    await open();
    await choose('Tipo', 'open');
    expect(dialog().querySelectorAll('[data-part]')).toHaveLength(5);
    await type('Ancho (X)', '150');
    expect(dialog().textContent).toMatch(/Conjunto: .* mm · 5 piezas/);
  });

  it('un valor inválido se marca en su campo y bloquea Insertar', async () => {
    await open();
    // Con 10 mm de grosor, 20 mm de ancho no alcanzan para los dientes: el motor lo rechaza.
    await type('Grosor del material', '10');
    await type('Ancho (X)', '20');
    expect(dialog().querySelector('[role="alert"]')?.textContent).toContain('demasiado pequeña');
    expect((button('Insertar') as HTMLButtonElement).disabled).toBe(true);
  });

  it('muestra los campos según el tipo (tapa deslizante) y usa las unidades de la app', async () => {
    await open();
    expect(() => byLabel('Juego de la tapa')).toThrow();
    await choose('Tipo', 'sliding_lid');
    expect(byLabel('Juego de la tapa')).toBeTruthy();
    expect(dialog().querySelectorAll('.field__unit')[0]!.textContent).toBe('mm');
  });

  it('los divisores y su campo de texto llegan a los parámetros (el mock solo avisa que no los dibuja)', async () => {
    await open();
    await type('Divisores a lo ancho (reparten X)', '2');
    const widths = 'Anchos de los compartimentos en X (opcional, p. ej. 40, 60)';
    await type(widths, '30, 40');
    expect((byLabel(widths) as HTMLInputElement).value).toBe('30, 40');
    expect(usePartsUi.getState().values.box).toMatchObject({ dividers_x: 2, divider_widths_x: '30, 40' });
    expect(dialog().textContent).toContain('no se dibujan en la vista previa');
  });

  it('«Bandeja y cajón»: cajón con caja exterior y campos según el tipo', async () => {
    await open();
    await act(async () => usePartsUi.getState().setGenerator('tray'));
    await settle();
    expect(dialog().querySelectorAll('[data-part]')).toHaveLength(5);
    expect(() => byLabel('Cuánto sobresale el frente por cada lado')).toThrow();
    await choose('Qué generar', 'drawer_case');
    expect(byLabel('Cuánto sobresale el frente por cada lado')).toBeTruthy();
    expect(byLabel('Juego entre cajón y caja (por lado y de fondo)')).toBeTruthy();
    expect(dialog().querySelectorAll('[data-part]')).toHaveLength(11);
    await choose('Qué generar', 'drawer');
    expect(dialog().querySelectorAll('[data-part]')).toHaveLength(6);
    expect(() => byLabel('Juego entre cajón y caja (por lado y de fondo)')).toThrow();
  });

  it('Insertar pone un objeto por pieza, agrupado, en la capa de corte, y guarda los parámetros', async () => {
    await open();
    await type('Ancho (X)', '120');
    await act(async () => click(button('Insertar')));
    await wait(30);
    const objs = doc().objects;
    expect(objs).toHaveLength(6);
    const group = objs[0]!.group!;
    expect(objs.every((o) => o.group === group)).toBe(true);
    expect(doc().layers.length).toBe(1);
    expect(new Set(objs.map((o) => o.layer)).size).toBe(1);
    const rec = doc().generators![0]!;
    expect(rec.group).toBe(group);
    expect(rec.generator).toBe('box');
    expect(rec.params.width_mm).toBe(120);
    expect(rec.cut_layer).toBe(objs[0]!.layer);
    expect(useStore.getState().selectedIds).toHaveLength(6);
    expect(document.querySelector('[role="dialog"]')).toBeNull();
    // Dentro de la cama y sin solaparse entre sí.
    const bed = selectActiveDevice(useStore.getState()).bed;
    const boxes = objs.map((o) => objectBounds(o)!);
    for (const [[x0, y0], [x1, y1]] of boxes) {
      expect(x0).toBeGreaterThanOrEqual(0);
      expect(y0).toBeGreaterThanOrEqual(0);
      expect(x1).toBeLessThanOrEqual(bed[0]);
      expect(y1).toBeLessThanOrEqual(bed[1]);
    }
    for (const [i, a] of boxes.entries()) for (const b of boxes.slice(i + 1)) {
      expect(a[1][0] <= b[0][0] || b[1][0] <= a[0][0] || a[1][1] <= b[0][1] || b[1][1] <= a[0][1]).toBe(true);
    }
    // Un solo paso de deshacer.
    act(() => useStore.getState().undo());
    expect(doc().objects).toHaveLength(0);
    expect(doc().generators ?? []).toEqual([]);
  });

  it('las piezas nuevas esquivan lo que ya hay', async () => {
    await open();
    await act(async () => click(button('Insertar')));
    await wait(30);
    const first = new Set(doc().objects.map((o) => o.id));
    await act(async () => commands.run('parts.open'));
    await settle();
    await act(async () => click(button('Insertar')));
    await wait(30);
    const second = doc().objects.filter((o) => !first.has(o.id));
    expect(second).toHaveLength(6);
    const a = doc().objects.filter((o) => first.has(o.id)).map((o) => objectBounds(o)!);
    const b = second.map((o) => objectBounds(o)!);
    const union = (bs: typeof a) => [Math.min(...bs.map((x) => x[0][0])), Math.min(...bs.map((x) => x[0][1])), Math.max(...bs.map((x) => x[1][0])), Math.max(...bs.map((x) => x[1][1]))] as const;
    const [ax0, ay0, ax1, ay1] = union(a);
    const [bx0, by0, bx1, by1] = union(b);
    expect(bx0 >= ax1 || ax0 >= bx1 || by0 >= ay1 || ay0 >= by1).toBe(true);
    expect(new Set(doc().objects.map((o) => o.group)).size).toBe(2);
    expect(doc().generators).toHaveLength(2);
  });

  it('los rótulos van a una capa nueva de poca potencia y se pueden quitar', async () => {
    await open();
    const check = [...dialog().querySelectorAll<HTMLLabelElement>('label.parts__check')].find((l) => l.textContent === 'Grabar el nombre de cada panel')!.querySelector('input')!;
    await act(async () => click(check));
    await settle();
    expect(byLabel('Capa de los rótulos grabados')).toBeTruthy();
    await act(async () => click(button('Insertar')));
    await wait(30);
    expect(doc().objects).toHaveLength(12);
    expect(doc().layers).toHaveLength(2);
    const engrave = doc().layers.find((l) => l.name === 'Rótulos de piezas')!;
    expect(engrave.power).toBeLessThanOrEqual(20);
    expect(doc().objects.filter((o) => o.layer === engrave.color)).toHaveLength(6);
    expect(doc().generators![0]!.engrave_layer).toBe(engrave.color);
  });

  it('avisa del kerf de la capa, o de que hay que activarlo', async () => {
    await open();
    expect(dialog().textContent).toContain('activa el kerf');
    const layer = useStore.getState().ensureLayer('#ff3b4a');
    act(() => useStore.getState().updateLayer(layer.color, { kerf: 0.1 }));
    await wait(30);
    expect(dialog().textContent).toContain('compensa un kerf de 0.1 mm');
  });

  it('pasa la auditoría de accesibilidad', async () => {
    await open();
    const r = await audit(dialog());
    expect(describeViolations(r)).toEqual([]);
    await act(async () => click(buttons().find((b) => b.textContent?.includes('Bisagra flexible'))!));
    await settle();
    expect(describeViolations(await audit(dialog()))).toEqual([]);
  });
});

describe('Regenerar', () => {
  const insertBox = async () => {
    await open();
    await type('Ancho (X)', '120');
    await act(async () => click(button('Insertar')));
    await wait(30);
  };

  it('al seleccionar el grupo aparece la barra y el comando se habilita', async () => {
    expect(commands.get('parts.regenerate').enabled?.(useStore.getState())).not.toBe(true);
    await insertBox();
    expect(document.querySelector('[data-testid="parts-bar"]')?.textContent).toContain('Caja con dientes');
    expect(commands.get('parts.regenerate').enabled?.(useStore.getState())).toBe(true);
    act(() => useStore.getState().select([]));
    await wait(10);
    expect(document.querySelector('[data-testid="parts-bar"]')).toBeNull();
  });

  it('reabre el diálogo con los parámetros guardados y reemplaza las piezas en el mismo lugar', async () => {
    await insertBox();
    const group = doc().objects[0]!.group!;
    const before = doc().objects.map((o) => objectBounds(o)!);
    const minX = Math.min(...before.map((b) => b[0][0]));
    const minY = Math.min(...before.map((b) => b[0][1]));
    await act(async () => click(document.querySelector('[data-testid="parts-bar"] button')!));
    await settle();
    expect(dialog().textContent).toContain('Regenerar piezas');
    expect((byLabel('Ancho (X)') as HTMLInputElement).value).toBe('120');
    await type('Ancho (X)', '90');
    await act(async () => click(button('Regenerar')));
    await wait(30);
    const objs = doc().objects;
    expect(objs).toHaveLength(6);
    expect(groupMembers(objs, group)).toHaveLength(6);
    expect(doc().generators).toHaveLength(1);
    expect(doc().generators![0]!.params.width_mm).toBe(90);
    expect(doc().generators![0]!.group).toBe(group);
    const after = objs.map((o) => objectBounds(o)!);
    expect(Math.min(...after.map((b) => b[0][0]))).toBeCloseTo(minX, 6);
    expect(Math.min(...after.map((b) => b[0][1]))).toBeCloseTo(minY, 6);
    const front = objs.find((o) => o.id.startsWith('box-front'))!;
    const [[x0], [x1]] = objectBounds(front)!;
    expect(x1 - x0).toBeCloseTo(90, 6);
  });

  it('un guardado antiguo sin campos nuevos abre con los valores de fábrica en lo que falta', async () => {
    await insertBox();
    const s = useStore.getState();
    // El documento guardado por una versión anterior solo traía algunos parámetros.
    act(() => s.applyDocChange('parts.history.regenerate', (d) => ({ ...d, generators: d.generators!.map((r) => ({ ...r, params: { width_mm: 77 } })) })));
    await act(async () => click(document.querySelector('[data-testid="parts-bar"] button')!));
    await settle();
    expect((byLabel('Ancho (X)') as HTMLInputElement).value).toBe('77');
    expect((byLabel('Grosor del material') as HTMLInputElement).value).toBe('3');
  });

  it('si el grupo se borró, regenerar inserta uno nuevo', async () => {
    await insertBox();
    await act(async () => click(document.querySelector('[data-testid="parts-bar"] button')!));
    await settle();
    act(() => useStore.getState().deleteObjects(doc().objects.map((o) => o.id)));
    await act(async () => click(button('Regenerar')));
    await wait(30);
    expect(doc().objects).toHaveLength(6);
    expect(doc().generators).toHaveLength(1);
  });
});

describe('pestañas', () => {
  it('el resultado se escribe en la pestaña donde se abrió el diálogo, aunque se cambie de pestaña antes', async () => {
    const api = getApi();
    let release: () => void = () => undefined;
    const gate = new Promise<void>((r) => void (release = r));
    let calls = 0;
    // La primera petición después de abrir (la de Insertar) espera a que la prueba la suelte.
    setApi({
      ...api,
      generatePart: async (args) => {
        calls++;
        if (calls > 1 && usePartsUi.getState().busy) await gate;
        return api.generatePart(args);
      },
    });
    await open();
    const first = useStore.getState().activeTabId;
    const pending = act(async () => click(button('Insertar')));
    let second = '';
    act(() => {
      second = useStore.getState().newTab();
    });
    expect(useStore.getState().activeTabId).toBe(second);
    release();
    await pending;
    await wait(30);
    // La pestaña activa (la segunda) sigue vacía; las piezas están en la primera.
    expect(doc().objects).toHaveLength(0);
    act(() => useStore.getState().activateTab(first));
    expect(doc().objects).toHaveLength(6);
    expect(doc().generators).toHaveLength(1);
  });
});

describe('textos', () => {
  const keys = (lang: 'es' | 'en') => {
    setLanguage(lang);
    const out: string[] = [];
    for (const g of schema as { id: string; params: { key: string; section: string; kind: { type: string; options?: string[] } }[] }[]) {
      out.push(`parts.generator.${g.id}`, `parts.generator.${g.id}.desc`);
      for (const p of g.params) {
        out.push(`parts.${g.id}.${p.key}`, `parts.section.${p.section}`);
        for (const o of p.kind.options ?? []) out.push(`parts.${g.id}.${p.key}.${o}`);
      }
    }
    return out;
  };

  it('cada generador, campo, opción y sección del formulario tiene texto en es y en en (y distinto)', () => {
    for (const k of keys('es')) {
      setLanguage('es');
      const es = t(k as I18nKey);
      setLanguage('en');
      const en = t(k as I18nKey);
      expect(es, k).not.toBe(k);
      expect(en, k).not.toBe(k);
      expect(en, k).not.toBe(es);
    }
    setLanguage('es');
  });
});
