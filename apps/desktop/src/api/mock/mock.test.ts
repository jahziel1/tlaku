import { DEFAULT_JOB_OPTIONS } from '@/store/preview';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { COMMAND_NAMES, EVENT_NAMES } from '../api';
import { resolveApiKind } from '../index';
import { polylineBez } from '@/lib/bezpath';
import type { BezPath, DesignObject, JobOptions, LogEvent, StatusReport } from '../types';
import { createMockApi, type MockApi } from './index';
import { sampleDevices, sampleDocument } from './sample';

let api: MockApi | null = null;
afterEach(() => api?.dispose());

const fast = () => (api = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 150, jobSpeedup: 1e6 }));
const jobArgs = () => {
  const options: JobOptions = { start_from: 'absolute', job_origin: 6, user_origin: null, selection: null, layer_order: 'auto', optimization: DEFAULT_JOB_OPTIONS.optimization, use_selection_origin: false, transform: null, tile: null };
  return { doc: sampleDocument(), device: sampleDevices()[0]!, options };
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

describe('selección de implementación', () => {
  it('por defecto es mock', () => {
    expect(resolveApiKind(undefined)).toBe('mock');
    expect(resolveApiKind('mock')).toBe('mock');
    expect(resolveApiKind('tauri')).toBe('tauri');
    expect(resolveApiKind('otra-cosa' as never)).toBe('mock');
  });

  it('cada comando del contrato tiene nombre snake_case para invoke', () => {
    expect(Object.values(COMMAND_NAMES)).toEqual(
      expect.arrayContaining(['import_file', 'compute_job', 'export_gcode', 'save_project', 'load_project', 'list_devices',
        'save_device', 'delete_device', 'list_ports', 'connect', 'disconnect', 'machine_command', 'machine_realtime',
        'jog', 'start_job', 'pause_job', 'resume_job', 'cancel_job', 'frame_job']),
    );
    expect(EVENT_NAMES).toHaveLength(12);
  });
});

describe('diseño y trabajo', () => {
  it('importa un diseño de ejemplo con objetos y capas', async () => {
    const r = await fast().importFile({ path: 'ejemplos/placa.svg' });
    expect(r.objects.length).toBeGreaterThan(2);
    expect(r.new_layers.length).toBeGreaterThan(1);
    expect(r.objects.every((o) => r.new_layers.some((l) => l.color === o.layer))).toBe(true);
    expect(new Set(r.objects.map((o) => o.id)).size).toBe(r.objects.length);
  });

  it('avisa de lo que no importa bien', async () => {
    expect((await fast().importFile({ path: 'plano.dxf' })).warnings.join(' ')).toMatch(/unidades/);
  });

  it('compute_job devuelve movimientos [kind,x,y,power], tiempo y límites coherentes', async () => {
    const p = await fast().computeJob(jobArgs());
    expect(p.moves.length).toBeGreaterThan(400);
    expect(p.moves.length % 4).toBe(0);
    for (let i = 0; i < p.moves.length; i += 4) expect([0, 1]).toContain(p.moves[i]);
    expect(p.total_s).toBeGreaterThan(30);
    expect(p.layers.reduce((a, l) => a + l.seconds, 0)).toBeCloseTo(p.total_s, 3);
    const [[x0, y0], [x1, y1]] = p.bounds;
    expect(x1).toBeGreaterThan(x0);
    expect(y1).toBeGreaterThan(y0);
    expect(p.layers.every((l) => l.explanations.length > 0)).toBe(true);
  });

  it('graba antes de cortar en orden automático', async () => {
    const p = await fast().computeJob(jobArgs());
    expect(p.layers.at(-1)?.mode).toBe('line');
    expect(p.layers[0]?.mode).not.toBe('line');
  });

  it('respeta la selección y la salida de capa', async () => {
    const a = jobArgs();
    a.options.selection = ['obj-estrella'];
    const p = await fast().computeJob(a);
    expect(p.layers).toHaveLength(1);
    const b = jobArgs();
    b.doc.layers.forEach((l) => (l.output = false));
    expect((await fast().computeJob(b)).warnings.join(' ')).toMatch(/nada que enviar/i);
  });

  it('advierte si el trabajo se sale de la cama', async () => {
    const a = jobArgs();
    a.device = { ...a.device, bed: [100, 100] };
    expect((await fast().computeJob(a)).warnings.join(' ')).toMatch(/sale de la cama/);
  });

  it('el texto sin contorno se omite con aviso y con contorno se corta', async () => {
    const text = (outline: BezPath[]): DesignObject => ({
      id: 'txt', layer: '#ff3b4a', transform: [1, 0, 0, 1, 10, 10], locked: false, hidden: false, group: null,
      kind: {
        type: 'text', text: 'Hola', font_family: 'Arial', bold: false, italic: false, size_mm: 10, letter_spacing_mm: 0,
        line_spacing: 1.2, align: 'left', on_path: null, outline,
      },
    });
    const empty = jobArgs();
    empty.doc.objects = [text([])];
    const p = await fast().computeJob(empty);
    expect(p.warnings.join(' ')).toMatch(/el texto no tiene contorno generado/);
    expect(p.moves).toHaveLength(0);
    const full = jobArgs();
    full.doc.objects = [text([polylineBez([[0, 0], [5, 0], [5, 8]], true)])];
    const q = await fast().computeJob(full);
    expect(q.warnings.join(' ')).not.toMatch(/contorno/);
    expect(q.moves.length).toBeGreaterThan(0);
  });

  it('guarda y carga proyectos en memoria', async () => {
    const m = fast();
    const doc = sampleDocument();
    doc.objects = doc.objects.slice(0, 1);
    await m.saveProject({ path: 'a.tlaku', doc });
    expect((await m.loadProject({ path: 'a.tlaku' })).objects).toHaveLength(1);
  });

  it('administra perfiles de dispositivo', async () => {
    const m = fast();
    const [d] = await m.listDevices();
    await m.saveDevice({ device: { ...d!, id: 'nuevo', name: 'Nuevo' } });
    expect((await m.listDevices()).some((x) => x.id === 'nuevo')).toBe(true);
    await m.deleteDevice({ id: 'nuevo' });
    expect((await m.listDevices()).some((x) => x.id === 'nuevo')).toBe(false);
  });
});

describe('máquina simulada', () => {
  it('rechaza comandos sin conexión', async () => {
    const m = fast();
    await expect(m.machineCommand({ line: '$I' })).rejects.toThrow(/conectada/);
    await expect(m.startJob(jobArgs())).rejects.toThrow(/conectada/);
  });

  it('conecta, emite estado y responde a comandos', async () => {
    const m = fast();
    const statuses: StatusReport[] = [];
    const logs: LogEvent[] = [];
    const conn: boolean[] = [];
    await m.on('machine://status', (s) => statuses.push(s));
    await m.on('machine://log', (l) => logs.push(l));
    await m.on('machine://connection', (c) => conn.push(c.connected));
    const { firmware } = await m.connect({ port: '/dev/cu.test', baud: 115200 });
    expect(firmware).toMatch(/Grbl/);
    await vi.waitFor(() => expect(statuses.length).toBeGreaterThan(1));
    expect(statuses[0]).toMatchObject({ state: 'idle', machine_pos: [0, 0] });
    const reply = await m.machineCommand({ line: '$I' });
    expect(reply.at(-1)).toBe('ok');
    expect(logs.some((l) => l.dir === 'out' && l.text === '$I')).toBe(true);
    await expect(m.connect({ port: 'x', baud: 1 })).rejects.toThrow(/Ya hay/);
    await m.disconnect();
    expect(conn).toEqual([true, false]);
  });

  it('cancelar la suscripción deja de entregar eventos', async () => {
    const m = fast();
    let n = 0;
    const off = await m.on('machine://status', () => n++);
    await m.connect({ port: 'p', baud: 1 });
    off();
    const seen = n;
    await sleep(80);
    expect(n).toBe(seen);
  });

  it('jog mueve el cabezal dentro de la cama', async () => {
    const m = fast();
    await m.connect({ port: 'p', baud: 1 });
    await m.jog({ dx: 10, dy: -50, speed: 1000 });
    expect(m.machine.pos).toEqual([10, 0]);
  });

  it('ejecuta un trabajo: progreso creciente y termina', async () => {
    const m = (api = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 500, jobSpeedup: 1e6 }));
    const progress: number[] = [];
    const states: string[] = [];
    await m.on('machine://progress', (p) => progress.push(p.acked));
    await m.on('machine://job', (j) => states.push(j.state));
    await m.connect({ port: 'p', baud: 1 });
    await m.startJob(jobArgs());
    await vi.waitFor(() => expect(states.at(-1)).toBe('finished'), { timeout: 5000 });
    expect(states[0]).toBe('started');
    expect(states.at(-1)).toBe('finished');
    expect(progress.length).toBeGreaterThan(2);
    expect([...progress].sort((a, b) => a - b)).toEqual(progress);
  });

  it('pausa, reanuda y cancela', async () => {
    const m = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 5000 });
    api = m;
    const states: string[] = [];
    await m.on('machine://job', (j) => states.push(j.state));
    await m.connect({ port: 'p', baud: 1 });
    await m.startJob(jobArgs());
    await m.pauseJob();
    expect(m.machine.state).toBe('hold');
    await m.resumeJob();
    expect(m.machine.state).toBe('run');
    await expect(m.startJob(jobArgs())).rejects.toThrow(/en curso/);
    await m.cancelJob();
    expect(states).toEqual(['started', 'paused', 'started', 'cancelled']);
    expect(m.machine.state).toBe('idle');
  });

  it('el encuadre recorre el rectángulo sin iniciar un trabajo', async () => {
    const m = fast();
    await m.connect({ port: 'p', baud: 1 });
    await m.frameJob({ ...jobArgs(), speed: 6000 });
    expect(m.machine.state).toBe('run');
    await vi.waitFor(() => expect(m.machine.state).toBe('idle'), { timeout: 12_000 });
  }, 15_000);
});

describe('paridad con el motor real', () => {
  it('compute_job entrega move_times: uno por movimiento, crecientes y terminando en total_s', async () => {
    const p = await fast().computeJob(jobArgs());
    expect(p.move_times).toHaveLength(p.moves.length / 4);
    expect(p.move_times.every((t, i) => i === 0 || t >= p.move_times[i - 1]!)).toBe(true);
    expect(p.move_times.at(-1)).toBeCloseTo(p.total_s, 6);
  });

  it('start_job y frame_job se niegan en Alarm con el consejo de qué hacer, sin iniciar nada', async () => {
    const m = fast();
    await m.connect({ port: 'p', baud: 1 });
    m.machine.triggerAlarm(1);
    expect(m.machine.state).toBe('alarm');
    for (const call of [() => m.startJob(jobArgs()), () => m.frameJob({ ...jobArgs(), speed: 1000 })]) {
      const e = await call().then(
        () => '',
        (err: Error) => err.message,
      );
      expect(e).toMatch(/ALARMA/);
      expect(e).toMatch(/No se transmitió nada/);
      expect(e).toMatch(/\$H/);
      expect(e).toMatch(/\$X/);
    }
    expect(m.machine.state).toBe('alarm');
  });

  it('el jog se rechaza en alarma y $X desbloquea; $H hace homing', async () => {
    const m = fast();
    await m.connect({ port: 'p', baud: 1 });
    m.machine.triggerAlarm(1);
    await expect(m.jog({ dx: 1, dy: 1, speed: 500 })).rejects.toThrow(/rechazó el movimiento/);
    expect((await m.machineCommand({ line: 'G0 X1' })).at(-1)).toMatch(/^error:9: /);
    expect((await m.machineCommand({ line: '$X' })).at(-1)).toBe('ok');
    expect(m.machine.state).toBe('idle');
    await m.jog({ dx: 5, dy: 5, speed: 500 });
    m.machine.triggerAlarm(3);
    await m.machineCommand({ line: '$H' });
    expect(m.machine.state).toBe('idle');
    expect(m.machine.pos).toEqual([0, 0]);
  });

  it('jog valida los datos y jog_cancel detiene el movimiento sin tocar una alarma', async () => {
    const m = fast();
    await m.connect({ port: 'p', baud: 1 });
    await expect(m.jog({ dx: 1, dy: 1, speed: 0 })).rejects.toThrow(/mayor que 0/);
    await m.jog({ dx: 10, dy: 0, speed: 800 });
    expect(m.machine.state).toBe('jog');
    await m.machineRealtime({ cmd: 'jog_cancel' });
    expect(m.machine.state).toBe('idle');
    m.machine.triggerAlarm();
    await m.machineRealtime({ cmd: 'jog_cancel' });
    expect(m.machine.state).toBe('alarm');
  });

  it('los comandos de línea siguen el formato real y se rechazan con un trabajo en curso', async () => {
    const m = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 5000 });
    api = m;
    await m.connect({ port: 'p', baud: 1 });
    expect(await m.machineCommand({ line: 'ZZ' })).toEqual(['error:20: comando G-code no soportado o no válido']);
    await expect(m.machineCommand({ line: '!' })).rejects.toThrow(/tiempo real/);
    await m.startJob(jobArgs());
    await expect(m.machineCommand({ line: '$$' })).rejects.toThrow(/trabajo en curso/);
    await expect(m.jog({ dx: 1, dy: 1, speed: 100 })).rejects.toThrow(/trabajo en curso/);
    await m.cancelJob();
  });

  it('el trabajo simulado reproduce move_times, emite estado y termina en reposo', async () => {
    const m = (api = createMockApi({ latencyMs: 0, statusIntervalMs: 15, minJobMs: 400, jobSpeedup: 1e6 }));
    const seen: [number, number][] = [];
    await m.on('machine://status', (s) => seen.push(s.machine_pos));
    await m.connect({ port: 'p', baud: 1 });
    await m.startJob(jobArgs());
    await vi.waitFor(() => expect(m.machine.state).toBe('idle'), { timeout: 5000 });
    expect(seen.length).toBeGreaterThan(5);
  });
});
