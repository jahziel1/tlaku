import { describe, expect, it } from 'vitest';
import type { BezPath, DesignObject, Document } from '@/api/types';
import { bezPointCount } from '@/lib/bezpath';
import { createAppStore } from './index';
import { emptyDocument, entryCost, type HistoryEntry } from './document';
import { INACTIVE_HISTORY_STEPS, INACTIVE_POINT_BUDGET, type TabData } from './tabs';

/**
 * Memoria del historial con varias pestañas. Cada paso de deshacer retiene la geometría que la acción
 * reemplazó, así que N pestañas multiplican lo que se guarda; por eso las inactivas conservan menos pasos
 * (`INACTIVE_HISTORY_STEPS`, `INACTIVE_POINT_BUDGET`). Esta prueba arma documentos grandes, los edita mucho y
 * mide, sin depender del recolector de basura, cuántos puntos de geometría siguen alcanzables desde cada
 * pestaña (recorriendo documento, lo guardado y los pasos; la geometría compartida entre pasos cuenta una vez).
 * Si Node corre con `--expose-gc` (`NODE_OPTIONS=--expose-gc npx vitest run src/store/tabsMemory.test.ts`),
 * además imprime el montón real (`heapUsed`) como referencia.
 */

const POINTS_PER_OBJECT = 40;
const OBJECTS = 6000;
const EDITS = 150;
const PER_EDIT = 80;
/** Bytes que ocupa un punto de una trayectoria en el motor de JS, medido con este mismo documento (ver el informe). */
const BYTES_PER_POINT = 120;

const path = (seed: number): BezPath => [
  { type: 'move_to', to: [seed, seed] },
  ...Array.from({ length: POINTS_PER_OBJECT - 1 }, (_, i) => ({ type: 'line_to' as const, to: [seed + i * 0.01, seed - i * 0.02] as [number, number] })),
  { type: 'close' },
];
const vector = (i: number, seed = i): DesignObject => ({
  id: `o${i}`, layer: '#ff0000', transform: [1, 0, 0, 1, 0, 0], locked: false, hidden: false, group: null,
  kind: { type: 'vector', paths: [path(seed)] },
});
const bigDoc = (): Document => ({ ...emptyDocument(), objects: Array.from({ length: OBJECTS }, (_, i) => vector(i)) });

/** Una edición que reemplaza la geometría de `PER_EDIT` objetos (como un recorte o una booleana). */
function editGeometry(store: ReturnType<typeof createAppStore>, round: number): void {
  const from = (round * PER_EDIT) % (OBJECTS - PER_EDIT);
  const ids = new Set(Array.from({ length: PER_EDIT }, (_, k) => `o${from + k}`));
  store.getState().applyDocChange('history.editObjects', (doc) => ({
    ...doc,
    objects: doc.objects.map((o) => (ids.has(o.id) ? vector(Number(o.id.slice(1)), round + 1) : o)),
  }));
}

/** Puntos de geometría distintos que se alcanzan desde estos documentos. */
function reachablePoints(docs: readonly Document[]): number {
  const seen = new Set<object>();
  let points = 0;
  for (const d of docs) {
    for (const o of d.objects) {
      if (seen.has(o.kind)) continue;
      seen.add(o.kind);
      if (o.kind.type === 'vector') points += o.kind.paths.reduce((n, p) => n + bezPointCount(p), 0);
    }
  }
  return points;
}

const docsOf = (d: Pick<TabData, 'doc' | 'savedDoc' | 'past' | 'future'>): Document[] => [d.doc, d.savedDoc, ...d.past.map((e: HistoryEntry) => e.doc), ...d.future.map((e: HistoryEntry) => e.doc)];
const mb = (points: number) => (points * BYTES_PER_POINT) / 1024 / 1024;

const gc = (globalThis as { gc?: () => void }).gc;
const heapMb = (): number => {
  gc?.();
  gc?.();
  gc?.();
  return process.memoryUsage().heapUsed / 1024 / 1024;
};

// Estas pruebas arman documentos de 6000 objetos y los editan cientos de veces; el costo es de CPU y de
// memoria, y con la máquina cargada pasa de los 5 s de vitest por defecto sin que haya un fallo. No miden
// tiempo (comprueban cuántos puntos quedan alcanzables): el tope explícito de la suite solo evita un falso
// rojo, y un cuelgue de verdad sigue cortándose a los 2 minutos.
describe('memoria del historial con varias pestañas', { timeout: 120_000 }, () => {
  it('cinco pestañas con un documento grande y mucha edición: las inactivas respetan su presupuesto', () => {
    const store = createAppStore();
    const heapBase = heapMb();
    const ids: string[] = [];
    let fullPoints = 0;
    let fullBase = 0;
    for (let t = 0; t < 5; t++) {
      if (t > 0) store.getState().newTab();
      store.getState().openInTab(bigDoc(), `/p/grande-${t}.tlaku`);
      ids.push(store.getState().activeTabId);
      for (let e = 0; e < EDITS; e++) editGeometry(store, e);
      if (t === 0) {
        fullPoints = reachablePoints(docsOf(store.getState()));
        fullBase = reachablePoints([store.getState().doc, store.getState().savedDoc]);
      }
    }
    const live = store.getState();
    expect(live.past.length).toBeGreaterThan(INACTIVE_HISTORY_STEPS); // la activa conserva su historial entero
    const inactive = live.tabs.filter((e) => e.id !== live.activeTabId);
    expect(inactive).toHaveLength(4);
    const trimmedPoints = reachablePoints(docsOf(inactive[0]!.data!));

    for (const e of inactive) {
      const d = e.data!;
      expect(d.past.length).toBeLessThanOrEqual(INACTIVE_HISTORY_STEPS);
      const retained = d.past.reduce((n, p) => n + (p.cost ?? 0), 0);
      // Un paso nunca se parte: se admite pasarse por el último paso que entra.
      expect(retained, 'puntos que retiene el historial de una pestaña inactiva').toBeLessThanOrEqual(INACTIVE_POINT_BUDGET + PER_EDIT * POINTS_PER_OBJECT + OBJECTS / 8);
    }
    // El costo de un paso se calcula sobre lo que el paso reemplazó, no sobre todo el documento.
    const sample = live.past.at(-1)!;
    expect(entryCost(sample.doc, live.doc)).toBeGreaterThan(0);
    expect(entryCost(sample.doc, live.doc)).toBeLessThan(PER_EDIT * POINTS_PER_OBJECT * 3);

    // Lo que pesa una pestaña: documento + lo guardado (inevitable: hace falta para saber si hay cambios y para
    // guardar) + historial. Recortar solo quita historial.
    const base = (d: Pick<TabData, 'doc' | 'savedDoc'>) => reachablePoints([d.doc, d.savedDoc]);
    const docOnly = reachablePoints([inactive[0]!.data!.doc]);
    expect(docOnly).toBe(OBJECTS * POINTS_PER_OBJECT);
    expect(trimmedPoints).toBeLessThan(fullPoints);
    const historyFull = fullPoints - fullBase;
    const historyTrimmed = trimmedPoints - base(inactive[0]!.data!);
    expect(historyTrimmed).toBeLessThanOrEqual(INACTIVE_POINT_BUDGET + PER_EDIT * POINTS_PER_OBJECT * 2);
    expect(historyTrimmed).toBeLessThan(historyFull * 0.5);

    const total = inactive.reduce((n, e) => n + reachablePoints(docsOf(e.data!)), 0) + reachablePoints(docsOf(live));
    // eslint-disable-next-line no-console
    console.log(
      `[memoria pestañas] documento de ${OBJECTS} objetos x ${POINTS_PER_OBJECT} puntos = ${(docOnly / 1e3).toFixed(0)}k puntos (~${mb(docOnly).toFixed(0)} MB), ${EDITS} ediciones de ${PER_EDIT} objetos.\n` +
        `  Una pestaña con todo su historial: ${(fullPoints / 1e3).toFixed(0)}k puntos (~${mb(fullPoints).toFixed(0)} MB; el historial solo: ${(historyFull / 1e3).toFixed(0)}k).\n` +
        `  Una pestaña inactiva (${INACTIVE_HISTORY_STEPS} pasos, presupuesto ${INACTIVE_POINT_BUDGET / 1e3}k puntos): ${(trimmedPoints / 1e3).toFixed(0)}k puntos (~${mb(trimmedPoints).toFixed(0)} MB; el historial solo: ${(historyTrimmed / 1e3).toFixed(0)}k).\n` +
        `  Las 5 pestañas juntas: ${(total / 1e3).toFixed(0)}k puntos (~${mb(total).toFixed(0)} MB); con el historial entero en todas serían ~${((fullPoints * 5) / 1e3).toFixed(0)}k (~${mb(fullPoints * 5).toFixed(0)} MB).` +
        (gc ? `\n  Montón real (heapUsed): base ${heapBase.toFixed(0)} MB, ahora ${heapMb().toFixed(0)} MB.` : ''),
    );
  });

  it('el recorte no cambia el documento ni lo guardado: solo el historial', () => {
    const store = createAppStore();
    store.getState().openInTab(bigDoc(), '/p/g.tlaku');
    const a = store.getState().activeTabId;
    for (let e = 0; e < 60; e++) editGeometry(store, e);
    const doc = store.getState().doc;
    const saved = store.getState().savedDoc;
    store.getState().newTab();
    store.getState().activateTab(a);
    expect(store.getState().doc).toBe(doc);
    expect(store.getState().savedDoc).toBe(saved);
    expect(store.getState().dirty).toBe(true);
    expect(store.getState().past.length).toBe(INACTIVE_HISTORY_STEPS);
  });
});
