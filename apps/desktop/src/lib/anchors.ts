import type { Affine, DesignObject, Document, Point } from '@/api/types';
import { applyAffine, multiplyAffine } from './geometry';

/** Inversa de una afín, o `null` si no la tiene. */
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

const round6 = (n: number) => Math.round(n * 1e6) / 1e6;
const movePoint = (m: Affine, p: Point): Point => {
  const q = applyAffine(m, p);
  return [round6(q[0]), round6(q[1])];
};

/** ¿Tiene el objeto datos anclados a puntos del documento (puentes manuales o inicio fijado)? */
export function hasAnchors(o: DesignObject): boolean {
  return (o.manual_tabs?.length ?? 0) > 0 || (o.start?.point ?? null) !== null;
}

/**
 * Los puentes manuales y el punto de inicio se guardan en coordenadas del documento. Si un cambio
 * mueve, gira o escala el objeto sin tocarlos, los lleva consigo (misma transformación relativa),
 * para que sigan sobre el mismo lugar del contorno. Si el cambio ya modificó esos datos, se respetan.
 * Devuelve `next` tal cual si no hay nada que ajustar.
 */
export function followAnchors(prev: Document, next: Document): Document {
  if (prev === next) return next;
  let before: Map<string, DesignObject> | null = null;
  let changed = false;
  const objects = next.objects.map((o) => {
    if (!hasAnchors(o)) return o;
    before ??= new Map(prev.objects.map((p) => [p.id, p]));
    const p = before.get(o.id);
    if (!p || p === o || p.transform === o.transform) return o;
    if (p.manual_tabs !== o.manual_tabs || p.start !== o.start) return o;
    const inv = invertAffine(p.transform);
    if (!inv) return o;
    const m = multiplyAffine(o.transform, inv);
    changed = true;
    const out: DesignObject = { ...o };
    if (o.manual_tabs?.length) out.manual_tabs = o.manual_tabs.map((t) => movePoint(m, t));
    if (o.start?.point) out.start = { ...o.start, point: movePoint(m, o.start.point) };
    return out;
  });
  return changed ? { ...next, objects } : next;
}
