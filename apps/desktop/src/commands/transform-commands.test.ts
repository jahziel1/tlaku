import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DesignObject } from '@/api/types';
import { ellipseBez } from '@/lib/bezpath';
import { objectBounds } from '@/lib/geometry';
import { newVectorObject } from '@/features/canvas/shapes';
import { lineObj, rectObj } from '@/features/canvas/testUtil';
import { makeLayer } from '@/lib/layers';
import { createAppStore } from '@/store';
import { ALL_COMMANDS } from './index';
import { CommandRegistry } from './registry';

let store: ReturnType<typeof createAppStore>;
let registry: CommandRegistry;
const s = () => store.getState();

function load(objects: DesignObject[], layers = [makeLayer('#ff0000', 'Rojo'), makeLayer('#00ff00', 'Verde', { mode: 'fill' })]) {
  s().applyImport({ objects, new_layers: layers, warnings: [] });
  s().clearHistory();
}
const box = (id: string) => objectBounds(s().doc.objects.find((o) => o.id === id)!)!;
const order = () => s().doc.objects.map((o) => o.id);

beforeEach(() => {
  store = createAppStore();
  registry = new CommandRegistry(ALL_COMMANDS, () => store.getState());
});

describe('voltear', () => {
  it('horizontal y vertical sobre el centro de la selección, en un paso que se deshace', async () => {
    load([rectObj('a', 0, 0, 10, 10), rectObj('b', 20, 0, 40, 30)]);
    s().select(['a', 'b']);
    await registry.run('arrange.flip.h');
    expect(box('a')[0][0]).toBeCloseTo(30, 9); // centro x = 20
    expect(box('b')[1][0]).toBeCloseTo(20, 9);
    expect(s().past).toHaveLength(1);
    await registry.run('arrange.flip.v');
    expect(box('a')[0][1]).toBeCloseTo(20, 9); // centro y = 15
    s().undo();
    s().undo();
    expect(box('a')[0][0]).toBe(0);
    expect(box('a')[0][1]).toBe(0);
    s().redo();
    expect(box('a')[0][0]).toBeCloseTo(30, 9);
  });

  it('necesita una selección movible y tiene atajos sin chocar con pegar en sitio', () => {
    load([rectObj('a', 0, 0, 1, 1, { locked: true })]);
    expect(registry.availability('arrange.flip.h')).toBe('arrange.reason.noSelection');
    expect(registry.match('Alt+H')?.id).toBe('arrange.flip.h');
    expect(registry.match('Mod+Shift+H')?.id).toBe('arrange.flip.h');
    expect(registry.match('Alt+V')?.id).toBe('arrange.flip.v');
    expect(registry.match('Mod+Shift+V')?.id).toBe('edit.pasteInPlace');
  });
});

describe('rotación rápida', () => {
  it('[ y ] giran 90° y el paso usa el valor configurable', async () => {
    load([rectObj('a', 0, 0, 20, 10)]);
    s().select(['a']);
    await registry.run('arrange.rotate.cw90');
    expect(box('a')[1][0] - box('a')[0][0]).toBeCloseTo(10, 6);
    expect(box('a')[1][1] - box('a')[0][1]).toBeCloseTo(20, 6);
    await registry.run('arrange.rotate.ccw90');
    expect(box('a')[1][0] - box('a')[0][0]).toBeCloseTo(20, 6);
    s().setPref('rotateStep', 30);
    await registry.run('arrange.rotate.ccwStep');
    const t = s().doc.objects[0]!.transform;
    expect(Math.atan2(t[1], t[0])).toBeCloseTo(Math.PI / 6, 9);
    await registry.run('arrange.rotate.cwStep');
    await registry.run('arrange.rotate.ccw5');
    expect((Math.atan2(s().doc.objects[0]!.transform[1], s().doc.objects[0]!.transform[0]) * 180) / Math.PI).toBeCloseTo(5, 6);
  });

  it('las imágenes no se pueden girar', () => {
    const img: DesignObject = { ...rectObj('i', 0, 0, 1, 1), kind: { type: 'image', source_path: 'x.png', width_px: 1, height_px: 1, size_mm: [1, 1] } };
    load([img]);
    s().select(['i']);
    expect(registry.availability('arrange.rotate.cw90')).toBe('canvas.bar.rotateImage');
  });

  it('atajos asignados', () => {
    expect(registry.match('[')?.id).toBe('arrange.rotate.ccw90');
    expect(registry.match(']')?.id).toBe('arrange.rotate.cw90');
    expect(registry.match('Shift+]')?.id).toBe('arrange.rotate.cwStep');
    expect(registry.match('Alt+[')?.id).toBe('arrange.rotate.ccw5');
  });
});

describe('orden de dibujo', () => {
  it('al frente, al fondo, subir y bajar, con motivo cuando no cambia nada', async () => {
    load([rectObj('a', 0, 0, 1, 1), rectObj('b', 0, 0, 1, 1), rectObj('c', 0, 0, 1, 1)]);
    s().select(['a']);
    await registry.run('arrange.order.front');
    expect(order()).toEqual(['b', 'c', 'a']);
    expect(registry.availability('arrange.order.front')).toBe('arrange.reason.orderNoChange');
    await registry.run('arrange.order.backward');
    expect(order()).toEqual(['b', 'a', 'c']);
    await registry.run('arrange.order.back');
    expect(order()).toEqual(['a', 'b', 'c']);
    await registry.run('arrange.order.forward');
    expect(order()).toEqual(['b', 'a', 'c']);
    s().undo();
    expect(order()).toEqual(['a', 'b', 'c']);
    s().select([]);
    expect(registry.availability('arrange.order.front')).toBe('arrange.reason.noSelection');
  });

  it('cambia lo que queda encima al hacer clic', async () => {
    load([rectObj('abajo', 0, 0, 10, 10), rectObj('arriba', 0, 0, 10, 10)]);
    s().select(['abajo']);
    await registry.run('arrange.order.front');
    expect(order().at(-1)).toBe('abajo');
  });
});

describe('bloquear', () => {
  it('un objeto bloqueado sale de la selección, no se mueve ni se borra y se desbloquea con "desbloquear todo"', async () => {
    load([rectObj('a', 0, 0, 10, 10), rectObj('b', 20, 0, 30, 10)]);
    s().select(['a']);
    await registry.run('arrange.lock');
    expect(s().doc.objects[0]!.locked).toBe(true);
    s().pruneSelection();
    expect(s().selectedIds).toEqual([]);
    s().moveObjects(['a'], 5, 5);
    s().deleteObjects(['a']);
    expect(box('a')[0][0]).toBe(0);
    expect(s().doc.objects).toHaveLength(2);
    s().selectAll();
    expect(s().selectedIds).toEqual(['b']);
    expect(registry.availability('arrange.unlockAll')).toBe(true);
    await registry.run('arrange.unlockAll');
    expect(s().doc.objects.every((o) => !o.locked)).toBe(true);
    expect(s().selectedIds).toEqual(['a']);
    expect(registry.availability('arrange.unlockAll')).toBe('arrange.reason.noLocked');
    s().undo();
    expect(s().doc.objects[0]!.locked).toBe(true);
  });

  it('Ctrl+L y Ctrl+Mayús+L', () => {
    expect(registry.match('Mod+L')?.id).toBe('arrange.lock');
    expect(registry.match('Mod+Shift+L')?.id).toBe('arrange.unlockAll');
  });
});

describe('grupos anidados con los comandos', () => {
  it('agrupar dos grupos los anida y desagrupar abre un nivel', async () => {
    load([rectObj('a', 0, 0, 1, 1), rectObj('b', 2, 0, 3, 1), rectObj('c', 4, 0, 5, 1)]);
    s().select(['a', 'b']);
    await registry.run('edit.group');
    s().select(['a', 'b', 'c']);
    await registry.run('edit.group');
    expect(s().doc.objects.map((o) => o.group)).toEqual(['grupo-2/grupo-1', 'grupo-2/grupo-1', 'grupo-2']);
    // Un clic en uno elige todo el grupo de más afuera (la selección ya viene expandida desde el lienzo).
    s().select(['a', 'b', 'c']);
    expect(registry.availability('edit.group')).toBe('history.reason.groupNeedsTwo'); // una sola unidad
    await registry.run('edit.ungroup');
    expect(s().doc.objects.map((o) => o.group)).toEqual(['grupo-1', 'grupo-1', null]);
    await registry.run('edit.ungroup');
    expect(s().doc.objects.every((o) => o.group === null)).toBe(true);
    s().undo();
    s().undo();
    expect(s().doc.objects.map((o) => o.group)).toEqual(['grupo-2/grupo-1', 'grupo-2/grupo-1', 'grupo-2']);
  });
});

describe('selecciones especiales con los comandos', () => {
  it('abiertas, abiertas en relleno y capa', async () => {
    load([rectObj('closed', 0, 0, 5, 5), lineObj('open', [0, 0], [5, 5]), lineObj('openFill', [0, 0], [5, 5], { layer: '#00ff00' })]);
    await registry.run('edit.selectOpen');
    expect(s().selectedIds).toEqual(['open', 'openFill']);
    await registry.run('edit.selectOpenFill');
    expect(s().selectedIds).toEqual(['openFill']);
    s().setActiveLayer('#ff0000');
    await registry.run('edit.selectLayer');
    expect(s().selectedIds).toEqual(['closed', 'open']);
  });

  it('sin coincidencias avisa y no cambia la selección', async () => {
    load([rectObj('closed', 0, 0, 5, 5)]);
    s().select(['closed']);
    await registry.run('edit.selectOpen');
    expect(s().selectedIds).toEqual(['closed']);
    expect(s().notices.at(-1)?.text).toBe('No hay formas abiertas.');
  });

  it('contenidas y menores usan la selección como referencia', async () => {
    load([rectObj('big', 0, 0, 100, 100), rectObj('in', 10, 10, 20, 20), rectObj('out', 150, 0, 160, 10), rectObj('huge', 200, 0, 400, 300)]);
    s().select(['big']);
    await registry.run('edit.selectContained');
    expect(s().selectedIds.sort()).toEqual(['big', 'in']);
    s().select(['big']);
    await registry.run('edit.selectSmaller');
    expect(s().selectedIds.sort()).toEqual(['in', 'out']);
  });

  it('círculos por tamaño abre su diálogo con el diámetro de la selección', async () => {
    const { useArrangeUi } = await import('@/features/arrange/uiStore');
    load([newVectorObject('c', '#ff0000', [ellipseBez(0, 0, 3, 3)])]);
    s().select(['c']);
    await registry.run('edit.selectCircles');
    expect(useArrangeUi.getState().dialog).toEqual({ kind: 'circles' });
    expect(useArrangeUi.getState().circles.min).toBeCloseTo(6, 6);
    useArrangeUi.getState().close();
  });
});

describe('igualar ancho y alto', () => {
  it('con los comandos, tomando la primera selección como referencia', async () => {
    load([rectObj('a', 0, 0, 10, 20), rectObj('b', 50, 0, 80, 10)]);
    s().select(['a', 'b']);
    await registry.run('arrange.same.width');
    expect(box('b')[1][0] - box('b')[0][0]).toBeCloseTo(10, 9);
    s().select(['b', 'a']);
    await registry.run('arrange.same.heightKeep');
    expect(box('a')[1][1] - box('a')[0][1]).toBeCloseTo(10, 9);
    expect(box('a')[1][0] - box('a')[0][0]).toBeCloseTo(5, 9); // conservó la proporción
    s().select(['a']);
    expect(registry.availability('arrange.same.width')).toBe('arrange.reason.alignNeedTwo');
  });
});

describe('espejo sobre una línea', () => {
  it('con los objetos y, de último, una recta de dos puntos: copia reflejada', async () => {
    load([rectObj('a', 10, 0, 20, 10), lineObj('eje', [0, -5], [0, 50])]);
    s().select(['a', 'eje']);
    await registry.run('arrange.mirrorLine');
    expect(s().doc.objects).toHaveLength(3);
    const copy = s().doc.objects[2]!;
    expect(s().selectedIds).toEqual([copy.id]);
    const b = objectBounds(copy)!;
    expect(b[0][0]).toBeCloseTo(-20, 9);
    expect(b[1][0]).toBeCloseTo(-10, 9);
    expect(s().past).toHaveLength(1);
    s().undo();
    expect(s().doc.objects).toHaveLength(2);
  });

  it('sin una recta elegida inicia el modo de dibujar la línea', async () => {
    const { useInteraction } = await import('@/features/canvas/interaction');
    load([rectObj('a', 10, 0, 20, 10)]);
    s().select(['a']);
    await registry.run('arrange.mirrorLine');
    expect(useInteraction.getState().pick).toEqual({ mode: 'mirror', points: [] });
    expect(useInteraction.getState().gesture).toBe('pick');
    useInteraction.setState({ pick: null, gesture: null });
  });

  it('girar y escalar por dos puntos inicia su modo', async () => {
    const { useInteraction } = await import('@/features/canvas/interaction');
    load([rectObj('a', 10, 0, 20, 10)]);
    s().select(['a']);
    await registry.run('arrange.twoPoint');
    expect(useInteraction.getState().pick?.mode).toBe('twoPoint');
    useInteraction.setState({ pick: null, gesture: null });
  });
});

describe('interruptores de transformación', () => {
  beforeEach(() => load([rectObj('a', 0, 0, 10, 10)]));

  it('apagar mover impide mover, empujar con flechas y avisa', async () => {
    s().select(['a']);
    await registry.run('arrange.xform.move');
    expect(s().transformAllow.move).toBe(false);
    expect(registry.checked('arrange.xform.move')).toBe(false);
    s().moveObjects(['a'], 5, 0);
    await registry.run('edit.nudge.right');
    expect(box('a')[0][0]).toBe(0);
    expect(s().notices.at(-1)?.text).toContain('Mover está desactivado');
    await registry.run('arrange.xform.move');
    s().moveObjects(['a'], 5, 0);
    expect(box('a')[0][0]).toBe(5);
  });

  it('apagar girar bloquea los giros pero no el movimiento ni la escala', async () => {
    s().select(['a']);
    s().setTransformAllow({ rotate: false });
    await registry.run('arrange.rotate.cw90');
    expect(s().doc.objects[0]!.transform).toEqual([1, 0, 0, 1, 0, 0]);
    s().moveObjects(['a'], 1, 1);
    expect(box('a')[0][0]).toBe(1);
    s().transformObjects(['a'], [2, 0, 0, 2, 0, 0]);
    expect(box('a')[1][0]).toBeCloseTo(22, 9);
  });

  it('apagar escalar bloquea escalar y voltear', async () => {
    s().select(['a']);
    s().setTransformAllow({ scale: false });
    s().transformObjects(['a'], [2, 0, 0, 2, 0, 0]);
    await registry.run('arrange.flip.h');
    expect(s().doc.objects[0]!.transform).toEqual([1, 0, 0, 1, 0, 0]);
    expect(s().past).toHaveLength(0);
  });

  it('escalar desde el centro y mantener proporción son interruptores', async () => {
    expect(registry.checked('arrange.xform.center')).toBe(false);
    await registry.run('arrange.xform.center');
    expect(s().scaleFromCenter).toBe(true);
    expect(registry.checked('arrange.xform.ratio')).toBe(true);
    await registry.run('arrange.xform.ratio');
    expect(s().lockRatio).toBe(false);
  });
});

describe('herramientas nuevas', () => {
  it('polígono, estrella y medir tienen su comando y su tecla', async () => {
    expect(registry.match('G')?.id).toBe('tool.polygon');
    expect(registry.match('Shift+G')?.id).toBe('tool.star');
    expect(registry.match('M')?.id).toBe('tool.measure');
    await registry.run('tool.star');
    expect(s().activeTool).toBe('star');
    expect(registry.checked('tool.star')).toBe(true);
  });

  it('los valores de polígono y estrella se limitan', () => {
    s().setPolygonSides(1);
    expect(s().polygonSides).toBe(3);
    s().setPolygonSides(7.4);
    expect(s().polygonSides).toBe(7);
    s().setStarRatio(5);
    expect(s().starRatio).toBe(0.95);
    s().setShapeRounding(-2);
    expect(s().shapeRounding).toBe(0);
  });

  it('la ventana de propiedades de forma se conmuta con su comando', async () => {
    expect(registry.checked('panel.shapeProps')).toBe(true);
    await registry.run('panel.shapeProps');
    expect(s().shapePropsOpen).toBe(false);
  });
});

describe('portapapeles entre aplicaciones', () => {
  const writes: string[] = [];
  let clip = '';
  beforeEach(() => {
    writes.length = 0;
    clip = '';
    vi.stubGlobal('navigator', {
      clipboard: {
        writeText: async (v: string) => {
          writes.push(v);
          clip = v;
        },
        readText: async () => clip,
      },
    });
  });
  afterEach(() => vi.unstubAllGlobals());

  const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="40mm" height="20mm" viewBox="0 0 40 20"><rect width="40" height="20" stroke="#0000ff" fill="none"/><circle cx="20" cy="10" r="5" stroke="#0000ff" fill="none"/></svg>';

  it('copiar escribe también un SVG en el portapapeles del sistema', async () => {
    load([rectObj('a', 0, 0, 10, 10)]);
    s().select(['a']);
    await registry.run('edit.copy');
    await Promise.resolve();
    expect(writes).toHaveLength(1);
    expect(writes[0]).toContain('<svg');
    expect(s().clipboard?.objects).toHaveLength(1);
  });

  it('pegar lo que copió esta misma app usa el portapapeles interno (conserva todo)', async () => {
    load([rectObj('a', 0, 0, 10, 10)]);
    s().select(['a']);
    await registry.run('edit.copy');
    await registry.run('edit.paste');
    expect(s().doc.objects).toHaveLength(2);
    expect(s().doc.objects[1]!.layer).toBe('#ff0000');
  });

  it('pega un SVG de otra aplicación: capas nuevas, agrupado, centrado en la vista y un solo paso', async () => {
    clip = svg;
    s().setViewport(800, 600);
    await registry.run('edit.paste');
    const { doc } = s();
    expect(doc.objects).toHaveLength(2);
    expect(doc.objects.every((o) => o.layer === '#0000ff')).toBe(true);
    expect(doc.layers.some((l) => l.color === '#0000ff')).toBe(true);
    expect(doc.objects[0]!.group).not.toBeNull();
    expect(doc.objects[0]!.group).toBe(doc.objects[1]!.group);
    expect(s().selectedIds).toHaveLength(2);
    const union = [objectBounds(doc.objects[0]!)!, objectBounds(doc.objects[1]!)!];
    expect(union[0][1][0] - union[0][0][0]).toBeCloseTo(40, 4);
    expect(s().past).toHaveLength(1);
    s().undo();
    expect(s().doc.objects).toHaveLength(0);
    expect(s().doc.layers).toHaveLength(0);
  });

  it('un texto cualquiera se pega como objeto de texto', async () => {
    clip = 'Hola láser';
    await registry.run('edit.paste');
    const o = s().doc.objects[0]!;
    expect(o.kind.type === 'text' && o.kind.text).toBe('Hola láser');
  });

  it('sin nada que pegar avisa', async () => {
    await registry.run('edit.paste');
    expect(s().doc.objects).toHaveLength(0);
    expect(s().notices.at(-1)?.text).toBeTruthy();
  });

  it('si el sistema no deja escribir, "copiar como SVG" avisa pero la copia interna queda', async () => {
    vi.stubGlobal('navigator', { clipboard: { writeText: async () => Promise.reject(new Error('no')), readText: async () => '' } });
    load([rectObj('a', 0, 0, 10, 10)]);
    s().select(['a']);
    await registry.run('edit.copySvg');
    expect(s().clipboard?.objects).toHaveLength(1);
    expect(s().notices.at(-1)?.level).toBe('warning');
  });
});
