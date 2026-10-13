import { useMemo } from 'react';
import type { DesignObject } from '@/api/types';
import { t } from '@/i18n';
import { objectBounds, type Bounds } from '@/lib/geometry';
import { worldToScreen } from '@/lib/viewport';
import { useStore } from '@/store';

/** Tope de números a la vez: con miles de objetos con prioridad solo se dibujan los visibles hasta aquí. */
export const MAX_BADGES = 400;

const boundsCache = new WeakMap<DesignObject, Bounds | null>();

function cachedBounds(o: DesignObject): Bounds | null {
  let b = boundsCache.get(o);
  if (b === undefined) {
    b = objectBounds(o);
    boundsCache.set(o, b);
  }
  return b;
}

export interface Badge {
  id: string;
  x: number;
  y: number;
  n: number;
}

/**
 * Posición en pantalla (esquina superior izquierda de la caja de cada objeto) y valor de los
 * objetos visibles que tienen prioridad de corte. Los de fuera de la vista se omiten.
 */
export function priorityBadges(
  objects: readonly DesignObject[],
  view: { zoom: number; panX: number; panY: number },
  size: { width: number; height: number },
  limit = MAX_BADGES,
): Badge[] {
  const out: Badge[] = [];
  for (const o of objects) {
    if (o.priority == null || o.hidden) continue;
    const b = cachedBounds(o);
    if (!b) continue;
    const [x, y] = worldToScreen(view, [b[0][0], b[1][1]]);
    if (x < -20 || y < -20 || x > size.width + 20 || y > size.height + 20) continue;
    out.push({ id: o.id, x, y, n: o.priority });
    if (out.length >= limit) break;
  }
  return out;
}

/** Números pequeños con la prioridad de corte de los objetos, si se activó en el menú Ver. */
export function PriorityBadges() {
  const on = useStore((s) => s.priorityBadges);
  const objects = useStore((s) => s.doc.objects);
  const zoom = useStore((s) => s.zoom);
  const panX = useStore((s) => s.panX);
  const panY = useStore((s) => s.panY);
  const viewport = useStore((s) => s.viewport);
  const badges = useMemo(
    () => (on ? priorityBadges(objects, { zoom, panX, panY }, viewport) : []),
    [on, objects, zoom, panX, panY, viewport],
  );
  if (badges.length === 0) return null;
  return (
    <div className="prio-badges" aria-hidden="true">
      {badges.map((b) => (
        <span key={b.id} className="prio-badge num" style={{ left: b.x, top: b.y }} title={t('canvas.obj.badge', { n: b.n })}>
          {b.n}
        </span>
      ))}
    </div>
  );
}
