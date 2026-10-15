import { getApi, type LaserApi } from '@/api';
import type { CodeResult, CsvTable, DesignObject, Document, FontFamily, ImageProbe, JobArgs } from '@/api/types';
import { effectiveFont, loadFonts } from '@/features/text/fonts';
import { outlineMatches, rememberOutline } from '@/features/text/outlineSigs';
import { isTextObject, resolveText, type TextDesignObject } from '@/features/text/signature';
import { getLanguage, t } from '@/i18n';
import { placeCodeResult } from './codeVar';
import { variableTextOf } from './defaults';
import { csvBaseDir, fitImageObject, resolveImagePath } from './imageVar';
import { issueMessage } from './messages';
import { makeContext, resolveTemplate, type Issue, type VarContext } from './resolve';
import { csvColumnsBySource, isTemplate, sourcesUsed, usesCsv, variablesOf } from './syntax';
import { codeSpecOf, variableCodes, variableImages, type VariableCodeObject, type VariableImageObject } from './varObjects';

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
  api?: () => Pick<LaserApi, 'textOutline' | 'readCsv' | 'probeImages' | 'generateCode'>;
  fonts?: () => Promise<FontFamily[]>;
  now?: () => Date;
  /** Avisos que no bloquean el envío (celdas vacías en los textos). Por omisión salen como aviso de la app. */
  warn?: (messages: string[]) => void;
}

const templated = (o: DesignObject): o is TextDesignObject => isTextObject(o) && isTemplate(o.kind.text);

/** ¿Algún objeto del documento lleva llaves o es una imagen o un código variable? Si no, no hay nada que resolver. */
export function hasTemplates(doc: Document): boolean {
  return doc.objects.some(templated) || variableImages(doc).length > 0 || variableCodes(doc).length > 0;
}

/** Cuántos avisos de celdas vacías se escriben antes de resumir el resto. */
const MAX_LISTED = 6;

/** Último aviso de celdas vacías: la vista previa resuelve muchas veces seguidas y no debe repetirlo. */
let lastWarned = { text: '', at: 0 };
const WARN_REPEAT_MS = 30_000;

async function defaultWarn(messages: string[]): Promise<void> {
  const text = messages.join('\n');
  const now = Date.now();
  if (text === lastWarned.text && now - lastWarned.at < WARN_REPEAT_MS) return;
  lastWarned = { text, at: now };
  const { useStore } = await import('@/store');
  for (const m of messages) useStore.getState().notify('warning', m);
}

/** Contexto del trabajo: los CSV que usan las plantillas se vuelven a leer del disco (pueden haber cambiado desde que se abrió el diálogo). */
async function jobContext(doc: Document, templates: string[], deps: Required<Omit<ResolveDeps, 'warn'>>): Promise<VarContext> {
  const vt = variableTextOf(doc);
  const main = vt.sources[0];
  const mainName = main?.name.toLowerCase();
  // Columnas que se piden por fuente (la clave `''` es la principal, también si se la nombra).
  const required = new Map<string, Set<string>>();
  for (const tpl of templates) {
    for (const [key, cols] of csvColumnsBySource(tpl)) {
      const k = key === mainName ? '' : key;
      const set = required.get(k) ?? new Set<string>();
      cols.forEach((c) => set.add(c));
      required.set(k, set);
    }
  }
  const usingCsv = templates.some(usesCsv);
  const used = new Set(templates.flatMap(sourcesUsed).map((k) => (k === mainName ? '' : k)));
  const read = async (path: string, key: string, label: string | null): Promise<CsvTable> => {
    try {
      return await deps.api().readCsv({ path, required: [...(required.get(key) ?? [])] });
    } catch (e) {
      const message = String(e instanceof Error ? e.message : e);
      throw new VariableError(label ? t('variables.job.csvFailedSource', { source: label, message }) : t('variables.job.csvFailed', { message }));
    }
  };
  let csv: CsvTable | null = null;
  const extra: Record<string, CsvTable | null> = {};
  if (usingCsv && main?.path) {
    csv = await read(main.path, '', null);
    for (const s of vt.sources.slice(1)) {
      const k = s.name.toLowerCase();
      if (used.has(k)) extra[k] = await read(s.path, k, s.name);
    }
  }
  return makeContext(vt, csv, deps.now(), getLanguage(), extra);
}

/** Fila (desde 1) que le toca a una plantilla: la de la primera variable de CSV o la actual. */
function rowOfTemplate(template: string, ctx: VarContext): number {
  const v = variablesOf(template).find((x) => x.name === 'csv' || x.name === 'row');
  return ctx.csvRow + (v?.offset ?? 0) + 1;
}

interface PlannedImage {
  obj: VariableImageObject;
  path: string;
}

/** Ruta del archivo de cada imagen variable, con los problemas (celda vacía, columna que no existe...). */
function planImages(images: VariableImageObject[], ctx: VarContext, problems: Issue[]): PlannedImage[] {
  const plan: PlannedImage[] = [];
  for (const o of images) {
    const tpl = o.kind.variable.source;
    const row = rowOfTemplate(tpl, ctx);
    const r = resolveTemplate(tpl, ctx);
    if (r.issues.length > 0) {
      problems.push(...r.issues.map((i) => ({ ...i, row: i.row ?? row })));
      continue;
    }
    const path = resolveImagePath(r.text, csvBaseDir(tpl, ctx));
    if (path === '') {
      problems.push({ code: 'imageEmpty', raw: tpl, detail: o.id, row });
      continue;
    }
    plan.push({ obj: o, path });
  }
  return plan;
}

/**
 * Documento listo para enviar, previsualizar o exportar: cada texto con variables se resuelve con
 * los valores de ahora y su contorno se regenera con el motor de texto (`textOutline`); cada imagen
 * variable pasa a apuntar al archivo de su fila (ajustado a su caja); cada código variable se
 * vuelve a generar con el contenido de su fila. El resto del documento va igual. Si no hay nada
 * variable devuelve el mismo documento. Falla con `VariableError` (nombrando todos los problemas de
 * una vez) si una variable no se puede resolver, un archivo de imagen falta o un código no se puede generar.
 *
 * Se hace aquí, en la interfaz, y no en el pipeline: el motor del trabajo solo conoce contornos
 * (nunca fuentes) y la app ya genera contornos con `text_outline`, con su biblioteca de fuentes.
 * El documento guardado no se toca: conserva las plantillas.
 */
export async function resolveDocForOutput(doc: Document, deps: ResolveDeps = {}): Promise<Document> {
  const texts = doc.objects.filter(templated);
  const images = variableImages(doc);
  const codes = variableCodes(doc);
  if (texts.length === 0 && images.length === 0 && codes.length === 0) return doc;
  const d: Required<Omit<ResolveDeps, 'warn'>> = {
    api: deps.api ?? getApi,
    fonts: deps.fonts ?? loadFonts,
    now: deps.now ?? (() => new Date()),
  };
  const templates = [...texts.map((o) => o.kind.text), ...images.map((o) => o.kind.variable.source), ...codes.map((o) => o.kind.code.data)];
  const ctx = await jobContext(doc, templates, d);

  // Primero se juntan todos los problemas, para avisarlos de una vez.
  const resolved = new Map<string, { text: string; sig: string; path: ReturnType<typeof resolveText> }>();
  const problems: Issue[] = [];
  const empties: Issue[] = [];
  for (const o of texts) {
    const r = resolveText(o, doc, ctx);
    if (!r.ok) throw new VariableError(t('text.onPath.missing'));
    problems.push(...(r.issues ?? []));
    resolved.set(o.id, { text: r.text ?? o.kind.text, sig: r.sig, path: r });
    empties.push(...(resolveTemplate(o.kind.text, ctx).empties ?? []));
  }

  // Imágenes: ruta de cada fila y comprobación de que el archivo existe (un solo sondeo).
  const plan = planImages(images, ctx, problems);
  const probes = new Map<string, ImageProbe>();
  if (plan.length > 0) {
    const paths = [...new Set(plan.map((p) => p.path))];
    let found: ImageProbe[];
    try {
      found = await d.api().probeImages({ paths });
    } catch (e) {
      throw new VariableError(t('variables.job.imageProbeFailed', { message: String(e instanceof Error ? e.message : e) }));
    }
    found.forEach((p, i) => probes.set(paths[i]!, p));
    for (const p of plan) {
      const probe = probes.get(p.path);
      if (!probe?.ok) problems.push({ code: 'imageMissing', raw: p.obj.kind.variable.source, detail: p.path, row: rowOfTemplate(p.obj.kind.variable.source, ctx) });
    }
  }

  // Códigos: contenido de cada fila y generación (los que repiten contenido se piden una vez).
  const generated = new Map<string, CodeResult>();
  const codeResults = new Map<string, CodeResult>();
  for (const o of codes) {
    const tpl = o.kind.code.data;
    const row = rowOfTemplate(tpl, ctx);
    const r = resolveTemplate(tpl, ctx);
    if (r.issues.length > 0) {
      problems.push(...r.issues.map((i) => ({ ...i, row: i.row ?? row })));
      continue;
    }
    if (r.text === '') {
      problems.push({ code: 'codeEmpty', raw: tpl, detail: o.id, row });
      continue;
    }
    const spec = codeSpecOf(o.kind.code, r.text);
    const key = JSON.stringify(spec);
    let result = generated.get(key);
    if (!result) {
      try {
        result = await d.api().generateCode({ spec });
      } catch (e) {
        problems.push({ code: 'codeFailed', raw: r.text, detail: String(e instanceof Error ? e.message : e), row });
        continue;
      }
      generated.set(key, result);
    }
    codeResults.set(o.id, result);
  }

  if (problems.length > 0) {
    const lines = [...new Set(problems.map(issueMessage))];
    throw new VariableError(t('variables.job.blocked', { problems: lines.join(' ') }), problems);
  }

  let families: FontFamily[] = [];
  if (texts.length > 0) {
    try {
      families = await d.fonts();
    } catch {
      // Sin lista de fuentes se respeta la familia pedida y el motor dirá si no existe.
    }
  }
  const replaced = new Map<string, DesignObject>();
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
      replaced.set(o.id, { ...o, kind: { ...o.kind, outline: out.paths } });
    } catch (e) {
      throw new VariableError(t('variables.job.outlineFailed', { text: info.text, message: String(e instanceof Error ? e.message : e) }));
    }
  }
  for (const p of plan) {
    const probe = probes.get(p.path)!;
    replaced.set(p.obj.id, fitImageObject(p.obj, p.path, { width: probe.width_px, height: probe.height_px }));
  }
  for (const o of codes as VariableCodeObject[]) {
    const result = codeResults.get(o.id);
    if (result) replaced.set(o.id, placeCodeResult(o, result));
  }

  if (empties.length > 0) void (deps.warn ?? defaultWarn)(emptyCellMessages(empties));
  if (replaced.size === 0) return doc;
  return { ...doc, objects: doc.objects.map((o) => replaced.get(o.id) ?? o) };
}

/** Texto de los avisos de celdas vacías: las primeras y cuántas más hay. */
export function emptyCellMessages(empties: Issue[]): string[] {
  const shown = empties.slice(0, MAX_LISTED).map((i) => t('variables.job.emptyCell', { raw: i.raw, row: i.row ?? '' }));
  const more = empties.length - shown.length;
  return [t('variables.job.emptyCells', { list: shown.join(' '), more: more > 0 ? t('variables.job.emptyMore', { count: more }) : '' }).trim()];
}

/** `args` con el documento ya resuelto (ver `resolveDocForOutput`). */
export async function resolveJobArgs<A extends Pick<JobArgs, 'doc'>>(args: A, deps?: ResolveDeps): Promise<A> {
  const doc = await resolveDocForOutput(args.doc, deps);
  return doc === args.doc ? args : { ...args, doc };
}
