import { useState } from 'react';
import { t } from '@/i18n';
import { usePref, useStore } from '@/store';
import { Modal } from './Modal';
import { WarnLine } from './RuidaDiff';

/** Texto que se escribe para encender las escrituras experimentales (sin distinguir mayúsculas ni acentos). */
export const WRITES_PHRASE = 'ENTIENDO';

export function phraseMatches(text: string): boolean {
  return text.trim().normalize('NFD').replace(/[̀-ͯ]/g, '').toUpperCase() === WRITES_PHRASE;
}

/**
 * Preferencia «Escrituras experimentales en Ruida» (apagada de fábrica, nunca se importa de un
 * paquete). Encenderla pide leer el aviso y escribir «ENTIENDO» o marcar una casilla; apagarla es
 * inmediato. Sin ella, todo lo que escribe en la controladora Ruida queda deshabilitado y el motor
 * lo rechaza; leer, listar documentos y exportar un respaldo no la necesitan.
 */
export function RuidaWritesToggle() {
  const on = usePref('ruidaExperimentalWrites');
  const setPref = useStore((s) => s.setPref);
  const [asking, setAsking] = useState(false);
  const [typed, setTyped] = useState('');
  const [checked, setChecked] = useState(false);
  const close = () => {
    setAsking(false);
    setTyped('');
    setChecked(false);
  };
  const ready = checked || phraseMatches(typed);

  return (
    <div className="ruida-writes">
      <label className="mc-check">
        <input
          type="checkbox"
          checked={on}
          onChange={(e) => {
            if (!e.target.checked) setPref('ruidaExperimentalWrites', false);
            else setAsking(true);
          }}
        />
        {t('ruida.exp.label')}
      </label>
      <p className="hint">{on ? t('ruida.exp.on') : t('ruida.exp.help')}</p>
      {asking && (
        <Modal
          title={t('ruida.exp.dlg.title')}
          subtitle={t('ruida.set.experimental')}
          tone="danger"
          size="md"
          onClose={close}
          footer={
            <>
              <button type="button" className="btn" data-autofocus onClick={close}>{t('ruida.doc.cancel')}</button>
              <button
                type="button"
                className="btn btn--danger"
                disabled={!ready}
                onClick={() => {
                  setPref('ruidaExperimentalWrites', true);
                  close();
                }}
              >
                {t('ruida.exp.dlg.enable')}
              </button>
            </>
          }
        >
          <div className="rset">
            <WarnLine role="alert">{t('ruida.exp.dlg.body')}</WarnLine>
            <p>{t('ruida.exp.dlg.list')}</p>
            <label className="field">
              <span className="field__label">{t('ruida.exp.dlg.type', { phrase: WRITES_PHRASE })}</span>
              <input className="input" type="text" value={typed} autoComplete="off" onChange={(e) => setTyped(e.target.value)} />
            </label>
            <label className="rset-confirm">
              <input type="checkbox" checked={checked} onChange={(e) => setChecked(e.target.checked)} />
              {t('ruida.exp.dlg.check')}
            </label>
          </div>
        </Modal>
      )}
    </div>
  );
}
