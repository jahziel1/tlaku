import type { ConnectionEvent, DeviceProfile, JobOptions, LayerSettings, MaterialPreset } from '@/api/types';
import { materialOfLayer } from '@/features/help/jobLogSnapshot';

/**
 * Modo producción: cuándo el diálogo de inicio puede omitir SOLO la lista de comprobación. Todo
 * lo de este archivo es puro (sin store) para poder probar cada condición por separado.
 *
 * Regla de oro: ante la menor duda se muestra el diálogo completo. Esta función solo dice «sí»
 * cuando TODAS las condiciones se cumplen a la vez.
 */

/** Lo que se compara entre el trabajo confirmado con la lista completa y el de ahora. */
export interface StartSnapshot {
  /** El evento de conexión: reconectar o cambiar de máquina crea otro objeto (misma convención que `FrameMark`). */
  connection: ConnectionEvent;
  port: string | null;
  deviceId: string;
  /** Ajustes de las capas con salida que tienen objetos. */
  layers: string;
  /** Origen del trabajo: desde dónde, esquina, origen de usuario, selección y transformaciones. */
  origin: string;
  /** Material de la biblioteca que reconocen las capas con salida. */
  material: string;
}

/** Texto canónico de un valor de JSON (llaves ordenadas): dos valores iguales dan el mismo texto. */
export function canon(v: unknown): string {
  if (Array.isArray(v)) return `[${v.map(canon).join(',')}]`;
  if (v !== null && typeof v === 'object') {
    const o = v as Record<string, unknown>;
    return `{${Object.keys(o)
      .filter((k) => o[k] !== undefined)
      .sort()
      .map((k) => `${JSON.stringify(k)}:${canon(o[k])}`)
      .join(',')}}`;
  }
  return JSON.stringify(v) ?? 'null';
}

interface DocLike {
  layers: readonly LayerSettings[];
  objects: readonly { layer: string; hidden?: boolean }[];
}

/** Capas que salen a la máquina: con salida y con algún objeto visible. */
export function outputLayers(doc: DocLike): LayerSettings[] {
  const used = new Set(doc.objects.filter((o) => !o.hidden).map((o) => o.layer.toLowerCase()));
  return doc.layers.filter((l) => l.output && used.has(l.color.toLowerCase())).sort((a, b) => a.color.localeCompare(b.color));
}

/** Huella de los ajustes de las capas con salida (todo lo que cambia lo que hace el láser). */
export function layersKey(doc: DocLike): string {
  return canon(outputLayers(doc).map(({ visible: _visible, ...rest }) => rest));
}

/** Huella del origen del trabajo. `options` ya viene con la selección resuelta (`selectJobOptions`). */
export function originKey(options: Pick<JobOptions, 'start_from' | 'job_origin' | 'user_origin' | 'use_selection_origin' | 'selection' | 'transform' | 'tile'>): string {
  const { start_from, job_origin, user_origin, use_selection_origin, selection, transform, tile } = options;
  return canon({ start_from, job_origin, user_origin, use_selection_origin, selectionOnly: selection !== null, transform, tile });
}

/** Huella del material: el de biblioteca que reconoce cada capa con salida (`-` si ninguno). */
export function materialKey(doc: DocLike, presets: readonly MaterialPreset[]): string {
  return outputLayers(doc)
    .map((l) => `${l.color.toLowerCase()}=${materialOfLayer(l, presets) ?? '-'}`)
    .join('|');
}

export function makeSnapshot(input: {
  connection: ConnectionEvent;
  deviceId: string;
  doc: DocLike;
  options: Parameters<typeof originKey>[0];
  presets: readonly MaterialPreset[];
}): StartSnapshot {
  return {
    connection: input.connection,
    port: input.connection.port,
    deviceId: input.deviceId,
    layers: layersKey(input.doc),
    origin: originKey(input.options),
    material: materialKey(input.doc, input.presets),
  };
}

/** Por qué se muestra el diálogo completo. `pending` = todavía no se sabe (se espera a la revisión). */
export type FullReason =
  | 'pref'
  | 'notFromUser'
  | 'device'
  | 'noMachine'
  | 'noBaseline'
  | 'connection'
  | 'layers'
  | 'origin'
  | 'material'
  | 'positionLost'
  | 'repeat'
  | 'preview'
  | 'warnings'
  | 'review'
  | 'placement'
  | 'pending';

/** Máquinas cuyo diálogo de inicio lleva avisos propios y confirmaciones de seguridad: nunca se omiten. */
export const ALWAYS_FULL_KINDS: readonly DeviceProfile['kind'][] = ['galvo', 'lhy_micro'];

/** Lo que dicen la vista previa y las revisiones, o `null` mientras no han terminado. */
export interface StartChecks {
  /** Avisos de la vista previa del motor (`preview.warnings`). */
  previewWarnings: readonly string[];
  /** Aviso de enlace saturado, o `null`. */
  linkWarning: string | null;
  /** Hallazgos graves de «Revisar diseño». */
  seriousFindings: number;
  /** El trabajo cabe y se puede colocar. */
  placementOk: boolean;
  /** La vista previa o la revisión fallaron: no se puede asegurar que no hay avisos. */
  failed: boolean;
}

export interface SkipInput {
  /** Preferencia `productionMode`. */
  enabled: boolean;
  /** El diálogo lo abrió una persona con Iniciar (no la cola guiada, la automatización ni el modo sencillo). */
  fromUser: boolean;
  deviceKind: DeviceProfile['kind'];
  connected: boolean;
  /** El trabajo anterior confirmado con la lista completa, ya terminado bien; `null` si no hay. */
  baseline: StartSnapshot | null;
  now: StartSnapshot;
  /** Posición de la máquina dudosa (reinicio de la placa, alarma en pleno trabajo). */
  positionDoubtful: boolean;
  /** «Repetir con el alimentador» tiene su propio lanzamiento. */
  repeating: boolean;
  checks: StartChecks | null;
}

/** `null` = se puede omitir la lista; si no, la primera causa que obliga a mostrar el diálogo completo. */
export function fullReason(i: SkipInput): FullReason | null {
  if (!i.enabled) return 'pref';
  if (!i.fromUser) return 'notFromUser';
  if (ALWAYS_FULL_KINDS.includes(i.deviceKind)) return 'device';
  if (!i.connected) return 'noMachine';
  const b = i.baseline;
  if (!b) return 'noBaseline';
  // Misma máquina y misma conexión, sin reconectar.
  if (b.connection !== i.now.connection || b.port !== i.now.port || b.deviceId !== i.now.deviceId) return 'connection';
  if (b.layers !== i.now.layers) return 'layers';
  if (b.origin !== i.now.origin) return 'origin';
  if (b.material !== i.now.material) return 'material';
  if (i.positionDoubtful) return 'positionLost';
  if (i.repeating) return 'repeat';
  const c = i.checks;
  if (!c) return 'pending';
  if (c.failed) return 'review';
  if (c.previewWarnings.length > 0 || c.linkWarning !== null) return 'warnings';
  if (c.seriousFindings > 0) return 'review';
  if (!c.placementOk) return 'placement';
  return null;
}
