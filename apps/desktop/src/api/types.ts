/**
 * Tipos del contrato app <-> motor (docs/app-api.md).
 *
 * Los del documento, el dispositivo y el trabajo vienen de `src/bindings/` (generado con ts-rs
 * desde el crate `laser-doc`; no editar a mano). Aquí se reexportan con los nombres que usa la app
 * y se definen solo los que todavía no tienen binding: resultados de comandos y eventos de la
 * máquina.
 *
 * TODO(bindings): cuando el motor exporte `StatusReport`, `ProgressEvent`, `LogEvent`, `JobEvent`,
 * `ConnectionEvent`, `ExportResult` y `ConnectResult`, reemplazar las definiciones manuales de
 * abajo por reexportaciones.
 */

import type { DeviceProfile } from '@/bindings/DeviceProfile';
import type { Document } from '@/bindings/Document';
import type { ImportResult } from '@/bindings/ImportResult';
import type { JobOptions } from '@/bindings/JobOptions';
import type { JobPreview } from '@/bindings/JobPreview';
import type { DesignObject } from '@/bindings/DesignObject';
import type { LayerSettings } from '@/bindings/LayerSettings';
import type { ImageSettings } from '@/bindings/ImageSettings';
import type { ImageMode } from '@/bindings/ImageMode';
import type { LayerMode } from '@/bindings/LayerMode';
import type { LayerOrder } from '@/bindings/LayerOrder';
import type { Origin } from '@/bindings/Origin';
import type { PathData } from '@/bindings/PathData';
import type { PreviewLayer } from '@/bindings/PreviewLayer';
import type { StartFrom } from '@/bindings/StartFrom';
import type { ImageObject } from '@/bindings/ImageObject';
import type { VectorObject } from '@/bindings/VectorObject';

export type {
  DesignObject,
  DeviceProfile,
  Document,
  ImageObject,
  ImageSettings,
  ImportResult,
  JobOptions,
  JobPreview,
  LayerMode,
  LayerOrder,
  LayerSettings,
  PathData,
  StartFrom,
  VectorObject,
};
export type DeviceOrigin = Origin;
export type ImageDitherMode = ImageMode;
export type JobLayerSummary = PreviewLayer;

export type Point = [number, number];

/** Afín [a b c d e f]: x' = a·x + c·y + e ; y' = b·x + d·y + f */
export type Affine = DesignObject['transform'];

// ---------------------------------------------------------------------------
// Comandos: entradas y salidas
// ---------------------------------------------------------------------------

export interface ExportResult {
  lines: number;
  estimate_s: number;
  warnings: string[];
}

export interface ConnectResult {
  firmware: string;
}

export type RealtimeCommand = 'status' | 'hold' | 'resume' | 'reset';

export interface JobArgs {
  doc: Document;
  device: DeviceProfile;
  options: JobOptions;
}

// ---------------------------------------------------------------------------
// Eventos
// ---------------------------------------------------------------------------

/**
 * TODO(bindings): el contrato solo dice "estado, posición máquina/trabajo, F, S, búfer".
 * La forma exacta la fija laser-grbl; esta es una suposición razonable a verificar.
 */
export type MachineState =
  | 'idle'
  | 'run'
  | 'hold'
  | 'jog'
  | 'alarm'
  | 'door'
  | 'check'
  | 'home'
  | 'sleep';

export interface StatusReport {
  state: MachineState;
  /** Posición de máquina [x, y] en mm. */
  machine_pos: Point;
  /** Posición de trabajo [x, y] en mm. */
  work_pos: Point;
  /** F: velocidad actual en mm/min. */
  feed: number;
  /** S: potencia actual (0..s_max). */
  spindle: number;
  /** Búfer: bloques libres del planificador y bytes libres del RX. */
  buffer: { planner: number; rx: number };
}

export interface ProgressEvent {
  acked: number;
  total: number;
}

export interface LogEvent {
  dir: 'in' | 'out';
  text: string;
}

export type JobState = 'started' | 'paused' | 'finished' | 'cancelled' | 'error';

export interface JobEvent {
  state: JobState;
  message: string;
}

export interface ConnectionEvent {
  connected: boolean;
  port: string | null;
  error: string | null;
}

/** Nombre del evento -> carga. */
export interface EventMap {
  'machine://status': StatusReport;
  'machine://progress': ProgressEvent;
  'machine://log': LogEvent;
  'machine://job': JobEvent;
  'machine://connection': ConnectionEvent;
}

export type EventName = keyof EventMap;
