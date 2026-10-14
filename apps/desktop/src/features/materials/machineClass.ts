/**
 * Clase de máquina (tipo de láser + potencia nominal): lo que usa la biblioteca para ofrecer
 * ajustes a quien no sabe de parámetros. El perfil de la máquina no guarda la potencia del láser
 * (salvo el galvo), así que la clase se infiere cuando se puede y, si no, la elige el usuario.
 */
import type { DeviceProfile, LaserKind, MachineClass } from '@/api/types';

/** Lo que se necesita del perfil de una máquina para deducir su clase. */
export type DeviceLike = Pick<DeviceProfile, 'kind'> & { galvo: Pick<DeviceProfile['galvo'], 'source' | 'watts'> };

export const MACHINE_CLASSES: readonly MachineClass[] = [
  'diode_5', 'diode_10', 'diode_20', 'diode_40',
  'co2_40', 'co2_60', 'co2_80', 'co2_100',
  'fiber_20', 'fiber_30', 'fiber_50',
];

const INFO: Record<MachineClass, { kind: LaserKind; watts: number }> = {
  diode_5: { kind: 'diode', watts: 5 },
  diode_10: { kind: 'diode', watts: 10 },
  diode_20: { kind: 'diode', watts: 20 },
  diode_40: { kind: 'diode', watts: 40 },
  co2_40: { kind: 'co2', watts: 40 },
  co2_60: { kind: 'co2', watts: 60 },
  co2_80: { kind: 'co2', watts: 80 },
  co2_100: { kind: 'co2', watts: 100 },
  fiber_20: { kind: 'fiber', watts: 20 },
  fiber_30: { kind: 'fiber', watts: 30 },
  fiber_50: { kind: 'fiber', watts: 50 },
};

export const classKind = (c: MachineClass): LaserKind => INFO[c].kind;
export const classWatts = (c: MachineClass): number => INFO[c].watts;

/** Clases de un tipo de láser, de menor a mayor potencia. */
export const classesOfKind = (kind: LaserKind): MachineClass[] => MACHINE_CLASSES.filter((c) => INFO[c].kind === kind);

/** La clase de `kind` con la potencia más cercana a `watts`; en un empate, la de menos potencia. */
export function nearestClass(kind: LaserKind, watts: number): MachineClass {
  const list = classesOfKind(kind);
  return list.reduce((best, c) => (Math.abs(INFO[c].watts - watts) < Math.abs(INFO[best].watts - watts) ? c : best));
}

/** Distancia en W entre dos clases del mismo tipo (para elegir la más cercana). */
export const classDistance = (a: MachineClass, b: MachineClass): number => Math.abs(INFO[a].watts - INFO[b].watts);

/** Tipo de láser de una máquina, o `null` si la biblioteca no tiene clases para él (UV). */
export function laserKindOf(device: DeviceLike): LaserKind | null {
  switch (device.kind) {
    case 'ruida':
    case 'lhy_micro':
      return 'co2';
    case 'galvo':
      switch (device.galvo.source) {
        case 'co2':
          return 'co2';
        case 'uv':
          return null;
        default:
          return 'fiber';
      }
    default:
      return 'diode';
  }
}

export interface MachineClassInfo {
  kind: LaserKind | null;
  /** `null` = no se conoce la potencia: la interfaz debe preguntarla o avisar. */
  machineClass: MachineClass | null;
}

/**
 * Clase de una máquina. `watts` es la potencia que dio el usuario (si la hay); el galvo trae la
 * suya en el perfil. Sin potencia, la clase queda en `null`.
 */
export function machineClassOf(device: DeviceLike, watts: number | null = null): MachineClassInfo {
  const kind = laserKindOf(device);
  const w = watts ?? (device.kind === 'galvo' ? device.galvo.watts : null);
  if (kind === null || w === null || !Number.isFinite(w) || w <= 0) return { kind, machineClass: null };
  return { kind, machineClass: nearestClass(kind, w) };
}

// ---------------------------------------------------------------------------
// Clase elegida por el usuario para cada máquina (solo de este equipo)
// ---------------------------------------------------------------------------

const STORE_KEY = 'tlaku.materials.machineClass';

/** Copia en memoria: la elección vale durante la sesión aunque no haya almacenamiento. */
let session: Record<string, MachineClass> | null = null;

function readStored(): Record<string, MachineClass> {
  try {
    const raw = globalThis.localStorage?.getItem(STORE_KEY);
    const value: unknown = raw ? JSON.parse(raw) : {};
    if (typeof value !== 'object' || value === null) return {};
    return Object.fromEntries(Object.entries(value).filter(([, c]) => MACHINE_CLASSES.includes(c as MachineClass))) as Record<string, MachineClass>;
  } catch {
    return {};
  }
}

/** Clase que el usuario eligió para la máquina `deviceId`, si eligió una. */
export function storedClassFor(deviceId: string): MachineClass | null {
  return (session ?? readStored())[deviceId] ?? null;
}

export function storeClassFor(deviceId: string, machineClass: MachineClass | null): void {
  const all = { ...(session ?? readStored()) };
  if (machineClass === null) delete all[deviceId];
  else all[deviceId] = machineClass;
  session = all;
  try {
    globalThis.localStorage?.setItem(STORE_KEY, JSON.stringify(all));
  } catch {
    // Sin almacenamiento (ventana privada, bloqueado): la elección vale solo durante la sesión.
  }
}

/** Clase de la máquina: la elegida por el usuario y, si no hay, la que se deduce del perfil. */
export function resolveMachineClass(device: DeviceLike & Pick<DeviceProfile, 'id'>): MachineClassInfo {
  const inferred = machineClassOf(device);
  const chosen = storedClassFor(device.id);
  // Una clase guardada de otro tipo de láser (cambió el perfil) ya no sirve.
  if (chosen && (inferred.kind === null || classKind(chosen) === inferred.kind)) return { kind: inferred.kind ?? classKind(chosen), machineClass: chosen };
  return inferred;
}
