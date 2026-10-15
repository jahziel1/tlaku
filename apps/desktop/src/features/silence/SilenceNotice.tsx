import { OctagonAlert, WifiOff } from 'lucide-react';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { isJobBusy } from '@/store/selectors';
import { silenceNotice } from './silenceModel';
import './silence.css';

/**
 * Aviso discreto (fuera de las secciones del panel Máquina) de que la placa no contesta: «La máquina no
 * contesta desde hace N s…», a partir de ~2 s y hasta que vuelva la respuesta. Tlaku NO desconecta por
 * silencio (como LightBurn, espera; la Falcon2 se detiene 2–20 s y sigue sola): el trabajo sigue sin
 * intervención. Los botones son decisión del usuario: Detener (feed hold y reinicio, como siempre) y
 * Desconectar. Pasado un minuto en pleno trabajo el aviso se vuelve más serio.
 */
export function SilenceNotice() {
  // Solo primitivos: un selector que devuelve un objeto nuevo en cada llamada entra en bucle.
  const connected = useStore((s) => s.connection.connected);
  const silenceMs = useStore((s) => s.silenceMs);
  const jobBusy = useStore(isJobBusy);
  const cancelJob = useStore((s) => s.cancelJob);
  const disconnect = useStore((s) => s.disconnect);

  const warn = silenceNotice({ connected, silenceMs, jobBusy });
  if (!warn) return null;
  const Icon = warn.serious ? OctagonAlert : WifiOff;

  return (
    <div className="silence-notices">
      <div className="silence-notice" data-kind={warn.serious ? 'serious' : 'silence'} role={warn.serious ? 'alert' : 'status'} aria-live={warn.serious ? 'assertive' : 'polite'}>
        <Icon size={16} aria-hidden="true" className="silence-notice__icon" />
        <div className="silence-notice__body">
          <p className="silence-notice__title">{warn.serious ? t('silence.serious.title') : t('silence.warn.title', { n: warn.seconds })}</p>
          {warn.serious ? (
            <p className="silence-notice__text">{t('silence.serious.text')}</p>
          ) : (
            <>
              <p className="silence-notice__text">{t('silence.warn.hint')}</p>
              {warn.duringJob && <p className="silence-notice__text">{t('silence.warn.job')}</p>}
            </>
          )}
          <div className="silence-notice__actions">
            <button type="button" className="btn" onClick={() => void cancelJob()}>
              {t('silence.stop')}
            </button>
            <button type="button" className="btn" onClick={() => void disconnect()}>
              {t('silence.disconnect')}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
