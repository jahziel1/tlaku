import { getApi } from '@/api';
import type { CodeResult, DesignObject, Document, ImageProbe } from '@/api/types';
import { placeCodeResult } from './codeVar';
import { displayContext, useVariablesUi } from './context';
import { csvBaseDir, fitImageObject, resolveImagePath } from './imageVar';
import { resolveTemplate, type VarContext } from './resolve';
import { hasVariables } from './syntax';
import { codeSpecOf, isVariableCode, isVariableImage, type VariableCodeObject, type VariableImageObject } from './varObjects';

/**
 * El lienzo muestra la imagen y el código de la fila actual (como ya hace con el texto), sin tocar
 * el documento: aquí solo se arma una copia de presentación de los objetos variables. El documento
 * (y lo guardado) conserva la imagen y el código de muestra; la selección, los controladores y la
 * edición siguen trabajando con el objeto real.
 *
 * Lo que necesita el motor (tamaño del archivo de cada imagen, formas de cada código) se pide una
 * sola vez por ruta o por contenido y se recuerda; mientras llega, se ve la muestra.
 */

type Loaded<T> = { state: 'pending' } | { state: 'failed' } | { state: 'ready'; value: T };

const probes = new Map<string, Loaded<ImageProbe>>();
const codes = new Map<string, Loaded<CodeResult>>();

/** Cuántas entradas se recuerdan antes de empezar de nuevo (un CSV de miles de filas no debe crecer sin fin). */
const MAX_CACHED = 400;

const listeners = new Set<() => void>();
/** Avisa cuando llega algo que cambia lo que se ve (para que el lienzo vuelva a dibujar). */
export function onCanvasDisplayChange(cb: () => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}
let epoch = 0;
function changed(): void {
  epoch++;
  listeners.forEach((cb) => cb());
}

function remember<T>(cache: Map<string, Loaded<T>>, key: string, value: Loaded<T>): void {
  if (cache.size >= MAX_CACHED && !cache.has(key)) {
    // Se descartan primero los que ya terminaron; los pendientes siguen su camino.
    for (const [k, v] of cache) if (v.state !== 'pending') cache.delete(k);
    if (cache.size >= MAX_CACHED) cache.clear();
  }
  cache.set(key, value);
}

function lookup<T>(cache: Map<string, Loaded<T>>, key: string, fetch: () => Promise<T>): T | null {
  const hit = cache.get(key);
  if (hit) return hit.state === 'ready' ? hit.value : null;
  remember(cache, key, { state: 'pending' });
  void fetch().then(
    (value) => {
      remember(cache, key, { state: 'ready', value });
      changed();
    },
    () => remember(cache, key, { state: 'failed' }),
  );
  return null;
}

/** Ruta del archivo de la imagen variable en la fila actual (`null` si no se puede resolver). */
function imagePathOf(o: VariableImageObject, ctx: VarContext): string | null {
  const tpl = o.kind.variable.source;
  const r = resolveTemplate(tpl, ctx);
  if (r.issues.length > 0) return null;
  const path = resolveImagePath(r.text, csvBaseDir(tpl, ctx));
  return path === '' ? null : path;
}

interface Memo {
  key: string;
  dep: unknown;
  out: DesignObject;
}
/** Copia de presentación por objeto del documento: mientras no cambie el objeto ni su dato, es la misma. */
const memo = new WeakMap<DesignObject, Memo>();

function displayImage(o: VariableImageObject, ctx: VarContext): DesignObject {
  const path = imagePathOf(o, ctx);
  if (path === null) return o;
  const probe = lookup(probes, path, async () => (await getApi().probeImages({ paths: [path] }))[0]!);
  if (!probe?.ok) return o;
  const hit = memo.get(o);
  if (hit && hit.key === path && hit.dep === probe) return hit.out;
  const out = fitImageObject(o, path, { width: probe.width_px, height: probe.height_px });
  memo.set(o, { key: path, dep: probe, out });
  return out;
}

function displayCode(o: VariableCodeObject, ctx: VarContext): DesignObject {
  const r = resolveTemplate(o.kind.code.data, ctx);
  if (r.issues.length > 0 || r.text === '') return o;
  const spec = codeSpecOf(o.kind.code, r.text);
  const key = JSON.stringify(spec);
  const result = lookup(codes, key, () => getApi().generateCode({ spec }));
  if (!result) return o;
  const hit = memo.get(o);
  if (hit && hit.key === key && hit.dep === result) return hit.out;
  const out = placeCodeResult(o, result);
  memo.set(o, { key, dep: result, out });
  return out;
}

interface Last {
  objects: DesignObject[];
  vt: Document['variable_text'];
  csv: unknown;
  extra: unknown;
  clock: number;
  show: boolean;
  epoch: number;
  out: DesignObject[];
}
let last: Last | null = null;
let lastData: { csv: unknown; extra: unknown } = { csv: null, extra: null };

/**
 * Objetos tal como se ven en el lienzo: las imágenes y los códigos variables con los datos de la
 * fila actual. Devuelve el mismo arreglo si no hay objetos variables, si se está viendo la
 * plantilla o si aún no hay nada que mostrar distinto de la muestra; y el mismo resultado de la
 * vez anterior mientras no cambie nada de lo que lo determina (se llama en cada cuadro).
 */
export function canvasDisplayObjects(doc: Document, objects: DesignObject[]): DesignObject[] {
  const ui = useVariablesUi.getState();
  if (
    last && last.objects === objects && last.vt === doc.variable_text && last.csv === ui.csv && last.extra === ui.extra &&
    last.clock === ui.clock && last.show === ui.showTemplate && last.epoch === epoch
  ) {
    return last.out;
  }
  if (lastData.csv !== ui.csv || lastData.extra !== ui.extra) {
    // Un archivo que faltaba puede haber aparecido: al cambiar los datos se vuelve a mirar.
    for (const [k, v] of probes) if (v.state === 'failed' || (v.state === 'ready' && !v.value.ok)) probes.delete(k);
    lastData = { csv: ui.csv, extra: ui.extra };
  }
  let out = objects;
  if (!ui.showTemplate) {
    let ctx: VarContext | null = null;
    let next: DesignObject[] | null = null;
    objects.forEach((o, i) => {
      const image = isVariableImage(o);
      if (!image && !(isVariableCode(o) && hasVariables(o.kind.code.data))) return;
      ctx ??= displayContext(doc);
      const shown = image ? displayImage(o, ctx) : displayCode(o as VariableCodeObject, ctx);
      if (shown === o) return;
      next ??= objects.slice();
      next[i] = shown;
    });
    if (next) out = next;
  }
  last = { objects, vt: doc.variable_text, csv: ui.csv, extra: ui.extra, clock: ui.clock, show: ui.showTemplate, epoch, out };
  return out;
}

/** Para pruebas: olvida todo lo recordado. */
export function resetCanvasDisplay(): void {
  probes.clear();
  codes.clear();
  last = null;
  lastData = { csv: null, extra: null };
  epoch++;
}
