import { House, TriangleAlert } from 'lucide-react';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { selectIsLhy } from './lhyPorts';

/**
 * Aviso de que la posición puede haberse perdido (la máquina se detuvo en movimiento o llegó
 * `ALARM:3`). Queda hasta hacer homing, fijar el origen o descartarlo. Con la máquina en alarma
 * lo dice ya el banner rojo, así que no se repite. Si la placa se reinició sola, el motor marca la
 * posición «sin confirmar» y el aviso no se puede descartar: solo lo quita un homing.
 */
export function PositionLostNote() {
  const unconfirmed = useStore((s) => s.connection.connected && s.status?.position_unconfirmed === true);
  const show = useStore((s) => (s.positionLost || unconfirmed) && s.connection.connected && s.status?.state !== 'alarm');
  const lhy = useStore(selectIsLhy);
  const dismiss = useStore((s) => s.dismissPositionLost);
  const openDialog = useStore((s) => s.openDialog);
  if (!show) return null;
  return (
    <div className="mc-positionlost" role="alert">
      <p className="warn-line">
        <TriangleAlert size={14} aria-hidden="true" />
        <strong>{t(unconfirmed ? 'machine.positionUnconfirmed.title' : 'machine.positionLost.title')}</strong>
      </p>
      <p>{t(unconfirmed ? 'machine.positionUnconfirmed.text' : lhy ? 'lhymicro.positionLost.text' : 'machine.positionLost.text')}</p>
      <div className="row">
        <button type="button" className="btn grow" onClick={() => openDialog({ kind: 'alarm', action: 'home' })}>
          <House size={14} aria-hidden="true" />
          {t(lhy ? 'lhymicro.positionLost.home' : 'machine.positionLost.home')}
        </button>
        {!unconfirmed && <button type="button" className="btn" onClick={dismiss}>{t('machine.positionLost.dismiss')}</button>}
      </div>
    </div>
  );
}
