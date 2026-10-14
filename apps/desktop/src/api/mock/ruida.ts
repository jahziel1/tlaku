import type { LaserApi } from '../api';
import type { RuidaApplyReport, RuidaBackup, RuidaControllerSettings, RuidaPlannedChange, RuidaSetting, RuidaSettingChange, RuidaSettingsPlan } from '../types';
import type { MockMachine } from './machine';

type Lag = <T>(value: () => T) => Promise<T>;

/** Un ajuste de la lista cerrada del motor (`tlaku_ruida::settings::WRITABLE`) con su valor de partida. */
interface Def {
  key: string;
  addr: string;
  group: RuidaSetting['group'];
  kind: RuidaSetting['kind'];
  min: number;
  max: number;
  signed?: boolean;
  start: number;
  source: string;
}

const d = (key: string, addr: string, group: string, kind: string, min: number, max: number, start: number, source: string, signed = false): Def => ({ key, addr, group, kind, min, max, start, source, signed });

/** Misma lista, orden y rangos que el motor; los valores de partida son los del emulador. */
export const MOCK_RUIDA_DEFS: readonly Def[] = [
  d('g0_speed', '0x005', 'general', 'speed', 1000, 600000, 200000, 'MEM_G0_VELOCITY'),
  d('home_speed', '0x00C', 'general', 'speed', 1000, 600000, 20000, 'MEM_HOME_VELOCITY'),
  ...(['x', 'y'] as const).flatMap((ax, i) => {
    const n = i === 0 ? 0x20 : 0x30;
    const A = ax.toUpperCase();
    const hex = (o: number) => `0x0${(n + o).toString(16).toUpperCase().padStart(2, '0')}`;
    const g = `axis_${ax}`;
    return [
      d(`${ax}_step`, hex(1), g, 'step', 1, 1000000, 6350, `MEM_PRECISION_${A}`),
      d(`${ax}_max_speed`, hex(3), g, 'speed', 1000, 600000, 400000, `MEM_MAX_VELOCITY_${A}`),
      d(`${ax}_start_speed`, hex(4), g, 'speed', 0, 100000, 8000, `MEM_START_VELOCITY_${A}`),
      d(`${ax}_max_accel`, hex(5), g, 'accel', 1000, 50000000, 3000000, `MEM_MAX_ACC_${A}`),
      d(`${ax}_travel`, hex(6), g, 'length', 10000, 3000000, i === 0 ? 600000 : 400000, `MEM_BED_SIZE_${A}`),
      d(`${ax}_button_speed`, hex(7), g, 'speed', 0, 100000, 10000, `MEM_BTN_START_VEL_${A}`),
      d(`${ax}_button_accel`, hex(8), g, 'accel', 1000, 50000000, 500000, `MEM_BTN_ACC_${A}`),
      d(`${ax}_estop_accel`, hex(9), g, 'accel', 1000, 50000000, 5000000, `MEM_ESTP_ACC_${A}`),
      d(`${ax}_home_offset`, hex(10), g, 'length', -1000000, 1000000, 0, `MEM_HOME_OFFSET_${A}`, true),
    ];
  }),
  d('z_step', '0x041', 'axis_z', 'step', 1, 1000000, 6350, 'MEM_PRECISION_Z'),
  d('u_step', '0x051', 'axis_u', 'step', 1, 1000000, 6350, 'MEM_PRECISION_U'),
  d('rotary_pulse', '0x11F', 'rotary', 'raw', 1, 100000000, 5000, 'MEM_ROTATE_PULSE'),
  d('rotary_diameter', '0x121', 'rotary', 'raw', 1, 100000000, 50000, 'MEM_ROTATE_D'),
];

const CARD = '0x65106510';
const FORMAT = 'tlaku-ruida-settings';
const ONLY_RUIDA = 'Esta función es solo de las controladoras Ruida.';
const WRITES_OFF = 'Las escrituras experimentales en Ruida están desactivadas: actívalas en la preferencia «Escrituras experimentales en Ruida» (Preferencias > Máquina o el panel de Ruida). Nada se envió a la controladora.';
const needWrites = (on: boolean) => {
  if (!on) throw new Error(WRITES_OFF);
};

/**
 * Ajustes, respaldos y calibración de la Ruida simulada (modo mock). Aplica las mismas reglas que el
 * motor: lista cerrada, rango, confirmación explícita, respaldo previo, verificación y un paso de
 * eje que no cambia más del doble. EXPERIMENTAL: sin probar con una controladora real.
 */
export function mockRuida(lag: Lag, machine: MockMachine): Pick<
  LaserApi,
  | 'ruidaPrepareStore'
  | 'ruidaFileDelete'
  | 'ruidaFileRun'
  | 'ruidaSettingsRead'
  | 'ruidaSettingsPreview'
  | 'ruidaSettingsPreviewFile'
  | 'ruidaSettingsApply'
  | 'ruidaSettingsRestore'
  | 'ruidaSettingsExport'
  | 'ruidaCalibrate'
  | 'ruidaBackups'
  | 'ruidaBackupText'
> {
  const values = new Map<string, number>(MOCK_RUIDA_DEFS.map((x) => [x.key, x.start]));
  const backups: { name: string; text: string }[] = [];
  let backupSeq = 0;

  const need = () => {
    machine.requireConnection();
    if (!machine.ruida) throw new Error(ONLY_RUIDA);
  };
  const def = (key: string) => {
    const x = MOCK_RUIDA_DEFS.find((y) => y.key === key);
    if (!x) throw new Error(`«${key}» no es un ajuste de Ruida que Tlaku pueda escribir`);
    return x;
  };
  const snapshot = (): RuidaControllerSettings => ({
    model: 'RDC6442S',
    card_id: CARD,
    settings: MOCK_RUIDA_DEFS.map((x) => ({ key: x.key, addr: x.addr, group: x.group, kind: x.kind, value: values.get(x.key) ?? null, min: x.min, max: x.max, source_name: x.source })),
  });
  const plan = (changes: RuidaSettingChange[]): { changes: RuidaPlannedChange[]; unchanged: number } => {
    const seen = new Set<string>();
    const out: RuidaPlannedChange[] = [];
    let unchanged = 0;
    for (const c of changes) {
      const x = def(c.key);
      if (seen.has(c.key)) throw new Error(`la dirección ${x.addr} aparece dos veces en los cambios`);
      seen.add(c.key);
      if (!Number.isInteger(c.value)) throw new Error(`${c.key}: el valor ${c.value} debe ser un número entero`);
      if (c.value < x.min || c.value > x.max) throw new Error(`${x.key} (${x.addr}): el valor ${c.value} está fuera del rango permitido, de ${x.min} a ${x.max}`);
      const cur = values.get(x.key)!;
      if (x.kind === 'step' && (c.value > cur * 2 || c.value * 2 < cur)) throw new Error(`${x.key}: un paso de eje solo puede cambiar hasta el doble o la mitad de una vez (ahora ${cur}, pedido ${c.value})`);
      if (cur === c.value) unchanged += 1;
      else out.push({ key: x.key, addr: x.addr, current: cur, new: c.value });
    }
    // Siempre en el orden de la lista cerrada.
    out.sort((a, b) => MOCK_RUIDA_DEFS.findIndex((y) => y.key === a.key) - MOCK_RUIDA_DEFS.findIndex((y) => y.key === b.key));
    return { changes: out, unchanged };
  };
  const fileText = (): string =>
    JSON.stringify({ format: FORMAT, version: 1, created: new Date().toISOString(), card_id: CARD, model: 'RDC6442S', settings: MOCK_RUIDA_DEFS.map((x) => ({ key: x.key, addr: x.addr, value: values.get(x.key) })) }, null, 2) + '\n';
  const parse = (json: string): { card: string; changes: RuidaSettingChange[] } => {
    let o: unknown;
    try {
      o = JSON.parse(json);
    } catch {
      throw new Error('el archivo no es un respaldo válido de ajustes de Ruida de Tlaku: no es JSON');
    }
    const f = o as { format?: string; version?: number; card_id?: string; settings?: { key: string; addr: string; value: number }[] };
    if (f.format !== FORMAT || f.version !== 1 || !Array.isArray(f.settings) || typeof f.card_id !== 'string') throw new Error('el archivo no es un respaldo válido de ajustes de Ruida de Tlaku: formato');
    for (const e of f.settings) {
      const x = def(e.key);
      if (e.addr.toLowerCase() !== x.addr.toLowerCase()) throw new Error(`el archivo no es un respaldo válido de ajustes de Ruida de Tlaku: ${e.key}: addr ${e.addr}`);
      if (!Number.isInteger(e.value) || e.value < x.min || e.value > x.max) throw new Error(`el archivo no es un respaldo válido de ajustes de Ruida de Tlaku: ${e.key} fuera de rango`);
    }
    return { card: f.card_id, changes: f.settings.map((e) => ({ key: e.key, value: e.value })) };
  };
  const save = (): string => {
    backupSeq += 1;
    const name = `ruida-20261014-1200${String(backupSeq).padStart(2, '0')}.tlakuruida`;
    backups.unshift({ name, text: fileText() });
    return `/mock/ruida-backups/${name}`;
  };
  const apply = (changes: RuidaSettingChange[], confirmed: boolean, expectCard?: string): RuidaApplyReport => {
    if (!confirmed) throw new Error('falta la confirmación explícita del usuario: no se cambió nada en la máquina');
    need();
    if (machine.hasJob()) throw new Error('Hay un trabajo en curso. Espera a que termine o detenlo.');
    if (expectCard !== undefined && expectCard.toLowerCase() !== CARD.toLowerCase()) {
      throw new Error(`el respaldo es de la tarjeta ${expectCard.replace(/^0x/i, '')} y la máquina conectada es la ${CARD.slice(2)}: no se restaura entre modelos distintos`);
    }
    const p = plan(changes);
    if (p.changes.length === 0) return { ...p, backup: null, verified: [], write_error: null, ok: true, restore_advised: false };
    const backup = save();
    for (const c of p.changes) values.set(c.key, c.new);
    const verified = p.changes.map((c) => ({ key: c.key, expected: c.new, actual: values.get(c.key) ?? null, ok: values.get(c.key) === c.new }));
    return { ...p, backup, verified, write_error: null, ok: true, restore_advised: false };
  };

  return {
    ruidaPrepareStore: ({ name, overwrite, experimental_writes }) =>
      lag(() => {
        need();
        if (overwrite) needWrites(experimental_writes);
        if (name !== null && !/^[A-Za-z0-9_-]{1,8}$/.test(name)) throw new Error(`«${name}» no es un nombre válido: de 1 a 8 letras, cifras, «_» o «-» (sin espacios ni acentos)`);
        machine.storePlan = { name, overwrite };
      }),
    ruidaFileDelete: ({ index, name, confirmed, experimental_writes }) =>
      lag(() => {
        needWrites(experimental_writes);
        if (!confirmed) throw new Error('falta la confirmación explícita del usuario: no se cambió nada en la máquina');
        need();
        if (machine.hasJob()) throw new Error('Hay un trabajo en curso. Espera a que termine o detenlo.');
        const found = machine.storedDocs[index - 1];
        if (found === undefined) throw new Error(`no existe el documento número ${index} en la memoria de la controladora`);
        if (found.toLowerCase() !== name.toLowerCase()) throw new Error(`el documento número ${index} se llama «${found}», no «${name}»: la lista cambió; no se hizo nada`);
        machine.storedDocs.splice(index - 1, 1);
        return { index, name, confirmed: true };
      }),
    ruidaFileRun: ({ index, name, confirmed, experimental_writes }) =>
      lag(() => {
        needWrites(experimental_writes);
        if (!confirmed) throw new Error('falta la confirmación explícita del usuario: no se cambió nada en la máquina');
        need();
        const found = machine.storedDocs[index - 1];
        if (found === undefined) throw new Error(`no existe el documento número ${index} en la memoria de la controladora`);
        if (found.toLowerCase() !== name.toLowerCase()) throw new Error(`el documento número ${index} se llama «${found}», no «${name}»: la lista cambió; no se hizo nada`);
        machine.runStoredDocument(found);
        return { index, name, confirmed: true };
      }),
    ruidaSettingsRead: () =>
      lag(() => {
        need();
        return snapshot();
      }),
    ruidaSettingsPreview: ({ changes }) =>
      lag(() => {
        need();
        return { ...plan(changes), warnings: [] };
      }),
    ruidaSettingsPreviewFile: ({ json }) =>
      lag((): RuidaSettingsPlan => {
        need();
        const f = parse(json);
        const warnings = f.card.toLowerCase() === CARD.toLowerCase() ? [] : [`El respaldo es de la tarjeta ${f.card.replace(/^0x/i, '')} y la máquina es la ${CARD.slice(2)}: no se podrá restaurar.`];
        return { ...plan(f.changes), warnings };
      }),
    ruidaSettingsApply: ({ changes, confirmed, experimental_writes }) =>
      lag(() => {
        needWrites(experimental_writes);
        return apply(changes, confirmed);
      }),
    ruidaSettingsRestore: ({ json, confirmed, experimental_writes }) =>
      lag(() => {
        needWrites(experimental_writes);
        const f = parse(json);
        return apply(f.changes, confirmed, f.card);
      }),
    ruidaSettingsExport: () =>
      lag(() => {
        need();
        return fileText();
      }),
    ruidaCalibrate: ({ key, commanded_mm, measured_mm, experimental_writes }) =>
      lag(() => {
        needWrites(experimental_writes);
        need();
        const x = def(key);
        if (x.kind !== 'step') throw new Error(`«${key}» no es el paso de un eje: solo se calibran X, Y, Z y U`);
        const ok = (v: number) => Number.isFinite(v) && v > 0 && v <= 10000;
        if (!ok(commanded_mm)) throw new Error('la distancia pedida debe ser un número mayor que 0 y no pasar de 10 000 mm');
        if (!ok(measured_mm)) throw new Error('la distancia medida debe ser un número mayor que 0 y no pasar de 10 000 mm');
        const ratio = measured_mm / commanded_mm;
        if (ratio < 0.8 || ratio > 1.25) throw new Error(`la corrección pedida (${((ratio - 1) * 100).toFixed(1)} %) es mayor que ±25 %: casi seguro es un error de medida`);
        const current = values.get(key)!;
        const proposed = Math.round(current * ratio);
        if (proposed === current) throw new Error(`el cambio (${((ratio - 1) * 100).toFixed(3)} %) es menor que la resolución del ajuste (el valor actual es ${current} y el cambio mínimo es de 1): no se puede corregir con esta resolución`);
        return { key, current, proposed, ratio, steps_per_mm_before: 1000 / current, steps_per_mm_after: 1000 / proposed };
      }),
    ruidaBackups: () =>
      lag((): RuidaBackup[] => {
        need();
        return backups.map((b) => ({ name: b.name, path: `/mock/ruida-backups/${b.name}`, created: new Date().toISOString(), card_id: CARD }));
      }),
    ruidaBackupText: ({ name }) =>
      lag(() => {
        const b = backups.find((x) => x.name === name);
        if (!b) throw new Error(`«${name}» no es el nombre de un respaldo de ajustes de Ruida`);
        return b.text;
      }),
  };
}
