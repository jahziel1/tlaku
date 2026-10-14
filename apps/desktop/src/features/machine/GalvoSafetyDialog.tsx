import { useState } from 'react';
import type { DeviceProfile, GalvoSafetyAck } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { selectActiveDevice, useStore } from '@/store';
import { completeAck, SOURCE_WAVELENGTH_NM } from './galvoPorts';
import { Modal } from './Modal';

const CHECKS: { id: 'eyewear' | 'enclosure' | 'emergencyStop'; label: I18nKey }[] = [
  { id: 'eyewear', label: 'galvo.safety.eyewear' },
  { id: 'enclosure', label: 'galvo.safety.enclosure' },
  { id: 'emergencyStop', label: 'galvo.safety.estop' },
];

export type AckChecks = { eyewear: boolean; enclosure: boolean; emergencyStop: boolean };

/** Las tres casillas empiezan desmarcadas SIEMPRE: el reconocimiento no se recuerda. */
export const NO_CHECKS: AckChecks = { eyewear: false, enclosure: false, emergencyStop: false };

/** El texto y las tres casillas del reconocimiento (sin el marco del diálogo). */
export function GalvoSafetyBody({ device, checked, onCheck }: { device: Pick<DeviceProfile, 'galvo'>; checked: AckChecks; onCheck: (id: keyof AckChecks, on: boolean) => void }) {
  const nm = SOURCE_WAVELENGTH_NM[device.galvo.source];
  const params = { nm, watts: fmt(device.galvo.watts, 0) };
  return (
    <>
      <p><strong>{t('galvo.safety.class4')}</strong></p>
      <p>{t('galvo.safety.invisible', params)}</p>
      <fieldset className="mc-group mc-galvo-ack">
        <legend>{t('galvo.safety.confirm')}</legend>
        {CHECKS.map((c) => (
          <label key={c.id} className="mc-check">
            <input type="checkbox" checked={checked[c.id]} onChange={(e) => onCheck(c.id, e.target.checked)} />
            <span>{t(c.label, params)}</span>
          </label>
        ))}
      </fieldset>
      <p className="hint">{t('galvo.safety.everyTime')}</p>
      <p className="hint">{t('galvo.safety.noSource')}</p>
    </>
  );
}

/**
 * Reconocimiento de seguridad ANTES de abrir una tarjeta galvo real. Las tres casillas empiezan
 * desmarcadas SIEMPRE (no se recuerda de una conexión a otra) y «Conectar» no se habilita hasta marcarlas
 * todas. Solo con las tres se entrega el reconocimiento; cancelar o cerrar no conecta.
 */
export function GalvoSafetyDialogView({ device, onAnswer }: { device: Pick<DeviceProfile, 'name' | 'galvo'>; onAnswer: (ack: GalvoSafetyAck | null) => void }) {
  const [checked, setChecked] = useState<AckChecks>(NO_CHECKS);
  const ack = completeAck(checked);
  return (
    <Modal
      title={t('galvo.safety.title')}
      subtitle={device.name}
      tone="danger"
      onClose={() => onAnswer(null)}
      footer={
        <>
          <button type="button" className="btn" data-autofocus onClick={() => onAnswer(null)}>{t('machine.dlg.cancel')}</button>
          <button type="button" className="btn btn--danger" disabled={ack === null} title={ack === null ? t('galvo.safety.needAll') : undefined} onClick={() => ack && onAnswer(ack)}>
            {t('galvo.safety.connect')}
          </button>
        </>
      }
    >
      <GalvoSafetyBody device={device} checked={checked} onCheck={(id, on) => setChecked((prev) => ({ ...prev, [id]: on }))} />
    </Modal>
  );
}

/** El diálogo de la conexión en curso: lo abre `connect` y la respuesta desbloquea (o cancela) la conexión. */
export function GalvoSafetyDialog() {
  const pending = useStore((s) => s.galvoAckPending);
  const id = useStore((s) => s.galvoAckId);
  const answer = useStore((s) => s.answerGalvoAck);
  const device = useStore(selectActiveDevice);
  if (!pending) return null;
  return <GalvoSafetyDialogView key={id} device={device} onAnswer={answer} />;
}
