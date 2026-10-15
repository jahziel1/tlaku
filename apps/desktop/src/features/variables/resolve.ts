import type { CsvTable, ShortRows, VariableText } from '@/api/types';
import { variableTextOf } from './defaults';
import {
  DEFAULT_DATE_FORMAT,
  DEFAULT_TIME_FORMAT,
  formatDate,
  formatSerial,
  parseSerialOption,
  type Lang,
} from './format';
import { isTemplate, parseTemplate, type Token } from './syntax';

/** Una fuente de datos ya leída. */
export interface SourceCtx {
  /** Nombre en minúsculas (así se busca). */
  name: string;
  /** Nombre como lo escribió el usuario (para los mensajes). */
  label: string;
  /** Ruta del CSV (las rutas de imágenes relativas parten de su carpeta). */
  path: string;
  table: CsvTable | null;
  onShort: ShortRows;
}

/** Todo lo que hace falta para sustituir las variables de un texto. */
export interface VarContext {
  /** Próximo número de serie. */
  serial: number;
  serialStep: number;
  serialDigits: number;
  /** El CSV principal leído; `null` si el documento no tiene CSV o aún no se cargó. */
  csv: CsvTable | null;
  /**
   * Todas las fuentes de datos, la principal primero. Sin esto (contextos armados a mano) solo
   * existe la principal, `csv`. Las fuentes secundarias usan la misma fila que la principal; si
   * son más cortas, se repiten o bloquean según su ajuste.
   */
  sources?: SourceCtx[];
  /** Fila actual de la fuente principal (0 = la primera de datos). */
  csvRow: number;
  now: Date;
  lang: Lang;
}

/** Problema al resolver un `{...}`: el texto de esa variable se deja escrito y se avisa. */
export type IssueCode =
  | 'unknown' //   nombre que no existe
  | 'unclosed' //  falta la llave de cierre
  | 'empty' //     `{}`
  | 'noCsv' //     usa el CSV pero el documento no tiene o no se pudo leer
  | 'noColumn' //  la columna no existe
  | 'noColumnArg' // `{csv}` sin columna
  | 'rowEnd' //    la fila pedida no existe (se acabó el CSV)
  | 'badFormat' // opción de serie que no se entiende
  | 'noSource' //  `{csv.fuente:...}` con una fuente que el proyecto no tiene
  | 'emptySource' // la fuente elegida no tiene filas
  | 'sourceRowEnd' // la fuente elegida es más corta y está en «bloquear»
  | 'emptyCell' // aviso: la celda de una columna está vacía
  | 'imageEmpty' // imagen variable: la celda de la ruta está vacía
  | 'imageMissing' // imagen variable: el archivo no existe
  | 'codeEmpty' // código variable: el contenido de la fila está vacío
  | 'codeFailed'; // código variable: el generador rechazó el contenido

export interface Issue {
  code: IssueCode;
  /** El `{...}` tal como está escrito. */
  raw: string;
  /** Dato extra para el mensaje: columna, número de fila... */
  detail: string;
  /** Fila del CSV (desde 1) a la que corresponde, si se conoce. */
  row?: number;
}

export interface Resolved {
  text: string;
  issues: Issue[];
  /** Variables de CSV que salieron vacías (celda vacía): no es un error, pero se avisa. */
  empties?: Issue[];
}

/** Contexto a partir de los ajustes del documento. */
export function makeContext(
  vt: VariableText | undefined,
  csv: CsvTable | null,
  now: Date,
  lang: Lang,
  /** Tablas de las fuentes secundarias, por nombre en minúsculas. */
  extra: Record<string, CsvTable | null> = {},
): VarContext {
  const v = variableTextOf({ variable_text: vt });
  const sources: SourceCtx[] = v.sources.map((s, i) => ({
    name: s.name.toLowerCase(),
    label: s.name,
    path: s.path,
    table: i === 0 ? csv : (extra[s.name.toLowerCase()] ?? null),
    onShort: s.on_short,
  }));
  return {
    serial: Number(v.serial),
    serialStep: Number(v.serial_step),
    serialDigits: v.serial_digits,
    csv,
    sources,
    csvRow: v.csv_row,
    now,
    lang,
  };
}

/** Posición de la columna por nombre (exacto o sin mayúsculas) o por número desde 1. */
export function csvColumnIndex(csv: CsvTable, arg: string): number {
  const name = arg.trim();
  const exact = csv.headers.indexOf(name);
  if (exact >= 0) return exact;
  const lower = name.toLowerCase();
  const loose = csv.headers.findIndex((h) => h.toLowerCase() === lower);
  if (loose >= 0) return loose;
  if (/^\d+$/.test(name)) {
    const n = Number(name);
    if (n >= 1 && n <= csv.headers.length) return n - 1;
  }
  return -1;
}

function resolveVar(tok: Extract<Token, { kind: 'var' }>, ctx: VarContext): { value: string; empty?: boolean; row?: number } | { issue: Issue } {
  const fail = (code: Issue['code'], detail = ''): { issue: Issue } => ({ issue: { code, raw: tok.raw, detail } });
  switch (tok.name) {
    case 'serial': {
      const opt = parseSerialOption(tok.arg);
      if (!opt) return fail('badFormat', tok.arg);
      return { value: formatSerial(ctx.serial + ctx.serialStep * tok.offset, opt.digits ?? ctx.serialDigits, opt.hex) };
    }
    case 'date':
      return { value: formatDate(ctx.now, tok.arg.trim() === '' ? DEFAULT_DATE_FORMAT : tok.arg, ctx.lang) };
    case 'time':
      return { value: formatDate(ctx.now, tok.arg.trim() === '' ? DEFAULT_TIME_FORMAT : tok.arg, ctx.lang) };
    case 'copy':
      return { value: String(1 + tok.offset) };
    case 'csv':
    case 'row': {
      if (tok.name === 'csv' && tok.arg.trim() === '') return fail('noColumnArg');
      if (!ctx.csv) return fail('noCsv', tok.source);
      const row = ctx.csvRow + tok.offset;
      // La fuente principal manda: pasada su última fila se acabaron los datos para todas.
      if (row < 0 || row >= ctx.csv.rows.length) return fail('rowEnd', String(row + 1));
      let table: CsvTable = ctx.csv;
      let used = row;
      if (tok.source !== '') {
        const idx = (ctx.sources ?? []).findIndex((s) => s.name === tok.source.toLowerCase());
        const src = (ctx.sources ?? [])[idx];
        if (!src) return fail('noSource', tok.source);
        if (idx > 0) {
          if (!src.table) return fail('noCsv', src.label);
          table = src.table;
          const n = table.rows.length;
          if (n === 0) return fail('emptySource', src.label);
          if (row >= n) {
            if (src.onShort === 'block') return fail('sourceRowEnd', `${src.label} ${row + 1}`);
            used = row % n;
          }
        }
      }
      if (tok.name === 'row') return { value: String(used + 1) };
      const col = csvColumnIndex(table, tok.arg);
      if (col < 0) return fail('noColumn', tok.arg.trim());
      return { value: table.rows[used]?.[col] ?? '', empty: (table.rows[used]?.[col] ?? '').trim() === '', row: row + 1 };
    }
  }
}

/**
 * Sustituye las variables de la plantilla. Lo que no se puede resolver se deja escrito tal cual
 * (con sus llaves) y se anota en `issues`; las llaves escapadas (`{{`, `}}`) salen como `{` y `}`.
 */
export function resolveTemplate(template: string, ctx: VarContext): Resolved {
  if (!isTemplate(template)) return { text: template, issues: [] };
  const issues: Issue[] = [];
  const empties: Issue[] = [];
  let text = '';
  for (const tok of parseTemplate(template)) {
    if (tok.kind === 'text') text += tok.text;
    else if (tok.kind === 'bad') {
      text += tok.raw;
      issues.push({ code: tok.reason, raw: tok.raw, detail: '' });
    } else {
      const r = resolveVar(tok, ctx);
      if ('value' in r) {
        text += r.value;
        if (r.empty) empties.push({ code: 'emptyCell', raw: tok.raw, detail: tok.arg.trim(), row: r.row });
      } else {
        text += tok.raw;
        issues.push(r.issue);
      }
    }
  }
  return empties.length > 0 ? { text, issues, empties } : { text, issues };
}
