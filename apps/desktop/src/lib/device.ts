import type { DeviceProfile, Point, RotaryKind, RotarySettings, ScanOffset } from '@/api/types';
import { MAX_MACROS, macroProblem } from '@/features/machine/macros';
import { t } from '@/i18n';

type GcodeDialect = DeviceProfile['dialect'];
export type DeviceKind = DeviceProfile['kind'];

/** Mismos valores que `RotarySettings::default()` del motor (xTool RA2 Pro: 32 mm/vuelta, rodillo de 17 mm). */
export const DEFAULT_ROTARY: RotarySettings = {
  enabled: false,
  kind: 'roller',
  mm_per_rotation: 32,
  roller_diameter: 17,
  object_diameter: 80,
  mirror: false,
};

/** Mismos valores que `GcodeDialect::default()` del motor (GRBL 1.1). */
export const DEFAULT_DIALECT: GcodeDialect = {
  s_decimals: 3,
  decimals: 3,
  constant_on: 'M3 S0',
  dynamic_on: 'M4 S0',
  laser_off: 'M5',
  s_inline: true,
  power_cmd: 'M3 S{s}',
  g0_laser_off: true,
  travel_s0: false,
  feed_on_g0: false,
  air_on: 'M8',
  air_off: 'M9',
  end_sync: null,
  comment: 'semicolon',
  dwell: 'p_seconds',
  max_line_len: 127,
  laser1_select: null,
  laser2_select: null,
};

/** Valores por defecto de los campos de la ola 4 (los mismos que `DeviceProfile::default()`). */
export const DEVICE_WAVE4_DEFAULTS: Pick<
  DeviceProfile,
  'connection' | 'macros' | 'positions' | 'laser2' | 'u_axis' | 'ruida' | 'lhymicro' | 'galvo' | 'feeder'
> = {
  connection: { type: 'serial' },
  macros: [],
  positions: [],
  laser2: false,
  u_axis: false,
  ruida: { magic: null, run_after_upload: false },
  lhymicro: { board: 'M2' },
  // Ola 5: mismos valores que `GalvoDevice::default()` y `FeederSettings::default()`.
  galvo: {
    field_mm: 110,
    source: 'fiber_q',
    watts: 20,
    freq_min_khz: 20,
    freq_max_khz: 80,
    pulse_widths_ns: [],
    timing: { laser_on_us: 100, laser_off_us: 100, end_us: 300, polygon_us: 100 },
    jump: { speed: 4000, delay_min_us: 200, delay_max_us: 400, distance_limit: 10 },
    correction: {
      scale: [1, 1],
      offset: [0, 0],
      rotation_deg: 0,
      invert: [false, false],
      swap_xy: false,
      bulge: [0, 0],
      trapezoid: [0, 0],
      skew: [0, 0],
      cor_file: null,
      cor_transpose: false,
      cor_invert: false,
    },
    max_power_pct: 100,
    light_speed: 2000,
    cylinder: { enabled: false, diameter_mm: 70, mirror_distance_mm: 160, axis: 'x', concave: false },
    taper: {
      enabled: false,
      top_diameter_mm: 70,
      bottom_diameter_mm: 50,
      length_mm: 100,
      axis: 'x',
      top_at_positive: true,
    },
    split: { enabled: false, piece_mm: 0, overlap_mm: 0, pause_after_move_ms: 0, order: 'snake', axes: 'xy', whole_shapes: false },
    repeat: { count: 1, increment_mm: [0, 0], pause_ms: 0 },
    rotary: { enabled: false, diameter_mm: 50, steps_per_rev: 0, axis: 'x', reverse: false },
    require_input: null,
    overscan_mm: 0,
  },
  feeder: { enabled: false, feed_gcode: 'G91\nG0 Y{mm}\nG90', feed_mm: 300 },
};

/** Valores por defecto de los campos de la ola 6C (los mismos que `DeviceProfile::default()`). */
export const DEVICE_WAVE6C_DEFAULTS: Pick<
  DeviceProfile,
  | 'is_default'
  | 'icon'
  | 'pointer_offset'
  | 'speed_unit'
  | 'swap_xy'
  | 'home_on_connect'
  | 'no_dtr_reset'
  | 'sync_streaming'
  | 'u_offset'
  | 'laser_watts'
> = {
  is_default: false,
  icon: '',
  pointer_offset: [0, 0],
  speed_unit: 'mm_per_min',
  swap_xy: false,
  home_on_connect: false,
  no_dtr_reset: false,
  sync_streaming: false,
  u_offset: 0,
  laser_watts: null,
};

/** Todos los campos con valor por defecto que un perfil guardado antes puede no traer. */
export const DEVICE_EXTRA_DEFAULTS = { ...DEVICE_WAVE4_DEFAULTS, ...DEVICE_WAVE6C_DEFAULTS };

/** Perfil provisional mientras no se ha cargado ninguno (cama de 400 x 400 mm). */
export const FALLBACK_DEVICE: DeviceProfile = {
  id: 'predeterminado',
  name: 'Máquina GRBL',
  kind: 'grbl',
  dialect: { ...DEFAULT_DIALECT },
  bed: [400, 400],
  origin: 'bottom_left',
  s_max: 1000,
  power_mode: 'dynamic',
  accel: [2000, 2000],
  max_rate: [20000, 20000],
  rapid_rate: 10000,
  baud: 115200,
  rx_buffer: 127,
  start_gcode: '',
  end_gcode: 'M5',
  return_to_origin: false,
  z_axis: false,
  z_invert: false,
  rotary: { ...DEFAULT_ROTARY },
  scan_offsets: [],
  scan_initial_offset: 0,
  ...DEVICE_EXTRA_DEFAULTS,
};


/**
 * Completa un perfil que llega sin los campos de la Fase 4 (guardado antes de ellos) con los
 * valores por defecto. Devuelve el mismo objeto si ya estaba completo.
 */
export function normalizeDevice(device: Partial<DeviceProfile> & Pick<DeviceProfile, 'id' | 'name'>): DeviceProfile {
  const complete =
    typeof device.z_axis === 'boolean' &&
    typeof device.z_invert === 'boolean' &&
    typeof device.scan_initial_offset === 'number' &&
    (Object.keys(DEVICE_EXTRA_DEFAULTS) as (keyof typeof DEVICE_EXTRA_DEFAULTS)[]).every((k) => k in device) &&
    device.dialect !== undefined &&
    (Object.keys(DEFAULT_DIALECT) as (keyof GcodeDialect)[]).every((k) => k in device.dialect!) &&
    Array.isArray(device.scan_offsets) &&
    device.rotary !== undefined &&
    (Object.keys(DEFAULT_ROTARY) as (keyof RotarySettings)[]).every((k) => k in device.rotary!);
  if (complete) return device as DeviceProfile;
  return {
    ...FALLBACK_DEVICE,
    ...device,
    z_axis: device.z_axis ?? false,
    z_invert: device.z_invert ?? false,
    // Un perfil sin dialecto toma el de su tipo (como el motor); los campos que falten se completan.
    dialect: { ...dialectForKind(device.kind ?? 'grbl'), ...device.dialect },
    rotary: { ...DEFAULT_ROTARY, ...device.rotary },
    scan_offsets: device.scan_offsets ?? [],
    scan_initial_offset: device.scan_initial_offset ?? 0,
    ...DEVICE_EXTRA_DEFAULTS,
    ...Object.fromEntries(
      (Object.keys(DEVICE_EXTRA_DEFAULTS) as (keyof typeof DEVICE_EXTRA_DEFAULTS)[])
        .filter((k) => device[k] !== undefined)
        .map((k) => [k, device[k]]),
    ),
  };
}

/** Familia GRBL (`$H` para el homing, conteo de caracteres, `$$`): las opciones de enlace de GRBL solo cuentan aquí. */
export function isGrblFamily(kind: DeviceKind): boolean {
  return kind === 'grbl' || kind === 'grbl_hal' || kind === 'fluid_nc' || kind === 'grbl_lpc' || kind === 'grbl_m3';
}

/** Velocidades de comunicación que ofrecen GRBL y sus derivados. */
export const BAUD_RATES = [9600, 19200, 38400, 57600, 115200, 230400, 250000, 460800] as const;

// ---------------------------------------------------------------------------
// Alta, copia e identificadores
// ---------------------------------------------------------------------------

/** Texto en minúsculas sin acentos y con guiones: base de un id legible. */
export function slugify(text: string): string {
  const s = text
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return s || 'maquina';
}

/** Id basado en `base` que no choca con ninguno de `taken`. */
export function uniqueDeviceId(base: string, taken: Iterable<string>): string {
  const used = new Set(taken);
  const slug = slugify(base);
  if (!used.has(slug)) return slug;
  let i = 2;
  while (used.has(`${slug}-${i}`)) i++;
  return `${slug}-${i}`;
}

/** Perfil nuevo con los valores habituales de un láser de diodo GRBL. */
export function newDeviceProfile(name: string, takenIds: Iterable<string>): DeviceProfile {
  return { ...FALLBACK_DEVICE, name, id: uniqueDeviceId(name, takenIds) };
}

/** Copia de un perfil con otro id y el nombre marcado como copia. */
export function duplicateDeviceProfile(source: DeviceProfile, copySuffix: string, takenIds: Iterable<string>): DeviceProfile {
  const name = `${source.name} ${copySuffix}`;
  return structuredClone({ ...source, name, id: uniqueDeviceId(name, takenIds) });
}

// ---------------------------------------------------------------------------
// Validación
// ---------------------------------------------------------------------------

export type DeviceField =
  | 'name'
  | 'bed'
  | 's_max'
  | 'accel'
  | 'max_rate'
  | 'rapid_rate'
  | 'baud'
  | 'rx_buffer'
  | 'mm_per_rotation'
  | 'roller_diameter'
  | 'object_diameter'
  | 'scan_initial_offset'
  | 'scan_offsets'
  | 'macros'
  | 'laser_watts';

export type DeviceIssue = 'required' | 'positive' | 'integer' | 'tooBig' | 'tooSmall' | 'duplicate' | 'macro';

const pos = (n: number) => Number.isFinite(n) && n > 0;

/** Tope de la potencia del láser que acepta el perfil (W). */
export const MAX_LASER_WATTS = 1000;

/**
 * Potencia nominal del láser del perfil (W), o `null` si no se sabe. En un galvo es la de su fuente;
 * en el resto, `laser_watts` (el asistente la llena desde el catálogo o desde lo que dice la placa).
 */
export function laserWattsOf(device: Pick<DeviceProfile, 'kind' | 'laser_watts' | 'galvo'>): number | null {
  const w = device.kind === 'galvo' ? (device.laser_watts ?? device.galvo?.watts) : device.laser_watts;
  return w != null && Number.isFinite(w) && w > 0 ? w : null;
}

/** Filas de la tabla de corrección de barrido con problema: velocidad no positiva o repetida. */
export function scanOffsetRowIssues(rows: readonly ScanOffset[]): (DeviceIssue | null)[] {
  const count = new Map<number, number>();
  for (const r of rows) count.set(r.speed, (count.get(r.speed) ?? 0) + 1);
  return rows.map((r) => {
    if (!pos(r.speed)) return 'positive';
    if (!Number.isFinite(r.offset)) return 'required';
    return (count.get(r.speed) ?? 0) > 1 ? 'duplicate' : null;
  });
}

/** Filas ordenadas por velocidad (sin tocar el original). */
export function sortScanOffsets(rows: readonly ScanOffset[]): ScanOffset[] {
  return [...rows].sort((a, b) => a.speed - b.speed);
}

/** Errores del perfil por campo; objeto vacío si es válido. */
export function validateDevice(d: DeviceProfile): Partial<Record<DeviceField, DeviceIssue>> {
  const out: Partial<Record<DeviceField, DeviceIssue>> = {};
  if (d.name.trim() === '') out.name = 'required';
  if (!d.bed.every(pos)) out.bed = 'positive';
  else if (d.bed.some((v) => v > 10000)) out.bed = 'tooBig';
  if (!pos(d.s_max)) out.s_max = 'positive';
  else if (!Number.isInteger(d.s_max)) out.s_max = 'integer';
  else if (d.s_max > 100000) out.s_max = 'tooBig';
  if (!d.accel.every(pos)) out.accel = 'positive';
  if (!d.max_rate.every(pos)) out.max_rate = 'positive';
  if (!pos(d.rapid_rate)) out.rapid_rate = 'positive';
  // Los watts son opcionales (`null` = no se sabe); si están, deben ser una potencia razonable.
  if (d.laser_watts != null) {
    if (!pos(d.laser_watts)) out.laser_watts = 'positive';
    else if (d.laser_watts > MAX_LASER_WATTS) out.laser_watts = 'tooBig';
  }
  if (!pos(d.baud)) out.baud = 'positive';
  else if (!Number.isInteger(d.baud)) out.baud = 'integer';
  if (!pos(d.rx_buffer)) out.rx_buffer = 'positive';
  else if (!Number.isInteger(d.rx_buffer)) out.rx_buffer = 'integer';
  else if (d.rx_buffer < 16) out.rx_buffer = 'tooSmall';
  // Un perfil viejo puede llegar sin estos campos: se valida lo que hay.
  const r = d.rotary;
  if (r?.enabled) {
    if (!pos(r.mm_per_rotation)) out.mm_per_rotation = 'positive';
    if (r.kind === 'roller' && !pos(r.roller_diameter)) out.roller_diameter = 'positive';
    if (!pos(r.object_diameter)) out.object_diameter = 'positive';
  }
  if (d.scan_initial_offset !== undefined && !Number.isFinite(d.scan_initial_offset)) out.scan_initial_offset = 'required';
  const rowIssues = scanOffsetRowIssues(d.scan_offsets ?? []);
  const firstBad = rowIssues.find((i) => i !== null);
  if (firstBad) out.scan_offsets = firstBad;
  const macros = d.macros ?? [];
  if (macros.length > MAX_MACROS || macros.some((m) => macroProblem(m, d.rx_buffer) !== null)) out.macros = 'macro';
  return out;
}

/** Dos perfiles con los mismos valores (para saber si hay cambios sin guardar). */
export function sameDevice(a: DeviceProfile, b: DeviceProfile): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

// ---------------------------------------------------------------------------
// Ajustes de GRBL (`$$`) -> propuesta de perfil
// ---------------------------------------------------------------------------

/** Ajustes de GRBL por número: `$130=400.000` -> `{ 130: 400 }`. */
export type GrblSettings = Record<number, number>;

/** Lee las líneas `$N=valor` de la respuesta de `$$`; ignora el resto (`ok`, avisos, comentarios). */
export function parseGrblSettings(lines: readonly string[]): GrblSettings {
  const out: GrblSettings = {};
  for (const line of lines) {
    const m = /^\s*\$(\d+)\s*=\s*(-?\d+(?:\.\d+)?)/.exec(line);
    if (m) out[Number(m[1])] = Number(m[2]);
  }
  return out;
}

/** Campos del perfil que se pueden proponer desde la máquina. */
export type ProposalField = 'bed' | 'max_rate' | 'accel' | 's_max';

export interface ProposalChange {
  field: ProposalField;
  /** Ajustes de GRBL de donde sale el valor, por ejemplo `$130/$131`. */
  codes: string;
  from: number | Point;
  to: number | Point;
}

export interface DeviceProposal {
  /** Solo lo que la máquina contestó y difiere del perfil. */
  changes: ProposalChange[];
  /** `$32`: true = modo láser, false = desactivado (M4 no funciona bien), null = no se pudo leer. */
  laserMode: boolean | null;
  /** La máquina no devolvió ninguno de los ajustes que se interpretan. */
  nothingFound: boolean;
  /** Los ajustes leídos coinciden con el perfil. */
  inSync: boolean;
}

/** Pareja de ajustes por eje; si falta uno se conserva el valor actual de ese eje. */
function axisPair(s: GrblSettings, x: number, y: number, current: Point): Point | null {
  const vx = s[x];
  const vy = s[y];
  const okX = vx !== undefined && vx > 0;
  const okY = vy !== undefined && vy > 0;
  if (!okX && !okY) return null;
  return [okX ? vx : current[0], okY ? vy : current[1]];
}

const samePoint = (a: Point, b: Point) => a[0] === b[0] && a[1] === b[1];

/**
 * Compara los ajustes de GRBL con el perfil y propone cambios:
 * `$130/$131` área (recorrido máximo), `$110/$111` velocidad máxima (mm/min),
 * `$120/$121` aceleración (mm/s²) y `$30` S máxima. Además verifica `$32` (modo láser).
 */
export function proposeFromSettings(settings: GrblSettings, device: DeviceProfile): DeviceProposal {
  const changes: ProposalChange[] = [];
  const pairs: [ProposalField, number, number, Point, string][] = [
    ['bed', 130, 131, device.bed, '$130/$131'],
    ['max_rate', 110, 111, device.max_rate, '$110/$111'],
    ['accel', 120, 121, device.accel, '$120/$121'],
  ];
  let found = false;
  for (const [field, x, y, current, codes] of pairs) {
    const to = axisPair(settings, x, y, current);
    if (!to) continue;
    found = true;
    if (!samePoint(to, current)) changes.push({ field, codes, from: [...current], to });
  }
  const sMax = settings[30];
  if (sMax !== undefined && sMax > 0) {
    found = true;
    if (sMax !== device.s_max) changes.push({ field: 's_max', codes: '$30', from: device.s_max, to: sMax });
  }
  const laser = settings[32];
  if (laser !== undefined) found = true;
  return {
    changes,
    laserMode: laser === undefined ? null : laser === 1,
    nothingFound: !found,
    inSync: found && changes.length === 0,
  };
}

/** Perfil con los cambios propuestos aplicados (no toca nada más). */
export function applyProposal(device: DeviceProfile, changes: readonly ProposalChange[]): DeviceProfile {
  const next = { ...device };
  for (const c of changes) {
    if (c.field === 's_max') next.s_max = c.to as number;
    else next[c.field] = [...(c.to as Point)] as Point;
  }
  return next;
}

// ---------------------------------------------------------------------------
// Coordenadas: documento <-> máquina
// ---------------------------------------------------------------------------

/** Lo que hace falta del perfil para pasar de coordenadas de documento a las de la máquina y al revés. */
export type DeviceFrame = Pick<DeviceProfile, 'bed' | 'origin'> & Partial<Pick<DeviceProfile, 'swap_xy' | 'rotary'>>;
type Frame = DeviceFrame;

const fromRight = (o: DeviceProfile['origin']) => o === 'bottom_right' || o === 'top_right';
const fromTop = (o: DeviceProfile['origin']) => o === 'top_left' || o === 'top_right';

/** El intercambio X/Y está en uso (con el rotativo encendido se ignora, como en el motor). */
export function swapActive(device: Partial<Pick<DeviceProfile, 'swap_xy' | 'rotary'>>): boolean {
  return device.swap_xy === true && device.rotary?.enabled !== true;
}

/**
 * Posición de trabajo de la máquina (el G-code crece hacia dentro de la cama desde la esquina
 * del perfil, con X e Y cruzadas si hay intercambio) a coordenadas del documento (origen abajo a
 * la izquierda, Y hacia arriba).
 */
export function workToDoc(device: Frame, p: Point): Point {
  const q: Point = swapActive(device) ? [p[1], p[0]] : p;
  return [fromRight(device.origin) ? device.bed[0] - q[0] : q[0], fromTop(device.origin) ? device.bed[1] - q[1] : q[1]];
}

/** Inversa de `workToDoc`: primero el cambio de origen y al final el intercambio X/Y. */
export function docToWork(device: Frame, p: Point): Point {
  const q: Point = [fromRight(device.origin) ? device.bed[0] - p[0] : p[0], fromTop(device.origin) ? device.bed[1] - p[1] : p[1]];
  return swapActive(device) ? [q[1], q[0]] : q;
}

/**
 * Desplazamiento de pantalla/documento (X a la derecha, Y arriba) a desplazamiento de G-code
 * relativo (`$J=G91`): se invierte el eje que crece en sentido contrario al del documento y,
 * con intercambio X/Y, se cruzan.
 */
export function docDeltaToWork(device: Pick<DeviceProfile, 'origin'> & Partial<Pick<DeviceProfile, 'swap_xy' | 'rotary'>>, dx: number, dy: number): Point {
  const q: Point = [fromRight(device.origin) ? -dx : dx, fromTop(device.origin) ? -dy : dy];
  return swapActive(device) ? [q[1], q[0]] : q;
}

// ---------------------------------------------------------------------------
// Unidades de velocidad (solo para mostrar: el perfil, las capas y el G-code siguen en mm/min)
// ---------------------------------------------------------------------------

export type SpeedUnit = DeviceProfile['speed_unit'];

export const SPEED_UNITS: readonly SpeedUnit[] = ['mm_per_min', 'mm_per_sec'];

/** Texto de la unidad, por ejemplo `mm/min`. */
export function speedUnitLabel(unit: SpeedUnit): string {
  return unit === 'mm_per_sec' ? 'mm/s' : 'mm/min';
}

/** De mm/min (lo interno) a la unidad con la que se muestra. */
export function speedToDisplay(mmPerMin: number, unit: SpeedUnit): number {
  return unit === 'mm_per_sec' ? mmPerMin / 60 : mmPerMin;
}

/** De la unidad que escribe el usuario a mm/min. */
export function speedFromDisplay(value: number, unit: SpeedUnit): number {
  return unit === 'mm_per_sec' ? value * 60 : value;
}

/** Desfase del puntero rojo respecto del rayo en el sistema del documento (ver `pointer_offset`). */
export function hasPointerOffset(device: Pick<DeviceProfile, 'pointer_offset'>): boolean {
  const [x, y] = device.pointer_offset ?? [0, 0];
  return x !== 0 || y !== 0;
}

// ---------------------------------------------------------------------------
// Firmware
// ---------------------------------------------------------------------------

/**
 * Versión de GRBL del saludo (`Grbl 1.1h ['$' for help]`). `modernEnough`: 1.1f o posterior, que
 * permite modo láser (M4) y jog con `$J=`. Null si el texto no parece de GRBL.
 */
export function parseGrblVersion(firmware: string): { major: number; minor: number; letter: string; modernEnough: boolean } | null {
  const m = /grbl\s+(\d+)\.(\d+)([a-z]?)/i.exec(firmware);
  if (!m) return null;
  const major = Number(m[1]);
  const minor = Number(m[2]);
  const letter = (m[3] ?? '').toLowerCase();
  const modernEnough = major > 1 || (major === 1 && (minor > 1 || (minor === 1 && letter >= 'f')));
  return { major, minor, letter, modernEnough };
}

// ---------------------------------------------------------------------------
// Tipo de controladora y valores por defecto
// ---------------------------------------------------------------------------

/** Tipos en el orden en que se ofrecen. Los textos salen de `devices.kind.<id>`. */
export const DEVICE_KINDS: readonly DeviceKind[] = ['grbl', 'grbl_hal', 'fluid_nc', 'grbl_lpc', 'grbl_m3', 'smoothie', 'marlin', 'custom_gcode', 'ruida', 'lhy_micro', 'galvo'];

/**
 * Tipos que la app todavía no sabe enviar a la máquina (solo exportar el G-code). Vaciar este
 * conjunto quita el aviso de toda la interfaz.
 */
export const EXPORT_ONLY_KINDS: ReadonlySet<DeviceKind> = new Set<DeviceKind>();

/** Marlin 2.x con potencia inline (`GcodeDialect::marlin_inline` del motor). */
function marlinInline(): GcodeDialect {
  return {
    ...DEFAULT_DIALECT,
    s_decimals: 0,
    constant_on: 'M3 S0 I',
    dynamic_on: null,
    travel_s0: true,
    feed_on_g0: true,
    end_sync: 'M400',
    dwell: 'p_milliseconds',
    max_line_len: 96,
  };
}

/** Dialecto habitual de cada firmware: copia de `GcodeDialect::for_kind` (crates/tlaku-doc/src/device.rs). */
export function dialectForKind(kind: DeviceKind): GcodeDialect {
  const grbl = { ...DEFAULT_DIALECT };
  switch (kind) {
    case 'grbl_m3':
      return { ...grbl, s_decimals: 0, dynamic_on: null, g0_laser_off: false, travel_s0: true };
    case 'smoothie':
      return { ...grbl, dynamic_on: null, end_sync: 'M400', dwell: 'p_milliseconds', max_line_len: 0 };
    case 'marlin':
      return marlinInline();
    default:
      return grbl;
  }
}

/** `S` para el 100 % que suele traer el firmware: copia de `DeviceKind::default_s_max`. */
export function defaultSMax(kind: DeviceKind): number {
  switch (kind) {
    case 'grbl_m3':
    case 'marlin':
      return 255;
    case 'grbl_lpc':
    case 'smoothie':
      return 1;
    default:
      return 1000;
  }
}

/** Velocidad serie habitual del firmware: copia de `DeviceKind::default_baud`. */
export function defaultBaud(kind: DeviceKind): number {
  return kind === 'marlin' ? 250_000 : 115_200;
}

/** Lo que cambia un tipo de controladora en un perfil (los campos que pisa `DeviceProfile::for_kind`). */
export type KindDefaults = Pick<DeviceProfile, 'kind' | 'dialect' | 's_max' | 'power_mode' | 'baud'>;

/** Valores del tipo, calculados aquí; el modo simulado y las pruebas los usan como el motor. */
export function kindDefaults(kind: DeviceKind): KindDefaults {
  const dialect = dialectForKind(kind);
  return {
    kind,
    dialect,
    // Sin modo dinámico en el firmware la potencia es fija.
    power_mode: dialect.dynamic_on !== null ? 'dynamic' : 'constant',
    s_max: defaultSMax(kind),
    baud: defaultBaud(kind),
  };
}

export type KindChangeField = 'dialect' | 's_max' | 'power_mode' | 'baud';

export interface KindChange {
  field: KindChangeField;
  from: DeviceProfile[KindChangeField];
  to: DeviceProfile[KindChangeField];
}

const sameJson = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

/**
 * Campos que pisaría aplicar los valores de `defaults` al perfil. Vacío si ya coinciden. Con
 * `custom_gcode` siempre queda vacío: el tipo personalizado parte de lo que el perfil ya tiene
 * (el dialecto actual es el punto de partida de la edición).
 */
export function planKindChange(draft: DeviceProfile, defaults: KindDefaults): KindChange[] {
  if (defaults.kind === 'custom_gcode') return [];
  const out: KindChange[] = [];
  for (const field of ['dialect', 's_max', 'power_mode', 'baud'] as const) {
    if (!sameJson(draft[field], defaults[field])) out.push({ field, from: draft[field], to: defaults[field] });
  }
  return out;
}

/** Perfil con el nuevo tipo; con `applyValues` además toma los valores del tipo (ver `planKindChange`). */
export function applyKindChange(draft: DeviceProfile, defaults: KindDefaults, applyValues: boolean): DeviceProfile {
  const next: DeviceProfile = { ...draft, kind: defaults.kind };
  // Ruida habla UDP a la IP del perfil; los demás tipos no usan UDP.
  if (defaults.kind === 'ruida' && draft.connection.type !== 'udp') next.connection = { type: 'udp', host: '' };
  else if (defaults.kind !== 'ruida' && draft.connection.type === 'udp') next.connection = { type: 'serial' };
  // El galvo coloca el campo con las coordenadas del documento: solo vale el origen abajo a la izquierda.
  if (defaults.kind === 'galvo') next.origin = 'bottom_left';
  if (applyValues) {
    for (const c of planKindChange(draft, defaults)) {
      if (c.field === 'dialect') next.dialect = structuredClone(defaults.dialect);
      else if (c.field === 'power_mode') next.power_mode = defaults.power_mode;
      else next[c.field] = defaults[c.field];
    }
  }
  return next;
}

// ---------------------------------------------------------------------------
// Dialecto de G-code: resumen, validación y ejemplo
// ---------------------------------------------------------------------------

/** Primera palabra de una orden (`M4 S0` -> `M4`). */
const word = (cmd: string) => cmd.trim().split(/\s+/)[0] ?? '';

/** Resumen de una línea de cómo escribe el G-code el perfil: «S 0-1000, M4 dinámico, aire M8/M9…». Sin `includeS` omite la escala de S. */
export function dialectSummary(device: Pick<DeviceProfile, 'dialect' | 's_max'>, includeS = true): string {
  const d = device.dialect;
  const parts = includeS ? [t('devices.summary.s', { max: String(device.s_max) })] : [];
  parts.push(d.dynamic_on ? t('devices.summary.dynamic', { cmd: word(d.dynamic_on) }) : t('devices.summary.noDynamic'));
  parts.push(d.s_inline ? t('devices.summary.sInline') : t('devices.summary.sSeparate'));
  if (!d.g0_laser_off) parts.push(t('devices.summary.g0Keeps'));
  parts.push(d.air_on && d.air_off ? t('devices.summary.air', { on: word(d.air_on), off: word(d.air_off) }) : t('devices.summary.noAir'));
  if (d.end_sync) parts.push(t('devices.summary.sync', { cmd: word(d.end_sync) }));
  return parts.join(', ');
}

export type DialectField = keyof GcodeDialect;
export type DialectIssue = 'empty' | 'multiline' | 'missingS' | 'badPlaceholder' | 'range' | 'lineTooShort' | 'airPair';

const MAX_DECIMALS = 6;
const MIN_LINE_LEN = 32;
const MAX_LINE_LEN = 4096;

const isLine = (s: string) => !/[\r\n]/.test(s);
const inRange = (n: number, lo: number, hi: number) => Number.isInteger(n) && n >= lo && n <= hi;

/**
 * Errores de las plantillas del dialecto, por campo; vacío si es válido. Cada orden debe ser una
 * sola línea; la línea de potencia aparte necesita `{s}` (y ningún otro marcador); el aire va
 * completo (encender y apagar) o no va.
 */
export function validateDialect(d: GcodeDialect): Partial<Record<DialectField, DialectIssue>> {
  const out: Partial<Record<DialectField, DialectIssue>> = {};
  const line = (field: 'constant_on' | 'laser_off' | 'dynamic_on' | 'air_on' | 'air_off' | 'end_sync' | 'laser1_select' | 'laser2_select', required: boolean) => {
    const v = d[field];
    if (v === null) return;
    if (!isLine(v)) out[field] = 'multiline';
    else if (required && v.trim() === '') out[field] = 'empty';
  };
  // El apagado es obligatorio: sin él el láser no se apaga al terminar.
  line('laser_off', true);
  line('dynamic_on', true);
  line('air_on', true);
  line('air_off', true);
  line('end_sync', true);
  line('laser1_select', true);
  line('laser2_select', true);
  // Sin `constant_on` el láser solo se enciende con la línea de potencia aparte.
  line('constant_on', false);
  if (d.s_inline && d.constant_on.trim() === '' && !out.constant_on) out.constant_on = 'empty';
  if ((d.air_on === null) !== (d.air_off === null)) out[d.air_on === null ? 'air_on' : 'air_off'] = 'airPair';
  if (!d.s_inline) {
    if (!isLine(d.power_cmd)) out.power_cmd = 'multiline';
    else if (!d.power_cmd.includes('{s}')) out.power_cmd = 'missingS';
    else if (/\{(?!s\})[^}]*\}/.test(d.power_cmd)) out.power_cmd = 'badPlaceholder';
  }
  if (!inRange(d.s_decimals, 0, MAX_DECIMALS)) out.s_decimals = 'range';
  if (!inRange(d.decimals, 0, MAX_DECIMALS)) out.decimals = 'range';
  if (!inRange(d.max_line_len, 0, MAX_LINE_LEN)) out.max_line_len = 'range';
  else if (d.max_line_len > 0 && d.max_line_len < MIN_LINE_LEN) out.max_line_len = 'lineTooShort';
  return out;
}

export type DialectWarning = 'g0StaysOn' | 'dynamicMissing';

/** Combinaciones válidas pero sospechosas, para avisar sin bloquear el guardado. */
export function dialectWarnings(device: Pick<DeviceProfile, 'dialect' | 'power_mode'>): DialectWarning[] {
  const d = device.dialect;
  const out: DialectWarning[] = [];
  // Con S en línea y sin G0 que apague, solo `S0` en el G0 evita disparar en los desplazamientos.
  if (d.s_inline && !d.g0_laser_off && !d.travel_s0) out.push('g0StaysOn');
  if (device.power_mode === 'dynamic' && d.dynamic_on === null) out.push('dynamicMissing');
  return out;
}

/** Cuántos decimales como máximo (sin ceros sobrantes), igual que `num` del motor. */
function num(v: number, decimals: number): string {
  let s = v.toFixed(Math.min(decimals, MAX_DECIMALS));
  if (s.includes('.')) s = s.replace(/0+$/, '').replace(/\.$/, '');
  return s === '-0' ? '0' : s;
}

/** Valor de `S` para un porcentaje de potencia (`s_value` del motor). */
function sValue(powerPct: number, sMax: number, decimals: number): string {
  const max = Number.isFinite(sMax) ? Math.max(sMax, 0) : 0;
  const s = Math.min(Math.max((powerPct / 100) * max, 0), max);
  return num(decimals === 0 ? Math.round(s) : s, decimals);
}

export interface SampleOptions {
  /** Comentario del bloque (el nombre de la capa en el G-code real). */
  label: string;
  /** Velocidad del bloque en mm/min. */
  speed?: number;
  /** Potencia en %. */
  power?: number;
  /** Lado del cuadrado en mm. */
  size?: number;
}

/**
 * G-code de ejemplo (un cuadrado pequeño con aire) como lo escribiría el dialecto del perfil.
 * Es una réplica aproximada y breve del generador del motor (`tlaku-grbl`), para ver el efecto de
 * editar el dialecto; el G-code real lo escribe siempre el motor (sin Z, rotativo ni puentes).
 */
export function sampleGcode(device: Pick<DeviceProfile, 'dialect' | 's_max' | 'power_mode' | 'rapid_rate' | 'start_gcode' | 'end_gcode' | 'return_to_origin'>, opts: SampleOptions): string[] {
  const d = device.dialect;
  const speed = opts.speed ?? 6000;
  const power = opts.power ?? 40;
  const side = opts.size ?? 20;
  const origin = 10;
  const out: string[] = [];
  const st = { x: null as string | null, y: null as string | null, s: null as string | null, f: null as string | null, off: false };

  const comment = (label: string) => {
    let text = label.replace(/[\r\n]+/g, ' ');
    if (d.comment === 'parentheses') text = text.replace(/[()]/g, '');
    text = text.trim();
    if (d.max_line_len > 0) text = text.slice(0, Math.max(d.max_line_len - 3, 0)).trimEnd();
    return d.comment === 'parentheses' ? `(${text})` : `; ${text}`;
  };
  const hot = () => !st.off && st.s !== null && st.s !== '0';
  /** Prepara un G0: devuelve si lleva `S0`; con potencia en línea aparte emite el apagado. */
  const prepareTravel = (): boolean => {
    if (!d.s_inline) {
      if (!d.g0_laser_off && !st.off && st.s !== null && d.laser_off !== '') {
        out.push(d.laser_off);
        st.off = true;
      }
      st.s = null;
      return false;
    }
    return d.travel_s0 && hot();
  };
  const motion = (g: string, x: number, y: number, s: string | null, feed: number | null) => {
    const xs = num(x, d.decimals);
    const ys = num(y, d.decimals);
    const dx = st.x !== xs;
    const dy = st.y !== ys;
    if (!dx && !dy) return;
    let cmd = g;
    if (dx) { cmd += ` X${xs}`; st.x = xs; }
    if (dy) { cmd += ` Y${ys}`; st.y = ys; }
    if (s !== null && st.s !== s) { cmd += ` S${s}`; st.s = s; }
    if (feed !== null) {
      const f = num(feed, d.decimals);
      if (st.f !== f) { cmd += ` F${f}`; st.f = f; }
    }
    out.push(cmd);
  };
  const travel = (x: number, y: number) => {
    if (st.x === num(x, d.decimals) && st.y === num(y, d.decimals)) return;
    const s0 = prepareTravel();
    motion('G0', x, y, s0 ? '0' : null, d.feed_on_g0 && device.rapid_rate > 0 ? device.rapid_rate : null);
  };
  const cut = (x: number, y: number) => {
    const s = sValue(power, device.s_max, d.s_decimals);
    if (d.s_inline) return motion('G1', x, y, s, speed);
    if (st.x !== num(x, d.decimals) || st.y !== num(y, d.decimals)) {
      if (!(st.s === s || (s === '0' && st.s === null))) {
        if (d.power_cmd !== '') out.push(d.power_cmd.replace('{s}', s));
        st.s = s;
        st.off = false;
      }
    }
    motion('G1', x, y, null, speed);
  };

  out.push(comment('Generado por Tlaku'), 'G90', 'G21');
  const script = (text: string) => out.push(...text.trim().split('\n').map((l) => l.trimEnd()));
  if (device.start_gcode.trim() !== '') script(device.start_gcode);
  const mode = device.power_mode === 'dynamic' ? (d.dynamic_on ?? d.constant_on) : d.constant_on;
  if (mode !== '') {
    out.push(mode);
    st.s = '0';
  }
  out.push(comment(opts.label));
  const air = d.air_on !== null && d.air_off !== null;
  if (air) out.push(d.air_on!);
  travel(origin, origin);
  cut(origin + side, origin);
  cut(origin + side, origin + side);
  cut(origin, origin + side);
  cut(origin, origin);
  if (air) out.push(d.air_off!);
  if (!st.off && d.laser_off !== '') out.push(d.laser_off);
  st.off = true;
  if (device.return_to_origin) {
    let cmd = 'G0 X0 Y0';
    if (d.feed_on_g0 && device.rapid_rate > 0) {
      const f = num(device.rapid_rate, d.decimals);
      if (st.f !== f) cmd += ` F${f}`;
    }
    out.push(cmd);
  }
  if (device.end_gcode.trim() !== '') script(device.end_gcode);
  if (d.end_sync !== null) out.push(d.end_sync);
  return out;
}

// ---------------------------------------------------------------------------
// Rotativo: modelos conocidos
// ---------------------------------------------------------------------------

/** Accesorio con cifras de fábrica conocidas (docs/hardware/taller.md, 1.2, y falcon2.md). */
export interface RotaryPreset {
  /** Texto: `devices.rotary.preset.<id>`. */
  id: 'xtool_ra2_roller' | 'xtool_ra2_chuck' | 'creality_chuck';
  kind: RotaryKind;
  mm_per_rotation: number;
  /** Solo con rodillo. */
  roller_diameter?: number;
}

export const ROTARY_PRESETS: readonly RotaryPreset[] = [
  { id: 'xtool_ra2_roller', kind: 'roller', mm_per_rotation: 32, roller_diameter: 17 },
  { id: 'xtool_ra2_chuck', kind: 'chuck', mm_per_rotation: 128 },
  { id: 'creality_chuck', kind: 'chuck', mm_per_rotation: 40 },
];

/** Rotativo con el modelo aplicado; conserva si está activo, el diámetro de la pieza y el espejo. */
export function applyRotaryPreset(r: RotarySettings, preset: RotaryPreset): RotarySettings {
  return {
    ...r,
    kind: preset.kind,
    mm_per_rotation: preset.mm_per_rotation,
    roller_diameter: preset.roller_diameter ?? r.roller_diameter,
  };
}

/** Modelo cuyos números coinciden con el rotativo, para mostrarlo elegido en el selector. */
export function matchRotaryPreset(r: RotarySettings): RotaryPreset | null {
  return (
    ROTARY_PRESETS.find((p) => p.kind === r.kind && p.mm_per_rotation === r.mm_per_rotation && (p.kind === 'chuck' || p.roller_diameter === r.roller_diameter)) ?? null
  );
}

/** Longitud de una vuelta de la pieza sobre su superficie (la línea de la prueba), en mm. */
export function rotaryCircumference(r: Pick<RotarySettings, 'object_diameter'>): number | null {
  return Number.isFinite(r.object_diameter) && r.object_diameter > 0 ? Math.PI * r.object_diameter : null;
}
