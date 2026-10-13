import type { DesignObject, DeviceProfile, JobArgs } from '@/api/types';
import { FALLBACK_DEVICE } from '@/lib/device';
import type { AppState } from './types';

export function selectActiveDevice(s: Pick<AppState, 'devices' | 'activeDeviceId'>): DeviceProfile {
  return s.devices.find((d) => d.id === s.activeDeviceId) ?? s.devices[0] ?? FALLBACK_DEVICE;
}

export function selectSelectedObjects(s: Pick<AppState, 'doc' | 'selectedIds'>): DesignObject[] {
  const ids = new Set(s.selectedIds);
  return s.doc.objects.filter((o) => ids.has(o.id));
}

/** ¿Hay algo que enviar? Objetos visibles en capas con salida activa. */
export function hasOutput(s: Pick<AppState, 'doc'>): boolean {
  const outputs = new Set(s.doc.layers.filter((l) => l.output).map((l) => l.color));
  return s.doc.objects.some((o) => !o.hidden && outputs.has(o.layer));
}

/** Argumentos de `compute_job`, `start_job`, `frame_job` y `export_gcode`. */
export function buildJobArgs(s: AppState): JobArgs {
  return { doc: s.doc, device: selectActiveDevice(s), options: s.jobOptions };
}

export function isJobBusy(s: Pick<AppState, 'jobState'>): boolean {
  return s.jobState === 'running' || s.jobState === 'paused';
}
