import { t } from '@/i18n';
import type { Bounds, TileSpec } from './plan';
import type { TileStatus } from './session';

interface Props {
  design: Bounds;
  specs: readonly TileSpec[];
  /** Estado de cada tramo; sin él, todos son vista previa. */
  status?: readonly TileStatus[];
  current?: number | null;
  onPick?: (index: number) => void;
}

const PAD = 4;
const WIDTH = 300;

/** Plano pequeño del diseño con los tramos numerados (el del lienzo, a escala del panel). */
export function TilePreview({ design, specs, status, current = null, onPick }: Props) {
  const w = design[1][0] - design[0][0];
  const h = design[1][1] - design[0][1];
  const scale = (WIDTH - 2 * PAD) / w;
  const height = Math.min(220, h * scale) + 2 * PAD;
  const k = Math.min(scale, (height - 2 * PAD) / h);
  // Y hacia arriba en el documento y hacia abajo en pantalla.
  const px = (x: number) => PAD + (x - design[0][0]) * k;
  const py = (y: number) => PAD + (design[1][1] - y) * k;
  return (
    <svg className="til-preview" viewBox={`0 0 ${WIDTH} ${height}`} role="img" aria-label={t('tiling.preview.label', { n: specs.length })}>
      <rect className="til-preview__design" x={PAD} y={PAD} width={w * k} height={h * k} />
      {specs.map((s, i) => {
        const state = status?.[i] ?? 'preview';
        const x = px(s.rect[0][0]);
        const y = py(s.rect[1][1]);
        const rw = (s.rect[1][0] - s.rect[0][0]) * k;
        const rh = (s.rect[1][1] - s.rect[0][1]) * k;
        return (
          <g
            key={i}
            className={`til-preview__tile til-preview__tile--${state}${i === current ? ' is-current' : ''}`}
            onClick={onPick ? () => onPick(i) : undefined}
            style={onPick ? { cursor: 'pointer' } : undefined}
          >
            <rect x={x} y={y} width={rw} height={rh} />
            <text x={x + rw / 2} y={y + rh / 2} textAnchor="middle" dominantBaseline="middle">{i + 1}</text>
          </g>
        );
      })}
    </svg>
  );
}
