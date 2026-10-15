import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi, type LaserApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { commands } from '@/commands';
import { createAppStore, useStore } from '@/store';
import { calibrationFactor, lastRealSeconds } from '@/features/costs/costMetrics';
import { JobLogDialog, JobRows, deltaLabel } from './JobLogDialog';
import { useHelpUi } from './helpStore';
import { JOB_LOG_KEY, JOB_LOG_MAX, flushJobLog, noteJobAlarm, noteJobEvent, readLegacyJobLog, resetJobLogRecorder, sanitizeJobLog, useJobLog, type JobLogContext, type JobSnapshot } from './jobLog';

const ctx = (over: Partial<JobLogContext> = {}): JobLogContext => ({ now: 1_000, machine: 'Láser A', document: 'caja.tlaku', estimatedS: 120, elapsedS: 0, ...over });
const job = (state: 'started' | 'paused' | 'finished' | 'cancelled' | 'error', message = '') => ({ state, message });
const snapshot = (over: Partial<JobSnapshot> = {}): JobSnapshot => ({
  documentPath: '/p/caja.tlaku',
  tab: 'caja.tlaku',
  layers: [{ color: '#ff0000', name: 'Corte', mode: 'line', material: 'MDF · Corte', settings: { speed: 600, power: 80, passes: 2 } }],
  material: 'MDF · Corte',
  areaMm2: 2500,
  lengthMm: 400,
  thumb: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><path d="M0 0L5 5"/></svg>',
  ...over,
});

/** Almacenamiento local en memoria (las pruebas corren en Node, sin navegador). */
const memory = new Map<string, string>();
let api: LaserApi;

beforeEach(async () => {
  memory.clear();
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => memory.get(k) ?? null,
    setItem: (k: string, v: string) => void memory.set(k, v),
    removeItem: (k: string) => void memory.delete(k),
  });
  // Las escrituras que dejó la prueba anterior terminan en SU api, no en la de esta.
  await flushJobLog();
  api = createMockApi({ latencyMs: 0 });
  setApi(api);
  resetJobLogRecorder();
  useJobLog.setState({ entries: [], loaded: false, saveError: null });
});

const stored = async () => (await api.jobHistoryLoad({})).map((e) => e as { id: string; result: string; at: number });

describe('historial de trabajos', () => {
  it('anota un trabajo terminado con máquina, archivo, estimado y real', () => {
    noteJobEvent(false, job('started'), ctx({ now: 5_000 }));
    noteJobEvent(true, job('finished', 'Listo.'), ctx({ now: 130_000, elapsedS: 125, estimatedS: 999 }));
    const [e, ...rest] = useJobLog.getState().entries;
    expect(rest).toEqual([]);
    expect(e).toMatchObject({ at: 5_000, machine: 'Láser A', document: 'caja.tlaku', estimatedS: 120, actualS: 125, result: 'finished', message: 'Listo.' });
  });

  it('guarda las capas con sus ajustes, el material, el área, la miniatura y la pestaña de origen', () => {
    noteJobEvent(false, job('started'), ctx({ snapshot: snapshot() }));
    noteJobEvent(true, job('finished'), ctx({ elapsedS: 9 }));
    const e = useJobLog.getState().entries[0]!;
    expect(e).toMatchObject({ documentPath: '/p/caja.tlaku', tab: 'caja.tlaku', material: 'MDF · Corte', areaMm2: 2500, lengthMm: 400 });
    expect(e.layers![0]).toMatchObject({ color: '#ff0000', mode: 'line', material: 'MDF · Corte', settings: { speed: 600, power: 80, passes: 2 } });
    expect(e.thumb).toContain('<svg');
  });

  it('al empezar queda una entrada «en curso» en disco que el final completa (sin duplicar)', async () => {
    noteJobEvent(false, job('started'), ctx({ snapshot: snapshot() }));
    await flushJobLog();
    expect((await stored()).map((e) => e.result)).toEqual(['running']);
    noteJobEvent(true, job('finished'), ctx({ elapsedS: 4 }));
    await flushJobLog();
    expect((await stored()).map((e) => e.result)).toEqual(['finished']);
    expect(useJobLog.getState().entries).toHaveLength(1);
  });

  it('cancelado y con error también quedan; las pausas y reanudaciones no anotan nada más', () => {
    noteJobEvent(false, job('started'), ctx());
    noteJobEvent(true, job('paused'), ctx());
    noteJobEvent(true, job('started'), ctx({ now: 9_999 })); // reanudar: no es un trabajo nuevo
    expect(useJobLog.getState().entries.map((e) => e.result)).toEqual(['running']);
    noteJobEvent(true, job('cancelled', 'Cancelado.'), ctx({ elapsedS: 3 }));
    noteJobEvent(false, job('started'), ctx({ now: 2_000_000 }));
    noteJobEvent(true, job('error', 'Falla.'), ctx({ elapsedS: 1 }));
    expect(useJobLog.getState().entries.map((e) => e.result)).toEqual(['error', 'cancelled']);
    expect(useJobLog.getState().entries[1]!.at).toBe(1_000); // la reanudación no movió el inicio
  });

  it('un final sin inicio conocido no inventa una entrada, y el final se anota una sola vez', () => {
    noteJobEvent(true, job('finished'), ctx());
    expect(useJobLog.getState().entries).toHaveLength(0);
    noteJobEvent(false, job('started'), ctx());
    noteJobAlarm('alarm', ctx({ elapsedS: 7 }));
    noteJobEvent(true, job('error', 'otra vez'), ctx());
    expect(useJobLog.getState().entries).toHaveLength(1);
    expect(useJobLog.getState().entries[0]).toMatchObject({ result: 'error', actualS: 7 });
  });

  it('sin vista previa al día el estimado queda vacío', () => {
    noteJobEvent(false, job('started'), ctx({ estimatedS: null, document: null }));
    noteJobEvent(true, job('finished'), ctx());
    expect(useJobLog.getState().entries[0]).toMatchObject({ estimatedS: null, document: null });
    expect(deltaLabel({ estimatedS: null, actualS: 5 })).toBeNull();
    expect(deltaLabel({ estimatedS: 100, actualS: 112 })).toBe('+12 %');
    expect(deltaLabel({ estimatedS: 100, actualS: 80 })).toBe('-20 %');
  });

  it('se guarda en el archivo del motor y se vuelve a leer en otra «ventana»', async () => {
    noteJobEvent(false, job('started'), ctx({ snapshot: snapshot() }));
    noteJobEvent(true, job('finished'), ctx({ elapsedS: 10 }));
    await flushJobLog();
    expect(await stored()).toHaveLength(1);
    // Otra ventana (otro estado en memoria) lee el mismo archivo.
    useJobLog.setState({ entries: [], loaded: false });
    await useJobLog.getState().load();
    expect(useJobLog.getState().loaded).toBe(true);
    expect(useJobLog.getState().entries).toHaveLength(1);
    expect(useJobLog.getState().entries[0]!.layers).toHaveLength(1);
  });

  it('ya no usa localStorage para los trabajos nuevos', async () => {
    noteJobEvent(false, job('started'), ctx());
    noteJobEvent(true, job('finished'), ctx());
    await flushJobLog();
    expect(memory.has(`tlaku.${JOB_LOG_KEY}`)).toBe(false);
  });

  it('guarda como mucho el tope de trabajos (preferencia jobHistoryMax) y descarta los más viejos', async () => {
    useStore.getState().setPref('jobHistoryMax', 25);
    for (let i = 0; i < 30; i++) {
      noteJobEvent(false, job('started'), ctx({ now: 1_000 + i }));
      noteJobEvent(true, job('finished'), ctx());
    }
    await flushJobLog();
    const entries = useJobLog.getState().entries;
    expect(entries).toHaveLength(25);
    expect(entries[0]!.at).toBe(1_029); // el más nuevo primero
    expect(await stored()).toHaveLength(25);
    useStore.getState().setPref('jobHistoryMax', 1000);
  });

  it('el tope de fábrica es de 1000', () => {
    expect(JOB_LOG_MAX).toBe(1000);
  });

  it('borrar vacía la lista y el archivo; quitar una entrada solo quita esa', async () => {
    for (const now of [1_000, 2_000, 3_000]) {
      noteJobEvent(false, job('started'), ctx({ now }));
      noteJobEvent(true, job('finished'), ctx());
    }
    const middle = useJobLog.getState().entries[1]!.id;
    useJobLog.getState().remove([middle]);
    await flushJobLog();
    expect((await stored()).map((e) => e.at)).toEqual([1_000, 3_000]);
    useJobLog.getState().clear();
    await flushJobLog();
    expect(useJobLog.getState().entries).toEqual([]);
    expect(await stored()).toEqual([]);
  });

  it('las notas se guardan, se pueden borrar y sobreviven a releer', async () => {
    noteJobEvent(false, job('started'), ctx());
    noteJobEvent(true, job('finished'), ctx());
    const id = useJobLog.getState().entries[0]!.id;
    useJobLog.getState().patch(id, { notes: 'Quemó un poco el borde' });
    await flushJobLog();
    expect(((await api.jobHistoryLoad({}))[0] as { notes?: string }).notes).toBe('Quemó un poco el borde');
    useJobLog.setState({ entries: [] });
    await useJobLog.getState().load();
    expect(useJobLog.getState().entries[0]!.notes).toBe('Quemó un poco el borde');
    useJobLog.getState().patch(id, { notes: '' });
    await flushJobLog();
    expect(useJobLog.getState().entries[0]!.notes).toBeUndefined();
    expect(((await api.jobHistoryLoad({}))[0] as { notes?: string }).notes).toBeUndefined();
  });

  it('lo dañado se descarta sin romper, y una miniatura con scripts no pasa', () => {
    expect(sanitizeJobLog('basura')).toEqual([]);
    const ok = { at: 1, machine: 'M', document: null, estimatedS: 5, actualS: 6, result: 'finished', message: '' };
    const out = sanitizeJobLog([ok, { ...ok, result: 'raro' }, null, { ...ok, at: 'x' }, { ...ok, estimatedS: -1, at: 2 }]);
    expect(out.map((e) => e.at)).toEqual([2, 1]);
    expect(out[0]!.estimatedS).toBeNull();
    const [bad] = sanitizeJobLog([{ ...ok, thumb: '<svg><script>alert(1)</script></svg>' }]);
    expect(bad!.thumb).toBeUndefined();
    const [fine] = sanitizeJobLog([{ ...ok, thumb: '<svg viewBox="0 0 1 1"/>', layers: [{ color: '#f00', name: 'a', mode: 'line', settings: { speed: 10, raro: 3, air: true } }, 'x'] }]);
    expect(fine!.thumb).toBe('<svg viewBox="0 0 1 1"/>');
    expect(fine!.layers).toEqual([{ color: '#f00', name: 'a', mode: 'line', settings: { speed: 10, air: true } }]);
  });
});

describe('calibración de tiempos de la calculadora de costos con el nuevo almacén', () => {
  it('usa los trabajos terminados leídos del archivo y también los migrados del registro viejo', async () => {
    // Dos trabajos viejos (localStorage) y dos nuevos (archivo): la mediana de real ÷ estimado sale de los cuatro.
    const old = [1, 2].map((i) => ({ id: `old-${i}`, at: i, machine: 'Falcon2', document: null, estimatedS: 100, actualS: 130, result: 'finished', message: '' }));
    memory.set(`tlaku.${JOB_LOG_KEY}`, JSON.stringify(old));
    for (const now of [10_000, 20_000]) {
      noteJobEvent(false, job('started'), ctx({ now, machine: 'Falcon2', estimatedS: 100 }));
      noteJobEvent(true, job('finished'), ctx({ elapsedS: 130 }));
    }
    await flushJobLog();
    useJobLog.setState({ entries: [] });
    await useJobLog.getState().load();
    const log = useJobLog.getState().entries;
    expect(log).toHaveLength(4);
    expect(calibrationFactor(log, 'Falcon2')).toEqual({ factor: 1.3, jobs: 4 });
    expect(lastRealSeconds(log, 'Falcon2')).toBe(130);
    // Los interrumpidos y los que siguen en curso no cuentan.
    noteJobEvent(false, job('started'), ctx({ now: 30_000, machine: 'Falcon2' }));
    expect(calibrationFactor(useJobLog.getState().entries, 'Falcon2')!.jobs).toBe(4);
  });
});

describe('migración del registro viejo (localStorage)', () => {
  const legacy = [
    { id: 'old-1', at: 100, machine: 'Falcon2', document: 'a.tlaku', estimatedS: 60, actualS: 70, result: 'finished', message: 'ok' },
    { id: 'old-2', at: 200, machine: 'Falcon2', document: 'b.tlaku', estimatedS: null, actualS: 5, result: 'cancelled', message: '' },
    { nada: 'valido' },
  ];

  it('pasa las entradas válidas al archivo, borra la clave vieja y no duplica al repetir', async () => {
    memory.set(`tlaku.${JOB_LOG_KEY}`, JSON.stringify(legacy));
    expect(readLegacyJobLog()).toHaveLength(2);
    await useJobLog.getState().load();
    expect(useJobLog.getState().entries.map((e) => e.id)).toEqual(['old-2', 'old-1']);
    expect(memory.has(`tlaku.${JOB_LOG_KEY}`)).toBe(false);
    expect((await stored()).map((e) => e.id)).toEqual(['old-1', 'old-2']);
    // Si el registro viejo reapareciera (otra ventana vieja), no se duplica.
    memory.set(`tlaku.${JOB_LOG_KEY}`, JSON.stringify(legacy));
    await useJobLog.getState().load();
    expect(await stored()).toHaveLength(2);
  });

  it('si el motor falla, el registro viejo se conserva', async () => {
    memory.set(`tlaku.${JOB_LOG_KEY}`, JSON.stringify(legacy));
    setApi({ ...api, jobHistoryImport: () => Promise.reject(new Error('disco lleno')) });
    await useJobLog.getState().load();
    expect(memory.has(`tlaku.${JOB_LOG_KEY}`)).toBe(true);
    expect(useJobLog.getState().saveError).toContain('disco lleno');
  });

  it('un JSON dañado en la clave vieja no rompe nada', () => {
    memory.set(`tlaku.${JOB_LOG_KEY}`, '{no es json');
    expect(readLegacyJobLog()).toEqual([]);
  });
});

describe('trabajos que la app dejó a medias', () => {
  it('un «en curso» que ninguna ventana ejecuta queda como interrumpido al leer', async () => {
    noteJobEvent(false, job('started'), ctx({ now: 1_000 }));
    await flushJobLog();
    // La app se cierra y se vuelve a abrir: no hay trabajo en curso ni latido.
    resetJobLogRecorder();
    useJobLog.setState({ entries: [] });
    await useJobLog.getState().load();
    expect(useJobLog.getState().entries[0]).toMatchObject({ result: 'interrupted' });
    expect(useJobLog.getState().entries[0]!.message).not.toBe('');
    await flushJobLog();
    expect((await stored())[0]!.result).toBe('interrupted');
  });

  it('si otra ventana lo sigue ejecutando (latido reciente) se deja «en curso»', async () => {
    noteJobEvent(false, job('started'), ctx({ now: Date.now() }));
    const id = useJobLog.getState().entries[0]!.id;
    await flushJobLog();
    resetJobLogRecorder(); // esta ventana no lo ejecuta...
    memory.set('tlaku.jobLog.live', JSON.stringify({ id, beat: Date.now() })); // ...pero otra sí
    useJobLog.setState({ entries: [] });
    await useJobLog.getState().load();
    expect(useJobLog.getState().entries[0]!.result).toBe('running');
  });

  it('el trabajo en curso de esta ventana no se marca al releer', async () => {
    noteJobEvent(false, job('started'), ctx());
    await useJobLog.getState().load();
    expect(useJobLog.getState().entries[0]!.result).toBe('running');
  });
});

describe('trabajos de la máquina anotados desde el store', () => {
  it('los eventos machine://job del slice llenan el historial', () => {
    const store = createAppStore();
    store.getState().onJob({ state: 'started', message: 'Enviando.', position_lost: false });
    store.getState().onJob({ state: 'finished', message: 'Terminado.', position_lost: false });
    const entries = useJobLog.getState().entries;
    expect(entries).toHaveLength(1);
    expect(entries[0]).toMatchObject({ result: 'finished', message: 'Terminado.' });
    expect(entries[0]!.machine.length).toBeGreaterThan(0);
    // El resumen de capas y la miniatura salen de la pestaña (aquí el documento vacío: sin recorrido).
    expect(Array.isArray(entries[0]!.layers)).toBe(true);
  });

  it('una alarma durante el trabajo lo anota como error', () => {
    const store = createAppStore();
    store.getState().onJob({ state: 'started', message: 'Enviando.', position_lost: false });
    store.getState().onStatus({ state: 'alarm', machine_pos: [0, 0, 0], work_pos: [0, 0, 0], feed: 0, spindle: 0, buffer: null } as never);
    expect(useJobLog.getState().entries.map((e) => e.result)).toEqual(['error']);
  });
});

describe('vista del historial', () => {
  it('el comando de Ayuda abre el diálogo', async () => {
    useHelpUi.getState().close();
    await commands.run('help.jobLog');
    expect(useHelpUi.getState().dialog).toBe('jobLog');
    useHelpUi.getState().close();
  });

  it('vacío explica cómo se llena; con trabajos muestra la tabla con su resultado', () => {
    expect(renderToStaticMarkup(<JobLogDialog />)).toContain('Todavía no hay trabajos');
    noteJobEvent(false, job('started'), ctx());
    noteJobEvent(true, job('cancelled'), ctx({ elapsedS: 65 }));
    // El render en servidor ve el estado inicial del store: la tabla se pinta con sus filas.
    const html = renderToStaticMarkup(<table><JobRows entries={useJobLog.getState().entries} /></table>);
    expect(html).toContain('Láser A');
    expect(html).toContain('Cancelado');
    expect(html).toContain('2:00'); // estimado 120 s
    expect(html).toContain('1:05'); // real 65 s
    expect(renderToStaticMarkup(<JobLogDialog />)).toContain('Borrar el registro');
  });

  it('la fila muestra el material, la miniatura y un botón para ver el detalle', () => {
    noteJobEvent(false, job('started'), ctx({ snapshot: snapshot() }));
    noteJobEvent(true, job('finished'), ctx({ elapsedS: 3 }));
    const html = renderToStaticMarkup(<table><JobRows entries={useJobLog.getState().entries} onSelect={() => undefined} selectedId={null} /></table>);
    expect(html).toContain('MDF · Corte');
    expect(html).toContain('data:image/svg+xml');
    expect(html).toContain('joblog__open');
  });
});

describe('historial: trabajos interrumpidos', () => {
  const entry = (over: Record<string, unknown> = {}) => ({
    at: Date.now() - 5_000,
    machine: 'Falcon2',
    document: 'caja.tlaku',
    estimatedS: 100,
    actualS: 4,
    result: 'error',
    message: 'Se perdió la conexión',
    ...over,
  });

  it('marca el último trabajo que no terminó bien con su porcentaje y lo conserva al releer', async () => {
    useJobLog.getState().append({ id: 'a', ...entry(), result: 'error' } as never);
    useJobLog.getState().markLatestInterrupted({ id: 'ck1', percent: 42.4, line: 120, total: 285 });
    await flushJobLog();
    const [e] = await api.jobHistoryLoad({});
    expect((e as { interrupted: unknown }).interrupted).toEqual({ id: 'ck1', percent: 42.4, line: 120, total: 285 });
  });

  it('no marca un trabajo terminado ni uno viejo', () => {
    useJobLog.getState().append({ id: 'a', ...entry({ result: 'finished' }) } as never);
    useJobLog.getState().markLatestInterrupted({ id: 'ck1', percent: 10, line: 1, total: 10 });
    expect(useJobLog.getState().entries[0]!.interrupted).toBeUndefined();
    useJobLog.getState().clear();
    useJobLog.getState().append({ id: 'b', ...entry({ at: 1_000 }) } as never);
    useJobLog.getState().markLatestInterrupted({ id: 'ck1', percent: 10, line: 1, total: 10 });
    expect(useJobLog.getState().entries[0]!.interrupted).toBeUndefined();
  });

  it('descarta datos dañados de la interrupción sin perder la entrada', () => {
    const [e] = sanitizeJobLog([{ ...entry(), interrupted: { id: 5, percent: 'x' } }]);
    expect(e!.interrupted).toBeUndefined();
    expect(e!.machine).toBe('Falcon2');
    const [ok] = sanitizeJobLog([{ ...entry(), interrupted: { id: 'k', percent: 250, line: 3, total: 9 } }]);
    expect(ok!.interrupted!.percent).toBe(100);
  });

  it('la tabla muestra el porcentaje y el botón Continuar solo mientras siga existiendo el punto de control', () => {
    const e = { id: 'a', at: 1, machine: 'M', document: null, estimatedS: null, actualS: 1, result: 'error' as const, message: '', interrupted: { id: 'ck1', percent: 42, line: 1, total: 2 } };
    const html = (resumable: string | null) => renderToStaticMarkup(<table><JobRows entries={[e]} resumableId={resumable} onContinue={() => undefined} /></table>);
    expect(html('ck1')).toContain('Interrumpido al 42 %');
    expect(html('ck1')).toContain('Continuar');
    expect(html('otro')).toContain('Interrumpido al 42 %');
    expect(html('otro')).not.toContain('Continuar');
    expect(html(null)).not.toContain('Continuar');
  });
});
