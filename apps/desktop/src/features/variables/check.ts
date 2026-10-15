import type { LaserApi } from '@/api';
import type { CsvTable, Document, ImageProbe } from '@/api/types';
import { getLanguage, t } from '@/i18n';
import { variableTextOf } from './defaults';
import { csvBaseDir, resolveImagePath } from './imageVar';
import { issueMessage } from './messages';
import { makeContext, resolveTemplate, type VarContext } from './resolve';
import { parseTemplate, serializeTemplate, variablesOf } from './syntax';
import { isVariableCode, isVariableImage } from './varObjects';
import { isTextObject } from '@/features/text/signature';

/** Un problema de datos de una fila: lo que falta o no se puede usar. */
export interface RowProblem {
  /** Fila del CSV principal, desde 1. */
  row: number;
  /** Mensajes ya escritos para el usuario. */
  messages: string[];
}

export interface DataCheck {
  /** Filas revisadas (las de la fuente principal). */
  rows: number;
  /** Filas con problemas, en orden; se corta en `MAX_LISTED_ROWS`. */
  problems: RowProblem[];
  /** Total de filas con problemas (puede ser mayor que `problems.length`). */
  problemRows: number;
  /** Problemas que no dependen de la fila (una columna que no existe, una fuente desconocida...). */
  general: string[];
}

/** Filas con problemas que se listan; el resto solo se cuenta. */
export const MAX_LISTED_ROWS = 200;
/** Rutas de imagen que se sondean por llamada al motor. */
const PROBE_CHUNK = 500;

/** La plantilla sin desplazamientos (`@N`): se revisa cada fila una vez, no una por copia. */
function withoutOffsets(template: string): string {
  return serializeTemplate(parseTemplate(template).map((tok) => (tok.kind === 'var' ? { ...tok, offset: 0 } : tok)));
}

/** Códigos de problema que no cambian de una fila a otra: se avisan una sola vez. */
const GENERAL = new Set(['noSource', 'noCsv', 'noColumn', 'noColumnArg', 'emptySource', 'unknown', 'unclosed', 'empty', 'badFormat']);

interface Templates {
  texts: string[];
  images: string[];
  codes: string[];
}

function templatesOf(doc: Pick<Document, 'objects'>): Templates {
  const texts = new Set<string>();
  const images = new Set<string>();
  const codes = new Set<string>();
  for (const o of doc.objects) {
    if (isTextObject(o)) {
      if (variablesOf(o.kind.text).some((v) => v.name === 'csv' || v.name === 'row')) texts.add(withoutOffsets(o.kind.text));
    } else if (isVariableImage(o)) images.add(withoutOffsets(o.kind.variable.source));
    else if (isVariableCode(o)) codes.add(withoutOffsets(o.kind.code.data));
  }
  return { texts: [...texts], images: [...images], codes: [...codes] };
}

/**
 * Revisa los datos de TODAS las filas contra lo que usa el diseño, sin enviar nada:
 * celdas vacías en las columnas que se usan, columnas o fuentes que no existen, fuentes
 * secundarias que se acaban (si están en «bloquear»), rutas de imagen vacías o de archivos que no
 * existen, y contenidos vacíos de códigos. Las filas con problemas se devuelven para listarlas.
 * Cada plantilla se revisa sin sus desplazamientos `@N`, así que una cuadrícula de «una copia por fila»
 * no multiplica los avisos.
 */
export async function checkData(
  doc: Document,
  tables: { main: CsvTable | null; extra: Record<string, CsvTable | null> },
  api: Pick<LaserApi, 'probeImages'>,
  now: Date = new Date(),
): Promise<DataCheck> {
  const result: DataCheck = { rows: 0, problems: [], problemRows: 0, general: [] };
  const vt = variableTextOf(doc);
  const tpl = templatesOf(doc);
  const main = tables.main;
  if (!main || (tpl.texts.length === 0 && tpl.images.length === 0 && tpl.codes.length === 0)) return result;
  result.rows = main.rows.length;

  const base: VarContext = makeContext(vt, main, now, getLanguage(), tables.extra);
  const byRow = new Map<number, Set<string>>();
  const add = (row: number, message: string) => {
    const set = byRow.get(row) ?? new Set<string>();
    set.add(message);
    byRow.set(row, set);
  };
  const general = new Set<string>();
  const imagePaths: { row: number; path: string }[] = [];

  for (let r = 0; r < main.rows.length; r++) {
    const ctx = { ...base, csvRow: r };
    const check = (template: string, kind: 'text' | 'image' | 'code') => {
      const res = resolveTemplate(template, ctx);
      for (const issue of res.issues) {
        if (GENERAL.has(issue.code)) general.add(issueMessage(issue));
        else add(r + 1, issueMessage(issue));
      }
      if (res.issues.length > 0) return;
      for (const e of res.empties ?? []) add(r + 1, t('variables.check.emptyCell', { column: e.detail || e.raw }));
      if (kind === 'code' && res.text === '') add(r + 1, t('variables.check.codeEmpty', { raw: template }));
      if (kind === 'image') {
        const dir = csvBaseDir(template, ctx);
        const path = resolveImagePath(res.text, dir);
        if (path === '') add(r + 1, t('variables.check.imageEmpty', { raw: template }));
        else imagePaths.push({ row: r + 1, path });
      }
    };
    tpl.texts.forEach((x) => check(x, 'text'));
    tpl.images.forEach((x) => check(x, 'image'));
    tpl.codes.forEach((x) => check(x, 'code'));
  }

  // Archivos de imagen: se sondean los distintos, por tandas.
  const unique = [...new Set(imagePaths.map((p) => p.path))];
  const missing = new Map<string, ImageProbe>();
  for (let i = 0; i < unique.length; i += PROBE_CHUNK) {
    const chunk = unique.slice(i, i + PROBE_CHUNK);
    const probes = await api.probeImages({ paths: chunk });
    probes.forEach((p, k) => {
      if (!p.ok) missing.set(chunk[k]!, p);
    });
  }
  for (const p of imagePaths) if (missing.has(p.path)) add(p.row, t('variables.check.imageMissing', { path: p.path }));

  const rowsWithProblems = [...byRow.keys()].sort((a, b) => a - b);
  result.problemRows = rowsWithProblems.length;
  result.problems = rowsWithProblems.slice(0, MAX_LISTED_ROWS).map((row) => ({ row, messages: [...byRow.get(row)!] }));
  result.general = [...general];
  return result;
}
