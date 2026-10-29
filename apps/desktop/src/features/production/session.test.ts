import { afterEach, describe, expect, it } from 'vitest';
import type { ConnectionEvent } from '@/api/types';
import { useStore } from '@/store';
import type { JobUiState } from '@/store/machine/reducer';
import { makeSnapshot } from './model';
import { noteFullConfirm, noteStartRejected, resetProductionSession, verifiedBaseline } from './session';

const connection: ConnectionEvent = { connected: true, port: 'COM3', error: null };
const snapshot = () =>
  makeSnapshot({
    connection,
    deviceId: 'd',
    doc: { layers: [], objects: [] },
    options: { start_from: 'absolute', job_origin: 0, user_origin: null, use_selection_origin: false, selection: null, transform: null, tile: null },
    presets: [],
  });
const jobIs = (jobState: JobUiState) => useStore.setState({ jobState });

afterEach(() => {
  resetProductionSession();
  jobIs('idle');
});

describe('referencia del modo producción (solo en esta sesión de la app)', () => {
  it('sin ningún trabajo confirmado con la lista completa no hay referencia', () => {
    expect(verifiedBaseline()).toBeNull();
  });

  it('un trabajo confirmado con la lista completa solo vale cuando TERMINA BIEN', () => {
    noteFullConfirm(snapshot());
    expect(verifiedBaseline()).toBeNull(); // todavía no terminó
    jobIs('running');
    expect(verifiedBaseline()).toBeNull();
    jobIs('finished');
    expect(verifiedBaseline()).not.toBeNull();
  });

  it('un trabajo cancelado no vale', () => {
    noteFullConfirm(snapshot());
    jobIs('running');
    jobIs('cancelled');
    expect(verifiedBaseline()).toBeNull();
  });

  it('un trabajo fallido no vale', () => {
    noteFullConfirm(snapshot());
    jobIs('running');
    jobIs('error');
    expect(verifiedBaseline()).toBeNull();
  });

  it('si después de una referencia buena un trabajo falla o se cancela, se pierde y hay que repetir la lista', () => {
    noteFullConfirm(snapshot());
    jobIs('running');
    jobIs('finished');
    expect(verifiedBaseline()).not.toBeNull();
    jobIs('running'); // un trabajo iniciado con el paso compacto
    jobIs('finished');
    expect(verifiedBaseline()).not.toBeNull(); // terminar bien la conserva
    jobIs('running');
    jobIs('error');
    expect(verifiedBaseline()).toBeNull();
  });

  it('si el motor rechazó el inicio, el intento no queda como referencia aunque otro trabajo termine bien después', () => {
    noteFullConfirm(snapshot());
    noteStartRejected();
    jobIs('running');
    jobIs('finished');
    expect(verifiedBaseline()).toBeNull();
  });

  it('una referencia ya verificada no la borra un rechazo posterior de otro inicio', () => {
    noteFullConfirm(snapshot());
    jobIs('running');
    jobIs('finished');
    noteStartRejected();
    expect(verifiedBaseline()).not.toBeNull();
  });
});
