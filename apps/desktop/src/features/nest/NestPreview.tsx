import { useMemo } from 'react';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { pointsBounds, unionBounds, type Bounds } from '@/lib/geometry';
import { flatPolylines, pathData, placedPieces, type FlatPoly } from './preview';
import type { Remnant } from './remnants';
import { offsetPlacements, sheetOffsets, sheetOutline, sheetsBounds } from './sheets';
import type { NestContainer, NestPlacement, NestSheet } from './types';

interface Props {
  container: NestContainer | null;
  /** Contornos de cada pieza por id de unidad (en su posición actual). */
  flat: ReadonlyMap<string, readonly FlatPoly[]>;
  /** `null` = aún no hay acomodo: se dibujan las piezas donde están. */
  placements: readonly NestPlacement[] | null;
  /** Hojas del acomodo; con varias (o retazos) se dibujan en fila, cada una con su contorno y su número. */
  sheets?: readonly NestSheet[] | null;
  remnants?: ReadonlyMap<string, Pick<Remnant, 'paths'>>;
}

const IDENTITY = [1, 0, 0, 1, 0, 0] as const;
const NO_REMNANTS: ReadonlyMap<string, Pick<Remnant, 'paths'>> = new Map();

/**
 * Vista previa del acomodo: el contenedor (cama o contorno del material) y cada pieza con su
 * transform aplicado. Eje Y hacia arriba. Con varias hojas, cada una va en su sitio de la fila (el
 * mismo que tendrá en el lienzo al aplicar). El trabajo pesado se memoiza por `placements`.
 */
export function NestPreview({ container, flat, placements, sheets = null, remnants = NO_REMNANTS }: Props) {
  const material = useMemo(
    () => (container?.type === 'object' ? flatPolylines([container.object]) : null),
    [container],
  );
  const multi = sheets !== null && sheets.length > 0 && (sheets.length > 1 || sheets[0]!.source === 'remnant');
  const offsets = useMemo(() => (sheets && sheets.length > 0 ? sheetOffsets(sheets) : []), [sheets]);
  const shifted = useMemo(() => (placements ? offsetPlacements(placements, offsets) : null), [placements, offsets]);
  const shown = useMemo(
    () =>
      placedPieces(
        flat,
        shifted ?? [...flat.keys()].map((piece) => ({ piece, sheet: 0, copy: 0, rotation_deg: 0, mirror: false, transform: [...IDENTITY] })),
      ),
    [flat, shifted],
  );
  const materialD = useMemo(() => (material ? pathData(material) : ''), [material]);
  const outlines = useMemo(
    () =>
      multi && sheets
        ? sheets.map((s, k) => {
            const [dx, dy] = offsets[k] ?? [0, 0];
            const polys = sheetOutline(s, container, remnants, material).map(
              (p): FlatPoly => ({ closed: true, points: p.points.map(([x, y]): [number, number] => [x + dx, y + dy]) }),
            );
            return { d: pathData(polys), label: [s.bounds[0] + dx, s.bounds[3] + dy] as const, pct: s.utilization };
          })
        : [],
    [multi, sheets, offsets, container, remnants, material],
  );

  const box: Bounds | null = useMemo(() => {
    if (multi && sheets) return unionBounds([sheetsBounds(sheets, offsets), shown.bounds]);
    const own: Bounds | null =
      container?.type === 'rect'
        ? [[0, 0], [container.width, container.height]]
        : pointsBounds((material ?? []).flatMap((p) => p.points));
    return unionBounds([own, shown.bounds]);
  }, [multi, sheets, offsets, container, material, shown.bounds]);

  if (!box) return <div className="ne-preview ne-preview--empty" role="img" aria-label={t('nest.preview.label')} />;
  const [[x0, y0], [x1, y1]] = box;
  const span = Math.max(x1 - x0, y1 - y0, 1);
  const pad = span * 0.04;
  const font = span * 0.035;

  return (
    <svg
      className="ne-preview"
      role="img"
      aria-label={t('nest.preview.label')}
      viewBox={`${x0 - pad} ${-(y1 + pad)} ${x1 - x0 + 2 * pad} ${y1 - y0 + 2 * pad}`}
      preserveAspectRatio="xMidYMid meet"
    >
      <g transform="scale(1 -1)">
        {multi ? (
          outlines.map((o, k) => <path key={k} className="ne-material ne-sheet" d={o.d} />)
        ) : (
          <>
            {container?.type === 'rect' && <rect className="ne-bed" x={0} y={0} width={container.width} height={container.height} />}
            {materialD && <path className="ne-material" d={materialD} />}
          </>
        )}
        {shown.pieces.map((p) => (
          <path
            key={p.key}
            className={`ne-piece${p.copy > 0 ? ' ne-piece--copy' : ''}${placements ? '' : ' ne-piece--ghost'}`}
            d={p.d}
          />
        ))}
      </g>
      {multi &&
        outlines.map((o, k) => (
          <text key={k} className="ne-sheet-label" x={o.label[0] + font * 0.4} y={-(o.label[1] - font * 0.4)} fontSize={font}>
            {t('nest.sheet.label', { n: k + 1, pct: fmt(Math.round(o.pct * 10) / 10) })}
          </text>
        ))}
    </svg>
  );
}
