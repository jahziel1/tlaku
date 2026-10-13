import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockApi, type MockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import { setApi } from '@/api';
import type { StatusReport } from '@/api/types';
import { createAppStore, bindMachineEvents } from './index';
import {
  INITIAL_MACHINE_DATA,
  jobElapsedSeconds,
  jogBlockedReason,
  LOG_LIMIT,
  reduceMachine,
  selectAlarm,
  selectFirmwareFamily,
  selectHeadDoc,
  selectIsVirtual,
  sendBlockedReason,
  type MachineData,
  type MachineEvent,
} from './machine';

const report = (over: Partial<StatusReport> = {}): StatusReport => ({
  state: 'idle',
  machine_pos: [0, 0],
  work_pos: [0, 0],
  feed: 0,
  spindle: 0,
  buffer: { planner: 15, rx: 127 },
  ...over,
});

/** Aplica una secuencia de eventos con un reloj que avanza 1 s por evento. */
function run(events: MachineEvent[], start: MachineData = INITIAL_MACHINE_DATA, t0 = 1000): MachineData {
  return events.reduce((m, ev, i) => reduceMachine(m, ev, t0 + i * 1000), start);
}

const connect: MachineEvent = { type: 'connection', connection: { connected: true, port: '/dev/x', error: null } };
const disconnect: MachineEvent = { type: 'connection', connection: { connected: false, port: null, error: null } };
const job = (state: 'started' | 'paused' | 'finished' | 'cancelled' | 'error', message = '', position_lost = false): MachineEvent => ({
  type: 'job',
  job: { state, message, position_lost },
});

describe('reducer de la máquina: estado y alarma', () => {
  it('guarda el último reporte de estado', () => {
    const m = run([connect, { type: 'status', report: report({ state: 'run', feed: 1500 }) }]);
    expect(m.status?.state).toBe('run');
    expect(m.status?.feed).toBe(1500);
  });

  it('recuerda la causa de la alarma que llega por el registro y la olvida al salir de ella', () => {
    let m = run([
      connect,
      { type: 'log', log: { dir: 'in', text: 'ALARM:9: fallo de homing' } },
      { type: 'status', report: report({ state: 'alarm' }) },
    ]);
    expect(m.alarmCode).toBe(9);
    expect(selectAlarm(m)).toEqual({ code: 9 });
    // Un reporte de homing no borra la causa: sigue la recuperación.
    m = run([{ type: 'status', report: report({ state: 'home' }) }], m);
    expect(m.alarmCode).toBe(9);
    m = run([{ type: 'status', report: report({ state: 'idle' }) }], m);
    expect(m.alarmCode).toBeNull();
    expect(selectAlarm(m)).toBeUndefined();
  });

  it('una alarma sin línea ALARM:N se muestra sin causa', () => {
    const m = run([connect, { type: 'status', report: report({ state: 'alarm' }) }]);
    expect(selectAlarm(m)).toEqual({ code: null });
  });

  it('solo las líneas recibidas cuentan como alarma', () => {
    const m = run([{ type: 'log', log: { dir: 'out', text: 'ALARM:3' } }]);
    expect(m.alarmCode).toBeNull();
  });

  it('una alarma con un trabajo en curso lo da por fallido', () => {
    const m = run([connect, job('started'), { type: 'status', report: report({ state: 'alarm' }) }]);
    expect(m.jobState).toBe('error');
    expect(m.jobEndedAt).not.toBeNull();
  });
});

describe('reducer de la máquina: trabajo', () => {
  it('un trabajo nuevo reinicia progreso, error y cronómetro', () => {
    let m = run([connect, { type: 'progress', progress: { acked: 5, total: 10 } }]);
    m = { ...m, jobError: 'antes' };
    m = reduceMachine(m, job('started'), 5000);
    expect(m.jobState).toBe('running');
    expect(m.progress).toBeNull();
    expect(m.jobError).toBeNull();
    expect(m.jobStartedAt).toBe(5000);
  });

  it('el tiempo transcurrido no cuenta las pausas', () => {
    let m = reduceMachine(INITIAL_MACHINE_DATA, job('started'), 0);
    m = reduceMachine(m, job('paused'), 10_000);
    expect(jobElapsedSeconds(m, 40_000)).toBe(10); // en pausa el reloj está parado
    m = reduceMachine(m, job('started'), 40_000); // reanudar
    expect(m.jobState).toBe('running');
    expect(m.jobPausedMs).toBe(30_000);
    expect(jobElapsedSeconds(m, 45_000)).toBe(15);
    m = reduceMachine(m, job('finished', 'ok'), 50_000);
    expect(m.jobState).toBe('finished');
    expect(jobElapsedSeconds(m, 999_999)).toBe(20); // al terminar queda fijo
  });

  it('reanudar no borra el progreso', () => {
    let m = reduceMachine(INITIAL_MACHINE_DATA, job('started'), 0);
    m = reduceMachine(m, { type: 'progress', progress: { acked: 3, total: 9 } }, 1);
    m = reduceMachine(m, job('paused'), 2);
    m = reduceMachine(m, job('started'), 3);
    expect(m.progress).toEqual({ acked: 3, total: 9 });
  });

  it('cancelado y error conservan el mensaje del motor', () => {
    expect(run([job('cancelled', 'Cancelado por el usuario.')]).jobMessage).toBe('Cancelado por el usuario.');
    expect(run([job('error', 'línea 4: error:20')])).toMatchObject({ jobState: 'error', jobMessage: 'línea 4: error:20' });
  });
});

describe('reducer de la máquina: conexión y registro', () => {
  it('al desconectar se limpia todo menos el registro', () => {
    const m = run([
      connect,
      { type: 'log', log: { dir: 'in', text: "Grbl 1.1h ['$' for help]" } },
      { type: 'status', report: report() },
      { type: 'settings', settings: { 32: 1 } },
      job('started'),
      disconnect,
    ]);
    expect(m.connection.connected).toBe(false);
    expect(m.status).toBeNull();
    expect(m.settings).toBeNull();
    expect(m.jobState).toBe('idle');
    expect(m.progress).toBeNull();
    expect(m.log).toHaveLength(1);
  });

  it('al conectar descarta ajustes y alarma de la sesión anterior', () => {
    const m = reduceMachine({ ...INITIAL_MACHINE_DATA, settings: { 32: 0 }, alarmCode: 1 }, connect, 0);
    expect(m.settings).toBeNull();
    expect(m.alarmCode).toBeNull();
  });

  it('el registro se recorta al tope y cada línea conserva su id', () => {
    let m = INITIAL_MACHINE_DATA;
    for (let i = 0; i < LOG_LIMIT + 20; i++) m = reduceMachine(m, { type: 'log', log: { dir: 'in', text: String(i) } }, 0);
    expect(m.log).toHaveLength(LOG_LIMIT);
    expect(m.log[0]!.id).toBe(21);
    expect(m.log.at(-1)).toMatchObject({ id: LOG_LIMIT + 20, text: String(LOG_LIMIT + 19) });
    expect(new Set(m.log.map((l) => l.id)).size).toBe(LOG_LIMIT);
  });

  it('es una función pura: no modifica el estado anterior', () => {
    const before = structuredClone(INITIAL_MACHINE_DATA);
    reduceMachine(INITIAL_MACHINE_DATA, { type: 'log', log: { dir: 'in', text: 'x' } }, 0);
    reduceMachine(INITIAL_MACHINE_DATA, job('started'), 0);
    expect(INITIAL_MACHINE_DATA).toEqual(before);
  });
});

describe('selectores', () => {
  const store = createAppStore();
  const withStatus = (report_: StatusReport | null, connected = true) => ({
    ...store.getState(),
    connection: { connected, port: 'x', error: null },
    status: report_,
  });

  it('no se puede mover el cabezal en alarma, en pausa ni sin conexión', () => {
    expect(jogBlockedReason(withStatus(report()))).toBeNull();
    expect(jogBlockedReason(withStatus(report({ state: 'jog' })))).toBeNull();
    expect(jogBlockedReason(withStatus(report({ state: 'alarm' })))).toBe('machine.reason.alarm');
    expect(jogBlockedReason(withStatus(report({ state: 'hold' })))).toBe('machine.reason.paused');
    expect(jogBlockedReason(withStatus(report({ state: 'run' })))).toBe('machine.reason.notIdle');
    expect(jogBlockedReason(withStatus(null, false))).toBe('reason.needMachine');
    expect(jogBlockedReason({ ...withStatus(report()), jobState: 'running' })).toBe('reason.jobRunning');
  });

  it('iniciar exige máquina en reposo (misma regla del preflight del motor)', () => {
    expect(sendBlockedReason(withStatus(report({ state: 'door' })))).not.toBeNull();
  });

  it('el cabezal se da en coordenadas del documento según el origen del perfil', () => {
    const s = {
      ...withStatus(report({ work_pos: [10, 20] })),
      devices: [{ id: 'd', name: 'D', kind: 'grbl' as const, bed: [400, 300] as [number, number], origin: 'top_left' as const } as never],
      activeDeviceId: 'd',
    };
    expect(selectHeadDoc(s)).toEqual([10, 280]);
    expect(selectHeadDoc(withStatus(null))).toBeNull();
    expect(selectHeadDoc(withStatus(report(), false))).toBeNull();
  });
});

describe('acciones del slice contra la máquina simulada', () => {
  let api: MockApi;
  let store: ReturnType<typeof createAppStore>;
  let sent: string[];
  const importSample = () => store.getState().applyImport({ objects: sampleObjects(), new_layers: sampleLayers(), warnings: [] });

  beforeEach(async () => {
    api = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 200, jobSpeedup: 1e6 });
    setApi(api);
    store = createAppStore();
    sent = [];
    const original = api.machineCommand.bind(api);
    api.machineCommand = (a) => {
      sent.push(a.line);
      return original(a);
    };
    await store.getState().loadDevices();
    await bindMachineEvents(api, store);
    await store.getState().refreshPorts();
    await store.getState().connect();
    // Llegan la lectura de $$ y el primer reporte.
    await vi.waitFor(() => expect(store.getState().settings?.[32] !== undefined && store.getState().status !== null).toBe(true));
  });

  it('al conectar lee los ajustes ($$) sin avisar de nada', () => {
    expect(store.getState().settings?.[32]).toBe(1);
    expect(store.getState().notices).toHaveLength(0);
  });

  it('solo sale lo de solo lectura sin confirmación; lo demás exige confirmed', async () => {
    sent.length = 0;
    expect(await store.getState().sendLine('$I')).toBe(true);
    expect(await store.getState().sendLine('?')).toBe(true);
    for (const line of ['$X', '$H', 'G0 X10', 'M3 S100', '$32=0', 'G92 X0 Y0']) {
      expect(await store.getState().sendLine(line), line).toBe(false);
    }
    expect(sent).toEqual(['$I']);
    expect(store.getState().notices.at(-1)?.level).toBe('warning');
  });

  it('confirmado, envía la línea normalizada que se mostró', async () => {
    sent.length = 0;
    expect(await store.getState().sendLine('g1 x5 s0 f100', { confirmed: true })).toBe(true);
    expect(sent).toEqual(['G1X5S0F100']);
  });

  it('rechaza caracteres de tiempo real y líneas que no caben en el búfer', async () => {
    sent.length = 0;
    expect(await store.getState().sendLine('G0 X1 !', { confirmed: true })).toBe(false);
    expect(await store.getState().sendLine('G1' + 'X1'.repeat(80), { confirmed: true })).toBe(false);
    expect(sent).toEqual([]);
  });

  it('sin conexión no envía nada', async () => {
    await store.getState().disconnect();
    sent.length = 0;
    expect(await store.getState().sendLine('$I')).toBe(false);
    expect(sent).toEqual([]);
  });

  it('iniciar abre el diálogo y no envía el trabajo hasta confirmar', async () => {
    importSample();
    store.getState().requestStart();
    expect(store.getState().dialog).toEqual({ kind: 'start' });
    expect(store.getState().jobState).toBe('idle');
    await store.getState().confirmStart();
    expect(store.getState().dialog).toBeNull();
    expect(store.getState().jobState).toBe('running');
  });

  it('el mensaje del motor al rechazar un inicio se guarda tal cual', async () => {
    importSample();
    api.machine.state = 'alarm';
    await store.getState().startJob();
    expect(store.getState().jobError).toMatch(/alarma/i); // el texto exacto es el del motor
  });

  it('fijar el origen guarda la posición de trabajo en las opciones del trabajo', async () => {
    api.machine.pos = [12.3456, 7];
    api.machine.emitStatus();
    store.getState().setUserOriginHere();
    expect(store.getState().jobOptions.user_origin).toEqual([12.346, 7]);
    store.getState().clearUserOrigin();
    expect(store.getState().jobOptions.user_origin).toBeNull();
  });

  it('ir al origen salta la diferencia entre la posición actual y el origen de usuario', async () => {
    api.machine.pos = [50, 40];
    api.machine.emitStatus();
    store.getState().setJobOptions({ user_origin: [20, 10] });
    await store.getState().goToOrigin();
    expect(api.machine.pos).toEqual([20, 10]);
  });

  it('no mueve el cabezal con la máquina en alarma', async () => {
    api.machine.state = 'alarm';
    api.machine.emitStatus();
    const before = [...api.machine.pos];
    await store.getState().jogDir({ x: 1, y: 0 }, 5);
    expect(api.machine.pos).toEqual(before);
  });

  it('el jog continuo se corta con jog cancel al soltar', async () => {
    const realtime: string[] = [];
    const original = api.machineRealtime.bind(api);
    api.machineRealtime = (a) => {
      realtime.push(a.cmd);
      return original(a);
    };
    await store.getState().jogContinuousStart({ x: 1, y: 0 });
    expect(store.getState().jogging).toBe(true);
    await store.getState().jogStop();
    expect(store.getState().jogging).toBe(false);
    expect(realtime).toEqual(['jog_cancel']);
    // Soltar sin haber empezado no envía nada.
    await store.getState().jogStop();
    expect(realtime).toEqual(['jog_cancel']);
  });

  it('confirmar $X o $H envía exactamente esa línea', async () => {
    sent.length = 0;
    await store.getState().confirmAlarmAction('unlock');
    await store.getState().confirmAlarmAction('home');
    expect(sent).toEqual(['$X', '$H']);
  });

  it('el gestor no deja borrar la última máquina ni la conectada', async () => {
    const [first] = store.getState().devices;
    expect(await store.getState().removeDevice(first!.id)).toBe(false); // única
    expect(await store.getState().saveDevice({ ...first!, id: 'otra', name: 'Otra' })).toBe(true);
    store.getState().setActiveDevice(first!.id);
    expect(await store.getState().removeDevice(first!.id)).toBe(false); // conectada
    expect(await store.getState().removeDevice('otra')).toBe(true);
  });

  it('no guarda un perfil inválido', async () => {
    const [first] = store.getState().devices;
    expect(await store.getState().saveDevice({ ...first!, bed: [0, 0] })).toBe(false);
  });
});

const connectTo = (port: string): MachineEvent => ({ type: 'connection', connection: { connected: true, port, error: null } });

describe('reducer de la máquina: posición posiblemente perdida', () => {
  it('un trabajo detenido en movimiento la marca; uno cancelado con calma no', () => {
    const calm = run([connectTo('/dev/x'), job('started'), job('cancelled', 'Trabajo cancelado.')]);
    expect(calm.positionLost).toBe(false);
    const lost = run([connectTo('/dev/x'), job('started'), job('cancelled', 'Trabajo cancelado.', true)]);
    expect(lost.positionLost).toBe(true);
    expect(lost.positionLostPort).toBe('/dev/x');
  });

  it('ALARM:3 en el registro también la marca', () => {
    const m = run([connectTo('/dev/x'), { type: 'log', log: { dir: 'in', text: 'ALARM:3: reinicio durante un movimiento' } }]);
    expect(m.positionLost).toBe(true);
    // Lo enviado por la app no cuenta.
    expect(run([connectTo('/dev/x'), { type: 'log', log: { dir: 'out', text: 'ALARM:3' } }]).positionLost).toBe(false);
  });

  it('sobrevive a la desconexión y sigue al reconectar a la misma máquina', () => {
    const lost = run([connectTo('/dev/x'), job('error', 'ALARM:3', true), disconnect]);
    expect(lost.positionLost).toBe(true);
    expect(lost.connection.connected).toBe(false);
    expect(run([connectTo('/dev/x')], lost).positionLost).toBe(true);
  });

  it('otra máquina no hereda el aviso; la virtual cuenta igual con cualquier velocidad', () => {
    const lost = run([connectTo('/dev/x'), job('error', 'ALARM:3', true), disconnect]);
    expect(run([connectTo('/dev/y')], lost).positionLost).toBe(false);
    const virt = run([connectTo('virtual:falcon2@5'), job('error', 'ALARM:3', true), disconnect]);
    expect(run([connectTo('virtual:falcon2')], virt).positionLost).toBe(true);
    expect(run([connectTo('virtual:grbl11')], virt).positionLost).toBe(false);
  });

  it('un homing que termina bien la borra; con alarma no', () => {
    let m = run([connectTo('/dev/x'), job('error', 'x', true), { type: 'status', report: report({ state: 'home' }) }]);
    expect(m.positionLost).toBe(true);
    expect(run([{ type: 'status', report: report({ state: 'alarm' }) }], m).positionLost).toBe(true);
    m = run([{ type: 'status', report: report({ state: 'idle' }) }], m);
    expect(m.positionLost).toBe(false);
    expect(m.positionLostPort).toBeNull();
  });
});

describe('selectores de máquina virtual y firmware', () => {
  it('la conexión virtual se reconoce por el puerto', () => {
    const conn = (connected: boolean, port: string | null) => ({ connection: { connected, port, error: null } });
    expect(selectIsVirtual(conn(true, 'virtual:falcon2@5'))).toBe(true);
    expect(selectIsVirtual(conn(true, '/dev/ttyUSB0'))).toBe(false);
    expect(selectIsVirtual(conn(false, null))).toBe(false);
  });

  it('la familia del firmware sale del nombre que devolvió la conexión', () => {
    expect(selectFirmwareFamily({ firmware: 'Grbl 1.1f' })).toBe('grbl');
    expect(selectFirmwareFamily({ firmware: 'grblHAL 1.1f' })).toBe('grblhal');
    expect(selectFirmwareFamily({ firmware: 'FluidNC v3.9.7' })).toBe('fluidnc');
    expect(selectFirmwareFamily({ firmware: null })).toBe('unknown');
  });

  it('un estado desconocido bloquea el jog y el inicio como "no está en reposo"', () => {
    const store = createAppStore();
    const s = { ...store.getState(), connection: { connected: true, port: 'x', error: null }, status: report({ state: 'other', other_state: 'Tool' }) };
    expect(jogBlockedReason(s)).toBe('machine.reason.notIdle');
  });
});

describe('máquina virtual en el slice', () => {
  let api: MockApi;
  let store: ReturnType<typeof createAppStore>;
  const settled = () => vi.waitFor(() => expect(store.getState().status !== null && store.getState().settings !== null).toBe(true));
  const pickVirtual = async (id = 'virtual:falcon2') => {
    await store.getState().refreshPorts();
    store.getState().setSelectedPort(id);
  };

  beforeEach(async () => {
    api = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 200, jobSpeedup: 1e6 });
    setApi(api);
    store = createAppStore();
    await store.getState().loadDevices();
    await bindMachineEvents(api, store);
  });

  it('las virtuales se ofrecen al final de la lista, con un prefijo que no choca con los puertos reales', async () => {
    await store.getState().refreshPorts();
    const ports = store.getState().ports;
    const firstVirtual = ports.findIndex((p) => p.startsWith('virtual:'));
    expect(firstVirtual).toBeGreaterThan(0);
    expect(ports.slice(firstVirtual).every((p) => p.startsWith('virtual:'))).toBe(true);
    expect(ports).toEqual(expect.arrayContaining(['virtual:falcon2', 'virtual:grbl11', 'virtual:grblhal', 'virtual:fluidnc']));
  });

  it('conecta con la velocidad elegida y queda marcada como virtual', async () => {
    await pickVirtual();
    store.getState().setVirtualSpeed(5);
    await store.getState().connect();
    await settled();
    expect(store.getState().connection.port).toBe('virtual:falcon2@5');
    expect(store.getState().firmware).toBe('Grbl 1.1f');
    expect(selectIsVirtual(store.getState())).toBe(true);
    expect(store.getState().log.some((l) => l.text.includes('Máquina virtual'))).toBe(true);
  });

  it('con velocidad ×1 el id no lleva sufijo', async () => {
    await pickVirtual('virtual:fluidnc');
    store.getState().setVirtualSpeed(1);
    await store.getState().connect();
    expect(store.getState().connection.port).toBe('virtual:fluidnc');
    expect(selectFirmwareFamily(store.getState())).toBe('fluidnc');
  });

  it('simular una alarma 3 pone la máquina en alarma con su causa', async () => {
    await pickVirtual();
    await store.getState().connect();
    await settled();
    await store.getState().simulateFault('alarm3');
    await vi.waitFor(() => expect(store.getState().status?.state).toBe('alarm'));
    expect(store.getState().alarmCode).toBe(3);
    expect(store.getState().positionLost).toBe(true);
  });

  it('simular un corte de cable desconecta con el error', async () => {
    await pickVirtual();
    await store.getState().connect();
    await settled();
    await store.getState().simulateFault('disconnect');
    await vi.waitFor(() => expect(store.getState().connection.connected).toBe(false));
    expect(store.getState().connection.error).toMatch(/conexión/);
    expect(store.getState().notices.some((n) => n.level === 'error' && /conexión/.test(n.text))).toBe(true);
  });

  it('simular una llama durante un trabajo lo da por fallido y la consola recibe ERROR:05', async () => {
    await pickVirtual();
    await store.getState().connect();
    await settled();
    store.getState().applyImport({ objects: sampleObjects(), new_layers: sampleLayers(), warnings: [] });
    await store.getState().startJob();
    await vi.waitFor(() => expect(store.getState().jobState).toBe('running'));
    await store.getState().simulateFault('flame');
    await vi.waitFor(() => expect(store.getState().jobState).toBe('error'));
    expect(store.getState().jobMessage).toMatch(/ERROR:05.*llama/);
    expect(store.getState().log.some((l) => l.dir === 'in' && l.text.startsWith('ERROR:05'))).toBe(true);
  });

  it('con una máquina real no se puede simular nada', async () => {
    await store.getState().refreshPorts();
    await store.getState().connect(); // el primer puerto es real
    await settled();
    expect(selectIsVirtual(store.getState())).toBe(false);
    await store.getState().simulateFault('flame');
    expect(api.machine.state).toBe('idle');
    expect(store.getState().notices.at(-1)).toMatchObject({ level: 'warning', text: expect.stringContaining('máquina virtual') });
    // Ni siquiera llega al motor: la interfaz lo impide.
    expect(store.getState().log.some((l) => l.text.startsWith('ERROR:05'))).toBe(false);
  });

  it('al reconectar tras perder la posición avisa que conviene hacer homing', async () => {
    await pickVirtual();
    await store.getState().connect();
    await settled();
    await store.getState().simulateFault('alarm3');
    await vi.waitFor(() => expect(store.getState().positionLost).toBe(true));
    await store.getState().disconnect();
    expect(store.getState().positionLost).toBe(true);
    const before = store.getState().notices.length;
    await store.getState().connect();
    await vi.waitFor(() => expect(store.getState().connection.connected).toBe(true));
    const notice = store.getState().notices.slice(before).find((n) => n.level === 'warning');
    expect(notice?.text).toMatch(/homing/);
    expect(store.getState().log.some((l) => l.text.includes('homing') && l.text.startsWith(';'))).toBe(true);
    // Descartar el aviso lo borra.
    store.getState().dismissPositionLost();
    expect(store.getState().positionLost).toBe(false);
  });

  it('fijar el origen a mano también descarta el aviso', async () => {
    await pickVirtual();
    await store.getState().connect();
    await settled();
    await store.getState().simulateFault('alarm3');
    await vi.waitFor(() => expect(store.getState().positionLost).toBe(true));
    store.getState().setUserOriginHere();
    expect(store.getState().positionLost).toBe(false);
  });
});
