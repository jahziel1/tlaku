/** Selectores y razones de bloqueo de la máquina (mover, enviar, overrides). */
import type { ConnectionEvent, Point } from '@/api/types';
import { firmwareFamily, type FirmwareFamily } from '@/features/machine/grblCodes';
import { isVirtualPort } from '@/features/machine/virtualPort';
import { type I18nKey } from '@/i18n';
import { workToDoc, type GrblSettings } from '@/lib/device';
import { hasOutput, isJobBusy, selectActiveDevice } from '../selectors';
import type { AppState } from '../types';

// ---------------------------------------------------------------------------
// Selectores
// ---------------------------------------------------------------------------

/** Posición del cabezal en coordenadas del documento (mm, abajo a la izquierda, Y arriba), o null. */
export function selectHeadDoc(s: Pick<AppState, 'status' | 'connection' | 'devices' | 'activeDeviceId'>): Point | null {
  if (!s.connection.connected || !s.status) return null;
  return workToDoc(selectActiveDevice(s), s.status.work_pos);
}

/**
 * Causa de la alarma en curso: `undefined` si no hay alarma, `code: null` si no se conoce.
 * Devuelve un objeto nuevo: en componentes selecciona `status?.state` y `alarmCode` por separado.
 */
export function selectAlarm(s: Pick<AppState, 'status' | 'alarmCode'>): { code: number | null } | undefined {
  return s.status?.state === 'alarm' ? { code: s.alarmCode } : undefined;
}

/** Familia del firmware conectado: decide cómo se leen los `error:N` y `ALARM:N` extendidos. */
export function selectFirmwareFamily(s: Pick<AppState, 'firmware'>): FirmwareFamily {
  return firmwareFamily(s.firmware);
}

/** ¿La conexión actual es una máquina virtual (emulador)? Nada se graba de verdad. */
export function selectIsVirtual(s: Pick<AppState, 'connection'>): boolean {
  return s.connection.connected && isVirtualPort(s.connection.port);
}

/** Valor de `$22` (homing) si se leyó: true = activado. */
export function homingEnabled(settings: GrblSettings | null): boolean | null {
  const v = settings?.[22];
  return v === undefined ? null : v !== 0;
}

/** `null` si se puede mover el cabezal a mano; si no, la clave del motivo. */
export function jogBlockedReason(s: Pick<AppState, 'connection' | 'jobState' | 'status'>): I18nKey | null {
  if (!s.connection.connected) return 'reason.needMachine';
  if (isJobBusy(s)) return 'reason.jobRunning';
  const state = s.status?.state;
  if (state === 'alarm') return 'machine.reason.alarm';
  if (state === 'hold' || state === 'door') return 'machine.reason.paused';
  if (state === 'run' || state === 'home' || state === 'check' || state === 'sleep' || state === 'other') return 'machine.reason.notIdle';
  return null;
}

/**
 * `null` si se puede posicionar el láser con un clic; si no, la clave del motivo. Además de poder
 * mover el cabezal, hace falta una cama plana: con rotativo la Y es el giro de la pieza.
 */
export function positionLaserBlockedReason(s: Pick<AppState, 'connection' | 'jobState' | 'status' | 'devices' | 'activeDeviceId'>): I18nKey | null {
  const jog = jogBlockedReason(s);
  if (jog) return jog;
  return selectActiveDevice(s).rotary.enabled ? 'machine.position.reason.rotary' : null;
}

/** ¿La controladora conectada admite overrides en tiempo real? Solo GRBL y derivados (Marlin y Smoothieware no). */
export function overridesSupported(s: Pick<AppState, 'firmware' | 'devices' | 'activeDeviceId'>): boolean {
  if (/marlin|smoothie/i.test(s.firmware ?? '')) return false;
  const kind = selectActiveDevice(s).kind;
  return kind !== 'marlin' && kind !== 'smoothie';
}

/** `null` si se pueden ajustar velocidad y potencia en vivo; si no, la clave del motivo. */
export function overrideBlockedReason(s: Pick<AppState, 'connection' | 'firmware' | 'devices' | 'activeDeviceId'>): I18nKey | null {
  if (!s.connection.connected) return 'reason.needMachine';
  return overridesSupported(s) ? null : 'machine.override.reason.family';
}

/**
 * Lo que había cuando se encuadró el trabajo por última vez (control remoto: el celular solo puede
 * iniciar un trabajo que ya se encuadró en la PC). Se guardan las referencias de lo que el trabajo
 * lee, porque el store es inmutable: cualquier cambio crea otro objeto y la marca deja de valer.
 */
export interface FrameMark {
  doc: AppState['doc'];
  jobOptions: AppState['jobOptions'];
  device: ReturnType<typeof selectActiveDevice>;
  /** El evento de conexión: reconectar o cambiar de máquina crea uno nuevo. */
  connection: ConnectionEvent;
  /** `selectedIds` si el trabajo es "solo la selección"; `null` si es todo el diseño. */
  selection: readonly string[] | null;
}

/** La marca de encuadre del estado de ahora. */
export function frameMarkOf(s: Pick<AppState, 'doc' | 'jobOptions' | 'devices' | 'activeDeviceId' | 'connection' | 'cutSelectionOnly' | 'selectedIds'>): FrameMark {
  return { doc: s.doc, jobOptions: s.jobOptions, device: selectActiveDevice(s), connection: s.connection, selection: s.cutSelectionOnly ? s.selectedIds : null };
}

/**
 * ¿El trabajo actual ya se encuadró después del último cambio del documento, de la máquina o de las
 * opciones del trabajo? Es la condición para iniciarlo desde el celular. Mismas causas de cambio que
 * invalidan la vista previa (`features/preview/sync.ts`), más la conexión.
 */
export function selectFramed(s: Pick<AppState, 'framedMark' | 'doc' | 'jobOptions' | 'devices' | 'activeDeviceId' | 'connection' | 'cutSelectionOnly' | 'selectedIds'>): boolean {
  const m = s.framedMark;
  if (!m || !s.connection.connected) return false;
  const now = frameMarkOf(s);
  return m.doc === now.doc && m.jobOptions === now.jobOptions && m.device === now.device && m.connection === now.connection && m.selection === now.selection;
}

/** `null` si se puede iniciar o encuadrar; si no, la clave del motivo. */
export function sendBlockedReason(s: AppState): I18nKey | null {
  if (!s.connection.connected) return 'reason.needMachine';
  if (isJobBusy(s)) return 'reason.jobRunning';
  if (!hasOutput(s)) return 'reason.nothingToSend';
  const state = s.status?.state;
  if (state === 'alarm') return 'machine.reason.alarm';
  if (state === 'hold' || state === 'door') return 'machine.reason.paused';
  if (state && state !== 'idle') return 'machine.reason.notIdle';
  return null;
}
