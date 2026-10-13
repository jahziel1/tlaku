import { afterEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { setLanguage } from '@/i18n';
import { parseRawSettings } from './MachineSettingsLogic';
import { readMachineSettings, saveBackupFile, openBackupFile } from './MachineSettingsIO';
import { outcomeNotice } from './MachineSettingsReport';
import { classifyReply, settingsBlockedReason, verifyWritten, writeSettingLines, type SendFn } from './MachineSettingsWrite';

const never = () => null;

describe('respuesta a una escritura', () => {
  it('solo `ok` en la última línea cuenta; los avisos de antes no estorban', () => {
    expect(classifyReply(['ok'])).toEqual({ kind: 'ok' });
    expect(classifyReply(['[MSG:Restart to apply]', 'ok', ''])).toEqual({ kind: 'ok' });
  });

  it('`error:N` (también con texto) es un error con su código', () => {
    expect(classifyReply(['error:3'])).toEqual({ kind: 'error', code: 3, text: 'error:3' });
    expect(classifyReply(['error:9: G-code bloqueado'])).toMatchObject({ kind: 'error', code: 9 });
  });

  it('una alarma, algo raro o nada no es `ok`', () => {
    expect(classifyReply(['ALARM:1'])).toEqual({ kind: 'reply', text: 'ALARM:1' });
    expect(classifyReply(['ok', 'ALARM:1'])).toMatchObject({ kind: 'reply' });
    expect(classifyReply([])).toEqual({ kind: 'reply', text: '' });
  });
});

describe('escritura una a una', () => {
  it('envía en orden, esperando cada respuesta, y nunca dos a la vez', async () => {
    const calls: string[] = [];
    let active = 0;
    let maxActive = 0;
    const send: SendFn = async (line) => {
      active++;
      maxActive = Math.max(maxActive, active);
      calls.push(line);
      await new Promise((r) => setTimeout(r, 1));
      active--;
      return ['ok'];
    };
    const progress: number[] = [];
    const out = await writeSettingLines(['$30=1000', '$32=1', '$130=400'], { send, blocked: never, onProgress: (d) => progress.push(d) });
    expect(calls).toEqual(['$30=1000', '$32=1', '$130=400']);
    expect(maxActive).toBe(1);
    expect(out).toEqual({ written: ['$30=1000', '$32=1', '$130=400'], failure: null, pending: [] });
    expect(progress).toEqual([1, 2, 3]);
  });

  it('se detiene en el primer error:N y deja el resto pendiente sin enviarlo', async () => {
    const calls: string[] = [];
    const send: SendFn = async (line) => {
      calls.push(line);
      return line === '$32=1' ? ['error:3'] : ['ok'];
    };
    const out = await writeSettingLines(['$30=1000', '$32=1', '$130=400', '$131=400'], { send, blocked: never });
    expect(calls).toEqual(['$30=1000', '$32=1']);
    expect(out.written).toEqual(['$30=1000']);
    expect(out.failure).toMatchObject({ line: '$32=1', kind: 'error', code: 3 });
    expect(out.pending).toEqual(['$130=400', '$131=400']);
  });

  it('una respuesta que no es ok, o un envío que lanza, también paran', async () => {
    const odd = await writeSettingLines(['$1=1', '$2=2'], { send: async () => ['ALARM:1'], blocked: never });
    expect(odd.failure).toMatchObject({ line: '$1=1', kind: 'reply' });
    expect(odd.pending).toEqual(['$2=2']);
    const boom = await writeSettingLines(['$1=1', '$2=2'], {
      send: async () => {
        throw new Error('puerto cerrado');
      },
      blocked: never,
    });
    expect(boom.failure).toMatchObject({ line: '$1=1', kind: 'exception', text: 'puerto cerrado' });
  });

  it('si pierde la conexión o empieza un trabajo entre dos líneas, no envía la siguiente', async () => {
    const calls: string[] = [];
    let n = 0;
    const out = await writeSettingLines(['$1=1', '$2=2', '$3=3'], {
      send: async (l) => {
        calls.push(l);
        return ['ok'];
      },
      blocked: () => (n++ === 1 ? 'reason.jobRunning' : null),
    });
    expect(calls).toEqual(['$1=1']);
    expect(out.failure).toMatchObject({ line: '$2=2', kind: 'blocked', blockedKey: 'reason.jobRunning' });
    expect(out.pending).toEqual(['$3=3']);
  });

  it('rechaza una segunda escritura mientras otra sigue en curso y se libera al terminar', async () => {
    let release: () => void = () => {};
    const slow: SendFn = () => new Promise((r) => (release = () => r(['ok'])));
    const first = writeSettingLines(['$1=1'], { send: slow, blocked: never });
    await expect(writeSettingLines(['$2=2'], { send: async () => ['ok'], blocked: never })).rejects.toThrow(/en curso/);
    release();
    await first;
    await expect(writeSettingLines(['$2=2'], { send: async () => ['ok'], blocked: never })).resolves.toMatchObject({ failure: null });
  });

  it('bloqueo: sin conexión o con trabajo en curso', () => {
    expect(settingsBlockedReason({ connection: { connected: false, port: null, error: null }, jobState: 'idle' })).toBe('reason.needMachine');
    expect(settingsBlockedReason({ connection: { connected: true, port: 'x', error: null }, jobState: 'running' })).toBe('reason.jobRunning');
    expect(settingsBlockedReason({ connection: { connected: true, port: 'x', error: null }, jobState: 'idle' })).toBeNull();
  });
});

describe('verificación y avisos', () => {
  const reread = parseRawSettings(['$30=1000', '$130=410.000']).settings;

  it('compara lo escrito con lo que informa la máquina después', () => {
    expect(verifyWritten(['$30=1000', '$130=410'], reread)).toEqual([]);
    expect(verifyWritten(['$30=500', '$99=1'], reread)).toEqual([
      { id: 30, expected: '500', actual: '1000' },
      { id: 99, expected: '1', actual: null },
    ]);
  });

  afterEach(() => setLanguage('es'));

  it('el aviso de un fallo dice la línea, el error y cuáles quedaron pendientes', () => {
    const notice = outcomeNotice(
      { written: ['$30=1000'], failure: { line: '$32=1', kind: 'error', code: 3, text: 'error:3' }, pending: ['$130=400'] },
      'grbl',
      null,
    );
    expect(notice.tone).toBe('error');
    expect(notice.text).toContain('$32=1');
    expect(notice.text).toContain('error:3');
    expect(notice.details).toEqual(['Quedaron sin escribir (2):', '$32=1', '$130=400']);
  });

  it('éxito comprobado, éxito sin poder releer y discrepancia al releer', () => {
    const done = { written: ['$30=1000'], failure: null, pending: [] };
    expect(outcomeNotice(done, 'grbl', []).tone).toBe('ok');
    expect(outcomeNotice(done, 'grbl', null).tone).toBe('warn');
    const warn = outcomeNotice(done, 'grbl', [{ id: 30, expected: '500', actual: '1000' }]);
    expect(warn.tone).toBe('warn');
    expect(warn.details?.[0]).toContain('$30');
  });

  it('el aviso sale también en inglés', () => {
    setLanguage('en');
    const n = outcomeNotice({ written: [], failure: { line: '$1=1', kind: 'reply', code: null, text: 'ALARM:1' }, pending: [] }, 'grbl', null);
    expect(n.text).toContain('Stopped at $1=1');
    expect(n.details?.[0]).toBe('Not written (1):');
  });
});

describe('contra la máquina simulada', () => {
  let api: MockApi | null = null;
  afterEach(() => {
    api?.dispose();
    api = null;
    vi.restoreAllMocks();
  });

  async function connected(): Promise<MockApi> {
    api = createMockApi({ latencyMs: 0 });
    setApi(api);
    await api.connect({ port: 'virtual:grbl11', baud: 115200 });
    return api;
  }

  it('lee $$ con el texto crudo de cada valor', async () => {
    await connected();
    const { settings } = await readMachineSettings();
    expect(settings.find((s) => s.id === 130)?.raw).toBe('400.000');
    expect(settings.find((s) => s.id === 32)?.raw).toBe('1');
  });

  it('escribe en orden, refleja los valores en $$ y se detiene en el primer error', async () => {
    await connected();
    const out = await writeSettingLines(['$30=800', '$999=1', '$32=0'], { blocked: never });
    expect(out.written).toEqual(['$30=800']);
    expect(out.failure).toMatchObject({ line: '$999=1', kind: 'error', code: 3 });
    expect(out.pending).toEqual(['$32=0']);
    const { settings } = await readMachineSettings();
    expect(settings.find((s) => s.id === 30)?.raw).toBe('800');
    // Lo pendiente no se tocó.
    expect(settings.find((s) => s.id === 32)?.raw).toBe('1');
  });

  it('un valor que la máquina rechaza (negativo) detiene la escritura', async () => {
    await connected();
    const out = await writeSettingLines(['$130=410', '$131=-1', '$100=81'], { blocked: never });
    expect(out.written).toEqual(['$130=410']);
    expect(out.failure).toMatchObject({ line: '$131=-1', code: 4 });
    expect(out.pending).toEqual(['$100=81']);
  });

  it('tras escribir, la relectura confirma los valores', async () => {
    await connected();
    const lines = ['$32=0', '$130=410.5'];
    const out = await writeSettingLines(lines, { blocked: never });
    const { settings } = await readMachineSettings();
    expect(verifyWritten(out.written, settings)).toEqual([]);
    expect(settings.find((s) => s.id === 130)?.raw).toBe('410.500');
  });

  it('el respaldo se guarda y se lee por ruta en el modo simulado', async () => {
    await connected();
    const path = await saveBackupFile('; Tlaku: respaldo de ajustes GRBL\n$0=10\n');
    expect(path).toBe('ajustes-grbl.tlakuset');
    const file = await openBackupFile();
    expect(file).toMatchObject({ name: 'ajustes-grbl.tlakuset', text: '; Tlaku: respaldo de ajustes GRBL\n$0=10\n' });
    await expect(api!.readSettingsBackup({ path: 'otra.tlakuset' })).rejects.toThrow(/no existe/);
    await expect(api!.writeSettingsBackup({ path: 'x.json', text: '' })).rejects.toThrow(/extensión/);
  });
});
