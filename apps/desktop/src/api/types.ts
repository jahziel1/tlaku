/**
 * Tipos del contrato app <-> motor (docs/app-api/README.md).
 *
 * Todo viene de `src/bindings/` (generado con ts-rs desde el crate `tlaku-doc`; no editar a
 * mano): documento, dispositivo, trabajo, resultados de comandos y eventos de la máquina. Aquí se
 * reexportan con los nombres que usa la app y se definen solo las piezas que no cruzan la
 * frontera como tipo propio (`JobArgs`, `EventMap`, `Point`, `Affine`).
 */

import type { RemoteClient } from './api';
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
import type { VariableText } from '@/bindings/VariableText';
import type { DataSource } from '@/bindings/DataSource';
import type { ImageFit } from '@/bindings/ImageFit';
import type { ShortRows } from '@/bindings/ShortRows';
import type { VariableCode } from '@/bindings/VariableCode';
import type { VariableImage } from '@/bindings/VariableImage';
import type { BufferStatus } from '@/bindings/BufferStatus';
import type { ConnectResult } from '@/bindings/ConnectResult';
import type { RuidaApplyReport } from '@/bindings/RuidaApplyReport';
import type { RuidaBackup } from '@/bindings/RuidaBackup';
import type { RuidaCalibration } from '@/bindings/RuidaCalibration';
import type { RuidaControllerSettings } from '@/bindings/RuidaControllerSettings';
import type { RuidaDocResult } from '@/bindings/RuidaDocResult';
import type { RuidaFile } from '@/bindings/RuidaFile';
import type { RuidaPlannedChange } from '@/bindings/RuidaPlannedChange';
import type { RuidaSetting } from '@/bindings/RuidaSetting';
import type { RuidaSettingChange } from '@/bindings/RuidaSettingChange';
import type { RuidaSettingsPlan } from '@/bindings/RuidaSettingsPlan';
import type { RuidaVerified } from '@/bindings/RuidaVerified';
import type { RuidaInfo } from '@/bindings/RuidaInfo';
import type { ConnectionEvent } from '@/bindings/ConnectionEvent';
import type { ExportResult } from '@/bindings/ExportResult';
import type { JobEvent } from '@/bindings/JobEvent';
import type { JobState } from '@/bindings/JobState';
import type { LogDir } from '@/bindings/LogDir';
import type { LogEvent } from '@/bindings/LogEvent';
import type { MachineState } from '@/bindings/MachineState';
import type { OverrideKind } from '@/bindings/OverrideKind';
import type { Overrides } from '@/bindings/Overrides';
import type { ProgressEvent } from '@/bindings/ProgressEvent';
import type { RealtimeCommand } from '@/bindings/RealtimeCommand';
import type { SimulatedFault } from '@/bindings/SimulatedFault';
import type { InterruptedJob } from '@/bindings/InterruptedJob';
import type { ResumeAir } from '@/bindings/ResumeAir';
import type { ResumeKind } from '@/bindings/ResumeKind';
import type { ResumeMachine } from '@/bindings/ResumeMachine';
import type { ResumeModal } from '@/bindings/ResumeModal';
import type { ResumePoint } from '@/bindings/ResumePoint';
import type { ResumeReadiness } from '@/bindings/ResumeReadiness';
import type { ResumeSpindle } from '@/bindings/ResumeSpindle';
import type { ResumeStart } from '@/bindings/ResumeStart';
import type { ResumeView } from '@/bindings/ResumeView';
import type { StatusReport } from '@/bindings/StatusReport';
import type { FillStyle } from '@/bindings/FillStyle';
import type { TabSettings } from '@/bindings/TabSettings';
import type { RotaryKind } from '@/bindings/RotaryKind';
import type { RotarySettings } from '@/bindings/RotarySettings';
import type { ScanOffset } from '@/bindings/ScanOffset';
import type { Confidence } from '@/bindings/Confidence';
import type { LaserKind } from '@/bindings/LaserKind';
import type { MachineClass } from '@/bindings/MachineClass';
import type { MaterialLibrary } from '@/bindings/MaterialLibrary';
import type { MaterialPreset } from '@/bindings/MaterialPreset';
import type { MaterialRestriction } from '@/bindings/MaterialRestriction';
import type { MaterialRole } from '@/bindings/MaterialRole';
import type { MaterialTestParams } from '@/bindings/MaterialTestParams';
import type { MaterialWarning } from '@/bindings/MaterialWarning';
import type { TestAxis } from '@/bindings/TestAxis';
import type { TestParam } from '@/bindings/TestParam';
import type { Guide } from '@/bindings/Guide';
import type { GuideAxis } from '@/bindings/GuideAxis';
import type { StartSpec } from '@/bindings/StartSpec';
import type { Macro } from '@/bindings/Macro';
import type { LbdevImport } from '@/bindings/LbdevImport';
import type { LbdevNotice } from '@/bindings/LbdevNotice';
import type { LbdevNoticeKind } from '@/bindings/LbdevNoticeKind';
import type { SavedPosition } from '@/bindings/SavedPosition';
import type { LaserSource } from '@/bindings/LaserSource';

export type {
  LbdevImport,
  LbdevNotice,
  LbdevNoticeKind,
  Confidence,
  LaserKind,
  MachineClass,
  LaserSource,
  Macro,
  SavedPosition,
  Guide,
  GuideAxis,
  StartSpec,
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
  MaterialRestriction,
  MaterialRole,
  MaterialTestParams,
  MaterialWarning,
  OverrideKind,
  Overrides,
  PathSeg,
  ProgressEvent,
  RealtimeCommand,
  RuidaApplyReport,
  RuidaBackup,
  RuidaCalibration,
  RuidaControllerSettings,
  RuidaDocResult,
  RuidaFile,
  RuidaInfo,
  RuidaPlannedChange,
  RuidaSetting,
  RuidaSettingChange,
  RuidaSettingsPlan,
  RuidaVerified,
  RotaryKind,
  RotarySettings,
  ScanOffset,
  SimulatedFault,
  InterruptedJob,
  ResumeAir,
  ResumeKind,
  ResumeMachine,
  ResumeModal,
  ResumePoint,
  ResumeReadiness,
  ResumeSpindle,
  ResumeStart,
  ResumeView,
  StartFrom,
  StatusReport,
  TabSettings,
  TestAxis,
  TestParam,
  TextAlign,
  TextObject,
  TextOnPath,
  VectorObject,
  VariableCode,
  VariableImage,
  VariableText,
  DataSource,
  ImageFit,
  ShortRows,
};
export type DeviceOrigin = Origin;
export type ImageDitherMode = ImageMode;
export type JobLayerSummary = PreviewLayer;

export type Point = [number, number];

/** Resultado de un toque de sonda (M48). Las posiciones son donde quedó el cabezal al tocar. */
export interface ProbeTouch {
  /** El pin de la sonda cambió dentro del recorrido; `false` = no tocó nada. */
  hit: boolean;
  /** Posición de máquina [x, y], mm. */
  machine_pos: Point;
  /** Posición de trabajo [x, y], mm (la de `work_pos` del reporte de estado). */
  work_pos: Point;
}

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
// K40 con placa M2 Nano (los devuelve `src-tauri/src/machine/lhymicro.rs`)
// ---------------------------------------------------------------------------

/** Qué avisa el codificador LHYMICRO-GL; cada uno tiene su texto en `lhymicro.warn.<kind>`. */
export type LhyWarningKind = 'power_ignored' | 'gray_thresholded' | 'air_assist_ignored' | 'z_ignored' | 'dwell_skipped';

export interface LhyWarning {
  kind: LhyWarningKind;
  /** Número del bloque del trabajo (desde 0). */
  block: number;
  /** Nombre de la capa del bloque, si se conoce. */
  layer: string | null;
  /** Potencia pedida (%) en `power_ignored`; milisegundos en `dwell_skipped`. */
  value: number | null;
  /** Solo en `dwell_skipped`: era un pulso (disparaba el láser) y no una pausa. */
  fires: boolean;
}

/** Lo que se sabe de un trabajo antes de enviarlo a un K40 (`lhy_preview`); no envía nada. */
export interface LhyPreview {
  board: string;
  warnings: LhyWarning[];
  packets: number;
  bytes: number;
  /** Tiempo estimado por exceso (s). */
  est_seconds: number;
  burn_mm: number;
  travel_mm: number;
  /** La posición del cabezal se desconoce: el envío hace home primero. */
  homes_first: boolean;
}

/** Estado propio de la conexión K40 (`lhy_state`). */
export interface LhyState {
  board: string;
  virtual_machine: boolean;
  /** Se conoce la posición del cabezal (se cuenta desde el último home). */
  position_known: boolean;
  /** Último código de estado de la placa (206 lista, 238 ocupada, 236 terminó, 239 sin energía...); 0 si no hay lectura. */
  status_code: number;
}

// ---------------------------------------------------------------------------
// Galvo con tarjeta JCZ/BJJCZ (los devuelve `src-tauri/src/machine/galvo.rs`)
// ---------------------------------------------------------------------------

/**
 * Lo que el usuario confirma en el diálogo de seguridad antes de abrir una tarjeta galvo REAL
 * (`usb:galvo:<n>`). Las tres casillas deben estar marcadas; se pide en cada conexión y no se recuerda.
 */
export interface GalvoSafetyAck {
  /** Gafas con la densidad óptica correcta para la longitud de onda de la fuente. */
  eyewear: boolean;
  /** Recinto cerrado. */
  enclosure: boolean;
  /** Sabe dónde está el paro de emergencia. */
  emergency_stop: boolean;
}

/** Un paso del marcado dividido, repetido o rotativo y lo que hay que hacer antes de marcarlo. */
export interface GalvoStepInfo {
  /** Paso actual y total (desde 1). */
  step: number;
  steps: number;
  /** Repetición y pieza dentro de ella (desde 1) y sus totales. */
  repeat: number;
  repeats: number;
  piece: number;
  pieces: number;
  /** Cuánto debe moverse el material (la mesa), mm. */
  dx: number;
  dy: number;
  /** Giro del rotativo, grados. */
  rotary_deg: number;
  /** Pausa tras confirmar el movimiento, ms. */
  pause_ms: number;
}

/** Lo que se sabe de un trabajo galvo antes de enviarlo (`galvo_preview`); no envía nada. */
export interface GalvoPreview {
  steps: number;
  pieces: number;
  repeats: number;
  /** Veces que habrá que mover el material (o girar el rotativo) a mano. */
  manual_moves: number;
  est_seconds: number;
  warnings: string[];
  field_mm: number;
  source: LaserSource;
  /** Potencia máxima del perfil (%). */
  max_power_pct: number;
}

/** Estado propio de la conexión galvo (`galvo_state`). */
export interface GalvoState {
  board: string;
  vendor_id: number;
  product_id: number;
  virtual_machine: boolean;
  serial: string;
  version: string;
  /** Palabra de estado de la tarjeta (sin verificar). */
  status_raw: number;
  busy: boolean;
  ready: boolean;
  axis: boolean;
  /** Entradas de la tarjeta (solo lectura). */
  inputs: number;
  inputs_known: boolean;
  /** Hay un trabajo, un encuadre o una espera en curso. */
  working: boolean;
  /** El puntero rojo está encendido en su sitio. */
  red_light_on: boolean;
  /** El último apagado NO se confirmó: la fuente puede seguir habilitada. */
  laser_may_be_on: boolean;
  /** Posición del haz según la última orden de salto (mm del documento). */
  position: [number, number];
  position_known: boolean;
  field_mm: number;
  source: LaserSource;
  /** El trabajo espera que muevas el material y confirmes. */
  waiting_move: GalvoStepInfo | null;
  /** Hay un encuadre con el puntero en curso (se puede mover y escalar en vivo). */
  framing: boolean;
  /** Enclavamiento por entrada del perfil y si ahora se cumple; `null` = sin enclavamiento. */
  interlock: GalvoInterlock | null;
  /** Desfase del puntero rojo con el que se conectó (mm) y overscan del perfil (mm). */
  pointer_offset: [number, number];
  overscan_mm: number;
}

/** La condición `(entradas & mask) == value` que el perfil exige antes de cada trabajo. */
export interface GalvoInterlock {
  mask: number;
  value: number;
  /** Se cumple con las últimas entradas leídas. */
  met: boolean;
}

/** Qué dibuja el puntero rojo al encuadrar con el galvo. */
export type GalvoFrameKind = 'rect' | 'contour' | 'hull' | 'objects';

/** Una comprobación de la verificación sin láser (`galvo_verify`). */
export interface GalvoVerifyCheck {
  /** `board`, `idle`, `frame` o `interlock`. */
  id: string;
  ok: boolean;
  detail: string;
}

/** Resultado de `galvo_verify`: lo que dijo la tarjeta (solo lectura) y las comprobaciones. */
export interface GalvoVerifyReport {
  board: string;
  vendor_id: number;
  product_id: number;
  virtual_machine: boolean;
  serial: string;
  version: string;
  status_raw: number;
  busy: boolean;
  ready: boolean;
  inputs: number;
  interlock: GalvoInterlock | null;
  checks: GalvoVerifyCheck[];
}

/** Evento `machine://galvo`. */
export type GalvoEvent =
  | { kind: 'step'; info: GalvoStepInfo }
  | { kind: 'wait_move'; info: GalvoStepInfo }
  /** `adjust_gen`: versión del último ajuste en vivo aplicado; `adjust_rejected`: el último no cabía en el campo. */
  | { kind: 'light'; loops: number; adjust_gen: number; adjust_rejected: boolean }
  | { kind: 'laser_may_be_on'; message: string };

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
  /** Lo propio del galvo: pasos del marcado dividido, vueltas del encuadre y el aviso de cortar la energía. */
  'machine://galvo': GalvoEvent;
  /** Un programa externo pidió abrir un archivo (`LOAD` de la automatización por UDP). */
  'automation://load': { path: string };
  /** Pidió iniciar el trabajo (`START`); con `confirmed` el usuario ya lo permitió en Preferencias. */
  'automation://start': { confirmed: boolean };
  /** Los celulares conectados al control remoto cambiaron (entró, salió o lo desconectaron). */
  'remote://clients': { clients: RemoteClient[] };
  /** Un celular pidió iniciar el trabajo (con su checklist completo); la ventana responde con `remoteStartResult`. */
  'remote://start': { id: number; client: string };
  /** Se pausó, reanudó o detuvo el trabajo desde un celular. */
  'remote://action': { action: 'pause' | 'resume' | 'stop'; client: string };
  /** Ventana aparte de la vista en vivo: la ruta del trabajo (texto JSON, ver `features/live/wire.ts`). Llega solo a la ventana `live`. */
  'live://route': string;
  /** Ventana aparte de la vista en vivo: el estado y el avance (texto JSON). Llega solo a la ventana `live`. */
  'live://state': string;
  /** La ventana aparte ya escucha y pide la ruta y el estado. Llega a la ventana que la abrió. */
  'live://ready': null;
  /** Se cerró la ventana aparte. Llega a la ventana que la abrió. */
  'live://closed': null;
}

export type EventName = keyof EventMap;

// ---------------------------------------------------------------------------------------------
// Fase 3. TODO(bindings): cada área reemplaza estos tipos por los generados con ts-rs desde su
// crate cuando conecte el backend (texto, geometría, vectorizado y cámara).

export interface FontFamily {
  family: string;
  styles: string[];
  /** Fuente de un solo trazo (Hershey o SHX): da trayectorias abiertas, no contornos. */
  single_line?: boolean;
}

// Códigos QR y de barras (crate tlaku-codes; `generate_code`).
export type CodeKind = 'qr' | 'data_matrix' | 'code128' | 'code39' | 'ean13' | 'ean8' | 'upc_a' | 'itf' | 'codabar';
export type QrEcc = 'l' | 'm' | 'q' | 'h';

export interface CodeSpec {
  kind: CodeKind;
  data: string;
  ecc: QrEcc;
  /** 2D: lado del área de módulos. 1D: ancho de las barras (mm). */
  size_mm: number;
  /** 1D: alto de las barras (mm). */
  height_mm: number;
  /** Zona en blanco; solo se dibuja al invertir (mm). */
  margin_mm: number;
  human_readable: boolean;
  invert: boolean;
  /** Fuente de contorno para el texto legible. */
  font_family: string;
}

export interface CodeResult {
  /** Formas cerradas (cada una, un exterior con sus agujeros), en mm con la esquina en (0, 0). */
  paths: BezPath[];
  width_mm: number;
  height_mm: number;
  modules: number;
  warnings: string[];
}

export interface TextOutlineResult {
  paths: BezPath[];
  warnings: string[];
}

/** Resultado de mirar el archivo de una imagen variable (comando `probe_images`). */
export interface ImageProbe {
  /** La ruta tal como se pidió. */
  path: string;
  /** El archivo existe y se pudo leer su cabecera. */
  ok: boolean;
  width_px: number;
  height_px: number;
  /** Por qué no (archivo ausente, formato desconocido...). */
  error: string | null;
}

/** CSV del texto variable (comando `read_csv`). */
export interface CsvTable {
  /** Nombres de columna (primera fila). */
  headers: string[];
  /** Filas de datos; cada una con al menos tantas celdas como encabezados. */
  rows: string[][];
  /** Separador detectado: `,`, `;` o tabulador. */
  delimiter: string;
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

/** Modelo de la lente: Brown-Conrady (`k1,k2,k3,p1,p2`) u ojo de pez de Kannala-Brandt (`k1..k4`). */
export type LensModel = 'brown_conrady' | 'fisheye';
/** Qué modelo ajustar al calibrar: uno fijo o `auto` (elige el motor). */
export type LensModelChoice = 'auto' | LensModel;

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
  /** Faltan en las calibraciones guardadas antes del ojo de pez: valen `brown_conrady`, 0 y 1. */
  model?: LensModel;
  k4?: number;
  /** Focal de la imagen corregida respecto de `fx`/`fy` (1 con Brown-Conrady; menor con ojo de pez). */
  view_zoom?: number;
  rms_error_px: number;
  views_used: number;
}

/** Cómo le fue a un modelo de lente con las mismas vistas. */
export interface ModelScore {
  model: LensModel;
  /** Error de reproyección rms con todas las vistas (px). */
  rms_px: number;
  /** Error rms en vistas reservadas (px); `null` con muy pocas vistas. */
  cv_rms_px: number | null;
  chosen: boolean;
}

export interface CalibrationReport {
  calibration: LensCalibration;
  warnings: string[];
  used_views: number[];
  view_rms_px: number[];
  /** Puntaje de cada modelo probado (uno solo si se fijó el modelo). Falta en el mock antiguo. */
  model_scores?: ModelScore[];
}

/** Una foto corregida de lente con su posición en la cama (píxel con origen en el borde -> mm). */
export interface BedTile {
  image_png_base64: string;
  to_bed: [[number, number, number], [number, number, number], [number, number, number]];
}

export interface ComposeBedArgs {
  tiles: BedTile[];
  bed: [number, number];
  /** Resolución (px por mm); `null` = la de la foto, con tope. */
  px_per_mm: number | null;
  grayscale: boolean;
  /** Ancho de la rampa de mezcla en los bordes de cada foto (px de la foto). */
  feather_px: number | null;
  /** Franja del borde de cada foto que se descarta (px de la foto). */
  crop_px: number | null;
  /** `[x0, y0, x1, y1]` en mm; `null` = lo que cubren las fotos dentro de la cama. */
  rect: [number, number, number, number] | null;
  /** Carpeta del proyecto; `null` = carpeta temporal. */
  dir: string | null;
  stem: string;
  /** Escribir el PNG (si no, solo vuelve la vista previa). */
  save: boolean;
}

/** Imagen compuesta de la cama y dónde queda en el documento. */
export interface BedImage {
  /** Archivo escrito; vacío sin `save`. */
  path: string;
  width_px: number;
  height_px: number;
  size_mm: [number, number];
  /** Esquina inferior izquierda en el documento (mm). */
  origin_mm: [number, number];
  in_temp_dir: boolean;
  /** Fracción cubierta por alguna foto (0..1). */
  coverage: number;
  px_per_mm: number;
  /** PNG en base64 reducido a un ancho máximo de 1600 px. */
  preview_png_base64: string;
}

export interface CircleHint {
  center: [number, number];
  radius?: number | null;
}

/** Círculo hallado en una foto (px, origen en el borde de la imagen). */
export interface CircleFit {
  center: [number, number];
  radius: number;
  rms_px: number;
  inliers: number;
  /** Fracción del contorno visible (0..1). */
  arc_coverage: number;
  /** Muestra de puntos del borde: se llevan a mm y allí se vuelve a ajustar el círculo. */
  points: [number, number][];
}

/** Calibración de la cámara montada en el cabezal (ver `HeadCalibration` en edit/camera.rs). */
export interface HeadCalibration {
  image_size: [number, number];
  ref_px: [number, number];
  /** Píxeles a mm, por filas: `[[a, b], [c, d]]`. */
  px_to_mm: [[number, number], [number, number]];
  /** Desfase del centro de la imagen respecto del láser (mm). */
  offset_mm: [number, number];
  rms_mm: number;
  observations: number;
}

export interface HeadCamera {
  enabled: boolean;
  calibration: HeadCalibration | null;
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

/** De dónde viene el video: una cámara local (`getUserMedia`) o una cámara de red (URL). */
export type CameraSource = 'local' | 'network';

export interface CameraPrefs {
  /** Fuente del video. Falta en configuraciones antiguas (= `local`). */
  source?: CameraSource;
  /**
   * URL de la cámara de red (`http://...`, snapshot o MJPEG). Puede llevar `usuario:clave@`: se
   * guarda solo en el perfil local (`camera.json`) y nunca entra en un paquete `.tlakubundle`.
   */
  network_url?: string | null;
  /** Resolución del fondo (puntos por mm) fijada a mano; `null` = la de la alineación. */
  dpm?: number | null;
  /** Desplazamiento del fondo (mm) para afinar la alineación a mano. */
  offset_x_mm?: number;
  offset_y_mm?: number;
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
  /** El fondo se muestra en escala de grises. */
  grayscale?: boolean;
  /** Refrescar el fondo al terminar un trabajo. */
  update_on_job_finish?: boolean;
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

/** Configuración de cámara con nombre: la fuente y los ajustes del fondo. */
export interface CameraPreset {
  name: string;
  source: CameraSource;
  device_id: string | null;
  device_label: string | null;
  width: number | null;
  height: number | null;
  /** URL de la cámara de red (puede llevar credenciales: ver `CameraPrefs.network_url`). */
  network_url: string | null;
  dpm: number | null;
  offset_x_mm: number;
  offset_y_mm: number;
  grayscale: boolean;
}

/** Un cuadro de una cámara de red, pedido por el motor (`fetch_camera_frame`). */
export interface NetworkFrame {
  /** `image/jpeg`, `image/png` o `image/bmp`. */
  mime: string;
  data_base64: string;
}

/** Lo que sobrevive al reinicio (archivo `camera.json` en la carpeta de configuración). */
export interface CameraSetup {
  version: number;
  calibration: LensCalibration | null;
  alignment: CameraAlignment | null;
  prefs: CameraPrefs;
  /** Cámara montada en el cabezal. Falta en configuraciones anteriores a la ola 4. */
  head?: HeadCamera;
  /** Configuraciones de cámara con nombre. Falta en configuraciones anteriores a la ola 6. */
  presets?: CameraPreset[];
}

// ---- Diseño avanzado (ola 3): deformar, contorno envolvente y círculo tangente ----------------------

/** Borde Bézier cúbico de una envolvente: inicio, dos controles y fin. */
export type EnvelopeEdge = [Point, Point, Point, Point];

/**
 * Envolvente de la deformación: `bottom` y `top` van de izquierda a derecha; `left` y `right`, de
 * abajo hacia arriba. Las esquinas las fijan `bottom` y `top`.
 */
export interface WarpEnvelope {
  bottom: EnvelopeEdge;
  top: EnvelopeEdge;
  left: EnvelopeEdge;
  right: EnvelopeEdge;
  /**
   * Malla de 4x4 puntos por los que pasa la superficie: `grid[j][i]` es la imagen del punto
   * `(i/3, j/3)` de la región de origen (`j` sube). Con ella se ignoran los cuatro bordes.
   */
  grid?: WarpGrid;
  /**
   * Proyección de cuatro esquinas (abajo-izquierda, abajo-derecha, arriba-derecha, arriba-izquierda).
   * Con ella se ignoran los cuatro bordes; si hay `grid`, manda la malla.
   */
  perspective?: [Point, Point, Point, Point];
}

/** Cuatro filas (de abajo hacia arriba) de cuatro puntos (de izquierda a derecha). */
export type WarpGrid = [
  [Point, Point, Point, Point],
  [Point, Point, Point, Point],
  [Point, Point, Point, Point],
  [Point, Point, Point, Point],
];

export interface WarpArgs {
  /** Objetos en coordenadas de documento; vuelven vectoriales, con el mismo id, y con transform identidad. */
  objects: DesignObject[];
  /** Región de origen que la envolvente deforma (normalmente la caja de la selección). */
  bounds: [Point, Point];
  envelope: WarpEnvelope;
  /** Error máximo (mm) de las curvas deformadas; por defecto 0,01. */
  tolerance?: number;
}

export interface HullArgs {
  objects: DesignObject[];
  /** Con valor: casco cóncavo con aristas de a lo sumo estos mm donde se pueda; sin valor: convexo. */
  max_edge?: number | null;
  /** Capa (`#rrggbb`) del contorno; por defecto la del primer objeto. */
  layer?: string | null;
}

export interface TangentArgs {
  /** Dos o tres operandos (un grupo viaja como un solo objeto). */
  objects: DesignObject[];
  /** Obligatorio con dos operandos; con tres sale de las formas y se ignora. */
  radius?: number | null;
}

export interface TangentCircle {
  center: Point;
  radius: number;
}

/** Copias virtuales de un objeto (`DesignObject.array`): rejilla o circular. */
export type { VirtualArray } from '@/bindings/VirtualArray';

// --- Operaciones de forma (ola 3): recortar, separar, optimizar, redondear, cortar y ranuras ---

/**
 * `trim`: borra el tramo de trazo más cercano a `point` entre dos intersecciones;
 * `break_apart`: un objeto por subtrayecto; `optimize`: menos nodos con un error acotado;
 * `fillet`: redondea, achaflana o muerde las esquinas de recta con recta;
 * `cut_shapes`: una forma cerrada (`knife_id`) parte a las demás en lo de dentro y lo de fuera;
 * `resize_slots`: cambia ranuras de un ancho a otro.
 */
export type ShapeOp = 'trim' | 'break_apart' | 'optimize' | 'fillet' | 'cut_shapes' | 'resize_slots';

export type FilletKind = 'round' | 'chamfer' | 'bite';
export type SlotAnchor = 'center' | 'start' | 'end';

/** Cada operación lee solo sus parámetros (ver `edit/shape_ops.rs`). */
export interface ShapeParams {
  point?: Point;
  max_dist?: number;
  keep_removed?: boolean;
  keep_holes?: boolean;
  tolerance?: number;
  smooth?: boolean;
  corner_angle?: number;
  radius?: number;
  style?: FilletKind;
  /** `fillet`: solo las esquinas en estos puntos (mm, documento). */
  nodes?: Point[];
  /** `fillet`: devuelve a punta la esquina redondeada o mordida más cercana a `point` (los chaflanes no se revierten). */
  revert?: boolean;
  knife_id?: string;
  close_pieces?: boolean;
  keep_knife?: boolean;
  from_width?: number;
  to_width?: number;
  anchor?: SlotAnchor;
  notches?: boolean;
  holes?: boolean;
  tabs?: boolean;
}

export interface ShapeOpArgs {
  op: ShapeOp;
  objects: DesignObject[];
  params?: ShapeParams;
}

/** Números del resultado; lo que una operación no usa viene en 0 o vacío. */
export interface ShapeStats {
  count: number;
  skipped: number;
  clamped: number;
  produced: number;
  found: number;
  nodes_before: number;
  nodes_after: number;
  /** `resize_slots`: `[ancho, cuántas]` de cada ancho detectado. */
  widths: [number, number][];
  /** `trim`: puntos del tramo que se borra. */
  removed: Point[];
}

/** Misma convención de resultado que `join`/`close`/`dedupe`: la lista completa de objetos de entrada. */
export interface ShapeOpResult {
  objects: DesignObject[];
  stats: ShapeStats;
}
