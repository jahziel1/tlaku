/**
 * Reglas de seguridad de la consola. Es un port 1:1 de `crates/tlaku-cli/src/link.rs`
 * (`prepare_line`, `classify`, `parse_command`): lo que el CLI pide confirmar, la app también,
 * y se envía exactamente la línea normalizada que se le muestra al usuario.
 */

/** Qué tan delicado es un comando; decide si se pide confirmación y qué se advierte. */
export type Risk =
  /** Solo lectura (o apaga el láser): se envía sin preguntar. */
  | 'safe'
  /** `$X`: desbloquea sin hacer home. */
  | 'unlock'
  /** `$H`: la máquina se mueve hasta los finales de carrera. */
  | 'home'
  /** G0/G1/G2/G3, `$J=`, G28...: mueve la máquina. */
  | 'moves'
  /** M3/M4 con potencia: enciende el láser. */
  | 'laser'
  /** `$N=valor`: cambia un ajuste guardado en la controladora. */
  | 'setting'
  /** Cualquier otro: no está en la lista de comandos de solo lectura. */
  | 'other';

export const REALTIME_NAMES = ['status', 'hold', 'resume', 'reset'] as const;
export type RealtimeName = (typeof REALTIME_NAMES)[number];

/** Byte que GRBL recibe por cada comando de tiempo real (solo para mostrarlo). */
export const REALTIME_BYTES: Record<RealtimeName, number> = { status: 0x3f, hold: 0x21, resume: 0x7e, reset: 0x18 };

export type PreparedCommand =
  | { kind: 'realtime'; name: RealtimeName; risk: 'safe'; shown: string }
  | { kind: 'line'; text: string; risk: Risk; shown: string };

export type CommandError =
  | { code: 'realtimeChar'; ch: string }
  | { code: 'empty' }
  | { code: 'tooLong'; text: string; max: number };

/** ¿Hay que pedir confirmación antes de enviarlo? Todo lo que no sea de solo lectura. */
export function needsConfirmation(risk: Risk): boolean {
  return risk !== 'safe';
}

/**
 * Quita comentarios (`;...` y `(...)`) y espacios. Devuelve `ch` si queda un carácter que GRBL
 * interpretaría como tiempo real (`?`, `!`, `~`), uno de control o no ASCII.
 */
export function prepareLine(raw: string): { ok: true; text: string } | { ok: false; ch: string } {
  let out = '';
  let inParen = false;
  for (const c of raw) {
    if (c === ';' && !inParen) break;
    if (c === '(') {
      inParen = true;
      continue;
    }
    if (c === ')') {
      inParen = false;
      continue;
    }
    if (inParen || /\s/u.test(c)) continue;
    if (c === '?' || c === '!' || c === '~') return { ok: false, ch: c };
    const code = c.codePointAt(0) ?? 0;
    if (code > 0x7f || code < 0x20 || code === 0x7f) return { ok: false, ch: c };
    out += c;
  }
  return { ok: true, text: out };
}

const isAsciiLetter = (c: string) => /^[A-Za-z]$/.test(c);
const isNumChar = (c: string) => /^[0-9.+-]$/.test(c);

/** Palabras G-code (letra y valor) de una línea ya sin espacios. */
export function gcodeWords(text: string): [string, number][] {
  const words: [string, number][] = [];
  for (let i = 0; i < text.length; ) {
    const letter = text[i]!;
    i++;
    if (!isAsciiLetter(letter)) continue;
    let num = '';
    while (i < text.length && isNumChar(text[i]!)) num += text[i++];
    // `Number('')` es 0: una letra sin número no es una palabra (en Rust, `"".parse()` falla).
    const v = num === '' ? NaN : Number(num);
    if (Number.isFinite(v)) words.push([letter, v]);
  }
  return words;
}

/** Clasifica una línea ya normalizada (mayúsculas, sin espacios ni comentarios). */
export function classify(text: string): Risk {
  switch (text) {
    case '$I':
    case '$$':
    case '$#':
    case '$G':
    case '$N':
    case 'M5':
      return 'safe';
    case '$X':
      return 'unlock';
    case '$H':
      return 'home';
  }
  if (text.startsWith('$J=')) return 'moves';
  if (text.startsWith('$')) {
    let rest = text.slice(1);
    if (rest.startsWith('N')) rest = rest.slice(1);
    const digits = /^\d*/.exec(rest)![0].length;
    return digits > 0 && rest.slice(digits).startsWith('=') ? 'setting' : 'other';
  }
  const words = gcodeWords(text);
  const spindleOn =
    words.some(([l, v]) => l === 'M' && (v === 3 || v === 4)) &&
    words.filter(([l]) => l === 'S').every(([, s]) => s > 0);
  const moves = words.some(
    ([l, v]) => l === 'G' && ([0, 1, 2, 3, 28, 30].includes(v) || (v >= 38 && v < 39)),
  );
  if (spindleOn) return 'laser';
  if (moves) return 'moves';
  return 'other';
}

function realtimeByName(name: string): RealtimeName | null {
  switch (name.toLowerCase()) {
    case 'status':
    case '?':
      return 'status';
    case 'hold':
      return 'hold';
    case 'resume':
      return 'resume';
    case 'reset':
      return 'reset';
    default:
      return null;
  }
}

const hex = (n: number) => n.toString(16).toUpperCase().padStart(2, '0');

/**
 * Interpreta lo que se escribe en la consola: nombre de tiempo real o línea de GRBL en
 * mayúsculas y sin espacios, tal como se enviará. `rxBuffer` es el búfer de la controladora.
 */
export function parseCommand(raw: string, rxBuffer: number): { ok: true; cmd: PreparedCommand } | { ok: false; error: CommandError } {
  const rt = realtimeByName(raw.trim());
  if (rt) return { ok: true, cmd: { kind: 'realtime', name: rt, risk: 'safe', shown: `${rt} (byte 0x${hex(REALTIME_BYTES[rt])})` } };
  const prepared = prepareLine(raw);
  if (!prepared.ok) return { ok: false, error: { code: 'realtimeChar', ch: prepared.ch } };
  const text = prepared.text.toUpperCase();
  if (text === '') return { ok: false, error: { code: 'empty' } };
  // Rust cuenta bytes; el texto ya es ASCII, así que longitud y bytes coinciden.
  if (text.length + 1 > rxBuffer) return { ok: false, error: { code: 'tooLong', text, max: rxBuffer } };
  return { ok: true, cmd: { kind: 'line', text, risk: classify(text), shown: text } };
}

/** Línea fija de la app (`$X`, `$H`, `$$`...) con su clasificación, sin pasar por la consola. */
export function fixedLine(text: string): PreparedCommand {
  return { kind: 'line', text, risk: classify(text), shown: text };
}
