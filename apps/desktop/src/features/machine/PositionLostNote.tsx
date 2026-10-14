import { House, TriangleAlert } from 'lucide-react';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { selectIsLhy } from './lhyPorts';

/**
 * Aviso de que la posición puede haberse perdido (la máquina se detuvo en movimiento o llegó
 * `ALARM:3`). Queda hasta hacer homing, fijar el origen o descartarlo. Con la máquina en alarma
 * lo dice ya el banner rojo, así que no se repite.
 */
export function PositionLostNote() {
  const show = useStore((s) => s.positionLost && s.connection.connected && s.status?.state !== 'alarm');
  const lhy = useStore(selectIsLhy);
  const dismiss = useStore((s) => s.dismissPositionLost);
  const openDialog = useStore((s) => s.openDialog);
  if (!show) return null;
  return (
    <div className="mc-positionlost" role="alert">
      <p className="warn-line">
        <TriangleAlert size={14} aria-hidden="true" />
        <strong>{t('machine.positionLost.title')}</strong>
      </p>
      <p>{t(lhy ? 'lhymicro.positionLost.text' : 'machine.positionLost.text')}</p>
      <div className="row">
        <button type="button" className="btn grow" onClick={() => openDialog({ kind: 'alarm', action: 'home' })}>
          <House size={14} aria-hidden="true" />
          {t(lhy ? 'lhymicro.positionLost.home' : 'machine.positionLost.home')}
        </button>
        <button type="button" className="btn" onClick={dismiss}>{t('machine.positionLost.dismiss')}</button>
      </div>
    </div>
  );
}
