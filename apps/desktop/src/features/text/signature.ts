import type { DesignObject, Document, TextObject } from '@/api/types';
import { transformBez, type BezPath } from '@/lib/bezpath';
import { multiplyAffine } from '@/lib/geometry';
import { displayContext, useVariablesUi } from '@/features/variables/context';
import { resolveTemplate, type Issue, type VarContext } from '@/features/variables/resolve';
import { isTemplate } from '@/features/variables/syntax';
import { invertAffine } from './affine';

/** Objeto del documento cuyo `kind` es un texto. */
export type TextDesignObject = DesignObject & { kind: Extract<DesignObject['kind'], { type: 'text' }> };

export function isTextObject(o: DesignObject): o is TextDesignObject {
  return o.kind.type === 'text';
}

/** Cadena estable con todo lo que cambia el contorno salvo el contorno mismo (y la guía). */
export function paramsKey(t: TextObject): string {
  return JSON.stringify([
    t.text,
    t.font_family.trim().toLowerCase(),
    t.bold,
    t.italic,
    t.size_mm,
    t.letter_spacing_mm,
    t.line_spacing,
    t.align,
    t.on_path && [t.on_path.object_id, t.on_path.offset_mm, t.on_path.flip],
    // Ausente y 0 / falso son lo mismo: un documento viejo no se regenera al abrirlo.
    t.bend_deg ?? 0,
    t.welded ?? false,
    // Lo mismo con las opciones avanzadas: ausentes equivalen a los valores por omisión.
    (t.features ?? []).map((f) => [f.tag, f.value]),
    (t.variations ?? []).map((a) => [a.tag, a.value]),
    t.uppercase ?? false,
    t.valign ?? 'baseline',
    t.word_spacing_mm ?? 0,
  ]);
}

/** Hash de 53 bits (cyrb53) de una cadena larga, para no guardar trayectorias enteras. */
export function hashString(s: string): string {
  let h1 = 0xdeadbeef;
  let h2 = 0x41c6ce57;
  for (let i = 0; i < s.length; i++) {
    const ch = s.charCodeAt(i);
    h1 = Math.imul(h1 ^ ch, 2654435761);
    h2 = Math.imul(h2 ^ ch, 1597334677);
  }
  h1 = Math.imul(h1 ^ (h1 >>> 16), 2246822507) ^ Math.imul(h2 ^ (h2 >>> 13), 3266489909);
  h2 = Math.imul(h2 ^ (h2 >>> 16), 2246822507) ^ Math.imul(h1 ^ (h1 >>> 13), 3266489909);
  return (4294967296 * (2097151 & h2) + (h1 >>> 0)).toString(36);
}

const r4 = (n: number) => Math.round(n * 1e4) / 1e4;

function pathText(path: BezPath): string {
  return path
    .map((s) => {
      switch (s.type) {
        case 'move_to':
        case 'line_to':
          return `${s.type[0]}${r4(s.to[0])},${r4(s.to[1])}`;
        case 'quad_to':
          return `q${r4(s.ctrl[0])},${r4(s.ctrl[1])},${r4(s.to[0])},${r4(s.to[1])}`;
        case 'cubic_to':
          return `c${r4(s.ctrl1[0])},${r4(s.ctrl1[1])},${r4(s.ctrl2[0])},${r4(s.ctrl2[1])},${r4(s.to[0])},${r4(s.to[1])}`;
        case 'close':
          return 'z';
      }
    })
    .join(' ');
}

export type Resolved =
  | {
      ok: true;
      sig: string;
      path: BezPath | null;
      /** Solo en textos con llaves: el texto con las variables ya sustituidas (o la plantilla, si así se pide). */
      text?: string;
      /** Variables que no se pudieron resolver (se dejan escritas en `text`). */
      issues?: Issue[];
    }
  /** `target`: el objeto guía no existe o no es un vector con trayectoria. */
  | { ok: false; reason: 'target' };

const paramsMemo = new WeakMap<TextObject, string>();
const pathMemo = new WeakMap<object, { text: DesignObject['transform']; target: DesignObject['transform']; value: ResolvedPath }>();
type ResolvedPath = { path: BezPath; hash: string } | null;

/** Primera trayectoria del objeto guía, en las coordenadas locales del texto. */
function guidePath(text: TextDesignObject, doc: Document): ResolvedPath {
  const link = text.kind.on_path;
  if (!link || link.object_id === text.id) return null;
  const target = doc.objects.find((o) => o.id === link.object_id);
  if (!target || target.kind.type !== 'vector') return null;
  const first = target.kind.paths.find((p) => p.length >= 2);
  if (!first) return null;
  // Se recalcula solo si cambió la guía o la colocación del texto.
  const hit = pathMemo.get(target.kind);
  if (hit && hit.target === target.transform && hit.text === text.transform) return hit.value;
  const inv = invertAffine(text.transform);
  let value: ResolvedPath = null;
  if (inv) {
    const path = transformBez(first, multiplyAffine(inv, target.transform));
    value = { path, hash: hashString(pathText(path)) };
  }
  pathMemo.set(target.kind, { text: text.transform, target: target.transform, value });
  return value;
}

/**
 * Firma de lo que determina el contorno del texto: sus parámetros y, si va sobre un trayecto,
 * la trayectoria guía vista desde el sistema local del texto (el contorno se dibuja con el
 * `transform` del texto, así que la guía se pasa ya con la inversa aplicada).
 */
export function resolveText(obj: TextDesignObject, doc: Document, ctx?: VarContext): Resolved {
  let key: string;
  let extra: { text: string; issues: Issue[] } | null = null;
  if (isTemplate(obj.kind.text)) {
    // Texto variable: la firma depende del texto ya resuelto, así que no se memoiza.
    const template = ctx === undefined && useVariablesUi.getState().showTemplate;
    const r = template ? { text: obj.kind.text, issues: [] } : resolveTemplate(obj.kind.text, ctx ?? displayContext(doc));
    extra = r;
    key = paramsKey({ ...obj.kind, text: r.text });
  } else {
    let memo = paramsMemo.get(obj.kind);
    if (memo === undefined) {
      memo = paramsKey(obj.kind);
      paramsMemo.set(obj.kind, memo);
    }
    key = memo;
  }
  if (!obj.kind.on_path) return { ok: true, sig: key, path: null, ...extra };
  const guide = guidePath(obj, doc);
  if (!guide) return { ok: false, reason: 'target' };
  return { ok: true, sig: `${key}|${guide.hash}`, path: guide.path, ...extra };
}
