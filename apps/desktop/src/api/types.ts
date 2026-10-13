/**
 * Tipos del contrato app <-> motor (docs/app-api.md).
 *
 * Todo viene de `src/bindings/` (generado con ts-rs desde el crate `tlaku-doc`; no editar a
 * mano): documento, dispositivo, trabajo, resultados de comandos y eventos de la máquina. Aquí se
 * reexportan con los nombres que usa la app y se definen solo las piezas que no cruzan la
 * frontera como tipo propio (`JobArgs`, `EventMap`, `Point`, `Affine`).
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
import type { BezPath } from '@/bindings/BezPath';
import type { PathSeg } from '@/bindings/PathSeg';
import type { TextAlign } from '@/bindings/TextAlign';
import type { TextObject } from '@/bindings/TextObject';
import type { TextOnPath } from '@/bindings/TextOnPath';
import type { PreviewLayer } from '@/bindings/PreviewLayer';
import type { StartFrom } from '@/bindings/StartFrom';
import type { ImageObject } from '@/bindings/ImageObject';
import type { VectorObject } from '@/bindings/VectorObject';
import type { BufferStatus } from '@/bindings/BufferStatus';
import type { ConnectResult } from '@/bindings/ConnectResult';
import type { ConnectionEvent } from '@/bindings/ConnectionEvent';
import type { ExportResult } from '@/bindings/ExportResult';
import type { JobEvent } from '@/bindings/JobEvent';
import type { JobState } from '@/bindings/JobState';
import type { LogDir } from '@/bindings/LogDir';
import type { LogEvent } from '@/bindings/LogEvent';
import type { MachineState } from '@/bindings/MachineState';
import type { ProgressEvent } from '@/bindings/ProgressEvent';
import type { RealtimeCommand } from '@/bindings/RealtimeCommand';
import type { StatusReport } from '@/bindings/StatusReport';

export type {
  BezPath,
  BufferStatus,
  ConnectResult,
  ConnectionEvent,
  DesignObject,
  DeviceProfile,
  Document,
  ExportResult,
  ImageObject,
  ImageSettings,
  ImportResult,
  JobEvent,
  JobOptions,
  JobPreview,
  JobState,
  LayerMode,
  LayerOrder,
  LayerSettings,
  LogDir,
  LogEvent,
  MachineState,
  PathSeg,
  ProgressEvent,
  RealtimeCommand,
  StartFrom,
  StatusReport,
  TextAlign,
  TextObject,
  TextOnPath,
  VectorObject,
};
export type DeviceOrigin = Origin;
export type ImageDitherMode = ImageMode;
export type JobLayerSummary = PreviewLayer;

export type Point = [number, number];

/** Afín [a b c d e f]: x' = a·x + c·y + e ; y' = b·x + d·y + f */
export type Affine = DesignObject['transform'];

// ---------------------------------------------------------------------------
// Comandos: entradas
// ---------------------------------------------------------------------------

export interface JobArgs {
  doc: Document;
  device: DeviceProfile;
  options: JobOptions;
}

// ---------------------------------------------------------------------------
// Eventos
// ---------------------------------------------------------------------------

/** Nombre del evento -> carga. */
export interface EventMap {
  'machine://status': StatusReport;
  'machine://progress': ProgressEvent;
  'machine://log': LogEvent;
  'machine://job': JobEvent;
  'machine://connection': ConnectionEvent;
}

export type EventName = keyof EventMap;
