import type { DeviceKind } from '@/bindings/DeviceKind';
import type { DeviceProfile, Point } from '@/api/types';
import { dialectForKind, defaultSMax, FALLBACK_DEVICE, uniqueDeviceId } from '@/lib/device';
import type { CatalogEntry } from './catalog';
import { compareWithMachine, type CompareField } from './compare';

/** De dónde sale cada dato del perfil nuevo. */
export type Provenance = 'machine' | 'catalog' | 'default';

export type ProfileField = CompareField | 'baud' | 'origin';

export interface BuiltProfile {
  profile: DeviceProfile;
  provenance: Record<ProfileField, Provenance>;
}

/** Velocidad serie habitual de cada firmware (`DeviceKind::default_baud` del motor). */
export function defaultBaud(kind: DeviceKind): number {
  return kind === 'marlin' ? 250000 : 115200;
}

const round = (n: number) => Math.round(n);

/**
 * Construye el perfil de la máquina nueva. Parte de los valores habituales del tipo de firmware
 * (los mismos que `DeviceProfile::for_kind` del motor), pone encima lo que sabe el catálogo y,
 * en los datos de `useMachine`, lo que la máquina contestó a `$$`.
 *
 * Lo que nadie sabe queda con el valor de partida y `provenance` lo marca como `default`, para
 * que la interfaz avise de que hay que leerlo de la máquina.
 */
export function buildProfile(args: {
  entry: CatalogEntry | null;
  kind: DeviceKind;
  name: string;
  takenIds: Iterable<string>;
  /** Baudios con los que la detección tuvo éxito; null si no se detectó. */
  probedBaud: number | null;
  settings: Record<string, string> | null;
  useMachine: ReadonlySet<CompareField>;
}): BuiltProfile {
  const { entry, kind, name, takenIds, probedBaud, settings, useMachine } = args;
  const dialect = dialectForKind(kind);
  const provenance: Record<ProfileField, Provenance> = {
    s_max: 'default',
    bed: 'default',
    max_rate: 'default',
    accel: 'default',
    baud: 'default',
    origin: 'default',
  };
  const profile: DeviceProfile = {
    ...structuredClone(FALLBACK_DEVICE),
    id: uniqueDeviceId(name, takenIds),
    name: name.trim(),
    kind,
    dialect,
    s_max: defaultSMax(kind),
    baud: defaultBaud(kind),
    power_mode: dialect.dynamic_on ? 'dynamic' : 'constant',
  };

  if (kind === 'ruida') {
    // Ruida habla UDP a una IP que se escribe a mano. Cama y home son valores de partida habituales.
    profile.connection = { type: 'udp', host: '' };
    profile.bed = [600, 400];
    profile.origin = 'top_left';
  }

  if (kind === 'lhy_micro') {
    // K40 con M2 Nano: USB, home arriba a la izquierda y un tope de corte de 140 mm/s (8400 mm/min).
    // Son valores de partida habituales, no datos de la máquina: quedan marcados como tales.
    profile.bed = [300, 200];
    profile.origin = 'top_left';
    profile.max_rate = [8400, 8400];
    profile.rapid_rate = 8400;
  }

  if (entry) {
    if (entry.bed) {
      profile.bed = [...entry.bed.value];
      provenance.bed = 'catalog';
    }
    if (entry.maxRate) {
      profile.max_rate = [entry.maxRate.value, entry.maxRate.value];
      provenance.max_rate = 'catalog';
    }
    if (entry.accel) {
      profile.accel = [entry.accel.value, entry.accel.value];
      provenance.accel = 'catalog';
    }
    if (entry.sMax) {
      profile.s_max = entry.sMax.value;
      provenance.s_max = 'catalog';
    }
    if (entry.m4) profile.power_mode = entry.m4.value && dialect.dynamic_on ? 'dynamic' : 'constant';
    if (entry.baud) {
      profile.baud = entry.baud.value;
      provenance.baud = 'catalog';
    }
    if (entry.origin) {
      profile.origin = entry.origin.value;
      provenance.origin = 'catalog';
    }
    if (entry.zAxis) profile.z_axis = entry.zAxis.value;
  }
  if (provenance.baud === 'default' && probedBaud) {
    profile.baud = probedBaud;
    provenance.baud = 'machine';
  }

  // Lo medido en la máquina, solo en lo que el usuario dejó marcado.
  for (const row of compareWithMachine(entry, settings).rows) {
    if (!useMachine.has(row.field) || row.machine === null) continue;
    provenance[row.field] = 'machine';
    if (row.field === 's_max') profile.s_max = round(row.machine as number);
    else if (row.field === 'bed') profile.bed = [...(row.machine as Point)];
    else if (row.field === 'max_rate') profile.max_rate = [...(row.machine as Point)];
    else profile.accel = [...(row.machine as Point)];
  }
  // Los movimientos rápidos van a la velocidad máxima del eje más lento (GRBL usa `$110`/`$111`).
  if (provenance.max_rate !== 'default') profile.rapid_rate = Math.min(...profile.max_rate);
  return { profile, provenance };
}
