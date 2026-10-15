import type { DeviceKind } from '@/bindings/DeviceKind';
import type { GenerateResult } from '@/bindings/GenerateResult';
import type { GeneratorInfo } from '@/bindings/GeneratorInfo';
import type {
  ConnectResult,
  DeviceProfile,
  LbdevImport,
  Document,
  EventMap,
  EventName,
  ExportResult,
  GalvoPreview,
  GalvoFrameKind,
  GalvoSafetyAck,
  GalvoState,
  GalvoVerifyReport,
  ImportResult,
  JobArgs,
  JobPreview,
  LhyPreview,
  LhyState,
  OverrideKind,
  ProbeTouch,
  RealtimeCommand,
  RuidaApplyReport,
  RuidaBackup,
  RuidaCalibration,
  RuidaControllerSettings,
  RuidaDocResult,
  RuidaFile,
  RuidaInfo,
  RuidaSettingChange,
  RuidaSettingsPlan,
  SimulatedFault,
  InterruptedJob,
  ResumeReadiness,
  ResumeStart,
  ResumeView,
} from './types';
import type {
  BezPath,
  BedImage,
  CalibrationReport,
  CircleFit,
  CircleHint,
  ComposeBedArgs,
  LensModelChoice,
  CameraPreset,
  CameraSetup,
  HeadCamera,
  NetworkFrame,
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
  GeneratePartArgs,
  HullArgs,
  TangentArgs,
  TangentCircle,
  WarpArgs,
  CatalogState,
  MaterialLibrary,
  MaterialTestParams,
  TextObject,
  TextOutlineResult,
  FontCapabilities,
  CsvTable,
  ImageProbe,
  TraceResult,
  TraceSettings,
} from './types';

import type { AddArtResult, UserArt } from '@/features/art/types';
import type { BitmapOptions, BitmapResult, ProcessedSummary } from '@/features/trace/types';
import type { ImagePreview, ImagePreviewArgs } from '@/features/images/types';
import type { TonalPreview, TonalRequest } from '@/features/tonal/types';
import type { FitChoice, FitResult, MarkPair } from '@/features/printcut/types';
import type { PlanArgs, PlanResult } from '@/features/tiling/types';
import type { ExportRequest, ExportSummary, ImportDialogOptions, PdfOptions } from '@/features/fileio/types';
import type { RemnantLibrary } from '@/features/nest/remnants';
import type { NestRequest, NestStatus } from '@/features/nest/types';
import type { Finding, ReviewArgs } from '@/features/review/types';
import type { ProjectSummary, SnapshotKind, VersionInfo, VersionListing, VersionsConfig } from '@/features/versions/types';

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

/**
 * Paquete de usuario `.tlakubundle` (ver `src-tauri/src/prefs.rs` y `features/bundle`): máquinas,
 * biblioteca de materiales, preferencias y atajos en un solo archivo. Cada sección es opcional al
 * importar: un paquete puede ser parcial. `devices` se lee sin tipar a fondo (cada perfil se
 * normaliza y valida en la interfaz).
 */
export interface UserBundle {
  format: 'tlaku-bundle';
  /** 1: máquinas, materiales, cámaras, arte y preferencias. 2: suma costos, retazos, plantillas e historial. */
  version: 1 | 2;
  /** Fecha de creación, ISO 8601. */
  created: string;
  /** Versión de la aplicación que lo creó. */
  app: string;
  devices?: DeviceProfile[];
  active_device?: string | null;
  materials?: MaterialLibrary | null;
  prefs?: PrefsBundle | null;
  /** Calibración, alineación, cámara del cabezal y configuraciones (sin credenciales ni ids de dispositivo). */
  cameras?: CamerasBundle | null;
  /** Biblioteca de arte propia (los SVG con su nombre). */
  art?: ArtBundle | null;
  /** Versión 2: perfiles de costos (por máquina, venta y perfiles de material). */
  costs?: CostsBundle | null;
  /** Versión 2: inventario de retazos. */
  remnants?: RemnantLibrary | null;
  /** Versión 2: plantillas de proyecto (nombre y documento). */
  templates?: TemplatesBundle | null;
  /** Versión 2: historial de trabajos (opcional al respaldar). */
  history?: HistoryBundle | null;
  /** Lo agrega el motor al respaldar a una carpeta: qué instalación lo hizo. */
  install_id?: string;
}

/** Sección `costs` de un paquete: ver `features/bundle/extras.ts`. */
export interface CostsBundle {
  machines: Record<string, unknown>;
  sales: unknown;
  materials: unknown[];
}

/** Sección `templates` de un paquete. */
export interface TemplatesBundle {
  items: { name: string; doc: Document }[];
}

/** Sección `history` de un paquete: las entradas del registro de trabajos. */
export interface HistoryBundle {
  entries: Record<string, unknown>[];
}

/** Respaldo a carpeta (ver `src-tauri/src/backup/mod.rs` y `features/backup`). */
export interface BackupSettings {
  /** Carpeta de destino; `null` = todavía no se eligió. */
  folder: string | null;
  /** Respaldar al cerrar la aplicación. */
  on_close: boolean;
  /** Respaldar al abrir si pasaron estos días desde el último (0 = no). */
  every_days: number;
  /** Cuántos respaldos conservar en la carpeta. */
  keep: number;
  /** Incluir el historial de trabajos. */
  include_history: boolean;
}

export interface BackupConfig extends BackupSettings {
  version: number;
  /** Marca (ms desde 1970) del último respaldo hecho aquí. */
  last_backup_ms: number;
  /** Marca del respaldo más nuevo ya importado o descartado. */
  last_seen_ms: number;
  install_id: string;
}

export interface BackupEntry {
  name: string;
  path: string;
  size_bytes: number;
  /** Ms desde 1970, tomados del nombre del archivo. */
  created_ms: number;
}

export interface BackupWrite {
  name: string;
  path: string;
  created_ms: number;
  /** Respaldos viejos que borró la rotación. */
  removed: string[];
}

export interface BackupList {
  configured: boolean;
  /** `false` si la carpeta no existe o no se puede leer (unidad desconectada). */
  available: boolean;
  entries: BackupEntry[];
}

/** Sección `cameras` de un paquete: ver `features/bundle/cameras.ts`. */
export interface CamerasBundle {
  calibration?: CameraSetup['calibration'];
  alignment?: CameraSetup['alignment'];
  head?: HeadCamera | null;
  presets?: CameraPreset[];
}

/** Sección `art` de un paquete: los SVG de la biblioteca del usuario. */
export interface ArtBundle {
  items: UserArt[];
}

/** Automatización por UDP (Preferencias): ver `docs/app-api/automatizacion.md` y `src-tauri/src/automation.rs`. */
export interface AutomationConfig {
  enabled: boolean;
  /** Puerto UDP en 127.0.0.1 (1024 a 65535). */
  port: number;
  /** Permite que `START` inicie sin confirmar en la interfaz. */
  allow_start: boolean;
}

/** Estado real del servidor de automatización tras aplicar la configuración. */
export interface AutomationStatus {
  running: boolean;
  port: number;
  /** Por qué no arrancó (puerto ocupado, etc.); `null` si todo bien o está apagada. */
  error: string | null;
}

/** Control remoto desde el celular (Preferencias): ver `docs/app-api/control-remoto.md` y `src-tauri/src/remote/`. */
export interface RemoteConfig {
  enabled: boolean;
  /** Puerto TCP (1024 a 65535). */
  port: number;
  /** IP de la red local donde escuchar; `null` = la recomendada. */
  ip: string | null;
}

/** Una dirección de la red local de este equipo. */
export interface RemoteAddr {
  ip: string;
  /** Interfaz (`en0`, `Wi-Fi`). */
  name: string;
  recommended: boolean;
}

/** Un celular (u otro navegador) conectado al control remoto. */
export interface RemoteClient {
  id: string;
  ip: string;
  /** Aparato y navegador aproximados (`iPhone · Safari`). */
  agent: string;
  /** Cuándo entró, ms desde 1970. */
  since_ms: number;
  /** Segundos desde su última actividad. */
  idle_s: number;
  connected: boolean;
}

/** Estado real del control remoto tras aplicar la configuración. */
export interface RemoteStatus {
  running: boolean;
  port: number;
  ip: string | null;
  /** Dirección sin la clave. */
  url: string | null;
  /** Dirección con la clave (`http://ip:puerto/#clave`): la del QR. */
  full_url: string | null;
  /** El QR de `full_url` como SVG. */
  qr_svg: string | null;
  error: string | null;
  addrs: RemoteAddr[];
  clients: RemoteClient[];
}

/** Capa en curso, para el celular. */
export interface RemoteLayer {
  index: number;
  count: number;
  color: string;
  mode: string;
}

/** Estado que la ventana informa al servidor remoto (sale de los eventos `machine://*`). */
export interface RemoteSnapshot {
  doc_name: string;
  device_name: string;
  bed: [number, number] | null;
  job_state: 'idle' | 'running' | 'paused' | 'finished' | 'cancelled' | 'error';
  job_message: string;
  /** 0 a 1. */
  progress: number;
  elapsed_s: number;
  remaining_s: number | null;
  total_s: number | null;
  layer: RemoteLayer | null;
  /** Cabezal en mm del documento. */
  head: [number, number] | null;
  has_output: boolean;
  /** El trabajo se encuadró en la PC después del último cambio. */
  framed: boolean;
  /** Dónde está la máquina sobre la ruta de la vista en vivo (solo si la PC la sigue). */
  live?: RemoteLiveReport | null;
  /** La cola guiada de trabajos (solo lectura en el celular), si hay una. */
  queue?: RemoteQueue | null;
}

/** Un trabajo de la cola guiada, como lo ve el celular. */
export interface RemoteQueueItem {
  name: string;
  /** Copias pedidas y terminadas. */
  copies: number;
  done: number;
  status: 'pending' | 'done' | 'skipped' | 'failed';
}

/**
 * La cola guiada (`features/queue`). Nunca avanza sola: en `awaiting` espera la confirmación de una persona,
 * en la PC o desde el celular con las mismas condiciones que «iniciar» (checklist en el teléfono + encuadre en la PC).
 */
export interface RemoteQueue {
  state: 'idle' | 'awaiting' | 'running' | 'paused' | 'stopped' | 'done';
  /** Trabajos (copias) terminados y total. */
  done: number;
  total: number;
  /** Segundos estimados que faltan; `null` si no se conocen todos. */
  eta_s: number | null;
  current: RemoteQueueItem | null;
  /** Copia en curso o por confirmar del trabajo actual (desde 1). */
  copy: number;
  /** Por qué se detuvo, si está detenida. */
  message: string;
  items: RemoteQueueItem[];
}

/** Ubicación de la máquina en la ruta de la vista en vivo (la página interpola entre informes). */
export interface RemoteLiveReport {
  /** Tiempo de trabajo (s) en que se ubicó la posición real. */
  t: number;
  /** La máquina está ejecutando (no en pausa). */
  running: boolean;
  /** Sube cada vez que la máquina retrocedió de verdad. */
  retreat_seq: number;
  src: 'position' | 'progress' | 'clock';
}

/** Un trazo continuo con láser de la ruta del celular. */
export interface RemoteLiveRun {
  /** 0 = corte, 1 = grabado. */
  k: 0 | 1;
  /** Capa (índice en `RemoteLiveRoute.layers`). */
  l: number;
  /** Ancho con que se dibuja (mm). */
  w: number;
  /** `[x, y, t, ...]`: mm del documento (Y arriba) y tiempo de trabajo (s) en cada vértice. */
  p: number[];
}

/** Ruta simplificada de la vista en vivo del celular: se manda una vez por trabajo. */
export interface RemoteLiveRoute {
  bed: [number, number];
  /** Duración de la ruta (s): el 100 %. */
  total: number;
  layers: string[];
  runs: RemoteLiveRun[];
  /** Hubo que simplificar más de lo normal para caber en el tope. */
  decimated: boolean;
}

/** Recorrido del trabajo para dibujarlo en el celular. */
export interface RemoteRoute {
  bed: [number, number];
  /** `M x y L x y ...` en mm del documento (Y arriba). */
  d: string;
}

/** Respuesta de la ventana a un pedido de inicio del celular. */
export interface RemoteStartResult {
  id: number;
  ok: boolean;
  code?: string;
  message?: string;
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
 * Contrato único entre la interfaz y el motor (docs/app-api/README.md).
 * Una implementación real (Tauri invoke/listen) y una simulada (mock en memoria).
 * Los componentes y los slices usan siempre esta interfaz, nunca `invoke` directamente.
 */
export interface LaserApi {
  /** Comando de prueba de src-tauri. */
  appVersion(): Promise<string>;
  /** Idioma de los avisos y errores del motor; se llama al arrancar y al cambiar la preferencia `language`. */
  setEngineLocale(args: { locale: 'es' | 'en' }): Promise<void>;

  // Archivos y proyecto
  importFile(args: { path: string }): Promise<ImportResult>;
  saveProject(args: { path: string; doc: Document }): Promise<void>;
  loadProject(args: { path: string }): Promise<Document>;

  // Trabajo
  computeJob(args: JobArgs): Promise<JobPreview>;
  exportGcode(args: JobArgs & { path: string }): Promise<ExportResult>;
  /**
   * Aviso (no bloquea) de que el enlace serie del perfil (baudios) no da abasto para el trabajo en su
   * tramo más denso. `null` si alcanza o no aplica (red, Ruida, K40, galvo). Lo muestra el diálogo de inicio.
   */
  jobLinkWarning(args: JobArgs): Promise<string | null>;

  // Dispositivos
  listDevices(): Promise<DeviceProfile[]>;
  saveDevice(args: { device: DeviceProfile }): Promise<void>;
  deleteDevice(args: { id: string }): Promise<void>;
  /** Ordena la lista de perfiles como `ids` (los que no se nombran quedan al final). */
  reorderDevices(args: { ids: string[] }): Promise<void>;

  // Máquina
  listPorts(): Promise<string[]>;
  /**
   * Detecta la controladora de un puerto sin moverla ni configurarla: escucha el saludo y, como
   * máximo, envía `?`, `0x18` (solo si no hubo saludo), `$I`, `$$` y `M115`. Los puertos
   * `virtual:<modelo>` los atiende el emulador. Falla con un mensaje en español si no abre.
   */
  probePort(args: { port: string; baud: number }): Promise<ProbeResult>;
  /** Enciende, apaga o reconfigura el servidor UDP de automatización (apagado por omisión, solo 127.0.0.1). */
  automationConfigure(args: { config: AutomationConfig }): Promise<AutomationStatus>;
  /** Enciende, apaga o reconfigura el control remoto desde el celular (apagado de fábrica, solo red local). */
  remoteConfigure(args: { config: RemoteConfig }): Promise<RemoteStatus>;
  remoteStatus(): Promise<RemoteStatus>;
  /** Clave nueva: la anterior deja de valer y los clientes conectados quedan expulsados. */
  remoteRegenerate(): Promise<RemoteStatus>;
  /** Desconecta a un cliente. */
  remoteKick(args: { id: string }): Promise<RemoteStatus>;
  /** La ventana informa el estado del trabajo al servidor remoto. */
  remoteUpdate(args: { snapshot: RemoteSnapshot }): Promise<void>;
  remoteSetRoute(args: { route: RemoteRoute | null }): Promise<void>;
  /** Ruta de la vista en vivo del celular (solo lectura); `null` la borra. */
  remoteSetLive(args: { route: RemoteLiveRoute | null }): Promise<void>;
  /** Un cuadro de la cámara (JPEG en base64; vacío = apagada). Devuelve si algún celular lo espera. */
  remoteCameraFrame(args: { jpeg_b64: string }): Promise<boolean>;
  remoteStartResult(args: { result: RemoteStartResult }): Promise<void>;
  /**
   * Conecta. `device` es el perfil activo: con `kind = ruida` el motor usa su IP (`connection`),
   * su magic y su cama en lugar del puerto serie (`port` solo se usa para `virtual:ruida`). Con
   * `kind = lhy_micro` abre la placa USB `usb:k40:<n>` o la máquina virtual `virtual:k40`.
   * `kind = galvo` abre la tarjeta USB `usb:galvo:<n>` (EXPERIMENTAL; exige `galvoAck` con las tres
   * confirmaciones de seguridad, en CADA conexión) o la tarjeta simulada `virtual:galvo`.
   */
  connect(args: { port: string; baud: number; device?: DeviceProfile; galvoAck?: GalvoSafetyAck }): Promise<ConnectResult>;
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
  startJob(args: JobArgs & { document?: string | null }): Promise<void>;
  pauseJob(): Promise<void>;
  resumeJob(): Promise<void>;
  /** Con trabajo: feed hold y soft reset. Sin trabajo: cancela el jog en curso (0x85). */
  cancelJob(): Promise<void>;
  /** Encuadre con el láser apagado; mismas comprobaciones de estado que `startJob`. */
  frameJob(args: JobArgs & { speed: number; power_pct?: number | null }): Promise<void>;
  /**
   * Encuadre de banda elástica: como `frameJob` pero siguiendo el casco convexo del trabajo en
   * lugar del rectángulo. Láser apagado; mismas comprobaciones de estado.
   */
  frameHull(args: JobArgs & { speed: number; power_pct?: number | null }): Promise<void>;
  /**
   * Mueve la cabeza a (`x`, `y`) en coordenadas de TRABAJO de la máquina (las de `work_pos`, ya
   * convertidas desde el documento con el origen del perfil), en mm y a `speed` mm/min, con el
   * láser apagado (`$J=G90 G21 X.. Y.. F..`). Falla con un trabajo en curso o en alarma.
   */
  jogTo(args: { x: number; y: number; speed: number }): Promise<void>;
  /**
   * Un toque de sonda con el cabezal (M48, `G38.3`; solo GRBL en reposo): avanza como máximo
   * `(dx, dy)` mm de TRABAJO a `feed` mm/min con el láser apagado, se detiene al tocar y retrocede
   * `retract` mm. Límites del motor: 100 mm de recorrido, 500 mm/min y 20 mm de retroceso.
   * `confirmed` debe ser `true` (la interfaz lo manda tras la confirmación del usuario).
   */
  probeTouch(args: { dx: number; dy: number; feed: number; retract: number; confirmed: boolean }): Promise<ProbeTouch>;
  /** Ruida (EXPERIMENTAL, solo lectura): modelo, tarjeta, cama, velocidades y contadores de la controladora. */
  ruidaInfo(): Promise<RuidaInfo>;
  /** Ruida (EXPERIMENTAL, solo lectura): documentos guardados en la memoria de la controladora. */
  ruidaFiles(): Promise<RuidaFile[]>;
  /**
   * Ruida (M23): nombre con que se guardará el próximo trabajo en la memoria (1 a 8 letras, cifras,
   * `_` o `-`; `null` = automático) y si puede reemplazar a un documento del mismo nombre (el usuario
   * lo confirmó; vale para un solo envío). Sin `overwrite`, un nombre repetido hace fallar `startJob`
   * sin tocar nada.
   */
  ruidaPrepareStore(args: { name: string | null; overwrite: boolean; experimental_writes: boolean }): Promise<void>;
  /**
   * Ruida (M24, EXPERIMENTAL: sin probar con una controladora real): borra el documento `index` si
   * se llama `name` (el motor lo comprueba). `confirmed` debe ser `true`. Nunca borra «todos».
   */
  ruidaFileDelete(args: { index: number; name: string; confirmed: boolean; experimental_writes: boolean }): Promise<RuidaDocResult>;
  /**
   * Ruida (M24, EXPERIMENTAL): ejecuta el documento `index` si se llama `name`. `confirmed` debe ser
   * `true`; la interfaz lo manda tras la lista de comprobación de seguridad de Iniciar.
   */
  ruidaFileRun(args: { index: number; name: string; confirmed: boolean; experimental_writes: boolean }): Promise<RuidaDocResult>;
  /**
   * Las escrituras de Ruida (`ruidaSettingsApply`, `ruidaSettingsRestore`, `ruidaCalibrate`, `ruidaFileDelete`,
   * `ruidaFileRun` y `ruidaPrepareStore` con `overwrite`) llevan `experimental_writes`: el valor de la
   * preferencia `ruidaExperimentalWrites` (apagada de fábrica). Con `false` el motor las rechaza sin
   * tocar la máquina.
   */
  /** Ruida (M25 y M28, EXPERIMENTAL): lee los ajustes que Tlaku puede escribir (lista cerrada). */
  ruidaSettingsRead(): Promise<RuidaControllerSettings>;
  /** Ruida: diferencias (actual y nuevo) de unos cambios, validadas por el motor, sin escribir. */
  ruidaSettingsPreview(args: { changes: RuidaSettingChange[] }): Promise<RuidaSettingsPlan>;
  /** Ruida: diferencias de un respaldo `.tlakuruida` (su texto) con la máquina, sin escribir. */
  ruidaSettingsPreviewFile(args: { json: string }): Promise<RuidaSettingsPlan>;
  /**
   * Ruida: escribe ajustes. El motor guarda antes un respaldo con fecha en la carpeta de
   * configuración, exige `confirmed = true` (casilla «Entiendo que esto cambia la configuración de
   * mi controladora»), escribe de a uno y vuelve a leer para comparar.
   */
  ruidaSettingsApply(args: { changes: RuidaSettingChange[]; confirmed: boolean; experimental_writes: boolean }): Promise<RuidaApplyReport>;
  /** Ruida: restaura un respaldo `.tlakuruida` (su texto) con las mismas protecciones. */
  ruidaSettingsRestore(args: { json: string; confirmed: boolean; experimental_writes: boolean }): Promise<RuidaApplyReport>;
  /** Ruida: texto `.tlakuruida` con los ajustes actuales de la controladora (respaldo manual). */
  ruidaSettingsExport(): Promise<string>;
  /** Ruida: paso nuevo de un eje a partir de lo que se pidió mover y lo que se midió (no escribe). */
  ruidaCalibrate(args: { key: string; commanded_mm: number; measured_mm: number; experimental_writes: boolean }): Promise<RuidaCalibration>;
  /** Ruida: respaldos automáticos de los ajustes, del más reciente al más antiguo. */
  ruidaBackups(): Promise<RuidaBackup[]>;
  /** Ruida: texto de un respaldo automático por el nombre de su archivo. */
  ruidaBackupText(args: { name: string }): Promise<string>;
  /**
   * K40 con M2 Nano (EXPERIMENTAL): calcula y codifica el trabajo SIN enviarlo y devuelve los avisos
   * del codificador (la potencia se ignora, los grises se binarizan...) y el resumen. No necesita conexión.
   */
  lhyPreview(args: JobArgs): Promise<LhyPreview>;
  /** K40: lleva el cabezal al home (finales de carrera); desde ahí se cuenta la posición. Vuelve al terminar. */
  lhyHome(): Promise<void>;
  /** K40: placa, si se conoce la posición y último código de estado. Falla si la conexión no es un K40. */
  lhyState(): Promise<LhyState>;
  /**
   * Galvo (EXPERIMENTAL): calcula el trabajo SIN enviarlo y devuelve los pasos del marcado dividido,
   * repetido o rotativo, cuántas veces hay que mover el material, el tiempo y los avisos. No necesita conexión.
   */
  galvoPreview(args: JobArgs): Promise<GalvoPreview>;
  /** Galvo: estado de la tarjeta, entradas (solo lectura), puntero rojo y si la fuente puede estar encendida. Falla si la conexión no es galvo. */
  galvoState(): Promise<GalvoState>;
  /** Galvo: recorre el trabajo con el láser DESHABILITADO y el puntero rojo; la fuente nunca se arma. */
  galvoRedRun(args: JobArgs): Promise<void>;
  /** Galvo: enciende o apaga el puntero rojo en su sitio (la fuente queda deshabilitada). */
  galvoRedLight(args: { on: boolean }): Promise<void>;
  /** Galvo: ya moviste el material (o giraste el rotativo): el marcado dividido o repetido sigue. */
  galvoConfirmMove(): Promise<void>;
  /**
   * Galvo: encuadre con el puntero rojo de la forma pedida (rectángulo, trazos, casco convexo u objetos
   * sueltos), en bucle hasta detener o durante `loops` vueltas. `gap_mm`: holgura con la que dos trazos son
   * el mismo objeto (solo con `objects`). El láser nunca se habilita.
   */
  galvoFrame(args: JobArgs & { shape: GalvoFrameKind; loops?: number | null; gap_mm?: number | null }): Promise<void>;
  /** Galvo: mueve (`dx`, `dy` mm) o escala (`scale`, 1 = tamaño real) el encuadre en curso, sin detenerlo. */
  galvoAdjustFrame(args: { dx: number; dy: number; scale: number }): Promise<void>;
  /**
   * Galvo, verificación SIN LÁSER del asistente de alta: lee la tarjeta (solo lectura) y comprueba que el
   * perfil compila. Con una tarjeta USB real exige el reconocimiento de seguridad de ESTA llamada.
   */
  galvoVerify(args: { device: DeviceProfile; port: string; galvo_ack?: GalvoSafetyAck | null }): Promise<GalvoVerifyReport>;
  /** Mueve un eje extra (`Z`, `U`, `A`...) `delta` mm respecto a donde está. `kind` decide la sintaxis (GRBL o RepRap). */
  jogAxis(args: { kind: DeviceKind; axis: string; delta: number; speed: number }): Promise<void>;
  /** Lleva un eje extra a la posición `pos` (mm, coordenadas de trabajo); lo usa «Enfocar». */
  jogAxisTo(args: { kind: DeviceKind; axis: string; pos: number; speed: number }): Promise<void>;
  /**
   * Disparo de prueba: láser a `power_pct` % (máx. 20) de `s_max` durante `duration_ms` (10 a 2000) y
   * apagado siempre al terminar, aunque falle. Falla con un trabajo en curso. Devuelve los ms que
   * tardó la máquina en aceptar el `M3` (su `ok`): la pausa pedida corre después de ese `ok`, pero si
   * el firmware enciende el láser y retrasa el `ok` (la Falcon2 tarda ~1 s en el primer disparo) el
   * láser estuvo encendido ese tiempo además de la duración pedida.
   */
  fireTest(args: { kind: DeviceKind; power_pct: number; s_max: number; duration_ms: number }): Promise<number>;
  /**
   * Lleva el override en vivo de velocidad (`feed`) o potencia (`spindle`) a `percent` por ciento
   * (10 a 200) con los bytes en tiempo real de GRBL. Vale durante un trabajo. Falla en Marlin y
   * Smoothieware. El valor vigente llega en `StatusReport.overrides`.
   */
  setOverride(args: { kind: OverrideKind; percent: number }): Promise<void>;
  /** Provoca una falla en la máquina virtual conectada (llama, alarma 3, desconexión...). Falla con una máquina real. */
  simulateFault(args: { fault: SimulatedFault }): Promise<void>;

  // Historial de trabajos (archivo JSONL en la carpeta de datos de la app; ver `features/help/jobLog.ts`)
  /**
   * Historial guardado (`job-history.jsonl`), de la entrada más vieja a la más nueva. `max`: tope de entradas
   * (20 a 20 000, por omisión 1000); si el archivo lo pasa se rota. Cada entrada es un objeto con `id` y `at`;
   * el resto lo define y sanea la interfaz.
   */
  jobHistoryLoad(args: { max?: number | null }): Promise<Record<string, unknown>[]>;
  /** Agrega una entrada o, si ya hay una con ese `id`, la reemplaza; rota al tope `max`. */
  jobHistoryAppend(args: { entry: Record<string, unknown>; max?: number | null }): Promise<void>;
  /** Mezcla campos de primer nivel en la entrada `id` (un `null` borra el campo; `id` y `at` no cambian). Devuelve si existía. */
  jobHistoryUpdate(args: { id: string; patch: Record<string, unknown> }): Promise<boolean>;
  /** Quita entradas por `id`; devuelve cuántas quitó. */
  jobHistoryRemove(args: { ids: string[] }): Promise<number>;
  /** Borra todo el historial. */
  jobHistoryClear(): Promise<void>;
  /** Migra entradas de otro almacén (el `localStorage` anterior): agrega las válidas cuyo `id` aún no está. Devuelve cuántas entraron. */
  jobHistoryImport(args: { entries: Record<string, unknown>[]; max?: number | null }): Promise<number>;
  /** Escribe el CSV del historial en `path` (extensión `.csv`, hasta 32 MiB; atómico). */
  jobHistoryExport(args: { path: string; text: string }): Promise<void>;

  // Continuar un trabajo interrumpido (corte de luz, app cerrada, conexión perdida)
  /**
   * El trabajo interrumpido del que queda punto de control, o `null`. El motor lo guarda mientras envía
   * (G-code exacto, perfil, origen y el avance cada ~1 s) y lo borra al terminar bien, al cancelar a
   * propósito o al conectar otra máquina. Ruida, K40 y galvo solo dejan un aviso (`can_resume: false`).
   */
  interruptedJob(): Promise<InterruptedJob | null>;
  /**
   * El recorrido guardado y el punto de reinicio propuesto: la línea real (comparando la última MPos con
   * la geometría; `ok` solo dice que la línea entró al búfer) menos `back_lines` líneas o `back_mm` mm
   * (manda lo que quede más atrás), sin cruzar un `G0` y, en imágenes, al inicio de la fila de barrido.
   */
  interruptedJobView(args: { back_lines?: number | null; back_mm?: number | null }): Promise<ResumeView>;
  /** Descarta el punto de control. */
  discardInterruptedJob(): Promise<void>;
  /** Con la máquina conectada: si tiene homing (`$22`) y si ya se hizo desde que perdió la posición. */
  continueCheck(): Promise<ResumeReadiness>;
  /** Encuadra el trabajo completo (el rectángulo del G-code guardado) con el láser apagado. */
  continueFrame(args: { speed: number }): Promise<void>;
  /**
   * Continúa el trabajo desde `args.line`. El motor exige homing hecho (o la confirmación de que nada se
   * movió, sin homing), el encuadre y la lista de comprobación, y rechaza el resto sin tocar la máquina.
   */
  continueJob(args: { args: ResumeStart }): Promise<void>;

  // Edición (fase 3)
  listFonts(): Promise<FontFamily[]>;
  /**
   * Contorno del texto. `path` es la trayectoria guía si va sobre una curva, en coordenadas LOCALES
   * del texto (con la inversa del `transform` del texto aplicada), no de documento.
   */
  textOutline(args: { text: TextObject; path: BezPath | null }): Promise<TextOutlineResult>;
  /**
   * Funciones OpenType (con vista previa) y ejes variables de la fuente del texto (familia, negrita y
   * cursiva; los ejes ya puestos en `variations` se aplican a las vistas previas). Solo lista lo que
   * la fuente declara; las fuentes de una línea devuelven todo vacío.
   */
  fontCapabilities(args: { text: TextObject }): Promise<FontCapabilities>;
  /** Importa fuentes SHX de AutoCAD; devuelve los nombres con que quedan disponibles en `listFonts`. */
  importShxFonts(args: { paths: string[] }): Promise<string[]>;
  /**
   * Lee el CSV del texto variable (UTF-8 con o sin BOM; separador `,`, `;` o tabulador). Con
   * `required` falla si falta alguna de esas columnas (por nombre o número desde 1).
   */
  readCsv(args: { path: string; required?: string[] }): Promise<CsvTable>;
  /** Para cada ruta de una imagen variable (mismo orden): si existe y su tamaño en píxeles. Un archivo ausente no es un error: sale con `ok: false`. */
  probeImages(args: { paths: string[] }): Promise<ImageProbe[]>;
  /** QR o código de barras como formas cerradas (rellenables). */
  generateCode(args: { spec: CodeSpec }): Promise<CodeResult>;
  /** Booleanas, soldar y offset; el resultado viene en coordenadas de documento con transform identidad. */
  geometryOp(args: GeometryOpArgs): Promise<DesignObject[]>;
  /** «Revisar diseño»: analiza el documento y devuelve los hallazgos (no cambia nada); ver `edit/review.rs`. */
  reviewDesign(args: ReviewArgs): Promise<Finding[]>;
  /** Recortar, separar, optimizar, redondear esquinas, cortar formas y redimensionar ranuras (ola 3). */
  shapeOp(args: ShapeOpArgs): Promise<ShapeOpResult>;
  /** Deforma los objetos con una envolvente de cuatro bordes Bézier; cada uno vuelve con su id, vectorial. */
  warpObjects(args: WarpArgs): Promise<DesignObject[]>;
  /** Una forma cerrada con el casco convexo (o cóncavo, con `max_edge`) de los objetos. */
  hullOutline(args: HullArgs): Promise<DesignObject[]>;
  /** Generadores de piezas con su formulario y valores de fábrica (`edit/parts.rs`). */
  listGenerators(): Promise<GeneratorInfo[]>;
  /** Genera las piezas de un generador y las acomoda en filas; los parámetros inválidos vuelven en `error`, no como fallo. */
  generatePart(args: GeneratePartArgs): Promise<GenerateResult>;
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
  /** Vista previa de lo que se grabaría de una imagen con los ajustes de su capa, sin calcular el trabajo (C51). */
  previewImage(args: ImagePreviewArgs): Promise<ImagePreview>;
  /** Vista previa «por tonos» (mejora 8): imagen aproximada de cuánto se oscurece el material con el trabajo; la calcula el motor. */
  tonalPreview(args: JobArgs & { request: TonalRequest }): Promise<TonalPreview>;
  /** Print & Cut (M37): transformación del diseño a la cama a partir de las marcas medidas; el cálculo es del motor. */
  fitRegistration(args: { pairs: MarkPair[]; mode: FitChoice }): Promise<FitResult>;
  /** Tiling (M39): reparte la caja del diseño en tramos con solape y da las cruces de alineación; el cálculo es del motor. */
  planTiles(args: PlanArgs): Promise<PlanResult>;
  /** Importa el texto de un SVG (curvas, mm); con `target_layer` (`#rrggbb`) todo cae en esa capa. */
  artImportSvg(args: { svg: string; target_layer: string | null }): Promise<ImportResult>;
  /** SVG propios del usuario, en `<configuración>/art/`. */
  artListUser(): Promise<UserArt[]>;
  /** Copia SVG a la biblioteca del usuario; los que no se leen como dibujo se devuelven en `errors`. */
  artAddUser(args: { paths: string[] }): Promise<AddArtResult>;
  artRemoveUser(args: { name: string }): Promise<void>;
  /** Guarda dibujos que llegan con su texto (importar un paquete). Nunca pisa uno existente: usa otro nombre. */
  artSaveUser(args: { items: UserArt[] }): Promise<AddArtResult>;

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
  /** Inventario de retazos (vacío si no hay archivo). */
  loadRemnants(): Promise<RemnantLibrary>;
  /** Guarda el inventario entero (valida ids únicos, nombres, medidas y contornos). */
  saveRemnants(args: { library: RemnantLibrary }): Promise<void>;

  // Cámara (fase 3): las imágenes viajan como PNG en base64, sin prefijo data:.
  /** Esquinas internas del tablero en píxeles, o `null` si no se encontró. */
  detectBoard(args: { image_png_base64: string; board: [number, number] }): Promise<[number, number][] | null>;
  /**
   * Calibra la lente con vistas del tablero. `model` fija el modelo (`brown_conrady`, `fisheye`) o lo
   * deja en `auto` (por defecto): se ajustan los dos y se elige por error de reproyección y por
   * validación cruzada con vistas reservadas.
   */
  calibrateLens(args: { images_png_base64: string[]; board: [number, number]; square_mm: number; model?: LensModelChoice }): Promise<CalibrationReport>;
  /** Calibración, alineación y preferencias guardadas (`camera.json`). Sin archivo, una configuración vacía. */
  loadCameraSetup(): Promise<CameraSetup>;
  /** Valida, recalcula la homografía y guarda; devuelve lo guardado. */
  saveCameraSetup(args: { setup: CameraSetup }): Promise<CameraSetup>;
  /** Escribe el tablero de calibración (`.svg` o `.pdf`) en la ruta que eligió el usuario. */
  saveBoardFile(args: { path: string; content_base64: string }): Promise<void>;
  /**
   * Pide un cuadro a una cámara de red (snapshot HTTP o MJPEG) desde el motor: el navegador no deja
   * leerlo por CORS. La URL la pone el usuario y debe ser de la red local (`http://`); puede traer
   * `usuario:clave@` (autenticación Basic).
   */
  fetchCameraFrame(args: { url: string }): Promise<NetworkFrame>;
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

  // Cámara (ola 4): imagen de la cama, mosaico del cabezal y buscador de centro
  /**
   * Compone fotos ya corregidas de lente, cada una con su posición en la cama (`to_bed`: píxel ->
   * mm), en una imagen alineada con la cama; con `save` la escribe como PNG en `dir` (o en la
   * carpeta temporal) como `<stem>-camara-<n>.png`. Las fotos se mezclan en los bordes.
   */
  composeBedImage(args: ComposeBedArgs): Promise<BedImage>;
  /** Busca un círculo en una foto (px, origen en el borde); `hint` acota la búsqueda a un objeto. */
  detectCircle(args: { image_png_base64: string; hint?: CircleHint | null }): Promise<CircleFit>;

  // Materiales (fase 4)
  /** Biblioteca del usuario (`materials.json`). Sin archivo, una vacía. */
  loadMaterialLibrary(): Promise<MaterialLibrary>;
  /**
   * Biblioteca de fábrica de esta versión: valores de partida de solo lectura, cada uno con su
   * fuente y confianza. Instala o actualiza su copia aparte; nunca toca la del usuario.
   */
  loadFactoryMaterials(): Promise<MaterialLibrary>;
  saveMaterialLibrary(args: { library: MaterialLibrary }): Promise<void>;
  /** Lee un archivo `.tlakumat`; la interfaz decide cómo mezclarlo con la biblioteca actual. */
  importMaterialLibrary(args: { path: string }): Promise<MaterialLibrary>;
  exportMaterialLibrary(args: { path: string; library: MaterialLibrary }): Promise<void>;
  /** Documento con la cuadrícula de prueba (una capa por celda más la de rótulos). */
  materialTest(args: { params: MaterialTestParams }): Promise<Document>;

  // Catálogo de ajustes de la comunidad (mejora 5): sin servidor propio, solo el sitio del proyecto.
  /** Última copia guardada del catálogo (índice ya validado y su fecha). **No usa la red.** */
  communityCatalogCached(): Promise<CatalogState>;
  /**
   * Baja el catálogo del sitio del proyecto (destino fijo en Rust; la interfaz no lo elige), lo valida
   * (esquema, tamaño, número de entradas, rangos) y lo guarda en la caché. Solo se llama cuando la
   * persona abre el navegador de la comunidad o pulsa «Actualizar»: nunca al arrancar ni en segundo
   * plano. Sin conexión o ante un índice inválido devuelve la última copia buena con `error`.
   */
  communityCatalogRefresh(): Promise<CatalogState>;

  // Perfiles de dispositivo (fase 4)
  /** Perfil de partida de un tipo de controladora (dialecto, `s_max`, modo de potencia y baudios): `DeviceProfile::for_kind`. */
  deviceDefaults(args: { kind: DeviceProfile['kind'] }): Promise<DeviceProfile>;
  /**
   * Prueba de una vuelta del rotativo: un documento con una línea de exactamente una circunferencia
   * de la pieza. `speed` en mm/min y `power` en %. Falla si el rotativo del perfil está apagado.
   */
  rotaryTest(args: { device: DeviceProfile; speed: number; power: number }): Promise<Document>;
  /**
   * Lee un perfil de LightBurn (`.lbdev`) y devuelve los perfiles propuestos (con id y nombre que
   * no chocan con los guardados) y los avisos. No guarda nada ni toca la máquina: quien llama
   * guarda con `saveDevice` lo que el usuario confirme.
   */
  readLbdev(args: { path: string }): Promise<LbdevImport>;

  // Aplicación: preferencias, autoguardado
  /** Preferencias guardadas (`preferences.json`); null si todavía no hay. */
  loadPreferences(): Promise<PrefsFile | null>;
  savePreferences(args: { data: PrefsFile }): Promise<void>;
  /** Escribe un paquete `.tlakuprefs`. */
  exportPreferences(args: { path: string; bundle: PrefsBundle }): Promise<void>;
  /** Lee un paquete `.tlakuprefs`; la interfaz sanea y aplica lo que trae. */
  importPreferences(args: { path: string }): Promise<PrefsBundle>;
  /** Escribe un paquete de usuario `.tlakubundle` (la extensión es obligatoria). */
  exportUserBundle(args: { path: string; bundle: UserBundle }): Promise<void>;
  /** Lee un paquete `.tlakubundle`; el motor valida el sobre, la interfaz el contenido. */
  importUserBundle(args: { path: string }): Promise<UserBundle>;
  /** Configuración del respaldo a carpeta (crea el identificador de esta instalación la primera vez). */
  backupConfigGet(): Promise<BackupConfig>;
  /** Guarda lo elegido (carpeta, cuándo, cuántos); el motor lo acota y devuelve la configuración. */
  backupConfigSet(args: { settings: BackupSettings }): Promise<BackupConfig>;
  /** Escribe un respaldo fechado del paquete en la carpeta elegida y rota. Rechaza paquetes con secretos. */
  backupRun(args: { bundle: UserBundle }): Promise<BackupWrite>;
  /** Respaldo local (en la carpeta de configuración) del estado actual, antes de aplicar una importación. */
  backupSafety(args: { bundle: UserBundle }): Promise<BackupWrite>;
  /** Respaldos de la carpeta elegida, el más nuevo primero. Una carpeta ausente no es un error: `available: false`. */
  backupList(): Promise<BackupList>;
  /** Respaldo más nuevo que el estado de esta computadora y hecho en otra; `null` si no hay. */
  backupCheckNewer(): Promise<BackupEntry | null>;
  /** Anota que el respaldo con esa marca ya se importó o se descartó. */
  backupMarkSeen(args: { created_ms: number }): Promise<void>;
  /** Para cada ruta, si el archivo existe. */
  pathsExist(args: { paths: string[] }): Promise<boolean[]>;
  /** Guarda el texto de un respaldo de ajustes GRBL (`.tlakuset` o `.txt`, hasta 256 KiB; atómico). */
  writeSettingsBackup(args: { path: string; text: string }): Promise<void>;
  /** Lee un respaldo de ajustes GRBL como texto UTF-8 (hasta 256 KiB). */
  readSettingsBackup(args: { path: string }): Promise<string>;
  /**
   * Respaldo automático de los ajustes `$` de GRBL (M25): guarda el texto en
   * `grbl-backups/grbl-AAAAMMDD-HHMMSS.tlakuset` de la carpeta de configuración (sin pisar otro) y
   * devuelve la ruta. La interfaz lo pide justo antes de escribir; si falla, no escribe.
   */
  grblSettingsAutobackup(args: { text: string }): Promise<string>;
  /** Guarda la copia de recuperación `id` (atómico; recorta las más viejas). */
  autosaveWrite(args: { id: string; name: string; path: string | null; doc: Document }): Promise<RecoveryInfo>;
  /** Copias pendientes de recuperar, de la más nueva a la más vieja (las superadas por un guardado se borran). */
  autosaveList(): Promise<RecoveryInfo[]>;
  autosaveRead(args: { id: string }): Promise<Document>;
  autosaveDiscard(args: { id: string }): Promise<void>;

  // Historial de versiones del proyecto (`src-tauri/src/versions/`, mejora 10 del plan de usuarios)
  /** Versiones de `path` (de la más nueva a la más vieja) y lo que ocupan; mira la carpeta de la app y la de junto al archivo. */
  versionsList(args: { path: string }): Promise<VersionListing>;
  /** Miniatura SVG de la versión, o null si no tiene (diseño vacío o demasiado denso). */
  versionsThumb(args: { path: string; id: string }): Promise<string | null>;
  /** El documento de la versión, sin tocar el abierto. */
  versionsRead(args: { path: string; id: string }): Promise<Document>;
  /** Antes de guardar: conserva lo que tiene ahora el archivo `path`. Null si no hay archivo, no cambió o el historial está apagado. */
  versionsCapture(args: { path: string }): Promise<VersionInfo | null>;
  /** Guarda `doc` como versión `auto`, `manual` (con nombre) o `restore` (estado anterior a restaurar). Null si no cambió. */
  versionsSnapshot(args: { path: string; doc: Document; kind: SnapshotKind; label?: string | null }): Promise<VersionInfo | null>;
  /** Borra versiones; devuelve cuántas. */
  versionsDelete(args: { path: string; ids: string[] }): Promise<number>;
  /** Borra todo el historial del proyecto; devuelve cuántas versiones había. */
  versionsClear(args: { path: string }): Promise<number>;
  /** Proyectos con historial en la carpeta de datos de la app, el más reciente primero (para abrir el historial de un archivo que no está abierto). */
  versionsProjects(): Promise<ProjectSummary[]>;
  versionsConfigGet(): Promise<VersionsConfig>;
  /** Guarda la configuración (el motor la acota a rangos válidos) y devuelve la que quedó. */
  versionsConfigSet(args: { config: VersionsConfig }): Promise<VersionsConfig>;

  // Plantillas de proyecto (ola 3)
  /** Plantillas guardadas, por nombre. */
  listTemplates(): Promise<TemplateInfo[]>;
  /** Guarda (o reemplaza) la plantilla `name` con el diseño, las capas y los ajustes de `doc`. */
  saveTemplate(args: { name: string; doc: Document }): Promise<TemplateInfo>;
  loadTemplate(args: { name: string }): Promise<Document>;
  deleteTemplate(args: { name: string }): Promise<void>;
  /**
   * Abre otra ventana de la app con un documento independiente (comparte la máquina y las preferencias). Devuelve su etiqueta.
   * `handoff` (opcional): texto seguro para una dirección (`encodeURIComponent`) que se añade como `?handoff=...` a la
   * ventana nueva; así recibe la pestaña que se movió (ver `features/tabs/handoff.ts`).
   */
  newWindow(args?: { handoff?: string }): Promise<string>;
  /**
   * Abre la ventana aparte de la vista en vivo (`live`, de solo lectura; una sola a la vez) o la trae al
   * frente. La ventana que llama pasa a ser la dueña: a ella le llegan `live://ready` y `live://closed`.
   */
  liveWindowOpen(): Promise<void>;
  /** Cierra la ventana aparte de la vista en vivo si está abierta. */
  liveWindowClose(): Promise<void>;
  /** Manda `data` (texto JSON) a la ventana aparte como evento `live://route` o `live://state`. Devuelve si la ventana existe. */
  liveWindowSend(args: { kind: 'route' | 'state'; data: string }): Promise<boolean>;
  /** Solo la ventana aparte: ya escucha y pide a la dueña que le reenvíe la ruta y el estado (`live://ready`). */
  liveWindowHello(): Promise<void>;

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
  setEngineLocale: 'set_engine_locale',
  importFile: 'import_file',
  saveProject: 'save_project',
  loadProject: 'load_project',
  computeJob: 'compute_job',
  exportGcode: 'export_gcode',
  listDevices: 'list_devices',
  saveDevice: 'save_device',
  deleteDevice: 'delete_device',
  reorderDevices: 'reorder_devices',
  jobLinkWarning: 'job_link_warning',
  listPorts: 'list_ports',
  probePort: 'probe_port',
  automationConfigure: 'automation_configure',
  remoteConfigure: 'remote_configure',
  remoteStatus: 'remote_status',
  remoteRegenerate: 'remote_regenerate',
  remoteKick: 'remote_kick',
  remoteUpdate: 'remote_update',
  remoteSetRoute: 'remote_set_route',
  remoteSetLive: 'remote_set_live',
  remoteCameraFrame: 'remote_camera_frame',
  remoteStartResult: 'remote_start_result',
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
  probeTouch: 'probe_touch',
  ruidaInfo: 'ruida_info',
  ruidaFiles: 'ruida_files',
  ruidaPrepareStore: 'ruida_prepare_store',
  ruidaFileDelete: 'ruida_file_delete',
  ruidaFileRun: 'ruida_file_run',
  ruidaSettingsRead: 'ruida_settings_read',
  ruidaSettingsPreview: 'ruida_settings_preview',
  ruidaSettingsPreviewFile: 'ruida_settings_preview_file',
  ruidaSettingsApply: 'ruida_settings_apply',
  ruidaSettingsRestore: 'ruida_settings_restore',
  ruidaSettingsExport: 'ruida_settings_export',
  ruidaCalibrate: 'ruida_calibrate',
  ruidaBackups: 'ruida_backups',
  ruidaBackupText: 'ruida_backup_text',
  lhyPreview: 'lhy_preview',
  lhyHome: 'lhy_home',
  lhyState: 'lhy_state',
  galvoPreview: 'galvo_preview',
  galvoState: 'galvo_state',
  galvoRedRun: 'galvo_red_run',
  galvoRedLight: 'galvo_red_light',
  galvoConfirmMove: 'galvo_confirm_move',
  galvoFrame: 'galvo_frame',
  galvoAdjustFrame: 'galvo_adjust_frame',
  galvoVerify: 'galvo_verify',
  jogAxis: 'jog_axis',
  jogAxisTo: 'jog_axis_to',
  fireTest: 'fire_test',
  setOverride: 'set_override',
  simulateFault: 'simulate_fault',
  jobHistoryLoad: 'job_history_load',
  jobHistoryAppend: 'job_history_append',
  jobHistoryUpdate: 'job_history_update',
  jobHistoryRemove: 'job_history_remove',
  jobHistoryClear: 'job_history_clear',
  jobHistoryImport: 'job_history_import',
  jobHistoryExport: 'job_history_export',
  interruptedJob: 'interrupted_job',
  interruptedJobView: 'interrupted_job_view',
  discardInterruptedJob: 'discard_interrupted_job',
  continueCheck: 'continue_check',
  continueFrame: 'continue_frame',
  continueJob: 'continue_job',
  listFonts: 'list_fonts',
  textOutline: 'text_outline',
  fontCapabilities: 'font_capabilities',
  importShxFonts: 'import_shx_fonts',
  readCsv: 'read_csv',
  probeImages: 'probe_images',
  generateCode: 'generate_code',
  geometryOp: 'geometry_op',
  reviewDesign: 'review_design',
  shapeOp: 'shape_op',
  warpObjects: 'warp_objects',
  hullOutline: 'hull_outline',
  listGenerators: 'list_generators',
  generatePart: 'generate_part',
  tangentCircles: 'tangent_circles',
  traceImage: 'trace_image',
  vectorsToBitmap: 'vectors_to_bitmap',
  saveProcessedImage: 'save_processed_image',
  previewImage: 'preview_image',
  tonalPreview: 'tonal_preview',
  fitRegistration: 'fit_registration',
  planTiles: 'plan_tiles',
  artImportSvg: 'art_import_svg',
  artListUser: 'art_list_user',
  artAddUser: 'art_add_user',
  artRemoveUser: 'art_remove_user',
  artSaveUser: 'art_save_user',
  nestStart: 'nest_start',
  nestPoll: 'nest_poll',
  nestCancel: 'nest_cancel',
  loadRemnants: 'load_remnants',
  saveRemnants: 'save_remnants',
  detectBoard: 'detect_board',
  calibrateLens: 'calibrate_lens',
  loadCameraSetup: 'load_camera_setup',
  saveCameraSetup: 'save_camera_setup',
  saveBoardFile: 'save_board_file',
  fetchCameraFrame: 'fetch_camera_frame',
  cameraMarks: 'camera_marks',
  detectCameraMarks: 'detect_camera_marks',
  composeBedImage: 'compose_bed_image',
  detectCircle: 'detect_circle_in_image',
  loadMaterialLibrary: 'load_material_library',
  loadFactoryMaterials: 'load_factory_materials',
  saveMaterialLibrary: 'save_material_library',
  importMaterialLibrary: 'import_material_library',
  exportMaterialLibrary: 'export_material_library',
  materialTest: 'material_test',
  communityCatalogCached: 'community_catalog_cached',
  communityCatalogRefresh: 'community_catalog_refresh',
  deviceDefaults: 'device_defaults',
  rotaryTest: 'rotary_test',
  readLbdev: 'read_lbdev',
  importFileWith: 'import_file_with',
  exportDesign: 'export_design_file',
  printDesign: 'print_design',
  loadPreferences: 'load_preferences',
  savePreferences: 'save_preferences',
  exportPreferences: 'export_preferences',
  importPreferences: 'import_preferences',
  exportUserBundle: 'export_user_bundle',
  importUserBundle: 'import_user_bundle',
  backupConfigGet: 'backup_config_get',
  backupConfigSet: 'backup_config_set',
  backupRun: 'backup_run',
  backupSafety: 'backup_safety',
  backupList: 'backup_list',
  backupCheckNewer: 'backup_check_newer',
  backupMarkSeen: 'backup_mark_seen',
  pathsExist: 'paths_exist',
  writeSettingsBackup: 'write_settings_backup',
  readSettingsBackup: 'read_settings_backup',
  grblSettingsAutobackup: 'grbl_settings_autobackup',
  autosaveWrite: 'autosave_write',
  autosaveList: 'autosave_list',
  autosaveRead: 'autosave_read',
  autosaveDiscard: 'autosave_discard',
  versionsList: 'versions_list',
  versionsThumb: 'versions_thumb',
  versionsRead: 'versions_read',
  versionsCapture: 'versions_capture',
  versionsSnapshot: 'versions_snapshot',
  versionsDelete: 'versions_delete',
  versionsClear: 'versions_clear',
  versionsProjects: 'versions_projects',
  versionsConfigGet: 'versions_config_get',
  versionsConfigSet: 'versions_config_set',
  listTemplates: 'list_templates',
  saveTemplate: 'save_template',
  loadTemplate: 'load_template',
  deleteTemplate: 'delete_template',
  newWindow: 'new_window',
  liveWindowOpen: 'live_window_open',
  liveWindowClose: 'live_window_close',
  liveWindowSend: 'live_window_send',
  liveWindowHello: 'live_window_hello',
} as const satisfies Record<Exclude<keyof LaserApi, 'on'>, string>;

export const EVENT_NAMES: readonly EventName[] = [
  'machine://status',
  'machine://progress',
  'machine://log',
  'machine://job',
  'machine://connection',
  'automation://load',
  'automation://start',
  'remote://clients',
  'remote://start',
  'remote://action',
  'menu://preferences',
];
