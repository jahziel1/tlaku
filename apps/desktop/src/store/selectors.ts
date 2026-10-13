import type { DesignObject, DeviceProfile, JobArgs, JobOptions } from '@/api/types';
import type { I18nKey } from '@/i18n';
import { FALLBACK_DEVICE } from '@/lib/device';
import type { AppState } from './types';

export function selectActiveDevice(s: Pick<AppState, 'devices' | 'activeDeviceId'>): DeviceProfile {
  return s.devices.find((d) => d.id === s.activeDeviceId) ?? s.devices[0] ?? FALLBACK_DEVICE;
}

export function selectSelectedObjects(s: Pick<AppState, 'doc' | 'selectedIds'>): DesignObject[] {
  const ids = new Set(s.selectedIds);
  return s.doc.objects.filter((o) => ids.has(o.id));
}

/**
 * Ids que se envían con "Solo la selección": la selección actual, o `null` si la opción está
 * apagada o no hay nada seleccionado (entonces sale todo el diseño). Devuelve un arreglo nuevo en
 * cada llamada; en componentes usa `useShallow`.
 */
export function selectJobSelection(s: Pick<AppState, 'cutSelectionOnly' | 'selectedIds'>): string[] | null {
  return s.cutSelectionOnly && s.selectedIds.length > 0 ? [...s.selectedIds] : null;
}

/** `null` si "Solo la selección" y "Usar el origen de la selección" se pueden usar; si no, la clave del motivo. */
export function selectionBlockedReason(s: Pick<AppState, 'selectedIds'>): I18nKey | null {
  return s.selectedIds.length === 0 ? 'machine.selection.reason.none' : null;
}

/** Opciones del trabajo con la selección ya resuelta: lo que viaja a compute, start, frame y export. */
export function selectJobOptions(s: Pick<AppState, 'jobOptions' | 'cutSelectionOnly' | 'selectedIds'>): JobOptions {
  const selection = selectJobSelection(s);
  return { ...s.jobOptions, selection, use_selection_origin: selection !== null && s.jobOptions.use_selection_origin };
}

/** ¿Hay algo que enviar? Objetos visibles en capas con salida activa (y, con "Solo la selección", seleccionados). */
export function hasOutput(s: Pick<AppState, 'doc'> & Partial<Pick<AppState, 'cutSelectionOnly' | 'selectedIds'>>): boolean {
  const outputs = new Set(s.doc.layers.filter((l) => l.output).map((l) => l.color));
  const only = s.cutSelectionOnly && s.selectedIds && s.selectedIds.length > 0 ? new Set(s.selectedIds) : null;
  return s.doc.objects.some((o) => !o.hidden && outputs.has(o.layer) && (!only || only.has(o.id)));
}

/** Argumentos de `compute_job`, `start_job`, `frame_job` y `export_gcode`. */
export function buildJobArgs(s: AppState): JobArgs {
  return { doc: s.doc, device: selectActiveDevice(s), options: selectJobOptions(s) };
}

export function isJobBusy(s: Pick<AppState, 'jobState'>): boolean {
  return s.jobState === 'running' || s.jobState === 'paused';
}
