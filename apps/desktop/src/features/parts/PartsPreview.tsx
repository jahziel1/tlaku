import type { GenerateResult } from '@/bindings/GenerateResult';
import type { PathDto } from '@/bindings/PathDto';
import { t } from '@/i18n';

const d = (p: PathDto) => p.points.map(([x, y], i) => `${i === 0 ? 'M' : 'L'}${x.toFixed(3)} ${y.toFixed(3)}`).join('') + (p.closed ? 'Z' : '');

/** Dibujo SVG de las piezas (Y hacia arriba, como en el documento). Cortes con el color de la capa de corte y rótulos con el de grabado. */
export function PartsPreview({ result, cutColor, engraveColor }: { result: GenerateResult; cutColor: string; engraveColor: string }) {
  const pad = 2;
  const { width, height } = result;
  return (
    <svg
      className="parts__svg"
      role="img"
      aria-label={t('parts.preview.alt')}
      viewBox={`${-pad} ${-pad} ${width + 2 * pad} ${height + 2 * pad}`}
      preserveAspectRatio="xMidYMid meet"
      data-testid="parts-preview"
    >
      <g transform={`translate(0 ${height}) scale(1 -1)`} fill="none" strokeLinejoin="round" strokeLinecap="round">
        {result.parts.map((p) => (
          <g key={p.key} data-part={p.key}>
            <title>{p.name}</title>
            {p.cuts.map((c, i) => (
              <path key={`c${i}`} d={d(c)} stroke={cutColor} strokeWidth={1.4} vectorEffect="non-scaling-stroke" />
            ))}
            {p.engraves.map((c, i) => (
              <path key={`e${i}`} d={d(c)} stroke={engraveColor} strokeWidth={1.2} vectorEffect="non-scaling-stroke" />
            ))}
          </g>
        ))}
      </g>
    </svg>
  );
}
