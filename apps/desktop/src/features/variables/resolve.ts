import type { CsvTable, VariableText } from '@/api/types';
import {
  DEFAULT_DATE_FORMAT,
  DEFAULT_TIME_FORMAT,
  formatDate,
  formatSerial,
  parseSerialOption,
  type Lang,
} from './format';
import { isTemplate, parseTemplate, type Token } from './syntax';

/** Todo lo que hace falta para sustituir las variables de un texto. */
export interface VarContext {
  /** Próximo número de serie. */
  serial: number;
  serialStep: number;
  serialDigits: number;
  /** El CSV leído; `null` si el documento no tiene CSV o aún no se cargó. */
  csv: CsvTable | null;
  /** Fila actual del CSV (0 = la primera de datos). */
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
  | 'badFormat'; // opción de serie que no se entiende

export interface Issue {
  code: IssueCode;
  /** El `{...}` tal como está escrito. */
  raw: string;
  /** Dato extra para el mensaje: columna, número de fila... */
  detail: string;
}

export interface Resolved {
  text: string;
  issues: Issue[];
}

/** Contexto a partir de los ajustes del documento. */
export function makeContext(
  vt: VariableText | undefined,
  csv: CsvTable | null,
  now: Date,
  lang: Lang,
): VarContext {
  const v = vt ?? { serial: 1, serial_step: 1, serial_digits: 0, csv_path: null, csv_row: 0, advance_after_job: true };
  return {
    serial: Number(v.serial),
    serialStep: Number(v.serial_step),
    serialDigits: v.serial_digits,
    csv,
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

function resolveVar(tok: Extract<Token, { kind: 'var' }>, ctx: VarContext): { value: string } | { issue: Issue } {
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
      if (!ctx.csv) return fail('noCsv');
      const row = ctx.csvRow + tok.offset;
      if (row < 0 || row >= ctx.csv.rows.length) return fail('rowEnd', String(row + 1));
      if (tok.name === 'row') return { value: String(row + 1) };
      const col = csvColumnIndex(ctx.csv, tok.arg);
      if (col < 0) return fail('noColumn', tok.arg.trim());
      return { value: ctx.csv.rows[row]?.[col] ?? '' };
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
  let text = '';
  for (const tok of parseTemplate(template)) {
    if (tok.kind === 'text') text += tok.text;
    else if (tok.kind === 'bad') {
      text += tok.raw;
      issues.push({ code: tok.reason, raw: tok.raw, detail: '' });
    } else {
      const r = resolveVar(tok, ctx);
      if ('value' in r) text += r.value;
      else {
        text += tok.raw;
        issues.push(r.issue);
      }
    }
  }
  return { text, issues };
}
