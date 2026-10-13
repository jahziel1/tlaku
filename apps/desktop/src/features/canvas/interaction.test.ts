import { beforeEach, describe, expect, it } from 'vitest';
import { makeLayer } from '@/lib/layers';
import { createAppStore } from '@/store';
import { CommandRegistry } from '@/commands/registry';
import { ALL_COMMANDS } from '@/commands';
import { cancelGesture, clearGesture, closePolyline, patchInteraction, polylineInProgress, registerGestureHooks, useInteraction } from './interaction';
import { lineObj, rectObj } from './testUtil';

let store: ReturnType<typeof createAppStore>;

beforeEach(() => {
  store = createAppStore();
  clearGesture();
});

describe('selección y objetos no seleccionables', () => {
  const load = () => {
    store.getState().addObjects([rectObj('a', 0, 0, 10, 10), rectObj('b', 20, 0, 30, 10), lineObj('c', [0, 20], [10, 20])]);
  };

  it('seleccionar todo omite bloqueados, ocultos y capas invisibles', () => {
    load();
    store.getState().setObjectFlags(['b'], { locked: true });
    store.getState().setObjectFlags(['c'], { hidden: true });
    store.getState().selectAll();
    expect(store.getState().selectedIds).toEqual(['a']);
    store.getState().updateLayer('#ff0000', { visible: false });
    store.getState().selectAll();
    expect(store.getState().selectedIds).toEqual([]);
  });

  it('bloquear u ocultar lo seleccionado lo saca de la selección', () => {
    load();
    store.getState().select(['a', 'b']);
    store.getState().setObjectFlags(['b'], { locked: true });
    store.getState().pruneSelection();
    expect(store.getState().selectedIds).toEqual(['a']);
    store.getState().setObjectFlags(['a'], { hidden: true });
    store.getState().pruneSelection();
    expect(store.getState().selectedIds).toEqual([]);
  });

  it('seleccionar una capa compara el color sin distinguir mayúsculas', () => {
    load();
    store.getState().selectLayerObjects('#FF0000');
    expect(store.getState().selectedIds).toEqual(['a', 'b', 'c']);
  });

  it('el ajuste magnético se puede encender y apagar', () => {
    expect(store.getState().snapEnabled).toBe(true);
    store.getState().toggleSnap();
    expect(store.getState().snapEnabled).toBe(false);
    store.getState().toggleSnapGrid();
    expect(store.getState().snapGrid).toBe(false);
    store.getState().toggleSnapObjects();
    expect(store.getState().snapObjects).toBe(false);
  });

  it('el ancla, el candado y el radio tienen valores razonables', () => {
    expect(store.getState().anchor).toBe(6);
    store.getState().setAnchor(99);
    expect(store.getState().anchor).toBe(8);
    store.getState().setCornerRadius(-4);
    expect(store.getState().cornerRadius).toBe(0);
    store.getState().setCornerRadius(2.5);
    expect(store.getState().cornerRadius).toBe(2.5);
  });
});

describe('un gesto es un solo paso de deshacer', () => {
  it('mover al soltar escribe una vez en el historial y se deshace de golpe', () => {
    const s = store.getState();
    s.addObjects([rectObj('a', 0, 0, 10, 10)]);
    const before = store.getState().past.length;
    store.getState().moveObjects(['a'], 12, 3, 'drag:1');
    expect(store.getState().past.length).toBe(before + 1);
    store.getState().undo();
    expect(store.getState().doc.objects[0].transform).toEqual([1, 0, 0, 1, 0, 0]);
  });
});

describe('gestos y comandos', () => {
  it('Esc cancela primero el gesto en curso y solo después quita la selección', async () => {
    const registry = new CommandRegistry(ALL_COMMANDS, () => store.getState());
    store.getState().addObjects([rectObj('a', 0, 0, 10, 10)]);
    store.getState().select(['a']);
    let cancelled = 0;
    const off = registerGestureHooks({ cancel: () => { cancelled++; clearGesture(); } });
    patchInteraction({ gesture: 'move' });
    await registry.run('tool.select');
    expect(cancelled).toBe(1);
    expect(store.getState().selectedIds).toEqual(['a']); // la selección sigue
    await registry.run('tool.select');
    expect(store.getState().selectedIds).toEqual([]); // ahora sí
    off();
  });

  it('cancelGesture dice si había algo que cancelar', () => {
    expect(cancelGesture()).toBe(false);
    registerGestureHooks({ cancel: () => clearGesture() });
    patchInteraction({ gesture: 'draw' });
    expect(cancelGesture()).toBe(true);
    expect(useInteraction.getState().gesture).toBeNull();
  });

  it('"C" solo está disponible con una polilínea en curso', async () => {
    const registry = new CommandRegistry(ALL_COMMANDS, () => store.getState());
    expect(registry.match('C')?.id).toBe('draw.closePath');
    expect(registry.availability('draw.closePath')).toBe('canvas.reason.noPolyline');
    let closed = 0;
    const off = registerGestureHooks({ closePolyline: () => { closed++; } });
    patchInteraction({ gesture: 'polyline' });
    expect(polylineInProgress()).toBe(true);
    expect(registry.availability('draw.closePath')).toBe(true);
    await registry.run('draw.closePath');
    closePolyline();
    expect(closed).toBe(2);
    off();
  });

  it('el atajo S alterna el ajuste magnético y no choca con otros', async () => {
    const registry = new CommandRegistry(ALL_COMMANDS, () => store.getState());
    expect(registry.match('S')?.id).toBe('view.snap');
    await registry.run('view.snap');
    expect(store.getState().snapEnabled).toBe(false);
    expect(registry.checked('view.snap')).toBe(false);
    // Con el ajuste apagado, sus variantes explican por qué no se pueden usar.
    expect(registry.availability('view.snapGrid')).toBe('canvas.reason.snapOff');
  });

  it('las herramientas de dibujo mantienen sus teclas', () => {
    const registry = new CommandRegistry(ALL_COMMANDS, () => store.getState());
    expect(registry.match('R')?.id).toBe('tool.rect');
    expect(registry.match('E')?.id).toBe('tool.ellipse');
    expect(registry.match('L')?.id).toBe('tool.line');
  });
});

describe('capa activa', () => {
  it('addObjects crea la capa si no existe', () => {
    store.getState().addObjects([rectObj('a', 0, 0, 1, 1)]);
    expect(store.getState().doc.layers.map((l) => l.color)).toContain('#ff0000');
    expect(makeLayer('#ff0000', 'x').mode).toBe('line');
  });
});
