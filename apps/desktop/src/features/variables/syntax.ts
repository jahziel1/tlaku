/**
 * Sintaxis de las variables dentro de un texto (sintaxis propia de Tlaku).
 *
 *   {nombre}              variable sin opciones
 *   {nombre:opción}       con formato o argumento
 *   {nombre@N}            desplazada N pasos (N entero, puede ser negativo)
 *   {nombre:opción@N}     las dos cosas
 *   {{  y  }}             llaves literales ({ y })
 *
 * Variables (cada una con su nombre en español y en inglés; no importan las mayúsculas):
 *
 *   serie | serial        número de serie del documento. Opción: dígitos (`6`), `hex` o `HEX`,
 *                         con dígitos opcionales (`hex4`). @N suma N pasos de la serie.
 *   csv                   dato de la fila actual del CSV. Opción obligatoria: el nombre de la
 *                         columna (`{csv:Nombre}`) o su número desde 1 (`{csv:2}`).
 *                         @N toma la fila actual + N.
 *   fila | row            número de la fila actual del CSV, desde 1 (@N igual que csv).
 *   fecha | date          fecha de hoy. Opción: formato (por defecto `dd/MM/yyyy`).
 *   hora | time           hora actual. Opción: formato (por defecto `HH:mm`).
 *   copia | copy          número de copia, desde 1. @N lo desplaza (`{copia@2}` vale 3).
 *
 * Formatos de fecha y hora: d dd ddd dddd (día, abreviado, nombre), M MM MMM MMMM (mes),
 * yy yyyy o aa aaaa (año), H HH (hora 0-23), h hh (hora 1-12), m mm (minutos; en una fecha sin
 * hora, `mm` es el mes), s ss, AP o ap (AM/PM). El texto entre comillas simples sale tal cual:
 * `{fecha:d 'de' MMMM}`.
 *
 * Un `{...}` que no se entiende no se sustituye: se queda escrito y se avisa.
 */

export type VarName = 'serial' | 'csv' | 'row' | 'date' | 'time' | 'copy';

/** Nombres aceptados (en minúsculas) y la variable a la que corresponde cada uno. */
export const VAR_ALIASES: Record<string, VarName> = {
  serie: 'serial',
  serial: 'serial',
  csv: 'csv',
  fila: 'row',
  row: 'row',
  fecha: 'date',
  date: 'date',
  hora: 'time',
  time: 'time',
  copia: 'copy',
  copy: 'copy',
};

export type Token =
  | { kind: 'text'; text: string }
  | {
      kind: 'var';
      name: VarName;
      /** Nombre tal como lo escribió el usuario (se conserva al reescribir la plantilla). */
      alias: string;
      arg: string;
      offset: number;
      raw: string;
    }
  | { kind: 'bad'; reason: 'unknown' | 'unclosed' | 'empty'; raw: string };

/** Separa `nombre[:arg][@N]`. `null` si el contenido está vacío. */
function parseBody(body: string): { alias: string; arg: string; offset: number } | null {
  let offset = 0;
  let rest = body;
  const at = /@(-?\d+)\s*$/.exec(rest);
  if (at) {
    offset = Number(at[1]);
    rest = rest.slice(0, at.index);
  }
  const colon = rest.indexOf(':');
  const alias = (colon < 0 ? rest : rest.slice(0, colon)).trim();
  const arg = colon < 0 ? '' : rest.slice(colon + 1);
  if (alias === '') return null;
  return { alias, arg, offset };
}

/** Descompone una plantilla en texto, variables y trozos que no se entienden. */
export function parseTemplate(source: string): Token[] {
  const out: Token[] = [];
  let lit = '';
  const flush = () => {
    if (lit) out.push({ kind: 'text', text: lit });
    lit = '';
  };
  let i = 0;
  while (i < source.length) {
    const c = source[i]!;
    if (c === '{' && source[i + 1] === '{') {
      lit += '{';
      i += 2;
    } else if (c === '}' && source[i + 1] === '}') {
      lit += '}';
      i += 2;
    } else if (c === '{') {
      const end = source.indexOf('}', i + 1);
      if (end < 0) {
        flush();
        out.push({ kind: 'bad', reason: 'unclosed', raw: source.slice(i) });
        i = source.length;
        continue;
      }
      const raw = source.slice(i, end + 1);
      const body = parseBody(source.slice(i + 1, end));
      flush();
      if (!body) {
        out.push({ kind: 'bad', reason: 'empty', raw });
      } else {
        const name = VAR_ALIASES[body.alias.toLowerCase()];
        if (name) out.push({ kind: 'var', name, alias: body.alias, arg: body.arg, offset: body.offset, raw });
        else out.push({ kind: 'bad', reason: 'unknown', raw });
      }
      i = end + 1;
    } else {
      // Una `}` suelta es un carácter normal.
      lit += c;
      i += 1;
    }
  }
  flush();
  return out;
}

/** Escapa un texto literal para meterlo en una plantilla (`{` y `}` se duplican). */
export function escapeLiteral(text: string): string {
  return text.replace(/[{}]/g, (c) => c + c);
}

/** Escribe una variable con la sintaxis canónica de Tlaku. */
export function formatVar(alias: string, arg: string, offset: number): string {
  return `{${alias}${arg ? `:${arg}` : ''}${offset !== 0 ? `@${offset}` : ''}}`;
}

/** Vuelve a escribir la plantilla a partir de sus partes (lo que `parseTemplate` entendió). */
export function serializeTemplate(tokens: readonly Token[]): string {
  return tokens
    .map((t) => {
      if (t.kind === 'text') return escapeLiteral(t.text);
      if (t.kind === 'var') return formatVar(t.alias, t.arg, t.offset);
      return t.raw;
    })
    .join('');
}

/** ¿El texto usa llaves? Entonces es una plantilla y hay que resolverla (también por los escapes). */
export function isTemplate(text: string): boolean {
  return text.includes('{') || text.includes('}');
}

/** ¿Tiene al menos una variable (no solo llaves escapadas)? */
export function hasVariables(text: string): boolean {
  return isTemplate(text) && parseTemplate(text).some((t) => t.kind === 'var');
}

/** Variables de la plantilla, en orden. */
export function variablesOf(text: string): Extract<Token, { kind: 'var' }>[] {
  return parseTemplate(text).filter((t): t is Extract<Token, { kind: 'var' }> => t.kind === 'var');
}

/** Columnas del CSV que menciona la plantilla (nombres o números tal como se escribieron). */
export function csvColumnsUsed(text: string): string[] {
  const cols = variablesOf(text)
    .filter((v) => v.name === 'csv')
    .map((v) => v.arg.trim())
    .filter((a) => a !== '');
  return [...new Set(cols)];
}

/** ¿La plantilla necesita datos del CSV (`csv` o `fila`)? */
export function usesCsv(text: string): boolean {
  return variablesOf(text).some((v) => v.name === 'csv' || v.name === 'row');
}

/**
 * La plantilla para la copia número `k` (0 = la original): suma `k` al desplazamiento de las
 * variables que avanzan por copia (serie, csv, fila, copia); fecha y hora no cambian.
 */
export function shiftTemplate(text: string, k: number): string {
  if (k === 0 || !isTemplate(text)) return text;
  return serializeTemplate(
    parseTemplate(text).map((t) =>
      t.kind === 'var' && t.name !== 'date' && t.name !== 'time' ? { ...t, offset: t.offset + k } : t,
    ),
  );
}
