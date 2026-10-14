import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { commands } from '@/commands';
import { createAppStore } from '@/store';
import { JobLogDialog, JobRows, deltaLabel } from './JobLogDialog';
import { useHelpUi } from './helpStore';
import { JOB_LOG_KEY, JOB_LOG_MAX, noteJobAlarm, noteJobEvent, readJobLog, resetJobLogRecorder, sanitizeJobLog, useJobLog, type JobLogContext } from './jobLog';

const ctx = (over: Partial<JobLogContext> = {}): JobLogContext => ({ now: 1_000, machine: 'Láser A', document: 'caja.tlaku', estimatedS: 120, elapsedS: 0, ...over });
const job = (state: 'started' | 'paused' | 'finished' | 'cancelled' | 'error', message = '') => ({ state, message });

/** Almacenamiento local en memoria (las pruebas corren en Node, sin navegador). */
const memory = new Map<string, string>();

beforeEach(() => {
  memory.clear();
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => memory.get(k) ?? null,
    setItem: (k: string, v: string) => void memory.set(k, v),
    removeItem: (k: string) => void memory.delete(k),
  });
  resetJobLogRecorder();
  useJobLog.getState().clear();
});

describe('registro de trabajos', () => {
  it('anota un trabajo terminado con máquina, archivo, estimado y real', () => {
    noteJobEvent(false, job('started'), ctx({ now: 5_000 }));
    noteJobEvent(true, job('finished', 'Listo.'), ctx({ now: 130_000, elapsedS: 125, estimatedS: 999 }));
    const [e, ...rest] = useJobLog.getState().entries;
    expect(rest).toEqual([]);
    expect(e).toMatchObject({ at: 5_000, machine: 'Láser A', document: 'caja.tlaku', estimatedS: 120, actualS: 125, result: 'finished', message: 'Listo.' });
  });

  it('cancelado y con error también quedan; las pausas y reanudaciones no anotan nada', () => {
    noteJobEvent(false, job('started'), ctx());
    noteJobEvent(true, job('paused'), ctx());
    noteJobEvent(true, job('started'), ctx({ now: 9_999 })); // reanudar: no es un trabajo nuevo
    expect(useJobLog.getState().entries).toHaveLength(0);
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

  it('se guarda en el almacenamiento local y se vuelve a leer', () => {
    noteJobEvent(false, job('started'), ctx());
    noteJobEvent(true, job('finished'), ctx({ elapsedS: 10 }));
    expect(readJobLog()).toHaveLength(1);
    expect(JSON.parse(localStorage.getItem(`tlaku.${JOB_LOG_KEY}`)!)).toHaveLength(1);
  });

  it('guarda como mucho los últimos trabajos', () => {
    for (let i = 0; i < JOB_LOG_MAX + 5; i++) {
      noteJobEvent(false, job('started'), ctx({ now: 1_000 + i }));
      noteJobEvent(true, job('finished'), ctx());
    }
    const entries = useJobLog.getState().entries;
    expect(entries).toHaveLength(JOB_LOG_MAX);
    expect(entries[0]!.at).toBe(1_000 + JOB_LOG_MAX + 4); // el más nuevo primero
  });

  it('borrar vacía la lista y el almacenamiento', () => {
    noteJobEvent(false, job('started'), ctx());
    noteJobEvent(true, job('finished'), ctx());
    useJobLog.getState().clear();
    expect(useJobLog.getState().entries).toEqual([]);
    expect(readJobLog()).toEqual([]);
  });

  it('lo dañado en el almacenamiento se descarta sin romper', () => {
    expect(sanitizeJobLog('basura')).toEqual([]);
    const ok = { at: 1, machine: 'M', document: null, estimatedS: 5, actualS: 6, result: 'finished', message: '' };
    const out = sanitizeJobLog([ok, { ...ok, result: 'raro' }, null, { ...ok, at: 'x' }, { ...ok, estimatedS: -1, at: 2 }]);
    expect(out.map((e) => e.at)).toEqual([2, 1]);
    expect(out[0]!.estimatedS).toBeNull();
    localStorage.setItem(`tlaku.${JOB_LOG_KEY}`, '{no es json');
    expect(readJobLog()).toEqual([]);
  });
});

describe('trabajos de la máquina anotados desde el store', () => {
  it('los eventos machine://job del slice llenan el registro', () => {
    const store = createAppStore();
    store.getState().onJob({ state: 'started', message: 'Enviando.', position_lost: false });
    store.getState().onJob({ state: 'finished', message: 'Terminado.', position_lost: false });
    const entries = useJobLog.getState().entries;
    expect(entries).toHaveLength(1);
    expect(entries[0]).toMatchObject({ result: 'finished', message: 'Terminado.' });
    expect(entries[0]!.machine.length).toBeGreaterThan(0);
  });

  it('una alarma durante el trabajo lo anota como error', () => {
    const store = createAppStore();
    store.getState().onJob({ state: 'started', message: 'Enviando.', position_lost: false });
    store.getState().onStatus({ state: 'alarm', machine_pos: [0, 0, 0], work_pos: [0, 0, 0], feed: 0, spindle: 0, buffer: null } as never);
    expect(useJobLog.getState().entries.map((e) => e.result)).toEqual(['error']);
  });
});

describe('vista del registro', () => {
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
});

