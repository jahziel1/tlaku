import { TriangleAlert, X } from 'lucide-react';
import { useEffect, useState } from 'react';
import { getApi } from '@/api';
import type { DeviceProfile, LhyPreview, LhyWarning } from '@/api/types';
import { resolveJobArgs } from '@/features/variables/jobResolve';
import { t, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { fmt, fmtClock } from '@/lib/platform';
import { useStore } from '@/store';
import { buildJobArgs } from '@/store/selectors';
import { LHY_UNCONFIRMED, selectIsLhy } from './lhyPorts';

/** Etiqueta «Experimental» con el aviso completo como ayuda. */
export function LhyExperimentalBadge() {
  return (
    <span className="badge" data-tone="warn" title={t('lhymicro.experimental.note')}>
      <span className="badge__dot" aria-hidden="true" />
      {t('lhymicro.experimental')}
    </span>
  );
}

/** Texto del botón de confirmar el envío con un K40; `null` con otro tipo de máquina. */
export function lhyConfirmKey(device: Pick<DeviceProfile, 'kind'>): I18nKey | null {
  return device.kind === 'lhy_micro' ? 'lhymicro.start.confirm' : null;
}

/** El aviso del codificador escrito en el idioma de la interfaz. */
export function warningText(w: LhyWarning): string {
  const layer = w.layer ?? t('lhymicro.warn.block', { block: w.block + 1 });
  const key: I18nKey = w.kind === 'dwell_skipped' && w.fires ? 'lhymicro.warn.dwell_fires' : `lhymicro.warn.${w.kind}`;
  return t(key, { layer, value: fmt(w.value ?? 0, 0) });
}

/** Los avisos sin repetir (varias capas iguales dan la misma frase una sola vez). */
export function warningLines(warnings: readonly LhyWarning[]): string[] {
  return [...new Set(warnings.map(warningText))];
}

type PreviewState = { status: 'loading' } | { status: 'ready'; preview: LhyPreview } | { status: 'error'; message: string };

/** El resumen y los avisos de un trabajo ya calculado. */
export function LhyPreviewBody({ state }: { state: PreviewState }) {
  if (state.status === 'loading') return <p className="hint" role="status">{t('lhymicro.start.computing')}</p>;
  if (state.status === 'error') {
    return (
      <p className="warn-line" role="alert">
        <TriangleAlert size={14} aria-hidden="true" />
        <span>{t('lhymicro.start.error', { error: state.message })}</span>
      </p>
    );
  }
  const p = state.preview;
  const lines = warningLines(p.warnings);
  return (
    <>
      <p className="hint num">
        {t('lhymicro.start.summary', { packets: fmt(p.packets, 0), burn: fmt(p.burn_mm, 0), travel: fmt(p.travel_mm, 0), time: fmtClock(p.est_seconds) })}
      </p>
      {p.homes_first && (
        <p className="warn-line">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('lhymicro.start.homeFirst')}</span>
        </p>
      )}
      <h4 className="mc-sub">{t('lhymicro.start.warnings')}</h4>
      {lines.length === 0 ? (
        <p className="hint">{t('lhymicro.start.noWarnings')}</p>
      ) : (
        <ul className="mc-warnings">
          {lines.map((line) => (
            <li key={line} className="warn-line">
              <TriangleAlert size={14} aria-hidden="true" />
              <span>{line}</span>
            </li>
          ))}
        </ul>
      )}
    </>
  );
}

/** Pide al motor el trabajo codificado (sin enviar nada) y lo muestra. */
function LhyStartBody() {
  const [state, setState] = useState<PreviewState>({ status: 'loading' });
  useEffect(() => {
    let alive = true;
    void (async () => {
      try {
        const preview = await getApi().lhyPreview(await resolveJobArgs(buildJobArgs(useStore.getState())));
        if (alive) setState({ status: 'ready', preview });
      } catch (e) {
        if (alive) setState({ status: 'error', message: errorMessage(e) });
      }
    })();
    return () => {
      alive = false;
    };
  }, []);
  return <LhyPreviewBody state={state} />;
}

/**
 * Aviso del diálogo de inicio con un K40: es experimental, la potencia no se controla por
 * software (se ajusta con la perilla) y los avisos del codificador, ANTES de enviar.
 */
export function LhyStartNotice({ device }: { device: Pick<DeviceProfile, 'kind'> }) {
  if (device.kind !== 'lhy_micro') return null;
  return (
    <section className="mc-lhy-start" role="note" aria-label={t('lhymicro.start.title')}>
      <div className="warn-line">
        <TriangleAlert size={14} aria-hidden="true" />
        <div>
          <p><strong>{t('lhymicro.start.title')}.</strong> {t('lhymicro.experimental.note')}</p>
          <p>{t('lhymicro.power.note')}</p>
        </div>
      </div>
      <LhyStartBody />
    </section>
  );
}

/** Motivo por el que algo de GRBL (consola, ajustes en vivo...) no aplica al K40. */
export function LhyNotApplicable({ reason }: { reason: 'lhymicro.console.reason' | 'lhymicro.override.reason' | 'lhymicro.mset.reason' | 'lhymicro.axes.reason' | 'lhymicro.fire.reason' }) {
  return <p className="hint" role="note">{t(reason)}</p>;
}

/** ¿El mensaje de un trabajo dice que detener no se confirmó? (el motor deja `LHY_UNCONFIRMED`). */
export function isUnconfirmedStop(...messages: (string | null | undefined)[]): boolean {
  return messages.some((m) => !!m && m.includes(LHY_UNCONFIRMED));
}

/**
 * Aviso destacado cuando la placa no confirmó detenerse ni apagar el láser: la única salida segura
 * es el paro de emergencia. Queda hasta descartarlo o hasta que llegue otro mensaje. Recibe los
 * mensajes del trabajo para poder probarse por separado.
 */
export function LhyStopNoticeView({ message, error }: { message: string; error: string | null }) {
  const [dismissed, setDismissed] = useState<string | null>(null);
  const text = isUnconfirmedStop(message, error) ? `${error ?? ''}|${message}` : null;
  if (text === null || dismissed === text) return null;
  return (
    <div className="mc-joberror mc-lhy-stop" role="alert">
      <TriangleAlert size={18} aria-hidden="true" />
      <div>
        <p className="mc-joberror__title">{t('lhymicro.stop.title')}</p>
        <p>{t('lhymicro.stop.text')}</p>
      </div>
      <button type="button" className="icon-btn" title={t('lhymicro.stop.dismiss')} aria-label={t('lhymicro.stop.dismiss')} onClick={() => setDismissed(text)}>
        <X size={14} aria-hidden="true" />
      </button>
    </div>
  );
}

/** El aviso del paro de emergencia, solo con un K40 conectado. */
export function LhyStopNotice() {
  const lhy = useStore(selectIsLhy);
  const message = useStore((s) => s.jobMessage);
  const error = useStore((s) => s.jobError);
  return lhy ? <LhyStopNoticeView message={message} error={error} /> : null;
}
