import type { Affine, DesignObject, Document, ImageObject } from '@/api/types';
import type { I18nKey } from '@/i18n';
import { transformBez, type BezPath } from '@/lib/bezpath';
import { multiplyAffine, objectLocalPaths } from '@/lib/geometry';

/** Inversa de una afín; `null` si no es invertible. */
export function invertAffine(m: Affine): Affine | null {
  const [a, b, c, d, e, f] = m;
  const det = a * d - b * c;
  if (!Number.isFinite(det) || Math.abs(det) < 1e-12) return null;
  const ia = d / det;
  const ib = -b / det;
  const ic = -c / det;
  const id = a / det;
  return [ia, ib, ic, id, -(ia * e + ic * f), -(ib * e + id * f)];
}

type ImageObj = DesignObject & { kind: ImageObject & { type: 'image' } };

const isImage = (o: DesignObject): o is ImageObj => o.kind.type === 'image';

/** Una imagen más una o más formas (vectores o texto) para recortarla. */
export function splitMaskSelection(picked: readonly DesignObject[]): { image: ImageObj; shapes: DesignObject[] } | null {
  const images = picked.filter(isImage);
  const shapes = picked.filter((o) => !isImage(o));
  if (images.length !== 1 || shapes.length === 0) return null;
  return { image: images[0]!, shapes };
}

/** Motivo (clave de i18n) por el que "Recortar imagen con forma" no se puede usar; `true` si se puede. */
export function maskEnabled(picked: readonly DesignObject[]): true | I18nKey {
  const images = picked.filter(isImage);
  if (images.length === 0) return 'trace.mask.reason.needImage';
  if (images.length > 1) return 'trace.mask.reason.oneImage';
  if (picked.length === 1) return 'trace.mask.reason.needShape';
  const image = images[0]!;
  if (image.locked) return 'trace.mask.reason.locked';
  if (invertAffine(image.transform) === null) return 'trace.mask.reason.degenerate';
  return true;
}

/** La imagen seleccionada ya tiene máscara (se puede quitar). */
export function unmaskEnabled(picked: readonly DesignObject[]): true | I18nKey {
  const images = picked.filter(isImage);
  if (images.length === 0) return 'trace.mask.reason.needImage';
  if (images.some((i) => i.locked)) return 'trace.mask.reason.locked';
  return images.some((i) => (i.kind.mask?.length ?? 0) > 0) ? true : 'trace.mask.reason.noMask';
}

/**
 * Curvas de las formas en coordenadas locales de la imagen (mm, Y hacia arriba): se les aplica
 * el `transform` de cada forma y luego la inversa del de la imagen.
 */
export function maskPaths(image: DesignObject, shapes: readonly DesignObject[]): BezPath[] {
  const inv = invertAffine(image.transform);
  if (!inv) return [];
  return shapes.flatMap((s) => {
    const m = multiplyAffine(inv, s.transform);
    return objectLocalPaths(s).map((p) => transformBez(p, m));
  });
}

/**
 * Pone las formas como máscara de la imagen y las quita del documento (quedan "dentro" de la
 * imagen). Una máscara anterior se reemplaza. Un solo cambio: un paso de deshacer.
 */
export function applyMaskToDoc(doc: Document, imageId: string, shapeIds: readonly string[]): Document {
  const image = doc.objects.find((o) => o.id === imageId);
  if (!image || !isImage(image)) return doc;
  const ids = new Set(shapeIds);
  const shapes = doc.objects.filter((o) => ids.has(o.id) && o.kind.type !== 'image');
  const mask = maskPaths(image, shapes);
  if (mask.length === 0) return doc;
  const gone = new Set(shapes.map((s) => s.id));
  return {
    ...doc,
    objects: doc.objects
      .filter((o) => !gone.has(o.id))
      .map((o) => (o.id === imageId ? { ...o, kind: { ...(o.kind as ImageObject & { type: 'image' }), mask } } : o)),
  };
}

/** Quita la máscara de las imágenes indicadas (la imagen vuelve a grabarse completa). */
export function removeMaskFromDoc(doc: Document, imageIds: readonly string[]): Document {
  const ids = new Set(imageIds);
  return {
    ...doc,
    objects: doc.objects.map((o) => {
      if (!ids.has(o.id) || !isImage(o) || (o.kind.mask?.length ?? 0) === 0) return o;
      const { mask: _mask, ...rest } = o.kind;
      return { ...o, kind: rest };
    }),
  };
}
