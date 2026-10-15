import { describe, expect, it } from 'vitest';
// El store se importa primero: sus módulos se importan unos a otros y el orden importa.
import '@/store';
import { SILENCE_SERIOUS_MS, SILENCE_SHOW_MS, silenceNotice } from './silenceModel';
import { INITIAL_MACHINE_DATA, reduceMachine } from '@/store/machine';

describe('aviso temprano de silencio', () => {
  it('no se muestra con la placa contestando ni antes de ~2 s', () => {
    expect(silenceNotice({ connected: true, silenceMs: 0, jobBusy: false })).toBeNull();
    expect(silenceNotice({ connected: true, silenceMs: SILENCE_SHOW_MS - 1, jobBusy: false })).toBeNull();
  });

  it('desde los 2 s dice los segundos enteros', () => {
    expect(silenceNotice({ connected: true, silenceMs: 2000, jobBusy: false })).toEqual({ seconds: 2, duringJob: false, serious: false });
    expect(silenceNotice({ connected: true, silenceMs: 3999, jobBusy: false })).toEqual({ seconds: 3, duringJob: false, serious: false });
  });

  it('durante un trabajo lo señala', () => {
    expect(silenceNotice({ connected: true, silenceMs: 2500, jobBusy: true })).toEqual({ seconds: 2, duringJob: true, serious: false });
  });

  it('pasado un minuto EN PLENO TRABAJO se vuelve más serio; en reposo no', () => {
    expect(silenceNotice({ connected: true, silenceMs: SILENCE_SERIOUS_MS - 1, jobBusy: true })?.serious).toBe(false);
    expect(silenceNotice({ connected: true, silenceMs: SILENCE_SERIOUS_MS, jobBusy: true })?.serious).toBe(true);
    expect(silenceNotice({ connected: true, silenceMs: 5 * SILENCE_SERIOUS_MS, jobBusy: false })?.serious).toBe(false);
  });

  it('sin conexión no hay aviso aunque quede un valor viejo', () => {
    expect(silenceNotice({ connected: false, silenceMs: 9000, jobBusy: false })).toBeNull();
  });
});

describe('reductor: machine://silence', () => {
  const connect = { type: 'connection', connection: { connected: true, port: '/dev/x', error: null } } as const;

  it('guarda los milisegundos y se limpia al volver a contestar', () => {
    let m = reduceMachine(INITIAL_MACHINE_DATA, connect, 0);
    expect(m.silenceMs).toBe(0);
    m = reduceMachine(m, { type: 'silence', silence: { silence_ms: 2300 } }, 1);
    expect(m.silenceMs).toBe(2300);
    m = reduceMachine(m, { type: 'silence', silence: { silence_ms: 0 } }, 2);
    expect(m.silenceMs).toBe(0);
  });

  it('una conexión nueva o una caída borran el aviso', () => {
    const silent = reduceMachine(reduceMachine(INITIAL_MACHINE_DATA, connect, 0), { type: 'silence', silence: { silence_ms: 4000 } }, 1);
    expect(reduceMachine(silent, connect, 2).silenceMs).toBe(0);
    const lost = reduceMachine(silent, { type: 'connection', connection: { connected: false, port: '/dev/x', error: 'x' } }, 2);
    expect(lost.silenceMs).toBe(0);
  });
});
