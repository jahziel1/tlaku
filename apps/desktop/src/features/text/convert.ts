import type { DesignObject, Document, TextObject } from '@/api/types';
import { useStore } from '@/store';
import { isTextObject } from './signature';

/**
 * Reemplaza el texto `id` por un vector con su contorno: conserva id, transform, capa, grupo y
 * banderas. Devuelve el mismo documento si no es un texto o aún no tiene contorno.
 */
export function textToVector(doc: Document, id: string): Document {
  let changed = false;
  const objects = doc.objects.map((o): DesignObject => {
    if (o.id !== id || !isTextObject(o) || o.kind.outline.length === 0) return o;
    changed = true;
    return { ...o, kind: { type: 'vector', paths: o.kind.outline } };
  });
  return changed ? { ...doc, objects } : doc;
}

/** Convierte el texto a trayectos con un solo paso de deshacer. Devuelve si hizo el cambio. */
export function convertTextToPaths(id: string): boolean {
  const before = useStore.getState().doc;
  if (textToVector(before, id) === before) return false;
  useStore.getState().applyDocChange('text.history.convert', (doc) => textToVector(doc, id));
  return true;
}

/** Cambia campos de un texto con un paso de deshacer; escribir seguido se funde en un solo paso. */
export function editText(id: string, patch: Partial<TextObject>, field: string): void {
  useStore.getState().applyDocChange(
    'text.history.edit',
    (doc) => {
      let changed = false;
      const objects = doc.objects.map((o) => {
        if (o.id !== id || !isTextObject(o)) return o;
        const same = (Object.keys(patch) as (keyof TextObject)[]).every((k) => Object.is(o.kind[k], patch[k]));
        if (same) return o;
        changed = true;
        return { ...o, kind: { ...o.kind, ...patch } };
      });
      return changed ? { ...doc, objects } : doc;
    },
    `text:${id}:${field}`,
  );
}

/** Objetos del documento sobre los que se puede escribir: vectores con una trayectoria utilizable. */
export function guideCandidates(doc: Document, selfId: string): DesignObject[] {
  return doc.objects.filter((o) => o.id !== selfId && o.kind.type === 'vector' && o.kind.paths.some((p) => p.length >= 2));
}
