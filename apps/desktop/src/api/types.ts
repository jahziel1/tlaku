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
import type { FillStyle } from '@/bindings/FillStyle';
import type { TabSettings } from '@/bindings/TabSettings';
import type { RotaryKind } from '@/bindings/RotaryKind';
import type { RotarySettings } from '@/bindings/RotarySettings';
import type { ScanOffset } from '@/bindings/ScanOffset';
import type { MaterialLibrary } from '@/bindings/MaterialLibrary';
import type { MaterialPreset } from '@/bindings/MaterialPreset';
import type { MaterialTestParams } from '@/bindings/MaterialTestParams';
import type { TestAxis } from '@/bindings/TestAxis';
import type { TestParam } from '@/bindings/TestParam';

export type {
  BezPath,
  BufferStatus,
  ConnectResult,
  ConnectionEvent,
  DesignObject,
  DeviceProfile,
  Document,
  ExportResult,
  FillStyle,
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
  MaterialLibrary,
  MaterialPreset,
  MaterialTestParams,
  PathSeg,
  ProgressEvent,
  RealtimeCommand,
  RotaryKind,
  RotarySettings,
  ScanOffset,
  StartFrom,
  StatusReport,
  TabSettings,
  TestAxis,
  TestParam,
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

// ---------------------------------------------------------------------------------------------
// Fase 3. TODO(bindings): cada área reemplaza estos tipos por los generados con ts-rs desde su
// crate cuando conecte el backend (texto, geometría, vectorizado y cámara).

export interface FontFamily {
  family: string;
  styles: string[];
}

export interface TextOutlineResult {
  paths: BezPath[];
  warnings: string[];
}

/**
 * `join`: une trayectorias abiertas cuyos extremos están a menos de `tolerance` mm;
 * `close`: cierra las abiertas cuyos extremos están a menos de `tolerance` mm;
 * `dedupe`: borra segmentos repetidos o superpuestos (a menos de `tolerance` mm) para no cortar
 * dos veces la misma línea. Las tres conservan la capa de cada objeto (fase 4).
 */
export type GeometryOp =
  | 'union'
  | 'difference'
  | 'intersection'
  | 'xor'
  | 'weld'
  | 'offset'
  | 'join'
  | 'close'
  | 'dedupe';

export interface GeometryOpArgs {
  op: GeometryOp;
  objects: DesignObject[];
  /** Solo `offset`: positivo agranda la pieza (los agujeros se encogen). */
  distance?: number;
  join?: 'round' | 'miter' | 'bevel';
  /** Solo `join`, `close` y `dedupe`, en mm. */
  tolerance?: number;
}

export interface TraceSettings {
  threshold: number;
  invert: boolean;
  blur_px: number;
  speckle_px: number;
  corner_threshold_deg: number;
  smoothing: number;
  mode: 'spline' | 'polygon';
}

export interface TraceResult {
  paths: BezPath[];
  warnings: string[];
}

export interface LensCalibration {
  width: number;
  height: number;
  fx: number;
  fy: number;
  cx: number;
  cy: number;
  k1: number;
  k2: number;
  p1: number;
  p2: number;
  k3: number;
  rms_error_px: number;
  views_used: number;
}

export interface CalibrationReport {
  calibration: LensCalibration;
  warnings: string[];
  used_views: number[];
  view_rms_px: number[];
}

type Quad = [Point, Point, Point, Point];

type Mat3 = [[number, number, number], [number, number, number], [number, number, number]];

/** Cómo se obtuvo la alineación: 4 puntos a mano o marcas grabadas detectadas en la foto. */
export type AlignmentMethod = 'manual' | 'marks';

/** Marcas medidas a una altura del material (base de la alineación con marcas). */
export interface AlignmentSample {
  /** Altura del plano de las marcas sobre la cama, en mm. */
  height_mm: number;
  /** Marcas usadas, en píxeles de la imagen corregida. */
  image_px: [number, number][];
  /** Sus posiciones en la cama (mm de documento), en el mismo orden. */
  bed_mm: [number, number][];
  /** Error de validación cruzada medido al detectar (mm); informativo. */
  loo_rms_mm?: number | null;
  loo_max_mm?: number | null;
}

/** `measured`: marcas a dos o más alturas; `modelled`: estimado con la matriz de la cámara. */
export type HeightSource = 'measured' | 'modelled';

/**
 * Modelo de altura `H(h) = [H1 H2 (H3 + (h - ref)·G)]` de la cama a la imagen: la tercera columna
 * cambia linealmente con la altura. Lo calcula el motor al guardar la alineación.
 */
export interface HeightModel {
  ref_height_mm: number;
  /** Cama (mm) -> imagen (px) a `ref_height_mm`, por filas (sin normalizar la escala). */
  bed_to_image: Mat3;
  /** Cuánto cambia la tercera columna por mm de altura. */
  gradient: [number, number, number];
  source: HeightSource;
  /** Rango de alturas medido (con `modelled`, solo la altura de referencia). */
  min_height_mm: number;
  max_height_mm: number;
}

/**
 * Alineación entre la imagen corregida y la cama, manual de 4 puntos o con marcas grabadas. Los
 * puntos van en píxeles de la imagen corregida (espacio de la calibración) y en mm de documento,
 * en este orden: inferior izquierda, inferior derecha, superior derecha, superior izquierda. Con
 * marcas grabadas son las esquinas del rectángulo de las marcas, proyectadas.
 * Es la misma forma que guarda `camera.json` (src-tauri/src/edit/camera.rs). Los campos opcionales
 * los rellena siempre el motor; faltan en configuraciones antiguas y en la memoria de la interfaz.
 */
export interface CameraAlignment {
  image_size: [number, number];
  image_px: Quad;
  bed_mm: Quad;
  /** Homografía píxel -> mm a la altura `reference_height_mm`, por filas. */
  to_bed: Mat3;
  rms_mm: number;
  method?: AlignmentMethod;
  /** Altura (mm sobre la cama) del plano al que corresponde `to_bed`. */
  reference_height_mm?: number;
  /** Marcas por altura (vacío en la alineación manual). */
  samples?: AlignmentSample[];
  /** Modelo para el grosor del material; `null` si no hay forma de estimarlo. */
  height?: HeightModel | null;
}

export interface CameraPrefs {
  device_id: string | null;
  device_label: string | null;
  width: number | null;
  height: number | null;
  /** Opacidad del fondo en el lienzo (0..1). */
  opacity: number;
  /** El fondo estaba visible al cerrar. */
  visible: boolean;
  /** Grosor del material actual (mm sobre la cama): el fondo se corrige a esa altura. */
  material_height_mm?: number;
}

/** Una marca de la disposición con lo que se encontró de ella (`detect_camera_marks`). */
export interface MarkResult {
  index: number;
  /** Posición teórica en la cama (mm). */
  bed: [number, number];
  /** Centro detectado en la imagen (px); `null` si no se encontró. */
  image: [number, number] | null;
  /** Calidad de la detección (0..1). */
  confidence: number;
  /** Participa en el ajuste final. */
  used: boolean;
  /** Distancia en la cama (mm) entre la marca proyectada con el ajuste y su posición. */
  residual_mm: number | null;
  /** Error de validación cruzada (mm): ajuste sin esta marca. */
  loo_mm: number | null;
}

export interface MarksReport {
  marks: MarkResult[];
  /** Todos los candidatos con forma de cruz que vio el detector (px). */
  candidates: [number, number][];
  /** Homografía imagen -> cama, por filas. */
  to_bed: Mat3;
  expected: number;
  found: number;
  used: number;
  /** Error del ajuste (rms, mm) sobre las marcas usadas. */
  rms_mm: number;
  max_residual_mm: number;
  /** Validación cruzada (dejar una fuera); `null` con menos de 5 marcas. */
  loo_rms_mm: number | null;
  loo_max_mm: number | null;
  /** Escala de la imagen en el centro de la disposición (px por mm). */
  scale_px_per_mm: number;
  /** Se usó la alineación provisional para asignar. */
  used_prior: boolean;
  warnings: string[];
}

/** Lo que sobrevive al reinicio (archivo `camera.json` en la carpeta de configuración). */
export interface CameraSetup {
  version: number;
  calibration: LensCalibration | null;
  alignment: CameraAlignment | null;
  prefs: CameraPrefs;
}
