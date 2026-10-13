import { useMemo } from 'react';
import { t } from '@/i18n';
import { pointsBounds, unionBounds, type Bounds } from '@/lib/geometry';
import { flatPolylines, pathData, placedPieces, type FlatPoly } from './preview';
import type { NestContainer, NestPlacement } from './types';

interface Props {
  container: NestContainer | null;
  /** Contornos de cada pieza por id de unidad (en su posición actual). */
  flat: ReadonlyMap<string, readonly FlatPoly[]>;
  /** `null` = aún no hay acomodo: se dibujan las piezas donde están. */
  placements: readonly NestPlacement[] | null;
}

const IDENTITY = [1, 0, 0, 1, 0, 0] as const;

/**
 * Vista previa del acomodo: el contenedor (cama o contorno del material) y cada pieza con su
 * transform aplicado. Eje Y hacia arriba. El trabajo pesado se memoiza por `placements`.
 */
export function NestPreview({ container, flat, placements }: Props) {
  const material = useMemo(
    () => (container?.type === 'object' ? flatPolylines([container.object]) : null),
    [container],
  );
  const shown = useMemo(
    () =>
      placedPieces(
        flat,
        placements ?? [...flat.keys()].map((piece) => ({ piece, copy: 0, rotation_deg: 0, mirror: false, transform: [...IDENTITY] })),
      ),
    [flat, placements],
  );
  const materialD = useMemo(() => (material ? pathData(material) : ''), [material]);

  const box: Bounds | null = useMemo(() => {
    const own: Bounds | null =
      container?.type === 'rect'
        ? [[0, 0], [container.width, container.height]]
        : pointsBounds((material ?? []).flatMap((p) => p.points));
    return unionBounds([own, shown.bounds]);
  }, [container, material, shown.bounds]);

  if (!box) return <div className="ne-preview ne-preview--empty" role="img" aria-label={t('nest.preview.label')} />;
  const [[x0, y0], [x1, y1]] = box;
  const pad = Math.max(x1 - x0, y1 - y0, 1) * 0.04;

  return (
    <svg
      className="ne-preview"
      role="img"
      aria-label={t('nest.preview.label')}
      viewBox={`${x0 - pad} ${-(y1 + pad)} ${x1 - x0 + 2 * pad} ${y1 - y0 + 2 * pad}`}
      preserveAspectRatio="xMidYMid meet"
    >
      <g transform="scale(1 -1)">
        {container?.type === 'rect' && <rect className="ne-bed" x={0} y={0} width={container.width} height={container.height} />}
        {materialD && <path className="ne-material" d={materialD} />}
        {shown.pieces.map((p) => (
          <path
            key={p.key}
            className={`ne-piece${p.copy > 0 ? ' ne-piece--copy' : ''}${placements ? '' : ' ne-piece--ghost'}`}
            d={p.d}
          />
        ))}
      </g>
    </svg>
  );
}
