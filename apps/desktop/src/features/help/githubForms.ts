import type { DeviceProfile, MachineClass, MaterialPreset } from '@/api/types';
import type { DeviceKind } from '@/bindings/DeviceKind';
import { isGrblFamily } from '@/lib/device';
import { effectiveRole } from '@/features/materials/role';
import { classKind, classWatts, laserKindOf } from '@/features/materials/machineClass';
import type { LogEntry } from '@/store/machine/reducer';
import { REPO_URL } from './links';

/**
 * Formularios de incidencias de GitHub (`.github/ISSUE_TEMPLATE/*.yml`) y cómo prellenarlos desde la
 * app. GitHub llena un campo cuando la dirección trae `?<id del campo>=<valor>`; los ids de abajo
 * deben ser los de los formularios (una prueba lo comprueba) y, en los campos desplegables, el valor
 * debe ser el texto exacto de una de sus opciones. La app solo arma la dirección: quien la abre revisa
 * todo en GitHub y decide si publica.
 */
export type IssueForm = 'bug' | 'machine' | 'material';

export const FORM_FILES: Record<IssueForm, string> = {
  bug: 'bug_report.yml',
  machine: 'machine_report.yml',
  material: 'material_settings.yml',
};

/** Campos que la app puede llenar en cada formulario (ids del YAML). */
export const FORM_FIELDS: Record<IssueForm, readonly string[]> = {
  bug: ['version', 'system', 'machine', 'controller'],
  machine: ['version', 'system', 'model', 'controller', 'firmware', 'grbl_i', 'connection'],
  material: ['laser_type', 'laser_watts', 'machine', 'material', 'thickness', 'operation', 'speed', 'power', 'passes', 'interval', 'air'],
};

/** Opciones exactas de los campos desplegables que se prellenan (deben coincidir con el YAML). */
export const DROPDOWN_VALUES = {
  connection: {
    serial: 'USB (serial) / USB (serie)',
    network: 'Network (TCP, WebSocket or UDP) / Red (TCP, WebSocket o UDP)',
    usbDirect: 'USB direct (K40, galvo) / USB directo (K40, galvo)',
  },
  laser_type: { diode: 'Diode / Diodo', co2: 'CO2', fiber: 'Fiber / Fibra' },
  operation: { cut: 'Cut / Corte', engrave: 'Engrave / Grabado', score: 'Score or mark / Marcado' },
  air: { yes: 'Yes / Sí', no: 'No' },
} as const;

/** Largo máximo de un valor y de toda la dirección (los navegadores y GitHub cortan las muy largas). */
const MAX_VALUE = 1500;
const MAX_URL = 6500;

export type FormValues = Partial<Record<string, string | number | null | undefined>>;

/** Dirección del formulario con los campos prellenados. Los vacíos se omiten; si no cabe, se recortan los más largos. */
export function formUrl(form: IssueForm, values: FormValues, title?: string): string {
  const entries: [string, string][] = [];
  for (const [id, raw] of Object.entries(values)) {
    if (raw === null || raw === undefined) continue;
    const text = String(raw).trim();
    if (text !== '') entries.push([id, text.slice(0, MAX_VALUE)]);
  }
  const build = () => {
    const url = new URL(`${REPO_URL}/issues/new`);
    url.searchParams.set('template', FORM_FILES[form]);
    if (title) url.searchParams.set('title', title);
    for (const [id, text] of entries) url.searchParams.set(id, text);
    return url.toString();
  };
  let out = build();
  while (out.length > MAX_URL && entries.length > 0) {
    const longest = entries.reduce((best, e, i) => (e[1].length > entries[best]![1].length ? i : best), 0);
    const [id, text] = entries[longest]!;
    if (text.length > 200) entries[longest] = [id, text.slice(0, Math.floor(text.length / 2))];
    else entries.splice(longest, 1);
    out = build();
  }
  return out;
}

/** Texto del sistema para el campo «Operating system». */
export function systemLine(nav: { platform?: string; userAgent?: string } = {}): string {
  const platform = nav.platform?.trim();
  const ua = nav.userAgent?.trim();
  if (platform && ua) return `${platform} (${ua})`.slice(0, 300);
  return (platform || ua || '').slice(0, 300);
}

const KIND_NAME: Record<DeviceKind, string> = {
  grbl: 'GRBL',
  grbl_m3: 'GRBL (M3)',
  grbl_hal: 'grblHAL',
  fluid_nc: 'FluidNC',
  grbl_lpc: 'GRBL-LPC',
  smoothie: 'Smoothie',
  marlin: 'Marlin',
  custom_gcode: 'Custom G-code',
  ruida: 'Ruida',
  lhy_micro: 'K40 (M2 Nano)',
  galvo: 'Galvo (JCZ/BJJCZ)',
};

export const controllerName = (kind: DeviceKind): string => KIND_NAME[kind];

/** Lo que la app ya sabe de la máquina activa. Nada de esto se pide a la máquina: es lo que ya hay en pantalla. */
export interface MachineContext {
  /** Nombre del perfil activo; vacío si la conexión es una máquina virtual (no es hardware real). */
  model: string;
  controller: string;
  /** Saludo del firmware si hay una máquina conectada. */
  firmware: string;
  /** Última respuesta a `$I` que ya tiene la consola (solo GRBL conectado). */
  info: string;
  connection: string;
  virtual: boolean;
}

/** Líneas de la última respuesta a `$I` en el registro de la consola, o `''`. */
export function lastInfoResponse(log: readonly Pick<LogEntry, 'dir' | 'text'>[]): string {
  let start = -1;
  for (let i = log.length - 1; i >= 0; i--) {
    const e = log[i]!;
    if (e.dir === 'out' && e.text.trim() === '$I') {
      start = i;
      break;
    }
  }
  if (start < 0) return '';
  const lines: string[] = [];
  for (const e of log.slice(start + 1)) {
    if (e.dir === 'out') break;
    lines.push(e.text.trim());
    if (e.text.trim().toLowerCase() === 'ok') break;
  }
  return lines.join('\n');
}

export function machineContext(
  device: DeviceProfile,
  connected: boolean,
  isVirtual: boolean,
  firmware: string | null,
  log: readonly Pick<LogEntry, 'dir' | 'text'>[],
): MachineContext {
  const conn = device.connection.type === 'serial' ? 'serial' : 'network';
  const usbDirect = device.kind === 'lhy_micro' || device.kind === 'galvo';
  return {
    model: isVirtual ? '' : device.name,
    controller: controllerName(device.kind),
    firmware: connected && !isVirtual ? (firmware ?? '') : '',
    info: connected && !isVirtual && isGrblFamily(device.kind) ? lastInfoResponse(log) : '',
    connection: usbDirect ? DROPDOWN_VALUES.connection.usbDirect : conn === 'serial' ? DROPDOWN_VALUES.connection.serial : DROPDOWN_VALUES.connection.network,
    virtual: isVirtual,
  };
}

export interface BugContext {
  version: string | null;
  nav: { platform?: string; userAgent?: string };
  machine: MachineContext;
}

export function bugReportUrl(c: BugContext): string {
  return formUrl('bug', {
    version: c.version,
    system: systemLine(c.nav),
    machine: c.machine.virtual ? 'Virtual machine (simulator) / Máquina virtual' : c.machine.model,
    controller: [c.machine.controller, c.machine.firmware].filter(Boolean).join(' · '),
  });
}

export function machineReportUrl(c: BugContext): string {
  return formUrl('machine', {
    version: c.version,
    system: systemLine(c.nav),
    model: c.machine.model,
    controller: c.machine.controller,
    firmware: c.machine.firmware,
    grbl_i: c.machine.info,
    connection: c.machine.virtual ? undefined : c.machine.connection,
  });
}

/** Valores del formulario de ajustes de material a partir de un ajuste propio. */
export function materialFormValues(preset: MaterialPreset, device: DeviceProfile, chosenClass: MachineClass | null = null): Record<string, string> {
  const machineClass = preset.machine_class ?? chosenClass;
  const kind = machineClass ? classKind(machineClass) : laserKindOf(device);
  const role = effectiveRole(preset);
  const s = preset.settings;
  const out: Record<string, string> = {
    machine: device.name,
    material: preset.material,
    operation: DROPDOWN_VALUES.operation[role],
    speed: String(Math.round(s.speed * 100) / 100),
    power: String(Math.round(s.power * 100) / 100),
    passes: String(s.passes),
    air: s.air ? DROPDOWN_VALUES.air.yes : DROPDOWN_VALUES.air.no,
  };
  if (kind) out.laser_type = DROPDOWN_VALUES.laser_type[kind];
  if (machineClass) out.laser_watts = String(classWatts(machineClass));
  if (preset.thickness !== null) out.thickness = String(preset.thickness);
  if (s.mode !== 'line') out.interval = String(s.interval);
  return out;
}

export function materialShareUrl(preset: MaterialPreset | null, device: DeviceProfile, chosenClass: MachineClass | null = null): string {
  return formUrl('material', preset ? materialFormValues(preset, device, chosenClass) : { machine: device.name });
}
