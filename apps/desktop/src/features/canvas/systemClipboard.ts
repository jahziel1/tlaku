import type { DesignObject, Document, Point } from '@/api/types';
import { t } from '@/i18n';
import { transformBez } from '@/lib/bezpath';
import { groupDoc } from '@/lib/groups';
import { makeLayer } from '@/lib/layers';
import { colorName } from '@/lib/palette';
import { screenToWorld } from '@/lib/viewport';
import { pinTab, type AppState } from '@/store';
import { uniqueId } from '@/store/document';
import { looksLikeSvg, objectsToSvg, svgToObjects } from './svgClip';
import { newTextObject } from './textTool';
import { pickDrawLayer } from './shapes';

/**
 * Puente con el portapapeles del sistema. Se copia SVG como texto (lo reconocen Inkscape,
 * Illustrator y los editores que aceptan SVG pegado) y al pegar se lee el texto del sistema: si es
 * un SVG ajeno se importa como dibujo; si es el mismo que copió esta app, manda el portapapeles
 * interno (conserva capas, textos editables y formas); un texto cualquiera se pega como objeto de texto.
 * Usa la API del navegador (`navigator.clipboard`); si el sistema la bloquea, la app sigue con su
 * portapapeles interno. Un plugin de Tauri no es imprescindible.
 */

/** Último SVG que escribió la app: pegarlo de vuelta usa el portapapeles interno. */
let lastWritten: string | null = null;

/** Máximo de caracteres de un texto pegado que se vuelve objeto de texto. */
const MAX_PASTED_TEXT = 2000;

/** ¿Se puede leer el portapapeles del sistema? Si sí, Pegar siempre puede tener algo que pegar. */
export const systemClipboardAvailable = (): boolean => clipboard() !== null;

const clipboard = (): Clipboard | null => (typeof navigator !== 'undefined' && navigator.clipboard ? navigator.clipboard : null);

/** Copia `objects` como SVG al portapapeles del sistema. `false` si no hay nada que copiar o el sistema lo impide. */
export async function copyAsSvg(objects: readonly DesignObject[], layers: Document['layers']): Promise<boolean> {
  const svg = objectsToSvg(objects, layers);
  const cb = clipboard();
  if (!svg || !cb) return false;
  try {
    await cb.writeText(svg);
    lastWritten = svg;
    return true;
  } catch {
    return false;
  }
}

async function readText(): Promise<string | null> {
  const cb = clipboard();
  if (!cb) return null;
  try {
    return await cb.readText();
  } catch {
    return null;
  }
}

export type PasteResult = 'svg' | 'text' | 'internal' | 'none';

/** Centro de lo visible en el lienzo, en mm: ahí se coloca lo que viene de otra aplicación. */
export function viewCenter(s: Pick<AppState, 'zoom' | 'panX' | 'panY' | 'viewport'>): Point {
  return screenToWorld({ zoom: s.zoom, panX: s.panX, panY: s.panY }, [s.viewport.width / 2, s.viewport.height / 2]);
}

/** Importa un SVG pegado: capas nuevas, objetos centrados en `center`, todo en un solo paso. Devuelve los ids. */
export function pasteSvgText(s: AppState, text: string, center: Point): string[] | null {
  const layer = pickDrawLayer(s.doc.layers, s.activeLayer);
  const parsed = svgToObjects(text, layer);
  if (!parsed) return null;
  const created: string[] = [];
  s.applyDocChange('canvas.history.pasteSvg', (doc) => {
    created.length = 0;
    const taken = new Set(doc.objects.map((o) => o.id));
    const dx = center[0] - parsed.width / 2;
    const dy = center[1] - parsed.height / 2;
    const objects = parsed.objects.map((o) => {
      const id = uniqueId('svg', taken);
      taken.add(id);
      created.push(id);
      const shifted = o.kind.type === 'vector' ? { type: 'vector' as const, paths: o.kind.paths.map((p) => transformBez(p, [1, 0, 0, 1, dx, dy])) } : o.kind;
      return { ...o, id, kind: shifted };
    });
    const have = new Set(doc.layers.map((l) => l.color.toLowerCase()));
    const fresh = parsed.layers
      .filter((l) => !have.has(l.color.toLowerCase()))
      .map((l) => makeLayer(l.color, colorName(l.color), l.fill ? { mode: 'fill' } : {}));
    const next: Document = { ...doc, objects: [...doc.objects, ...objects], layers: [...doc.layers, ...fresh] };
    // Varias formas pegadas juntas forman un grupo, como lo que se importa de un archivo.
    return objects.length > 1 ? groupDoc(next, created) : next;
  });
  s.select(created);
  return created;
}

/** Texto pegado a un objeto de texto en `center` (el panel de texto genera su contorno). */
function pasteText(s: AppState, text: string, center: Point): string {
  const id = uniqueId('text', new Set(s.doc.objects.map((o) => o.id)));
  const layer = pickDrawLayer(s.doc.layers, s.activeLayer);
  const base = newTextObject(id, layer, center);
  const obj: DesignObject = base.kind.type === 'text' ? { ...base, kind: { ...base.kind, text } } : base;
  s.addObjects([obj], 'canvas.draw.text');
  s.select([id]);
  return id;
}

/** Pega según lo que haya en el portapapeles del sistema o, si no sirve, el interno. */
export async function pasteFromSystem(s: AppState): Promise<PasteResult> {
  // Se pega en la pestaña activa al pedirlo, aunque se cambie de pestaña mientras se lee el portapapeles.
  const inTab = pinTab(s);
  const text = await readText();
  return inTab(() => pasteInto(s, text)) ?? 'none';
}

function pasteInto(s: AppState, text: string | null): PasteResult {
  const trimmed = text?.trim() ?? '';
  if (trimmed && text !== lastWritten) {
    const center = viewCenter(s);
    if (looksLikeSvg(trimmed)) {
      const ids = pasteSvgText(s, trimmed, center);
      if (ids) {
        const parsedSkipped = svgToObjects(trimmed, '#000000')?.skipped ?? 0;
        if (parsedSkipped > 0) s.notify('warning', t('canvas.clip.skipped', { n: parsedSkipped }));
        return 'svg';
      }
    } else if (trimmed.length <= MAX_PASTED_TEXT) {
      pasteText(s, trimmed, center);
      return 'text';
    }
  }
  const ids = s.pasteObjects();
  if (ids.length > 0) {
    s.select(ids);
    return 'internal';
  }
  return 'none';
}
