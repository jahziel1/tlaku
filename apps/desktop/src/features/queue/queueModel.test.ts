import { describe, expect, it } from 'vitest';
import {
  EMPTY_QUEUE,
  MAX_COPIES,
  MAX_ITEMS,
  addItem,
  begin,
  bindTab,
  clearFinished,
  currentCopy,
  currentItem,
  jobEnded,
  jobStarted,
  makeItem,
  moveItem,
  pause,
  remainingEstimate,
  removeItem,
  restoreItem,
  resume,
  setCopies,
  skipItem,
  stop,
  totals,
  type QueueState,
} from './queueModel';

const item = (name: string, tabId: string | null = name, copies = 1) => makeItem({ id: name, source: 'tab', tabId, name, copies });
const queueOf = (...items: ReturnType<typeof item>[]): QueueState => items.reduce(addItem, EMPTY_QUEUE);
const ids = (q: QueueState) => q.items.map((i) => i.id);

/** Empieza la cola y deja que la máquina arranque el trabajo actual (lo que hace una persona al confirmar). */
const run = (q: QueueState): QueueState => jobStarted(q, currentItem(q)!.tabId);

describe('cola: nunca avanza sola', () => {
  it('empezar solo pide confirmar el primero: no hay trabajo en la máquina', () => {
    const q = begin(queueOf(item('a'), item('b')));
    expect(q.run).toBe('awaiting');
    expect(q.currentId).toBe('a');
  });

  it('al terminar un trabajo bien, la cola se detiene a esperar confirmación del siguiente', () => {
    let q = run(begin(queueOf(item('a'), item('b'), item('c'))));
    expect(q.run).toBe('running');
    q = jobEnded(q, 'finished');
    expect(q.run).toBe('awaiting');
    expect(q.currentId).toBe('b');
    expect(q.items[0]).toMatchObject({ status: 'done', done: 1 });
    // Pasa el tiempo, llegan eventos sueltos: sigue esperando.
    q = jobEnded(jobEnded(q, 'finished'), 'finished');
    expect(q.run).toBe('awaiting');
    expect(q.currentId).toBe('b');
  });

  it('las copias de un mismo documento también piden confirmar cada una', () => {
    let q = begin(queueOf(item('a', 'a', 3)));
    const seen: string[] = [];
    for (let i = 0; i < 3; i++) {
      expect(q.run).toBe('awaiting');
      expect(currentCopy(q)).toBe(i + 1);
      q = jobEnded(run(q), 'finished');
      seen.push(q.run);
    }
    expect(seen).toEqual(['awaiting', 'awaiting', 'done']);
    expect(q.items[0]).toMatchObject({ status: 'done', done: 3 });
  });

  it('solo un trabajo de la pestaña esperada pone la cola en marcha', () => {
    const q = begin(queueOf(item('a'), item('b')));
    expect(jobStarted(q, 'b').run).toBe('awaiting'); // otro documento
    expect(jobStarted(q, null).run).toBe('awaiting'); // origen desconocido
    expect(jobStarted(q, 'a').run).toBe('running');
    // Sin esperar confirmación (pausada, detenida, sin empezar) no se engancha a nada.
    expect(jobStarted(pause(q), 'a').run).toBe('paused');
    expect(jobStarted(stop(q), 'a').run).toBe('stopped');
    expect(jobStarted(queueOf(item('a')), 'a').run).toBe('idle');
  });

  it('ninguna secuencia de acciones llega a «running» sin un jobStarted', () => {
    // Todas las operaciones del modelo salvo jobStarted, combinadas de todas las formas (profundidad 4).
    const ops: ((q: QueueState) => QueueState)[] = [
      begin,
      pause,
      resume,
      (q) => stop(q),
      (q) => skipItem(q, q.currentId ?? 'a'),
      (q) => removeItem(q, 'b'),
      (q) => moveItem(q, 'a', 1),
      (q) => setCopies(q, 'a', 5),
      (q) => restoreItem(q, 'a'),
      clearFinished,
      (q) => jobEnded(q, 'finished'),
      (q) => jobEnded(q, 'error', 'x'),
      (q) => jobEnded(q, 'cancelled'),
    ];
    const walk = (q: QueueState, depth: number) => {
      expect(q.run, JSON.stringify(q)).not.toBe('running');
      if (depth === 0) return;
      for (const op of ops) walk(op(q), depth - 1);
    };
    walk(queueOf(item('a'), item('b')), 4);
  });

  it('terminar un trabajo jamás deja la cola en «running»', () => {
    for (const pauseFirst of [false, true]) {
      let q = run(begin(queueOf(item('a', 'a', 2), item('b'))));
      if (pauseFirst) q = pause(q);
      for (let i = 0; i < 3; i++) {
        q = jobEnded(q, 'finished');
        expect(q.run).not.toBe('running');
        if (q.run === 'awaiting' || q.run === 'paused') q = run(resume(q));
      }
    }
  });
});

describe('cola: se detiene en fallos y cancelaciones', () => {
  it('un error detiene la cola, marca el trabajo y no ofrece el siguiente', () => {
    let q = run(begin(queueOf(item('a'), item('b'))));
    q = jobEnded(q, 'error', 'Alarma 3');
    expect(q.run).toBe('stopped');
    expect(q.stopReason).toBe('failed');
    expect(q.stopMessage).toBe('Alarma 3');
    expect(q.items[0]!.status).toBe('failed');
    expect(q.items[1]!.status).toBe('pending');
    expect(jobEnded(q, 'finished').run).toBe('stopped');
  });

  it('una cancelación también detiene la cola, pero el trabajo sigue pendiente', () => {
    let q = run(begin(queueOf(item('a'), item('b'))));
    q = jobEnded(q, 'cancelled', 'Detenido');
    expect(q).toMatchObject({ run: 'stopped', stopReason: 'cancelled' });
    expect(q.items[0]).toMatchObject({ status: 'pending', done: 0 });
  });

  it('reintentar tras detenerse vuelve a pedir confirmar (el fallido vuelve a pendiente)', () => {
    let q = jobEnded(run(begin(queueOf(item('a'), item('b')))), 'error');
    q = begin(q);
    expect(q).toMatchObject({ run: 'awaiting', currentId: 'a', stopReason: null });
    expect(q.items[0]!.status).toBe('pending');
  });

  it('detener la cola con un trabajo en la máquina no lo cancela y su final ya no mueve nada', () => {
    let q = run(begin(queueOf(item('a'), item('b'))));
    q = stop(q);
    expect(q.run).toBe('stopped');
    expect(jobEnded(q, 'finished')).toBe(q);
  });
});

describe('cola: pausar', () => {
  it('pausada esperando confirmar, no pide nada hasta reanudar (y reanudar vuelve a pedir, no inicia)', () => {
    let q = pause(begin(queueOf(item('a'), item('b'))));
    expect(q.run).toBe('paused');
    q = resume(q);
    expect(q).toMatchObject({ run: 'awaiting', currentId: 'a' });
  });

  it('pausar con un trabajo en marcha: al terminar queda en pausa, no pide el siguiente', () => {
    let q = pause(run(begin(queueOf(item('a'), item('b')))));
    expect(q.run).toBe('running');
    expect(q.pauseAfter).toBe(true);
    q = jobEnded(q, 'finished');
    expect(q).toMatchObject({ run: 'paused', currentId: 'b', pauseAfter: false });
    expect(resume(q).run).toBe('awaiting');
  });

  it('quitar la pausa pedida durante un trabajo la cancela', () => {
    const q = resume(pause(run(begin(queueOf(item('a'), item('b'))))));
    expect(q.pauseAfter).toBe(false);
    expect(jobEnded(q, 'finished').run).toBe('awaiting');
  });
});

describe('cola: edición', () => {
  it('reordena y el orden manda en lo que sigue', () => {
    let q = queueOf(item('a'), item('b'), item('c'));
    q = moveItem(q, 'c', 0);
    expect(ids(q)).toEqual(['c', 'a', 'b']);
    q = moveItem(q, 'c', 99);
    expect(ids(q)).toEqual(['a', 'b', 'c']);
    q = moveItem(q, 'b', -4);
    expect(ids(q)).toEqual(['b', 'a', 'c']);
    expect(currentItem(begin(q))!.id).toBe('b');
    expect(moveItem(q, 'nada', 0)).toBe(q);
  });

  it('reordenar mientras se espera no cambia el trabajo que se está confirmando', () => {
    let q = begin(queueOf(item('a'), item('b'), item('c')));
    q = moveItem(q, 'c', 0);
    expect(q.currentId).toBe('a');
    expect(run(q).run).toBe('running');
  });

  it('saltar el actual pasa al siguiente (que también pide confirmación) o termina', () => {
    let q = begin(queueOf(item('a'), item('b')));
    q = skipItem(q, 'a');
    expect(q).toMatchObject({ run: 'awaiting', currentId: 'b' });
    expect(q.items[0]!.status).toBe('skipped');
    q = skipItem(q, 'b');
    expect(q.run).toBe('done');
    expect(totals(q)).toEqual({ done: 0, total: 0 });
  });

  it('no se puede quitar ni saltar el trabajo que está en la máquina', () => {
    const q = run(begin(queueOf(item('a'), item('b'))));
    expect(removeItem(q, 'a')).toBe(q);
    expect(skipItem(q, 'a')).toBe(q);
    expect(ids(removeItem(q, 'b'))).toEqual(['a']);
    expect(skipItem(q, 'b').items[1]!.status).toBe('skipped');
  });

  it('quitar el que espera confirmación pasa al siguiente', () => {
    const q = removeItem(begin(queueOf(item('a'), item('b'))), 'a');
    expect(q).toMatchObject({ run: 'awaiting', currentId: 'b' });
    expect(removeItem(q, 'b').run).toBe('done');
  });

  it('las copias se limitan entre 1 y el máximo, y pedir menos que las hechas completa el trabajo', () => {
    let q = queueOf(item('a', 'a', 3));
    expect(setCopies(q, 'a', 0).items[0]!.copies).toBe(1);
    expect(setCopies(q, 'a', 1000).items[0]!.copies).toBe(MAX_COPIES);
    expect(setCopies(q, 'a', Number.NaN).items[0]!.copies).toBe(1);
    q = jobEnded(run(begin(q)), 'finished'); // 1 de 3
    q = setCopies(q, 'a', 1);
    expect(q.items[0]).toMatchObject({ copies: 1, done: 1, status: 'done' });
    expect(q.run).toBe('done');
  });

  it('restaurar un saltado o fallido lo vuelve a poner en juego', () => {
    let q = skipItem(queueOf(item('a'), item('b')), 'a');
    q = restoreItem(q, 'a');
    expect(q.items[0]!.status).toBe('pending');
    expect(restoreItem(q, 'b')).toBe(q);
  });

  it('el máximo de trabajos y la limpieza de terminados', () => {
    let q = EMPTY_QUEUE;
    for (let i = 0; i < MAX_ITEMS + 5; i++) q = addItem(q, item(`t${i}`));
    expect(q.items).toHaveLength(MAX_ITEMS);
    let small = jobEnded(run(begin(queueOf(item('a'), item('b')))), 'finished');
    small = clearFinished(small);
    expect(ids(small)).toEqual(['b']);
    expect(small).toMatchObject({ run: 'awaiting', currentId: 'b' });
    expect(clearFinished(jobEnded(run(small), 'finished'))).toEqual(EMPTY_QUEUE);
  });

  it('enlazar con una pestaña distinta cambia qué trabajo se reconoce como el esperado', () => {
    const q = bindTab(begin(queueOf(item('a', null))), 'a', 'tab-9');
    expect(jobStarted(q, 'tab-9').run).toBe('running');
    expect(jobStarted(begin(queueOf(item('a', null))), null).run).toBe('awaiting');
  });
});

describe('cola: progreso y estimado', () => {
  it('cuenta copias terminadas sobre el total sin contar lo saltado', () => {
    let q = queueOf(item('a', 'a', 2), item('b', 'b', 3), item('c'));
    expect(totals(q)).toEqual({ done: 0, total: 6 });
    q = jobEnded(run(begin(q)), 'finished');
    expect(totals(q)).toEqual({ done: 1, total: 6 });
    q = skipItem(q, 'c');
    expect(totals(q)).toEqual({ done: 1, total: 5 });
  });

  it('suma el estimado de las copias que faltan y cuenta las que no tienen', () => {
    let q = queueOf(item('a', 'a', 2), item('b', 'b', 3));
    const est = (i: { id: string }) => (i.id === 'a' ? 60 : null);
    expect(remainingEstimate(q, est)).toEqual({ seconds: 120, unknown: 3 });
    q = jobEnded(run(begin(q)), 'finished');
    expect(remainingEstimate(q, est)).toEqual({ seconds: 60, unknown: 3 });
    expect(remainingEstimate(skipItem(q, 'b'), est)).toEqual({ seconds: 60, unknown: 0 });
  });
});
