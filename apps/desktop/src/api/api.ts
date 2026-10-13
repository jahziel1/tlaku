import type {
  ConnectResult,
  DeviceProfile,
  Document,
  EventMap,
  EventName,
  ExportResult,
  ImportResult,
  JobArgs,
  JobPreview,
  RealtimeCommand,
} from './types';
import type {
  BezPath,
  CalibrationReport,
  CameraSetup,
  MarksReport,
  DesignObject,
  FontFamily,
  GeometryOpArgs,
  MaterialLibrary,
  MaterialTestParams,
  TextObject,
  TextOutlineResult,
  TraceResult,
  TraceSettings,
} from './types';

export type Unlisten = () => void;

/**
 * Contrato único entre la interfaz y el motor (docs/app-api.md).
 * Una implementación real (Tauri invoke/listen) y una simulada (mock en memoria).
 * Los componentes y los slices usan siempre esta interfaz, nunca `invoke` directamente.
 */
export interface LaserApi {
  /** Comando de prueba de src-tauri. */
  appVersion(): Promise<string>;

  // Archivos y proyecto
  importFile(args: { path: string }): Promise<ImportResult>;
  saveProject(args: { path: string; doc: Document }): Promise<void>;
  loadProject(args: { path: string }): Promise<Document>;

  // Trabajo
  computeJob(args: JobArgs): Promise<JobPreview>;
  exportGcode(args: JobArgs & { path: string }): Promise<ExportResult>;

  // Dispositivos
  listDevices(): Promise<DeviceProfile[]>;
  saveDevice(args: { device: DeviceProfile }): Promise<void>;
  deleteDevice(args: { id: string }): Promise<void>;

  // Máquina
  listPorts(): Promise<string[]>;
  connect(args: { port: string; baud: number }): Promise<ConnectResult>;
  disconnect(): Promise<void>;
  /**
   * Devuelve las líneas de respuesta; la última es `ok`, o `error:N: ...` / `ALARM:N: ...` con su
   * explicación. No acepta `?`, `!` ni `~` (usa `machineRealtime`). Falla con un trabajo en curso.
   */
  machineCommand(args: { line: string }): Promise<string[]>;
  machineRealtime(args: { cmd: RealtimeCommand }): Promise<void>;
  /** `$J=G91 G21 X.. Y.. F..` relativo, en mm y mm/min. Falla con un trabajo en curso o en alarma. */
  jog(args: { dx: number; dy: number; speed: number }): Promise<void>;
  /**
   * Calcula, comprueba el estado de la máquina y transmite. Rechaza (sin enviar nada y con un
   * mensaje que explica qué hacer) si la máquina está en Alarm, Hold, Door, Run o Jog.
   */
  startJob(args: JobArgs): Promise<void>;
  pauseJob(): Promise<void>;
  resumeJob(): Promise<void>;
  /** Con trabajo: feed hold y soft reset. Sin trabajo: cancela el jog en curso (0x85). */
  cancelJob(): Promise<void>;
  /** Encuadre con el láser apagado; mismas comprobaciones de estado que `startJob`. */
  frameJob(args: JobArgs & { speed: number }): Promise<void>;

  // Edición (fase 3)
  listFonts(): Promise<FontFamily[]>;
  /**
   * Contorno del texto. `path` es la trayectoria guía si va sobre una curva, en coordenadas LOCALES
   * del texto (con la inversa del `transform` del texto aplicada), no de documento.
   */
  textOutline(args: { text: TextObject; path: BezPath | null }): Promise<TextOutlineResult>;
  /** Booleanas, soldar y offset; el resultado viene en coordenadas de documento con transform identidad. */
  geometryOp(args: GeometryOpArgs): Promise<DesignObject[]>;
  traceImage(args: { path: string; settings: TraceSettings; size_mm: [number, number] }): Promise<TraceResult>;

  // Cámara (fase 3): las imágenes viajan como PNG en base64, sin prefijo data:.
  /** Esquinas internas del tablero en píxeles, o `null` si no se encontró. */
  detectBoard(args: { image_png_base64: string; board: [number, number] }): Promise<[number, number][] | null>;
  calibrateLens(args: { images_png_base64: string[]; board: [number, number]; square_mm: number }): Promise<CalibrationReport>;
  /** Calibración, alineación y preferencias guardadas (`camera.json`). Sin archivo, una configuración vacía. */
  loadCameraSetup(): Promise<CameraSetup>;
  /** Valida, recalcula la homografía y guarda; devuelve lo guardado. */
  saveCameraSetup(args: { setup: CameraSetup }): Promise<CameraSetup>;
  /** Escribe el tablero de calibración (`.svg` o `.pdf`) en la ruta que eligió el usuario. */
  saveBoardFile(args: { path: string; content_base64: string }): Promise<void>;
  /**
   * Documento con 4 o 9 cruces en posiciones conocidas de la cama (capa line de baja potencia) para
   * grabarlas sobre un cartón. La esquina inferior izquierda lleva además un círculo.
   */
  cameraMarks(args: { bed: [number, number]; count: 4 | 9 }): Promise<Document>;
  /**
   * Busca las marcas en una foto ya corregida de lente, las asigna a sus posiciones, ajusta la
   * homografía con RANSAC y mide la precisión por validación cruzada. `prior_to_bed` es una
   * alineación provisional (imagen -> mm, por filas) que afianza la asignación. Los píxeles tienen
   * el origen en el borde de la imagen, como el resto de la alineación.
   */
  detectCameraMarks(args: {
    image_png_base64: string;
    bed: [number, number];
    count: 4 | 9;
    prior_to_bed: [[number, number, number], [number, number, number], [number, number, number]] | null;
  }): Promise<MarksReport>;

  // Materiales (fase 4)
  /** Biblioteca del usuario (`materials.json`). Sin archivo, una vacía. */
  loadMaterialLibrary(): Promise<MaterialLibrary>;
  saveMaterialLibrary(args: { library: MaterialLibrary }): Promise<void>;
  /** Lee un archivo `.tlakumat`; la interfaz decide cómo mezclarlo con la biblioteca actual. */
  importMaterialLibrary(args: { path: string }): Promise<MaterialLibrary>;
  exportMaterialLibrary(args: { path: string; library: MaterialLibrary }): Promise<void>;
  /** Documento con la cuadrícula de prueba (una capa por celda más la de rótulos). */
  materialTest(args: { params: MaterialTestParams }): Promise<Document>;

  /** Suscripción a eventos `machine://*`. Devuelve la función para cancelar. */
  on<K extends EventName>(event: K, handler: (payload: EventMap[K]) => void): Promise<Unlisten>;
}

/** Nombres de comando tal como los ve `invoke` (snake_case). */
export const COMMAND_NAMES = {
  appVersion: 'app_version',
  importFile: 'import_file',
  saveProject: 'save_project',
  loadProject: 'load_project',
  computeJob: 'compute_job',
  exportGcode: 'export_gcode',
  listDevices: 'list_devices',
  saveDevice: 'save_device',
  deleteDevice: 'delete_device',
  listPorts: 'list_ports',
  connect: 'connect',
  disconnect: 'disconnect',
  machineCommand: 'machine_command',
  machineRealtime: 'machine_realtime',
  jog: 'jog',
  startJob: 'start_job',
  pauseJob: 'pause_job',
  resumeJob: 'resume_job',
  cancelJob: 'cancel_job',
  frameJob: 'frame_job',
  listFonts: 'list_fonts',
  textOutline: 'text_outline',
  geometryOp: 'geometry_op',
  traceImage: 'trace_image',
  detectBoard: 'detect_board',
  calibrateLens: 'calibrate_lens',
  loadCameraSetup: 'load_camera_setup',
  saveCameraSetup: 'save_camera_setup',
  saveBoardFile: 'save_board_file',
  cameraMarks: 'camera_marks',
  detectCameraMarks: 'detect_camera_marks',
  loadMaterialLibrary: 'load_material_library',
  saveMaterialLibrary: 'save_material_library',
  importMaterialLibrary: 'import_material_library',
  exportMaterialLibrary: 'export_material_library',
  materialTest: 'material_test',
} as const satisfies Record<Exclude<keyof LaserApi, 'on'>, string>;

export const EVENT_NAMES: readonly EventName[] = [
  'machine://status',
  'machine://progress',
  'machine://log',
  'machine://job',
  'machine://connection',
];
