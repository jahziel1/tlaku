import { describe, expect, it } from 'vitest';
import type { ConnectionEvent, JobEvent } from '../types';
import { MOCK_VIRTUAL_PORTS, MockMachine } from './machine';

const quick = { jobSpeedup: 1, minJobMs: 100, statusIntervalMs: 1000, latencyMs: 0 };

describe('máquina simulada: puertos virtuales', () => {
  it('ofrece las mismas virtuales que el motor real, con prefijo', () => {
    expect(MOCK_VIRTUAL_PORTS.every((p) => p.startsWith('virtual:'))).toBe(true);
    expect(MOCK_VIRTUAL_PORTS).toContain('virtual:falcon2');
  });

  it('el firmware sale del perfil virtual y la velocidad del id no importa', () => {
    const m = new MockMachine(quick);
    expect(m.connect('virtual:falcon2@5')).toBe('Grbl 1.1f');
    expect(m.virtual).toBe(true);
    m.disconnect();
    expect(m.virtual).toBe(false);
    expect(m.connect('virtual:grblhal')).toBe('grblHAL 1.1f');
    m.dispose();
    const real = new MockMachine(quick);
    expect(real.connect('/dev/ttyUSB0')).toBe('Grbl 1.1h');
    expect(real.virtual).toBe(false);
    real.dispose();
  });

  it('simular fallas solo existe con una máquina virtual', () => {
    const m = new MockMachine(quick);
    m.connect('/dev/ttyUSB0');
    expect(() => m.simulateFault('flame')).toThrow(/máquina virtual/);
    m.dispose();
    expect(() => new MockMachine(quick).simulateFault('flame')).toThrow(/conectada/);
  });

  it('una llama deja ERROR:05 en el registro y la alarma 3 pone la máquina en alarma', () => {
    const m = new MockMachine(quick);
    const log: string[] = [];
    const jobs: JobEvent[] = [];
    m.on('machine://log', (l) => log.push(l.text));
    m.on('machine://job', (j) => jobs.push(j));
    m.connect('virtual:falcon2');
    m.simulateFault('flame');
    expect(log.some((l) => l.startsWith('ERROR:05'))).toBe(true);
    m.simulateFault('alarm3');
    expect(m.state).toBe('alarm');
    expect(m.alarmCode).toBe(3);
    expect(log.some((l) => l.startsWith('ALARM:3'))).toBe(true);
    m.dispose();
  });

  it('el corte de cable cierra la conexión con un error', () => {
    const m = new MockMachine(quick);
    const events: ConnectionEvent[] = [];
    m.on('machine://connection', (c) => events.push(c));
    m.connect('virtual:grbl11');
    m.simulateFault('disconnect');
    expect(events.at(-1)).toMatchObject({ connected: false, port: 'virtual:grbl11' });
    expect(events.at(-1)?.error).toMatch(/conexión/);
    expect(m.connected).toBe(false);
  });
});
