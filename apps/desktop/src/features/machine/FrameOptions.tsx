import { Repeat, TriangleAlert } from 'lucide-react';
import { t } from '@/i18n';
import { selectActiveDevice, useStore } from '@/store';
import { FRAME_MAX_PCT } from '@/store/moveControl';
import { NumberField } from '@/ui/NumberField';
import { selectIsRuida } from './ruidaPorts';

/**
 * Opciones del encuadre: láser apagado (lo normal) o encendido a muy baja potencia para verlo en el
 * material, y repetirlo en bucle hasta pulsar Detener. El láser encendido nunca se recuerda entre sesiones.
 */
export function FrameOptions() {
  const laser = useStore((s) => s.frameLaser);
  const setLaser = useStore((s) => s.setFrameLaser);
  const loop = useStore((s) => s.frameLoop);
  const setLoop = useStore((s) => s.setFrameLoop);
  const looping = useStore((s) => s.frameLooping);
  const stop = useStore((s) => s.stopFrameLoop);
  // Ruida encuadra siempre con el láser apagado (cuatro saltos), nunca encendido.
  const ruida = useStore(selectIsRuida);
  const canLaser = useStore((s) => selectActiveDevice(s).dialect.s_inline) && !ruida;
  return (
    <div className="mc-frameopts">
      <label className="mc-check">
        <input type="checkbox" checked={laser.on} disabled={!canLaser} onChange={(e) => setLaser({ on: e.target.checked })} />
        <span>{t('machine.frame.laserOn')}</span>
      </label>
      {!canLaser && <p className="hint">{t(ruida ? 'ruida.frame.laserOff' : 'machine.frame.laserNo')}</p>}
      {laser.on && (
        <>
          <NumberField label={t('machine.frame.power')} unit="%" value={laser.powerPct} step={0.5} min={0.1} max={FRAME_MAX_PCT} onCommit={(powerPct) => setLaser({ powerPct })} />
          <p className="warn-line">
            <TriangleAlert size={14} aria-hidden="true" />
            <span>{t('machine.frame.laserWarn', { max: FRAME_MAX_PCT })}</span>
          </p>
        </>
      )}
      <label className="mc-check">
        <input type="checkbox" checked={loop} onChange={(e) => setLoop(e.target.checked)} />
        <span>{t('machine.frame.loop')}</span>
      </label>
      {looping && (
        <p className="hint" role="status">
          <Repeat size={13} aria-hidden="true" />{' '}{t('machine.frame.looping')}{' '}
          <button type="button" className="btn" onClick={stop}>{t('machine.frame.stopLoop')}</button>
        </p>
      )}
    </div>
  );
}
