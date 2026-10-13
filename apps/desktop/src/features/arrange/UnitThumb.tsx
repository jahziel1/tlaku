import { useMemo } from 'react';
import type { DesignObject } from '@/api/types';
import { unitThumbnail } from './unitLabel';

/** Miniatura del contorno en el color de la capa; si la forma no es simple (imagen, texto, muy detallada), solo la muestra de color. */
export function UnitThumb({ unit }: { unit: { objects: readonly DesignObject[] } }) {
  const color = unit.objects[0]!.layer;
  const thumb = useMemo(() => unitThumbnail(unit), [unit]);
  if (!thumb) return <span className="ar-swatch" style={{ background: color }} aria-hidden="true" />;
  return (
    <svg className="ar-thumb" viewBox={`0 0 ${thumb.width} ${thumb.height}`} preserveAspectRatio="xMidYMid meet" aria-hidden="true">
      <path d={thumb.d} fill="none" stroke={color} strokeWidth="6" strokeLinejoin="round" strokeLinecap="round" vectorEffect="non-scaling-stroke" />
    </svg>
  );
}
