import { useId, useState } from 'react';
import { t } from '@/i18n';
import { Modal } from '@/ui/Modal';
import { NumberField } from '@/ui/NumberField';
import './fileio.css';

/** S con el que se propone tomar los cortes de potencia 0 (la escala habitual de GRBL: 1000 = 100 %). */
export const SUGGESTED_GCODE_POWER = 500;

/**
 * El G-code importó vacío porque todos sus cortes llegan con potencia 0 (`M3 S0`): ofrece fijar una
 * potencia «para cortes sin S» y volver a importarlo. Devuelve el valor de S, o `null` si se descarta.
 */
export function GcodePowerDialog({ file, onDone }: { file: string; onDone: (power: number | null) => void }) {
  const labelId = useId();
  const [power, setPower] = useState(SUGGESTED_GCODE_POWER);
  return (
    <Modal labelId={labelId} onClose={() => onDone(null)} backdropClassName="modal" cardClassName="modal__card" initialFocus="[data-autofocus]" guardOutside>
      <div className="modal__head">
        <h2 id={labelId} className="modal__title">{t('fileio.import.zeroPower.title')}</h2>
        <span className="fio-hint">{file}</span>
      </div>
      <div className="modal__body fio-body">
        <p>{t('fileio.import.zeroPower.text')}</p>
        <NumberField label={t('fileio.import.gcode.defaultPower')} value={power} min={1} max={100000} step={100} unit="S" onCommit={setPower} />
        <p className="fio-hint">{t('fileio.import.gcode.defaultPower.help')}</p>
      </div>
      <div className="modal__foot">
        <button type="button" className="btn" onClick={() => onDone(null)}>{t('fileio.import.cancel')}</button>
        <button type="button" className="btn btn--accent" data-autofocus onClick={() => onDone(power)}>{t('fileio.import.zeroPower.apply')}</button>
      </div>
    </Modal>
  );
}
