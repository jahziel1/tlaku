import type { DeviceProfile, Point, RotarySettings, ScanOffset } from '@/api/types';

/** Mismos valores que `RotarySettings::default()` del motor. */
export const DEFAULT_ROTARY: RotarySettings = {
  enabled: false,
  kind: 'roller',
  mm_per_rotation: 360,
  roller_diameter: 20,
  object_diameter: 80,
};

/** Perfil provisional mientras no se ha cargado ninguno (cama de 400 x 400 mm). */
export const FALLBACK_DEVICE: DeviceProfile = {
  id: 'predeterminado',
  name: 'Máquina GRBL',
  kind: 'grbl',
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
  rotary: { ...DEFAULT_ROTARY },
  scan_offsets: [],
};

/**
 * Completa un perfil que llega sin los campos de la Fase 4 (guardado antes de ellos) con los
 * valores por defecto. Devuelve el mismo objeto si ya estaba completo.
 */
export function normalizeDevice(device: Partial<DeviceProfile> & Pick<DeviceProfile, 'id' | 'name'>): DeviceProfile {
  const complete =
    typeof device.z_axis === 'boolean' &&
    Array.isArray(device.scan_offsets) &&
    device.rotary !== undefined &&
    (Object.keys(DEFAULT_ROTARY) as (keyof RotarySettings)[]).every((k) => k in device.rotary!);
  if (complete) return device as DeviceProfile;
  return {
    ...FALLBACK_DEVICE,
    ...device,
    z_axis: device.z_axis ?? false,
    rotary: { ...DEFAULT_ROTARY, ...device.rotary },
    scan_offsets: device.scan_offsets ?? [],
  };
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
  | 'scan_offsets';

export type DeviceIssue = 'required' | 'positive' | 'integer' | 'tooBig' | 'tooSmall' | 'duplicate';

const pos = (n: number) => Number.isFinite(n) && n > 0;

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
  const rowIssues = scanOffsetRowIssues(d.scan_offsets ?? []);
  const firstBad = rowIssues.find((i) => i !== null);
  if (firstBad) out.scan_offsets = firstBad;
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

type Frame = Pick<DeviceProfile, 'bed' | 'origin'>;

const fromRight = (o: DeviceProfile['origin']) => o === 'bottom_right' || o === 'top_right';
const fromTop = (o: DeviceProfile['origin']) => o === 'top_left' || o === 'top_right';

/**
 * Posición de trabajo de la máquina (el G-code crece hacia dentro de la cama desde la esquina
 * del perfil) a coordenadas del documento (origen abajo a la izquierda, Y hacia arriba).
 */
export function workToDoc(device: Frame, p: Point): Point {
  return [fromRight(device.origin) ? device.bed[0] - p[0] : p[0], fromTop(device.origin) ? device.bed[1] - p[1] : p[1]];
}

/** Inversa de `workToDoc`. */
export function docToWork(device: Frame, p: Point): Point {
  return workToDoc(device, p); // la transformación es su propia inversa
}

/**
 * Desplazamiento de pantalla/documento (X a la derecha, Y arriba) a desplazamiento de G-code
 * relativo (`$J=G91`): se invierte el eje que crece en sentido contrario al del documento.
 */
export function docDeltaToWork(device: Pick<DeviceProfile, 'origin'>, dx: number, dy: number): Point {
  return [fromRight(device.origin) ? -dx : dx, fromTop(device.origin) ? -dy : dy];
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
