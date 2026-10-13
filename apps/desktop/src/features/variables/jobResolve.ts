import { getApi, type LaserApi } from '@/api';
import type { CsvTable, DesignObject, Document, FontFamily, JobArgs } from '@/api/types';
import { effectiveFont, loadFonts } from '@/features/text/fonts';
import { outlineMatches, rememberOutline } from '@/features/text/outlineSigs';
import { isTextObject, resolveText, type TextDesignObject } from '@/features/text/signature';
import { getLanguage, t } from '@/i18n';
import { issueMessage } from './messages';
import { makeContext, type Issue, type VarContext } from './resolve';
import { csvColumnsUsed, isTemplate, usesCsv } from './syntax';

/** Algo impide resolver los textos variables: el trabajo no sale (nunca se graba un texto a medias). */
export class VariableError extends Error {
  constructor(
    message: string,
    readonly issues: Issue[] = [],
  ) {
    super(message);
    this.name = 'VariableError';
  }
}

export interface ResolveDeps {
  api?: () => Pick<LaserApi, 'textOutline' | 'readCsv'>;
  fonts?: () => Promise<FontFamily[]>;
  now?: () => Date;
}

const templated = (o: DesignObject): o is TextDesignObject => isTextObject(o) && isTemplate(o.kind.text);

/** ¿Algún texto del documento lleva llaves (variables o llaves escapadas)? Si no, no hay nada que resolver. */
export function hasTemplates(doc: Document): boolean {
  return doc.objects.some(templated);
}

/** Contexto del trabajo: el CSV se vuelve a leer del disco (puede haber cambiado desde que se abrió el diálogo). */
async function jobContext(doc: Document, texts: TextDesignObject[], deps: Required<ResolveDeps>): Promise<VarContext> {
  const vt = doc.variable_text;
  let csv: CsvTable | null = null;
  const usingCsv = texts.some((o) => usesCsv(o.kind.text));
  if (vt?.csv_path && usingCsv) {
    const required = [...new Set(texts.flatMap((o) => csvColumnsUsed(o.kind.text)))];
    try {
      csv = await deps.api().readCsv({ path: vt.csv_path, required });
    } catch (e) {
      throw new VariableError(t('variables.job.csvFailed', { message: String(e instanceof Error ? e.message : e) }));
    }
  }
  return makeContext(vt, csv, deps.now(), getLanguage());
}

/**
 * Documento listo para enviar, previsualizar o exportar: cada texto con variables se resuelve con
 * los valores de ahora y su contorno se regenera con el motor de texto (`textOutline`). El resto
 * del documento va igual. Si no hay textos con variables devuelve el mismo documento. Falla con
 * `VariableError` si una variable no se puede resolver.
 *
 * Se hace aquí, en la interfaz, y no en el pipeline: el motor del trabajo solo conoce contornos
 * (nunca fuentes) y la app ya genera contornos con `text_outline`, con su biblioteca de fuentes.
 * El documento guardado no se toca: conserva las plantillas.
 */
export async function resolveDocForOutput(doc: Document, deps: ResolveDeps = {}): Promise<Document> {
  const texts = doc.objects.filter(templated);
  if (texts.length === 0) return doc;
  const d: Required<ResolveDeps> = {
    api: deps.api ?? getApi,
    fonts: deps.fonts ?? loadFonts,
    now: deps.now ?? (() => new Date()),
  };
  const ctx = await jobContext(doc, texts, d);

  // Primero se juntan todos los problemas, para avisarlos de una vez.
  const resolved = new Map<string, { text: string; sig: string; path: ReturnType<typeof resolveText> }>();
  const problems: Issue[] = [];
  for (const o of texts) {
    const r = resolveText(o, doc, ctx);
    if (!r.ok) throw new VariableError(t('text.onPath.missing'));
    problems.push(...(r.issues ?? []));
    resolved.set(o.id, { text: r.text ?? o.kind.text, sig: r.sig, path: r });
  }
  if (problems.length > 0) {
    const lines = [...new Set(problems.map(issueMessage))];
    throw new VariableError(t('variables.job.blocked', { problems: lines.join(' ') }), problems);
  }

  let families: FontFamily[] = [];
  try {
    families = await d.fonts();
  } catch {
    // Sin lista de fuentes se respeta la familia pedida y el motor dirá si no existe.
  }
  const outlines = new Map<string, DesignObject['kind']>();
  for (const o of texts) {
    const info = resolved.get(o.id)!;
    if (outlineMatches(o.kind.outline, info.sig)) continue;
    const r = info.path;
    const font = effectiveFont(families, o.kind.font_family);
    try {
      const out = await d.api().textOutline({
        text: { ...o.kind, text: info.text, font_family: font.family, outline: [] },
        path: r.ok ? r.path : null,
      });
      rememberOutline(out.paths, info.sig);
      outlines.set(o.id, { ...o.kind, outline: out.paths });
    } catch (e) {
      throw new VariableError(t('variables.job.outlineFailed', { text: info.text, message: String(e instanceof Error ? e.message : e) }));
    }
  }
  if (outlines.size === 0) return doc;
  return { ...doc, objects: doc.objects.map((o) => (outlines.has(o.id) ? { ...o, kind: outlines.get(o.id)! } : o)) };
}

/** `args` con el documento ya resuelto (ver `resolveDocForOutput`). */
export async function resolveJobArgs<A extends Pick<JobArgs, 'doc'>>(args: A, deps?: ResolveDeps): Promise<A> {
  const doc = await resolveDocForOutput(args.doc, deps);
  return doc === args.doc ? args : { ...args, doc };
}
