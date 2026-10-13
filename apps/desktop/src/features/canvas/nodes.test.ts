import { beforeEach, describe, expect, it } from 'vitest';
import type { Affine, DesignObject, Point } from '@/api/types';
import { ALL_COMMANDS } from '@/commands';
import { CommandRegistry } from '@/commands/registry';
import { ellipseBez, polylineBez } from '@/lib/bezpath';
import { screenToWorld, type ViewTransform } from '@/lib/viewport';
import { createAppStore } from '@/store';
import { clearGesture, patchInteraction, useInteraction } from './interaction';
import {
  nodeOpAvailability, nodeTargets, nudgeNodes, selectedNodeCenter, setNodeCoord, type NodeOp,
} from './nodeActions';
import { applyPoint, nodeAt, nodeCount, parseModel, positionKey } from './nodeedit';
import { createNodeTool, type Mods } from './nodeTool';
import { modelOf } from './nodeview';
import { newVectorObject, rectPoints } from './shapes';
import { createTextAt, defaultFontFamily, editText, newTextObject } from './textTool';

let store: ReturnType<typeof createAppStore>;
let registry: CommandRegistry;
const st = () => store.getState();

const square = (id = 'a', transform?: Affine): DesignObject => ({
  ...newVectorObject(id, '#ff0000', [polylineBez(rectPoints(0, 0, 10, 10), true)]),
  ...(transform ? { transform } : {}),
});

/** Prepara el store: un objeto vectorial seleccionado con la herramienta de nodos. */
function setup(obj: DesignObject = square()) {
  st().addObjects([obj]);
  st().select([obj.id]);
  st().setTool('nodes');
}
const nodes = (id = 'a') => {
  const kind = st().doc.objects.find((o) => o.id === id)!.kind;
  return parseModel(kind.type === 'vector' ? kind.paths : []);
};

beforeEach(() => {
  store = createAppStore();
  registry = new CommandRegistry(ALL_COMMANDS, () => store.getState());
  clearGesture();
  patchInteraction({ nodeHover: null });
});

describe('selección de nodos en el store', () => {
  it('guarda los nodos por objeto y los descarta al cambiar la selección de objetos o salir de la herramienta', () => {
    setup();
    st().setNodeSel({ a: [0, 2] });
    expect(st().nodeSel).toEqual({ a: [0, 2] });
    st().select([]);
    expect(st().nodeSel).toEqual({});
    st().select(['a']);
    st().setNodeSel({ a: [1] });
    st().setTool('select');
    expect(st().nodeSel).toEqual({});
    expect(st().selectedIds).toEqual(['a']); // el objeto sigue elegido
  });

  it('un conjunto vacío no deja entradas', () => {
    setup();
    st().setNodeSel({ a: [] });
    expect(st().nodeSel).toEqual({});
  });
});

describe('comandos de nodos y atajos', () => {
  it('cada operación tiene su tecla, sin repetirse con otro comando, y solo vale con la herramienta de nodos', () => {
    expect(registry.match('I')?.id).toBe('nodes.insert');
    expect(registry.match('B')?.id).toBe('nodes.break');
    expect(registry.match('J')?.id).toBe('nodes.join');
    expect(registry.match('O')?.id).toBe('nodes.close');
    expect(registry.match('Shift+L')?.id).toBe('nodes.toLine');
    expect(registry.match('Shift+U')?.id).toBe('nodes.toCurve');
    expect(registry.match('Shift+E')?.id).toBe('nodes.corner');
    expect(registry.match('Shift+S')?.id).toBe('nodes.smooth');
    expect(registry.match('Shift+M')?.id).toBe('nodes.symmetric');
    // las teclas de herramienta y de edición de siempre siguen donde estaban
    expect(registry.match('N')?.id).toBe('tool.nodes');
    expect(registry.match('T')?.id).toBe('tool.text');
    expect(registry.match('Delete')?.id).toBe('edit.delete');
    st().addObjects([square()]);
    st().select(['a']);
    expect(registry.availability('nodes.insert')).toBe('nodes.reason.notTool');
  });

  it('los comandos explican por qué no están disponibles', () => {
    setup();
    expect(registry.availability('nodes.insert')).toBe('nodes.reason.noNodes');
    st().setNodeSel({ a: [0] });
    expect(registry.availability('nodes.insert')).toBe('nodes.reason.needSegment');
    expect(registry.availability('nodes.toLine')).toBe(true); // con un nodo, sus dos segmentos
    expect(registry.availability('nodes.break')).toBe(true); // el cuadrado es cerrado
    expect(registry.availability('nodes.join')).toBe('nodes.reason.needEnds');
    expect(registry.availability('nodes.close')).toBe('nodes.reason.noOpen');
    st().setNodeSel({ a: [0, 1] });
    expect(registry.availability('nodes.insert')).toBe(true);
  });

  it('insertar en el punto medio, romper, unir y cerrar por el registro', async () => {
    setup();
    st().setNodeSel({ a: [0, 1] });
    await registry.run('nodes.insert');
    expect(nodeCount(nodes())).toBe(5);
    expect(st().nodeSel.a).toHaveLength(1);
    expect(nodeAt(nodes(), st().nodeSel.a[0])!.p).toEqual([5, 0]);

    // romper en el nodo nuevo: el cerrado se abre ahí
    await registry.run('nodes.break');
    expect(nodes().subs[0].closed).toBe(false);
    expect(nodeCount(nodes())).toBe(6);

    // unir los dos extremos que se tocan: queda un trayecto abierto con el nodo fundido
    const ends = [0, nodeCount(nodes()) - 1];
    st().setNodeSel({ a: ends });
    expect(registry.availability('nodes.join')).toBe(true);
    await registry.run('nodes.join');
    expect(nodes().subs[0].closed).toBe(true);
    expect(nodeCount(nodes())).toBe(5);
  });

  it('esquina, suave y simétrico cambian el tipo y se deshacen de una vez', async () => {
    setup(newVectorObject('e', '#ff0000', [ellipseBez(0, 0, 10, 10)]));
    st().select(['e']);
    st().setNodeSel({ e: [0] });
    const before = st().past.length;
    await registry.run('nodes.corner');
    // sigue alineado geométricamente, pero se recuerda como esquina
    expect(st().nodeCorners.e).toEqual([positionKey(nodeAt(nodes('e'), 0)!.p)]);
    await registry.run('nodes.smooth');
    expect(st().nodeCorners.e ?? []).toEqual([]);
    expect(st().past.length).toBe(before + 2);
  });

  it('segmento a recta y a curva', async () => {
    setup();
    st().setNodeSel({ a: [0, 1] });
    await registry.run('nodes.toCurve');
    const path = (st().doc.objects[0].kind as { paths: { type: string }[][] }).paths[0];
    expect(path[1].type).toBe('cubic_to');
    await registry.run('nodes.toLine');
    expect((st().doc.objects[0].kind as { paths: { type: string }[][] }).paths[0][1].type).toBe('line_to');
  });

  it('borrar todos los nodos de un objeto lo elimina y suelta la selección', async () => {
    setup();
    st().setNodeSel({ a: [0, 1, 2, 3] });
    await registry.run('nodes.delete');
    expect(st().doc.objects).toHaveLength(0);
    expect(st().selectedIds).toEqual([]);
    expect(st().nodeSel).toEqual({});
  });
});

describe('deshacer y rehacer con nodos seleccionados (queja del foro)', () => {
  it('deshacer y rehacer funcionan con nodos elegidos y no piden quitar la selección', async () => {
    setup();
    st().clearHistory();
    st().setNodeSel({ a: [0, 1] });
    expect(registry.availability('edit.undo')).toBe('reason.nothingToUndo');
    await registry.run('nodes.insert');
    expect(nodeCount(nodes())).toBe(5);
    expect(registry.availability('edit.undo')).toBe(true); // nada lo bloquea
    expect(registry.match('Mod+Z')?.id).toBe('edit.undo');
    await registry.run('edit.undo');
    expect(nodeCount(nodes())).toBe(4);
    expect(registry.availability('edit.redo')).toBe(true);
    await registry.run('edit.redo');
    expect(nodeCount(nodes())).toBe(5);
  });

  it('tras deshacer, la selección de nodos queda dentro del objeto (sin índices huérfanos)', async () => {
    setup();
    st().setNodeSel({ a: [0, 1] });
    await registry.run('nodes.insert'); // selecciona el nodo 1 de 5
    st().setNodeSel({ a: [4] });
    await registry.run('edit.undo'); // vuelve a 4 nodos: el índice 4 ya no existe
    st().pruneSelection();
    expect(st().nodeSel).toEqual({});
    expect(st().selectedIds).toEqual(['a']);
  });

  it('deshacer un borrado de objeto con nodos elegidos no deja nodos fantasma', async () => {
    setup();
    st().setNodeSel({ a: [0, 1, 2, 3] });
    await registry.run('nodes.delete');
    expect(st().doc.objects).toHaveLength(0);
    await registry.run('edit.undo');
    expect(st().doc.objects).toHaveLength(1);
    expect(st().nodeSel).toEqual({}); // sin selección de nodos pendiente de un objeto que no estaba elegido
  });

  it('una ráfaga de flechas es un solo paso y se deshace de golpe', () => {
    setup();
    st().setNodeSel({ a: [2] });
    const before = st().past.length;
    for (let i = 0; i < 5; i++) nudgeNodes(st(), [1, 0], 'nodenudge:ArrowRight:normal:a');
    expect(st().past.length).toBe(before + 1);
    expect(nodeAt(nodes(), 2)!.p).toEqual([15, 10]);
    st().undo();
    expect(nodeAt(nodes(), 2)!.p).toEqual([10, 10]);
  });

  it('cada operación de nodos es un solo paso de historial con su etiqueta', async () => {
    setup();
    st().setNodeSel({ a: [0, 1] });
    await registry.run('nodes.insert');
    const last = st().past[st().past.length - 1]!;
    expect(last.label).toBe('nodes.history.insert');
  });
});

describe('editar en coordenadas locales conservando el transform', () => {
  const T: Affine = [2 * Math.cos(Math.PI / 6), 2 * Math.sin(Math.PI / 6), -3 * Math.sin(Math.PI / 6), 3 * Math.cos(Math.PI / 6), 40, 25];

  it('X e Y exactas: el nodo cae en el punto de documento pedido y el objeto conserva su transform', () => {
    setup(square('a', T));
    st().setNodeSel({ a: [2] });
    const before = applyPoint(T, [10, 10]);
    expect(selectedNodeCenter(st())![0]).toBeCloseTo(before[0], 9);
    setNodeCoord(st(), 'x', 55.5);
    setNodeCoord(st(), 'y', 31.25);
    const obj = st().doc.objects[0];
    expect(obj.transform).toEqual(T);
    const doc = applyPoint(obj.transform, nodeAt(modelOf(obj)!, 2)!.p);
    expect(doc[0]).toBeCloseTo(55.5, 9);
    expect(doc[1]).toBeCloseTo(31.25, 9);
    // los otros tres nodos no se movieron
    const other = applyPoint(obj.transform, nodeAt(modelOf(obj)!, 0)!.p);
    expect(other[0]).toBeCloseTo(40, 9);
    expect(other[1]).toBeCloseTo(25, 9);
  });

  it('las flechas mueven en milímetros de documento aunque el objeto esté rotado y escalado', () => {
    setup(square('a', T));
    st().setNodeSel({ a: [0] });
    nudgeNodes(st(), [1, 0], 'k1');
    const obj = st().doc.objects[0];
    const p = applyPoint(obj.transform, nodeAt(modelOf(obj)!, 0)!.p);
    expect(p[0]).toBeCloseTo(41, 9);
    expect(p[1]).toBeCloseTo(25, 9);
  });

  it('con varios nodos, X e Y son el centro de su caja y mueven todos juntos', () => {
    setup();
    st().setNodeSel({ a: [0, 2] });
    expect(selectedNodeCenter(st())).toEqual([5, 5]);
    setNodeCoord(st(), 'x', 20);
    expect(nodeAt(nodes(), 0)!.p).toEqual([15, 0]);
    expect(nodeAt(nodes(), 2)!.p).toEqual([25, 10]);
  });

  it('un objeto con transform singular no se edita (no se puede invertir)', () => {
    setup(square('a', [0, 0, 0, 0, 1, 1]));
    st().setNodeSel({ a: [0] });
    expect(nodeTargets(st())).toEqual([]);
    expect(nodeOpAvailability(st(), 'delete' as NodeOp)).toBe('nodes.reason.noNodes');
  });

  it('los textos y las imágenes no tienen nodos editables', () => {
    st().addObjects([newTextObject('t', '#ff0000', [0, 0])]);
    st().select(['t']);
    st().setTool('nodes');
    st().setNodeSel({ t: [0] });
    expect(nodeTargets(st())).toEqual([]);
  });
});

// ---- gestos con el ratón ------------------------------------------------------------------------------------------------------

const VIEW: ViewTransform = { zoom: 1, panX: 0, panY: 200 };
const NOMODS: Mods = { shift: false, alt: false, ctrl: false };
const toScreen = (p: Point): Point => [p[0] * VIEW.zoom + VIEW.panX, VIEW.panY - p[1] * VIEW.zoom];
const ev = (id = 1) => ({ pointerId: id }) as PointerEvent;

function tool() {
  return createNodeTool({
    el: {} as HTMLElement,
    st,
    viewOf: () => VIEW,
    world: (sx, sy) => screenToWorld(VIEW, [sx, sy]),
    capture: () => {},
    release: () => {},
    snapOpts: () => null,
    targetsFor: () => ({ xs: [], ys: [] }),
    setTip: () => {},
    pick: () => null,
  });
}

/** Arrastra de un punto de documento a otro, con un paso intermedio. */
function drag(tl: ReturnType<typeof tool>, from: Point, to: Point, mods: Mods = NOMODS) {
  const [fx, fy] = toScreen(from);
  const [tx, ty] = toScreen(to);
  tl.down(ev(), fx, fy, mods);
  tl.move(ev(), (fx + tx) / 2, (fy + ty) / 2, mods);
  tl.move(ev(), tx, ty, mods);
  tl.up(ev(), tx, ty, mods);
}

describe('herramienta de nodos con el ratón', () => {
  // cuadrado ampliado 2x y desplazado: nodos en (20,20) (40,20) (40,40) (20,40)
  const T: Affine = [2, 0, 0, 2, 20, 20];
  beforeEach(() => setup(square('a', T)));

  it('un clic elige un nodo; Mayús+clic suma; otro clic en uno de varios deja solo ese', () => {
    const tl = tool();
    const click = (p: Point, mods = NOMODS) => {
      const [x, y] = toScreen(p);
      tl.down(ev(), x, y, mods);
      tl.up(ev(), x, y, mods);
    };
    click([20, 20]);
    expect(st().nodeSel).toEqual({ a: [0] });
    click([40, 40], { ...NOMODS, shift: true });
    expect(st().nodeSel).toEqual({ a: [0, 2] });
    click([40, 40], { ...NOMODS, shift: true }); // Mayús sobre uno elegido lo quita
    expect(st().nodeSel).toEqual({ a: [0] });
    click([40, 40]);
    click([20, 20], { ...NOMODS, shift: true });
    click([20, 20]); // sin Mayús, entre varios: se queda solo con ese
    expect(st().nodeSel).toEqual({ a: [0] });
    click([200, 200]); // en vacío, sin Mayús: quita la selección de nodos
    expect(st().nodeSel).toEqual({});
    expect(st().selectedIds).toEqual(['a']);
  });

  it('un clic en un segmento elige sus dos nodos', () => {
    const tl = tool();
    const [x, y] = toScreen([30, 20]);
    tl.down(ev(), x, y, NOMODS);
    tl.up(ev(), x, y, NOMODS);
    expect(st().nodeSel).toEqual({ a: [0, 1] });
  });

  it('arrastrar un nodo lo mueve en coordenadas locales y es un solo paso de deshacer', () => {
    const tl = tool();
    const before = st().past.length;
    drag(tl, [20, 20], [30, 30]);
    expect(st().past.length).toBe(before + 1);
    expect(st().past[st().past.length - 1]!.label).toBe('nodes.history.move');
    const obj = st().doc.objects[0];
    expect(obj.transform).toEqual(T);
    expect(nodeAt(modelOf(obj)!, 0)!.p).toEqual([5, 5]); // (30 - 20) / 2
    expect(useInteraction.getState().nodePreview).toBeNull();
    st().undo();
    expect(nodeAt(modelOf(st().doc.objects[0])!, 0)!.p).toEqual([0, 0]);
  });

  it('durante el arrastre solo hay una vista previa: el documento no cambia hasta soltar', () => {
    const tl = tool();
    const [fx, fy] = toScreen([20, 20]);
    const [tx, ty] = toScreen([30, 30]);
    const docBefore = st().doc;
    tl.down(ev(), fx, fy, NOMODS);
    tl.move(ev(), tx, ty, NOMODS);
    expect(st().doc).toBe(docBefore);
    expect(useInteraction.getState().nodePreview?.a).toBeDefined();
    expect(useInteraction.getState().gesture).toBe('nodes');
    tl.cancel();
    clearGesture();
    expect(st().doc).toBe(docBefore);
    expect(useInteraction.getState().nodePreview).toBeNull();
  });

  it('Mayús restringe el arrastre a horizontal o vertical', () => {
    drag(tool(), [20, 20], [34, 23], { ...NOMODS, shift: true });
    expect(nodeAt(modelOf(st().doc.objects[0])!, 0)!.p).toEqual([7, 0]);
  });

  it('arrastrar un nodo elegido mueve todos los elegidos', () => {
    st().setNodeSel({ a: [0, 1] });
    drag(tool(), [20, 20], [24, 20]);
    const m = modelOf(st().doc.objects[0])!;
    expect(nodeAt(m, 0)!.p).toEqual([2, 0]);
    expect(nodeAt(m, 1)!.p).toEqual([12, 0]);
    expect(nodeAt(m, 2)!.p).toEqual([10, 10]);
  });

  it('la caja de nodos elige los que caen dentro', () => {
    const tl = tool();
    const [ax, ay] = toScreen([10, 10]);
    const [bx, by] = toScreen([45, 25]);
    tl.down(ev(), ax, ay, NOMODS);
    tl.move(ev(), bx, by, NOMODS);
    tl.up(ev(), bx, by, NOMODS);
    expect(st().nodeSel).toEqual({ a: [0, 1] });
    // con Mayús se suma a lo que ya había
    const [cx, cy] = toScreen([35, 35]);
    const [dx, dy] = toScreen([45, 45]);
    tl.down(ev(), cx, cy, { ...NOMODS, shift: true });
    tl.move(ev(), dx, dy, { ...NOMODS, shift: true });
    tl.up(ev(), dx, dy, { ...NOMODS, shift: true });
    expect(st().nodeSel).toEqual({ a: [0, 1, 2] });
  });

  it('doble clic en un segmento agrega un nodo sin cambiar la forma; en un nodo no hace nada', () => {
    const tl = tool();
    const [nx, ny] = toScreen([20, 20]);
    expect(tl.doubleClick(nx, ny)).toBe(false);
    const [x, y] = toScreen([30, 20]);
    expect(tl.doubleClick(x, y)).toBe(true);
    const m = modelOf(st().doc.objects[0])!;
    expect(nodeCount(m)).toBe(5);
    expect(nodeAt(m, 1)!.p).toEqual([5, 0]);
    expect(st().nodeSel).toEqual({ a: [1] });
    expect(st().past[st().past.length - 1]!.label).toBe('nodes.history.insert');
    expect(tl.doubleClick(...toScreen([500, 500]))).toBe(false);
  });

  it('mover un manejador: en un nodo suave el otro lo acompaña; con Alt, no', () => {
    const e = newVectorObject('e', '#ff0000', [ellipseBez(30, 30, 10, 10)]);
    st().addObjects([e]);
    st().select(['e']);
    st().setNodeSel({ e: [0] }); // el nodo de la derecha (40, 30): manejadores arriba y abajo
    const tl = tool();
    const m0 = modelOf(st().doc.objects.find((o) => o.id === 'e')!)!;
    const n0 = nodeAt(m0, 0)!;
    const grab = n0.outH!; // (40, 35.52)
    drag(tl, grab, [43, grab[1] + 2]);
    const m1 = modelOf(st().doc.objects.find((o) => o.id === 'e')!)!;
    const n1 = nodeAt(m1, 0)!;
    expect(n1.outH![0]).toBeCloseTo(43, 6);
    expect(n1.inH![0]).toBeCloseTo(37, 6); // simétrico: el otro refleja al nodo
    // con Alt solo se mueve ese manejador y el nodo pasa a esquina
    st().undo();
    drag(tool(), grab, [43, grab[1] + 2], { ...NOMODS, alt: true });
    const n2 = nodeAt(modelOf(st().doc.objects.find((o) => o.id === 'e')!)!, 0)!;
    expect(n2.outH![0]).toBeCloseTo(43, 6);
    expect(n2.inH).toEqual(n0.inH);
    expect(st().nodeCorners.e).toEqual([positionKey(n0.p)]);
  });

  it('Supr borra los nodos elegidos y no el objeto; las flechas los mueven (comandos de edición)', async () => {
    st().setNodeSel({ a: [2] });
    expect(registry.match('Shift+ArrowRight')?.id).toBe('edit.nudge.right.big');
    await registry.run('edit.nudge.right.big');
    expect(nodeAt(modelOf(st().doc.objects[0])!, 2)!.p).toEqual([15, 10]); // 10 mm de documento = 5 locales en X
    expect(st().doc.objects[0]!.transform).toEqual([2, 0, 0, 2, 20, 20]); // el objeto no se movió
    expect(registry.availability('edit.delete')).toBe(true);
    await registry.run('edit.delete');
    expect(st().doc.objects).toHaveLength(1);
    expect(nodeCount(modelOf(st().doc.objects[0])!)).toBe(3);
    // sin nodos elegidos los mismos comandos borran/mueven el objeto entero
    const before = st().doc.objects[0]!.transform;
    await registry.run('edit.nudge.left');
    expect(st().doc.objects[0]!.transform[4]).toBe(before[4] - 1);
    await registry.run('edit.delete');
    expect(st().doc.objects).toHaveLength(0);
  });

  it('fuera de la herramienta de nodos, Supr y las flechas actúan sobre el objeto aunque haya nodos elegidos', async () => {
    st().setTool('select');
    st().setNodeSel({ a: [2] });
    await registry.run('edit.delete');
    expect(st().doc.objects).toHaveLength(0);
  });

  it('Esc vuelve a Seleccionar sin perder el objeto', async () => {
    st().setNodeSel({ a: [0] });
    await registry.run('tool.select');
    expect(st().activeTool).toBe('select');
    expect(st().selectedIds).toEqual(['a']);
    expect(st().nodeSel).toEqual({});
  });

  it('un clic en otro objeto con la herramienta de nodos pasa a editar el suyo', () => {
    const other = square('b', [1, 0, 0, 1, 100, 100]);
    st().addObjects([other]);
    st().select(['a']);
    const tl = createNodeTool({
      el: {} as HTMLElement, st, viewOf: () => VIEW, world: (sx, sy) => screenToWorld(VIEW, [sx, sy]),
      capture: () => {}, release: () => {}, snapOpts: () => null, targetsFor: () => ({ xs: [], ys: [] }), setTip: () => {},
      pick: (p) => (p[0] >= 100 && p[0] <= 110 && p[1] >= 100 && p[1] <= 110 ? other : null),
    });
    const [x, y] = toScreen([105, 100]);
    tl.down(ev(), x, y, NOMODS);
    tl.up(ev(), x, y, NOMODS);
    expect(st().selectedIds).toEqual(['b']);
  });
});

describe('herramienta de texto', () => {
  it('crea un texto en el punto, en la capa activa, lo selecciona y pide el foco del panel', () => {
    st().setActiveLayer('#00ff00');
    const tokens = st().textFocusToken;
    const id = createTextAt(st(), [12.5, 30]);
    const obj = st().doc.objects.find((o) => o.id === id)!;
    expect(obj.layer).toBe('#00ff00');
    expect(obj.transform).toEqual([1, 0, 0, 1, 12.5, 30]);
    expect(obj.kind.type).toBe('text');
    if (obj.kind.type === 'text') {
      expect(obj.kind.text).toBe('Texto');
      expect(obj.kind.size_mm).toBe(10);
      expect(obj.kind.outline).toEqual([]); // lo genera el panel de texto
      expect(obj.kind.font_family).toBe('Arial');
    }
    expect(st().selectedIds).toEqual([id]);
    expect(st().textFocusToken).toBe(tokens + 1);
    expect(st().past[st().past.length - 1]!.label).toBe('canvas.draw.text');
    st().undo();
    expect(st().doc.objects).toHaveLength(0);
  });

  it('dos textos reciben ids distintos', () => {
    const a = createTextAt(st(), [0, 0]);
    const b = createTextAt(st(), [5, 5]);
    expect(a).not.toBe(b);
  });

  it('la fuente por defecto es Arial o, si falta, la primera disponible', () => {
    expect(defaultFontFamily(null)).toBe('Arial');
    expect(defaultFontFamily([])).toBe('Arial');
    expect(defaultFontFamily(['Helvetica', 'Arial', 'Zapfino'])).toBe('Arial');
    expect(defaultFontFamily(['Helvetica', 'Zapfino'])).toBe('Helvetica');
  });

  it('editar un texto existente lo selecciona y pide el foco', () => {
    const id = createTextAt(st(), [0, 0]);
    st().select([]);
    const tokens = st().textFocusToken;
    editText(st(), id);
    expect(st().selectedIds).toEqual([id]);
    expect(st().textFocusToken).toBe(tokens + 1);
  });

  it('T elige la herramienta de texto', async () => {
    await registry.run('tool.text');
    expect(st().activeTool).toBe('text');
  });
});
