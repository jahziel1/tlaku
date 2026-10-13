import type { DeviceKind } from '@/bindings/DeviceKind';
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
  OverrideKind,
  RealtimeCommand,
  SimulatedFault,
} from './types';
import type {
  BezPath,
  CalibrationReport,
  CameraSetup,
  MarksReport,
  DesignObject,
  ImageObject,
  LayerSettings,
  CodeResult,
  CodeSpec,
  FontFamily,
  GeometryOpArgs,
  ShapeOpArgs,
  ShapeOpResult,
  HullArgs,
  TangentArgs,
  TangentCircle,
  WarpArgs,
  MaterialLibrary,
  MaterialTestParams,
  TextObject,
  TextOutlineResult,
  CsvTable,
  TraceResult,
  TraceSettings,
} from './types';

import type { AddArtResult, UserArt } from '@/features/art/types';
import type { BitmapOptions, BitmapResult, ProcessedSummary } from '@/features/trace/types';
import type { ExportRequest, ExportSummary, ImportDialogOptions, PdfOptions } from '@/features/fileio/types';
import type { NestRequest, NestStatus } from '@/features/nest/types';

export type Unlisten = () => void;

/** Copia de recuperación del autoguardado (ver `src-tauri/src/autosave.rs`). */
export interface RecoveryInfo {
  id: string;
  /** Nombre para mostrar: el del proyecto o «Sin título». */
  name: string;
  /** Proyecto del que viene la copia; null si nunca se guardó. */
  path: string | null;
  /** Momento de la copia, en milisegundos desde 1970. */
  saved_at_ms: number;
  size_bytes: number;
}

/** Plantilla de proyecto guardada en la carpeta de configuración (ver `src-tauri/src/templates.rs`). */
export interface TemplateInfo {
  name: string;
  /** Última modificación, en milisegundos desde 1970. */
  saved_at_ms: number;
  size_bytes: number;
}

/** Sobre de `preferences.json` y de los paquetes `.tlakuprefs` (ver `src-tauri/src/prefs.rs`). */
export interface PrefsFile {
  version: 1;
  prefs: Record<string, unknown>;
  /** Atajos del usuario: id de comando -> atajos (ver `commands/shortcuts.ts`). */
  shortcuts: Record<string, string[]>;
}

export interface PrefsBundle extends PrefsFile {
  format: 'tlaku-prefs';
}

/** Familia de firmware que reconoce `probe_port`. */
export type ProbeFamily = 'grbl' | 'grbl_hal' | 'fluid_nc' | 'grbl_lpc' | 'marlin' | 'smoothie' | 'unknown';

/** Resultado de `probe_port`: lo que dijo la placa al abrir el puerto y a `$I`, `$$` o `M115`. */
export interface ProbeResult {
  family: ProbeFamily;
  /** Nombre y versión legibles; vacío si no se supo. */
  firmware: string;
  /** Todo lo que la placa dijo al abrir el puerto, una línea por renglón. */
  banner: string;
  /** Respuesta de `$I` o `M115`, sin el `ok` final. */
  info: string[];
  /** Ajustes de `$$` con su `$` (`"$30"` -> `"1000"`); null fuera de la familia GRBL o si no contestó. */
  settings: Record<string, string> | null;
  suggested_kind: DeviceKind;
}

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
  /**
   * Detecta la controladora de un puerto sin moverla ni configurarla: escucha el saludo y, como
   * máximo, envía `?`, `0x18` (solo si no hubo saludo), `$I`, `$$` y `M115`. Los puertos
   * `virtual:<modelo>` los atiende el emulador. Falla con un mensaje en español si no abre.
   */
  probePort(args: { port: string; baud: number }): Promise<ProbeResult>;
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
  /**
   * Encuadre de banda elástica: como `frameJob` pero siguiendo el casco convexo del trabajo en
   * lugar del rectángulo. Láser apagado; mismas comprobaciones de estado.
   */
  frameHull(args: JobArgs & { speed: number }): Promise<void>;
  /**
   * Mueve la cabeza a (`x`, `y`) en coordenadas de TRABAJO de la máquina (las de `work_pos`, ya
   * convertidas desde el documento con el origen del perfil), en mm y a `speed` mm/min, con el
   * láser apagado (`$J=G90 G21 X.. Y.. F..`). Falla con un trabajo en curso o en alarma.
   */
  jogTo(args: { x: number; y: number; speed: number }): Promise<void>;
  /**
   * Lleva el override en vivo de velocidad (`feed`) o potencia (`spindle`) a `percent` por ciento
   * (10 a 200) con los bytes en tiempo real de GRBL. Vale durante un trabajo. Falla en Marlin y
   * Smoothieware. El valor vigente llega en `StatusReport.overrides`.
   */
  setOverride(args: { kind: OverrideKind; percent: number }): Promise<void>;
  /** Provoca una falla en la máquina virtual conectada (llama, alarma 3, desconexión...). Falla con una máquina real. */
  simulateFault(args: { fault: SimulatedFault }): Promise<void>;

  // Edición (fase 3)
  listFonts(): Promise<FontFamily[]>;
  /**
   * Contorno del texto. `path` es la trayectoria guía si va sobre una curva, en coordenadas LOCALES
   * del texto (con la inversa del `transform` del texto aplicada), no de documento.
   */
  textOutline(args: { text: TextObject; path: BezPath | null }): Promise<TextOutlineResult>;
  /** Importa fuentes SHX de AutoCAD; devuelve los nombres con que quedan disponibles en `listFonts`. */
  importShxFonts(args: { paths: string[] }): Promise<string[]>;
  /**
   * Lee el CSV del texto variable (UTF-8 con o sin BOM; separador `,`, `;` o tabulador). Con
   * `required` falla si falta alguna de esas columnas (por nombre o número desde 1).
   */
  readCsv(args: { path: string; required?: string[] }): Promise<CsvTable>;
  /** QR o código de barras como formas cerradas (rellenables). */
  generateCode(args: { spec: CodeSpec }): Promise<CodeResult>;
  /** Booleanas, soldar y offset; el resultado viene en coordenadas de documento con transform identidad. */
  geometryOp(args: GeometryOpArgs): Promise<DesignObject[]>;
  /** Recortar, separar, optimizar, redondear esquinas, cortar formas y redimensionar ranuras (ola 3). */
  shapeOp(args: ShapeOpArgs): Promise<ShapeOpResult>;
  /** Deforma los objetos con una envolvente de cuatro bordes Bézier; cada uno vuelve con su id, vectorial. */
  warpObjects(args: WarpArgs): Promise<DesignObject[]>;
  /** Una forma cerrada con el casco convexo (o cóncavo, con `max_edge`) de los objetos. */
  hullOutline(args: HullArgs): Promise<DesignObject[]>;
  /** Círculos tangentes a 2 o 3 operandos, de menor a mayor radio; vacío si no hay solución. */
  tangentCircles(args: TangentArgs): Promise<TangentCircle[]>;
  traceImage(args: { path: string; settings: TraceSettings; size_mm: [number, number] }): Promise<TraceResult>;
  /**
   * Convierte las formas de `selection` (vectores y texto) en un PNG negro sobre blanco a `options.dpi`,
   * escrito en `dir` (o en la carpeta temporal si es `null`) con el nombre `<stem>-bitmap-<n>.png`.
   * Devuelve dónde queda la imagen en el documento.
   */
  vectorsToBitmap(args: { doc: Document; selection: string[]; dir: string | null; stem: string; options: BitmapOptions }): Promise<BitmapResult>;
  /** Guarda como PNG lo que se grabaría de una imagen con los ajustes de imagen de su capa (y su máscara). */
  saveProcessedImage(args: { image: ImageObject; layer: LayerSettings; path: string }): Promise<ProcessedSummary>;
  /** Importa el texto de un SVG (curvas, mm); con `target_layer` (`#rrggbb`) todo cae en esa capa. */
  artImportSvg(args: { svg: string; target_layer: string | null }): Promise<ImportResult>;
  /** SVG propios del usuario, en `<configuración>/art/`. */
  artListUser(): Promise<UserArt[]>;
  /** Copia SVG a la biblioteca del usuario; los que no se leen como dibujo se devuelven en `errors`. */
  artAddUser(args: { paths: string[] }): Promise<AddArtResult>;
  artRemoveUser(args: { name: string }): Promise<void>;

  // Nesting: acomodar piezas en la cama o en un material irregular
  /**
   * Arranca el cálculo en segundo plano y devuelve el `job_id`. Rechaza con un mensaje en español si
   * la petición no es válida. Un `nestStart` nuevo cancela el anterior.
   */
  nestStart(args: { request: NestRequest }): Promise<string>;
  /** Mejor solución hasta ahora; se sondea cada ~250 ms mientras `running`. */
  nestPoll(args: { job_id: string }): Promise<NestStatus>;
  /** Pide parar; el siguiente `nestPoll` devuelve `running: false` con la mejor solución. */
  nestCancel(args: { job_id: string }): Promise<void>;

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

  // Perfiles de dispositivo (fase 4)
  /** Perfil de partida de un tipo de controladora (dialecto, `s_max`, modo de potencia y baudios): `DeviceProfile::for_kind`. */
  deviceDefaults(args: { kind: DeviceProfile['kind'] }): Promise<DeviceProfile>;
  /**
   * Prueba de una vuelta del rotativo: un documento con una línea de exactamente una circunferencia
   * de la pieza. `speed` en mm/min y `power` en %. Falla si el rotativo del perfil está apagado.
   */
  rotaryTest(args: { device: DeviceProfile; speed: number; power: number }): Promise<Document>;

  // Aplicación: preferencias, autoguardado
  /** Preferencias guardadas (`preferences.json`); null si todavía no hay. */
  loadPreferences(): Promise<PrefsFile | null>;
  savePreferences(args: { data: PrefsFile }): Promise<void>;
  /** Escribe un paquete `.tlakuprefs`. */
  exportPreferences(args: { path: string; bundle: PrefsBundle }): Promise<void>;
  /** Lee un paquete `.tlakuprefs`; la interfaz sanea y aplica lo que trae. */
  importPreferences(args: { path: string }): Promise<PrefsBundle>;
  /** Para cada ruta, si el archivo existe. */
  pathsExist(args: { paths: string[] }): Promise<boolean[]>;
  /** Guarda la copia de recuperación `id` (atómico; recorta las más viejas). */
  autosaveWrite(args: { id: string; name: string; path: string | null; doc: Document }): Promise<RecoveryInfo>;
  /** Copias pendientes de recuperar, de la más nueva a la más vieja (las superadas por un guardado se borran). */
  autosaveList(): Promise<RecoveryInfo[]>;
  autosaveRead(args: { id: string }): Promise<Document>;
  autosaveDiscard(args: { id: string }): Promise<void>;

  // Plantillas de proyecto (ola 3)
  /** Plantillas guardadas, por nombre. */
  listTemplates(): Promise<TemplateInfo[]>;
  /** Guarda (o reemplaza) la plantilla `name` con el diseño, las capas y los ajustes de `doc`. */
  saveTemplate(args: { name: string; doc: Document }): Promise<TemplateInfo>;
  loadTemplate(args: { name: string }): Promise<Document>;
  deleteTemplate(args: { name: string }): Promise<void>;
  /** Abre otra ventana de la app con un documento independiente (comparte la máquina y las preferencias). Devuelve su etiqueta. */
  newWindow(): Promise<string>;

  // Formatos de archivo (ola 1)
  /**
   * Importa con las opciones del diálogo de importación (escala, DPI de SVG e imágenes, unidad de
   * un DXF sin unidades, página del PDF, agrupar, capa destino y G-code por potencia).
   */
  importFileWith(args: { path: string; options: Partial<ImportDialogOptions> }): Promise<ImportResult>;
  /**
   * Exporta el documento (o solo `selection`, ids de objeto; `null` = todo) a SVG, DXF, PNG o PDF en
   * `path`. Las opciones que falten en `request` toman su valor por defecto.
   */
  exportDesign(args: { doc: Document; selection: string[] | null; path: string; request: ExportRequest }): Promise<ExportSummary>;
  /** Genera el PDF a escala 1:1 y lo abre en el visor del sistema para imprimirlo. */
  printDesign(args: { doc: Document; selection: string[] | null; options: Partial<PdfOptions> }): Promise<ExportSummary>;

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
  probePort: 'probe_port',
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
  frameHull: 'frame_hull',
  jogTo: 'jog_to',
  setOverride: 'set_override',
  simulateFault: 'simulate_fault',
  listFonts: 'list_fonts',
  textOutline: 'text_outline',
  importShxFonts: 'import_shx_fonts',
  readCsv: 'read_csv',
  generateCode: 'generate_code',
  geometryOp: 'geometry_op',
  shapeOp: 'shape_op',
  warpObjects: 'warp_objects',
  hullOutline: 'hull_outline',
  tangentCircles: 'tangent_circles',
  traceImage: 'trace_image',
  vectorsToBitmap: 'vectors_to_bitmap',
  saveProcessedImage: 'save_processed_image',
  artImportSvg: 'art_import_svg',
  artListUser: 'art_list_user',
  artAddUser: 'art_add_user',
  artRemoveUser: 'art_remove_user',
  nestStart: 'nest_start',
  nestPoll: 'nest_poll',
  nestCancel: 'nest_cancel',
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
  deviceDefaults: 'device_defaults',
  rotaryTest: 'rotary_test',
  importFileWith: 'import_file_with',
  exportDesign: 'export_design_file',
  printDesign: 'print_design',
  loadPreferences: 'load_preferences',
  savePreferences: 'save_preferences',
  exportPreferences: 'export_preferences',
  importPreferences: 'import_preferences',
  pathsExist: 'paths_exist',
  autosaveWrite: 'autosave_write',
  autosaveList: 'autosave_list',
  autosaveRead: 'autosave_read',
  autosaveDiscard: 'autosave_discard',
  listTemplates: 'list_templates',
  saveTemplate: 'save_template',
  loadTemplate: 'load_template',
  deleteTemplate: 'delete_template',
  newWindow: 'new_window',
} as const satisfies Record<Exclude<keyof LaserApi, 'on'>, string>;

export const EVENT_NAMES: readonly EventName[] = [
  'machine://status',
  'machine://progress',
  'machine://log',
  'machine://job',
  'machine://connection',
];
