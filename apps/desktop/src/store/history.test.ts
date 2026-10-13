import { beforeEach, describe, expect, it } from 'vitest';
import type { DesignObject, Document } from '@/api/types';
import { ALL_COMMANDS, CommandRegistry } from '@/commands';
import { makeLayer } from '@/lib/layers';
import { createAppStore } from './index';
import { entryCost, HISTORY_LIMIT, HISTORY_POINT_BUDGET, trimHistory, type HistoryEntry } from './document';
import { isSticky, MAX_NOTICES, MAX_STICKY } from './notices';
import { bezPointCount, polylineBez } from '@/lib/bezpath';

let store: ReturnType<typeof createAppStore>;
let registry: CommandRegistry;
const s = () => store.getState();

function vector(id: string, layer: string, x: number, points = 4): DesignObject {
  return {
    id,
    layer,
    transform: [1, 0, 0, 1, x, 0],
    kind: { type: 'vector', paths: [polylineBez(Array.from({ length: points }, (_, i) => [i, i % 2] as [number, number]), true)] },
    locked: false,
    hidden: false,
    group: null,
  };
}

function load(objects: DesignObject[], layers: string[] = ['#ff0000']) {
  s().applyImport({
    objects,
    new_layers: layers.map((c, i) => makeLayer(c, `Capa ${i}`)),
    warnings: [],
  });
}

beforeEach(() => {
  store = createAppStore();
  registry = new CommandRegistry(ALL_COMMANDS, () => store.getState());
});

describe('portapapeles', () => {
  it('copia y pega con ids nuevos, desplazado y conservando la capa', async () => {
    load([vector('a', '#ff0000', 10), vector('b', '#00ff00', 20)], ['#ff0000', '#00ff00']);
    s().select(['a', 'b']);
    await registry.run('edit.copy');
    expect(s().clipboard?.objects).toHaveLength(2);
    await registry.run('edit.paste');
    const objs = s().doc.objects;
    expect(objs).toHaveLength(4);
    const pasted = objs.slice(2);
    expect(new Set(objs.map((o) => o.id)).size).toBe(4); // ids únicos
    expect(pasted.map((o) => o.id)).not.toContain('a');
    expect(pasted.map((o) => o.layer)).toEqual(['#ff0000', '#00ff00']);
    expect(pasted[0]!.transform[4]).toBe(15); // 5 mm a la derecha
    expect(pasted[0]!.transform[5]).toBe(5);
    expect(s().selectedIds).toEqual(pasted.map((o) => o.id)); // lo pegado queda seleccionado
  });

  it('cada pegado se desplaza un poco más; pegar en sitio no', async () => {
    load([vector('a', '#ff0000', 0)]);
    s().select(['a']);
    await registry.run('edit.copy');
    await registry.run('edit.paste');
    await registry.run('edit.paste');
    await registry.run('edit.pasteInPlace');
    const xs = s().doc.objects.map((o) => o.transform[4]);
    expect(xs).toEqual([0, 5, 10, 0]);
  });

  it('pegar es un solo paso de deshacer, incluida la capa que faltaba', async () => {
    load([vector('a', '#ff0000', 0)]);
    s().select(['a']);
    await registry.run('edit.copy');
    // Un proyecto nuevo no tiene la capa roja: el pegado la recrea con sus ajustes.
    s().updateLayer('#ff0000', { speed: 777 });
    s().select(['a']);
    await registry.run('edit.copy');
    s().newDocument();
    expect(s().doc.layers).toHaveLength(0);
    const before = s().past.length;
    await registry.run('edit.paste');
    expect(s().past.length - before).toBe(1);
    expect(s().doc.layers.find((l) => l.color === '#ff0000')?.speed).toBe(777);
    await registry.run('edit.undo');
    expect(s().doc.objects).toHaveLength(0);
    expect(s().doc.layers).toHaveLength(0);
  });

  it('cortar quita lo cortado, no corta lo bloqueado y el primer pegado vuelve al mismo lugar', async () => {
    load([vector('a', '#ff0000', 10), vector('b', '#ff0000', 30)]);
    s().setObjectFlags(['b'], { locked: true });
    s().select(['a', 'b']);
    await registry.run('edit.cut');
    expect(s().doc.objects.map((o) => o.id)).toEqual(['b']);
    expect(s().clipboard?.objects.map((o) => o.id)).toEqual(['a']);
    await registry.run('edit.paste');
    expect(s().doc.objects.find((o) => o.id !== 'b')!.transform[4]).toBe(10);
  });

  it('explica por qué no se puede pegar ni cortar', async () => {
    expect(registry.availability('edit.paste')).toBe('history.reason.clipboardEmpty');
    expect(registry.availability('edit.cut')).toBe('reason.noSelection');
    expect(await registry.run('edit.paste')).toBe(false);
  });

  it('invierte la selección sin incluir lo oculto ni lo bloqueado', async () => {
    load([vector('a', '#ff0000', 0), vector('b', '#ff0000', 0), vector('c', '#ff0000', 0), vector('d', '#ff0000', 0)]);
    s().setObjectFlags(['d'], { hidden: true });
    s().select(['a']);
    await registry.run('edit.selectInvert');
    expect(s().selectedIds.sort()).toEqual(['b', 'c']);
  });
});

describe('empujar con flechas', () => {
  it('una ráfaga es un solo paso y los pasos grande y fino mueven 10 y 0,1 mm', async () => {
    load([vector('a', '#ff0000', 10)]);
    s().select(['a']);
    const base = s().past.length;
    for (let i = 0; i < 5; i++) await registry.run('edit.nudge.right');
    expect(s().past.length - base).toBe(1);
    expect(s().doc.objects[0]!.transform[4]).toBe(15);
    for (let i = 0; i < 3; i++) await registry.run('edit.nudge.up.fine');
    expect(s().doc.objects[0]!.transform[5]).toBe(0.3); // sin ruido de coma flotante
    await registry.run('edit.nudge.left.big');
    expect(s().doc.objects[0]!.transform[4]).toBe(5);
    await registry.run('edit.undo');
    await registry.run('edit.undo');
    await registry.run('edit.undo');
    expect(s().doc.objects[0]!.transform).toEqual([1, 0, 0, 1, 10, 0]);
  });

  it('empujar objetos distintos no se funde en el mismo paso', async () => {
    load([vector('a', '#ff0000', 0), vector('b', '#ff0000', 0)]);
    const base = s().past.length;
    s().select(['a']);
    await registry.run('edit.nudge.right');
    s().select(['b']);
    await registry.run('edit.nudge.right');
    expect(s().past.length - base).toBe(2);
  });
});

describe('saltos en el historial', () => {
  function threeSteps() {
    load([vector('a', '#ff0000', 0)]); // paso 1
    s().moveObjects(['a'], 10, 0, 'k1'); // paso 2
    s().moveObjects(['a'], 10, 0, 'k2'); // paso 3
  }
  const x = () => s().doc.objects[0]!.transform[4];

  it('salta hacia atrás y hacia delante conservando los pasos', () => {
    threeSteps();
    expect(s().past).toHaveLength(3);
    s().jumpTo(1);
    expect(x()).toBe(0);
    expect(s().past).toHaveLength(1);
    expect(s().future).toHaveLength(2);
    s().jumpTo(3);
    expect(x()).toBe(20);
    expect(s().future).toHaveLength(0);
    s().jumpTo(2);
    expect(x()).toBe(10);
    s().redo();
    expect(x()).toBe(20);
  });

  it('el punto 0 es el estado inicial y se limita a los extremos', () => {
    threeSteps();
    s().jumpTo(0);
    expect(s().doc.objects).toHaveLength(0);
    s().jumpTo(99);
    expect(x()).toBe(20);
    s().jumpTo(-4);
    expect(s().doc.objects).toHaveLength(0);
  });

  it('un salto equivale a deshacer o rehacer paso a paso', () => {
    threeSteps();
    const viaUndo = createAppStore();
    // mismo escenario, deshaciendo dos veces
    viaUndo.getState().applyImport({ objects: [vector('a', '#ff0000', 0)], new_layers: [], warnings: [] });
    viaUndo.getState().moveObjects(['a'], 10, 0, 'k1');
    viaUndo.getState().moveObjects(['a'], 10, 0, 'k2');
    viaUndo.getState().undo();
    viaUndo.getState().undo();
    s().jumpTo(1);
    expect(s().doc.objects[0]!.transform).toEqual(viaUndo.getState().doc.objects[0]!.transform);
    expect(s().future.map((e) => e.label)).toEqual(viaUndo.getState().future.map((e) => e.label));
  });

  it('una acción nueva tras saltar descarta lo deshecho', () => {
    threeSteps();
    s().jumpTo(1);
    s().moveObjects(['a'], 1, 0, 'otra');
    expect(s().future).toHaveLength(0);
    expect(s().past).toHaveLength(2);
  });
});

describe('cambios sin guardar por referencia', () => {
  it('deshacer hasta lo guardado deja el proyecto limpio', () => {
    load([vector('a', '#ff0000', 0)]);
    s().markSaved('p.tlaku');
    expect(s().dirty).toBe(false);
    s().moveObjects(['a'], 1, 0, 'k');
    expect(s().dirty).toBe(true);
    s().undo();
    expect(s().dirty).toBe(false);
    s().redo();
    expect(s().dirty).toBe(true);
    s().jumpTo(1);
    expect(s().dirty).toBe(false);
  });

  it('si se edita mientras se guarda, sigue sucio', () => {
    load([vector('a', '#ff0000', 0)]);
    const saved = s().doc;
    s().moveObjects(['a'], 1, 0, 'k');
    s().markSaved('p.tlaku', saved);
    expect(s().filePath).toBe('p.tlaku');
    expect(s().dirty).toBe(true);
  });
});

describe('memoria del historial', () => {
  /** Geometría distinta (por referencia) con los mismos puntos. */
  const reshape = (doc: Document): Document => ({
    ...doc,
    objects: doc.objects.map((o) =>
      o.kind.type === 'vector' ? { ...o, kind: { ...o.kind, paths: o.kind.paths.map((p) => [...p]) } } : o,
    ),
  });

  /** Puntos de geometría realmente retenidos: cada trayectoria compartida se cuenta una sola vez. */
  function retainedPoints(docs: Document[]): number {
    const seen = new Set<object>();
    let n = 0;
    for (const d of docs) {
      for (const o of d.objects) {
        if (o.kind.type !== 'vector' || seen.has(o.kind)) continue;
        seen.add(o.kind);
        n += o.kind.paths.reduce((sum, p) => sum + bezPointCount(p), 0);
      }
    }
    return n;
  }

  it('150 pasos sobre un diseño denso comparten la geometría: costo casi nulo', () => {
    const objects = Array.from({ length: 100 }, (_, i) => vector(`o${i}`, '#ff0000', i, 5000));
    load(objects);
    for (let i = 0; i < 150; i++) s().moveObjects(['o0', 'o1', 'o2'], 1, 0, `k${i}`);
    expect(s().past).toHaveLength(151);
    const docs = [...s().past.map((e) => e.doc), s().doc];
    const retained = retainedPoints(docs);
    const naive = docs.length * 100 * 5000; // si cada paso copiara la geometría
    // Los 3 objetos que se mueven conservan su geometría (kind compartido): solo existen 100 trayectorias.
    expect(retained).toBe(100 * 5000);
    expect(retained / naive).toBeLessThan(0.01);
    const costs = s().past.reduce((sum, e) => sum + (e.cost ?? 0), 0);
    expect(costs).toBeLessThan(HISTORY_POINT_BUDGET / 10);
  });

  it('entryCost solo cuenta la geometría que el documento actual ya no usa', () => {
    load([vector('a', '#ff0000', 0, 1000), vector('b', '#ff0000', 0, 500)]);
    const before = s().doc;
    s().deleteObjects(['a']);
    expect(entryCost(before, s().doc)).toBeGreaterThanOrEqual(1000);
    expect(entryCost(before, s().doc)).toBeLessThan(1010);
    const moved = { ...before, objects: before.objects.map((o) => ({ ...o, transform: [1, 0, 0, 1, 9, 9] as DesignObject['transform'] })) };
    expect(entryCost(before, moved)).toBeLessThan(1); // mover no retiene geometría
  });

  it('acota la memoria cuando las acciones reemplazan mucha geometría', () => {
    load(Array.from({ length: 10 }, (_, i) => vector(`o${i}`, '#ff0000', i, 50_000))); // 500k puntos
    for (let i = 0; i < 12; i++) s().applyDocChange('history.transform', reshape, `r${i}`);
    const docs = [...s().past.map((e) => e.doc), s().doc];
    expect(retainedPoints(docs)).toBeLessThanOrEqual(HISTORY_POINT_BUDGET + 500_000 * 2);
    expect(s().past.length).toBeLessThan(12);
    expect(s().past.length).toBeGreaterThanOrEqual(1); // el último paso siempre se puede deshacer
    s().undo();
    expect(s().doc.objects).toHaveLength(10);
  });

  it('trimHistory respeta el tope de pasos y conserva siempre el más reciente', () => {
    const doc: Document = { version: 2, objects: [], layers: [], device_id: null };
    const mk = (cost: number): HistoryEntry => ({ label: 'history.move', doc, at: 0, cost });
    expect(trimHistory(Array.from({ length: HISTORY_LIMIT + 10 }, () => mk(0)))).toHaveLength(HISTORY_LIMIT);
    expect(trimHistory([mk(0), mk(0), mk(HISTORY_POINT_BUDGET * 3)])).toHaveLength(1);
    expect(trimHistory([mk(10), mk(10)], 200, 15)).toHaveLength(1);
  });

  it('clearHistory libera los pasos sin tocar el documento', () => {
    load([vector('a', '#ff0000', 0)]);
    s().moveObjects(['a'], 1, 0, 'k');
    const doc = s().doc;
    s().clearHistory();
    expect(s().past).toHaveLength(0);
    expect(s().future).toHaveLength(0);
    expect(s().doc).toBe(doc);
  });
});

describe('patchSilently', () => {
  const recolor = (d: Document): Document => ({ ...d, objects: d.objects.map((o) => (o.layer === '#00ff00' ? o : { ...o, layer: '#00ff00' })) });

  it('cambia doc, savedDoc, past y future sin crear un paso ni ensuciar', () => {
    load([vector('a', '#ff0000', 0)]);
    s().markSaved('/x.tlaku');
    s().moveObjects(['a'], 1, 0);
    s().moveObjects(['a'], 2, 0, 'otra');
    s().undo();
    const steps = s().past.length;
    const futureSteps = s().future.length;
    s().patchSilently(recolor);
    expect(s().past).toHaveLength(steps);
    expect(s().future).toHaveLength(futureSteps);
    expect(s().doc.objects[0]!.layer).toBe('#00ff00');
    expect(s().savedDoc.objects[0]!.layer).toBe('#00ff00');
    for (const e of [...s().past, ...s().future]) for (const o of e.doc.objects) expect(o.layer).toBe('#00ff00');
    s().redo();
    expect(s().doc.objects[0]!.layer).toBe('#00ff00');
  });

  it('un documento limpio sigue limpio y uno sucio sigue sucio', () => {
    load([vector('a', '#ff0000', 0)]);
    s().markSaved('/x.tlaku');
    s().patchSilently(recolor);
    expect(s().dirty).toBe(false);
    s().moveObjects(['a'], 1, 0);
    expect(s().dirty).toBe(true);
    s().patchSilently((d) => ({ ...d, device_id: 'zzz' }));
    expect(s().dirty).toBe(true);
    // Si lo guardado también es un paso del historial, deshacer vuelve a un documento limpio.
    s().undo();
    expect(s().dirty).toBe(false);
  });

  it('si change no cambia nada, no toca el estado', () => {
    load([vector('a', '#ff0000', 0)]);
    const before = s();
    s().patchSilently((d) => d);
    expect(s().doc).toBe(before.doc);
    expect(s().past).toBe(before.past);
  });
});

describe('avisos persistentes', () => {
  it('las advertencias y los errores no se van solos ni los desplaza un aviso efímero', () => {
    expect(isSticky('warning')).toBe(true);
    expect(isSticky('error')).toBe(true);
    expect(isSticky('success')).toBe(false);
    s().notify('warning', 'importación', ['Máscara ignorada']);
    for (let i = 0; i < MAX_NOTICES + 3; i++) s().notify('success', `ok ${i}`);
    const kept = s().notices;
    expect(kept.some((n) => n.level === 'warning' && n.details?.[0] === 'Máscara ignorada')).toBe(true);
    expect(kept.filter((n) => !n.sticky)).toHaveLength(MAX_NOTICES);
  });

  it('un aviso idéntico no se apila y el tope de persistentes es finito', () => {
    s().notify('error', 'igual');
    s().notify('error', 'igual');
    expect(s().notices.filter((n) => n.text === 'igual')).toHaveLength(1);
    for (let i = 0; i < MAX_STICKY + 5; i++) s().notify('error', `fallo ${i}`);
    expect(s().notices.filter((n) => n.sticky)).toHaveLength(MAX_STICKY);
  });
});
