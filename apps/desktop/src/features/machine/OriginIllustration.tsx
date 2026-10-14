import type { Point } from '@/api/types';
import { t } from '@/i18n';
import { workToDoc, type DeviceFrame } from '@/lib/device';
import { anchorFraction, type Placement } from './placement';

interface Props {
  device: DeviceFrame;
  placement: Placement;
  /** Punto 0..8 marcado en el rectángulo del trabajo. */
  jobOrigin: number;
  /** Cabezal en posición de trabajo de la máquina, o null si no hay conexión. */
  head: Point | null;
  /** Origen de usuario en posición de trabajo, o null. */
  userOrigin: Point | null;
  /** Mostrar la leyenda debajo (en el diálogo de inicio es más corta). */
  legend?: boolean;
}

/** Esquina de la cama donde la máquina tiene su X0 Y0, en coordenadas de documento. */
function machineCorner(device: DeviceFrame): Point {
  return workToDoc(device, [0, 0]);
}

/**
 * Mini ilustración de la cama vista desde arriba: dónde quedará el trabajo respecto al cabezal y al
 * origen de la máquina. Es el mismo cálculo que usan "Encuadrar" e "Iniciar" (`computeJobPlacement`).
 */
export function OriginIllustration({ device, placement, jobOrigin, head, userOrigin, legend = true }: Props) {
  const [bw, bh] = device.bed;
  const pad = Math.max(bw, bh) * 0.06;
  // Se amplía el dibujo si el trabajo se sale de la cama, para que se vea por dónde.
  const o = placement.overflow;
  const x0 = -pad - o.left;
  const y0 = -pad - o.top;
  const w = bw + 2 * pad + o.left + o.right;
  const h = bh + 2 * pad + o.top + o.bottom;
  // SVG: Y hacia abajo. Documento: Y hacia arriba.
  const sy = (y: number) => bh - y;
  const mark = Math.max(bw, bh) * 0.028;
  const corner = machineCorner(device);
  const headDoc = head ? workToDoc(device, head) : null;
  const userDoc = userOrigin ? workToDoc(device, userOrigin) : null;
  const rect = placement.rect;
  const stroke = Math.max(bw, bh) * 0.004;

  return (
    <figure className="mc-illus">
      <svg viewBox={`${x0} ${y0} ${w} ${h}`} role="img" aria-label={t('machine.illus.label')} preserveAspectRatio="xMidYMid meet">
        <rect x={0} y={0} width={bw} height={bh} fill="var(--cv-bed)" stroke="var(--cv-bed-edge)" strokeWidth={stroke} />
        {/* origen de la máquina: una L en la esquina donde el G-code vale X0 Y0 */}
        <path
          d={`M ${corner[0] + (corner[0] === 0 ? 1 : -1) * mark * 2} ${sy(corner[1])} H ${corner[0]} V ${sy(corner[1]) - (corner[1] === 0 ? 1 : -1) * mark * 2}`}
          stroke="var(--cv-origin)"
          strokeWidth={stroke * 2.5}
          fill="none"
        />
        {rect && (
          <g>
            <rect
              x={rect[0][0]}
              y={sy(rect[1][1])}
              width={rect[1][0] - rect[0][0]}
              height={rect[1][1] - rect[0][1]}
              fill={placement.fits ? 'var(--accent-subtle)' : 'var(--danger-subtle)'}
              stroke={placement.fits ? 'var(--accent)' : 'var(--danger)'}
              strokeWidth={stroke * 1.6}
              strokeDasharray={placement.target ? undefined : `${mark * 0.6} ${mark * 0.4}`}
            />
            {/* los 9 puntos del trabajo; el elegido va relleno */}
            {Array.from({ length: 9 }, (_, i) => {
              const [fx, fy] = anchorFraction(i);
              const cx = rect[0][0] + fx * (rect[1][0] - rect[0][0]);
              const cy = sy(rect[1][1]) + fy * (rect[1][1] - rect[0][1]);
              const active = i === jobOrigin && placement.target !== null;
              return (
                <circle
                  key={i}
                  cx={cx}
                  cy={cy}
                  r={active ? mark * 0.7 : mark * 0.28}
                  fill={active ? 'var(--accent)' : 'var(--text-faint)'}
                  stroke={active ? 'var(--on-accent)' : 'none'}
                  strokeWidth={stroke}
                />
              );
            })}
          </g>
        )}
        {userDoc && (
          <path
            d={`M ${userDoc[0]} ${sy(userDoc[1]) - mark} l ${mark} ${mark} l ${-mark} ${mark} l ${-mark} ${-mark} z`}
            fill="var(--warn)"
            stroke="var(--bg-chrome)"
            strokeWidth={stroke}
          />
        )}
        {headDoc && (
          <g stroke="var(--cv-head)" strokeWidth={stroke * 2} fill="none">
            <circle cx={headDoc[0]} cy={sy(headDoc[1])} r={mark * 0.75} />
            <path d={`M ${headDoc[0] - mark * 1.3} ${sy(headDoc[1])} h ${mark * 2.6} M ${headDoc[0]} ${sy(headDoc[1]) - mark * 1.3} v ${mark * 2.6}`} />
          </g>
        )}
      </svg>
      {legend && (
        <figcaption className="mc-illus__legend">
          <span><i className="mc-key mc-key--bed" aria-hidden="true" />{t('machine.illus.bed')}</span>
          <span><i className="mc-key mc-key--origin" aria-hidden="true" />{t('machine.illus.machineOrigin')}</span>
          <span><i className="mc-key mc-key--job" aria-hidden="true" />{t('machine.illus.job')}</span>
          <span><i className="mc-key mc-key--head" aria-hidden="true" />{t('machine.illus.head')}</span>
          <span><i className="mc-key mc-key--user" aria-hidden="true" />{t('machine.illus.userOrigin')}</span>
        </figcaption>
      )}
    </figure>
  );
}
