import { TriangleAlert } from 'lucide-react';
import { useEffect, useState } from 'react';
import { getApi } from '@/api';
import type { DeviceProfile, GalvoPreview, GalvoStepInfo } from '@/api/types';
import { resolveJobArgs } from '@/features/variables/jobResolve';
import { t, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { fmt, fmtClock } from '@/lib/platform';
import { selectActiveDevice, useStore } from '@/store';
import { buildJobArgs } from '@/store/selectors';
import { SOURCE_WAVELENGTH_NM } from './galvoPorts';
import { Modal } from './Modal';

/** Etiqueta «Experimental» con el aviso completo como ayuda. */
export function GalvoExperimentalBadge() {
  return (
    <span className="badge" data-tone="warn" title={t('galvo.experimental.note')}>
      <span className="badge__dot" aria-hidden="true" />
      {t('galvo.experimental')}
    </span>
  );
}

/** Texto del botón de confirmar el envío con un galvo; `null` con otro tipo de máquina. */
export function galvoConfirmKey(device: Pick<DeviceProfile, 'kind'>): I18nKey | null {
  return device.kind === 'galvo' ? 'galvo.start.confirm' : null;
}

/** Motivo por el que algo de GRBL (consola, ajustes en vivo, pausa...) no aplica al galvo. */
export function GalvoNotApplicable({ reason }: { reason: 'galvo.console.reason' | 'galvo.override.reason' | 'galvo.mset.reason' | 'galvo.pause.reason' | 'galvo.axes.reason' | 'galvo.fire.reason' | 'galvo.home.reason' }) {
  return <p className="hint" role="note">{t(reason)}</p>;
}

/** «Paso 2 de 6 · pieza 2 de 3 · repetición 1 de 2». */
export function stepLabel(info: GalvoStepInfo): string {
  const parts = [t('galvo.step.of', { n: info.step, total: info.steps })];
  if (info.pieces > 1) parts.push(t('galvo.step.piece', { n: info.piece, total: info.pieces }));
  if (info.repeats > 1) parts.push(t('galvo.step.repeat', { n: info.repeat, total: info.repeats }));
  return parts.join(' · ');
}

/**
 * Qué mover antes del paso: «X +100,00 mm · Y 0,00 mm · rotativo +45,0°». Si el rotativo del galvo del perfil activo
 * conoce los pasos del motor por vuelta, el giro también se da en pasos.
 */
export function moveText(info: GalvoStepInfo, stepsPerRev = selectActiveDevice(useStore.getState()).galvo.rotary?.steps_per_rev ?? 0): string {
  const parts: string[] = [];
  if (info.dx !== 0 || info.dy !== 0) parts.push(t('galvo.move.table', { x: signed(info.dx, 2), y: signed(info.dy, 2) }));
  if (info.rotary_deg !== 0) {
    const deg = signed(info.rotary_deg, 1);
    parts.push(stepsPerRev > 0 ? t('galvo.move.rotarySteps', { deg, steps: signed(Math.round((info.rotary_deg / 360) * stepsPerRev), 0) }) : t('galvo.move.rotary', { deg }));
  }
  return parts.join(' · ');
}

function signed(v: number, d: number): string {
  return `${v > 0 ? '+' : ''}${fmt(v, d)}`;
}

// ---------------------------------------------------------------------------
// Resumen antes de marcar
// ---------------------------------------------------------------------------

type PreviewState = { status: 'loading' } | { status: 'ready'; preview: GalvoPreview } | { status: 'error'; message: string };

/** El resumen y los avisos de un trabajo ya calculado. */
export function GalvoPreviewBody({ state }: { state: PreviewState }) {
  if (state.status === 'loading') return <p className="hint" role="status">{t('galvo.start.computing')}</p>;
  if (state.status === 'error') {
    return (
      <p className="warn-line" role="alert">
        <TriangleAlert size={14} aria-hidden="true" />
        <span>{t('galvo.start.error', { error: state.message })}</span>
      </p>
    );
  }
  const p = state.preview;
  return (
    <>
      <p className="hint num">{t('galvo.start.summary', { field: fmt(p.field_mm, 0), time: fmtClock(p.est_seconds), max: fmt(p.max_power_pct, 0) })}</p>
      {p.steps > 1 && (
        <p className="warn-line" role="status">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('galvo.start.steps', { steps: p.steps, pieces: p.pieces, repeats: p.repeats, moves: p.manual_moves })}</span>
        </p>
      )}
      <h4 className="mc-sub">{t('galvo.start.warnings')}</h4>
      {p.warnings.length === 0 ? (
        <p className="hint">{t('galvo.start.noWarnings')}</p>
      ) : (
        <ul className="mc-warnings">
          {[...new Set(p.warnings)].map((line) => (
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

/** Pide al motor el trabajo calculado (sin enviar nada) y lo muestra. */
function GalvoStartBody() {
  const [state, setState] = useState<PreviewState>({ status: 'loading' });
  useEffect(() => {
    let alive = true;
    void (async () => {
      try {
        const preview = await getApi().galvoPreview(await resolveJobArgs(buildJobArgs(useStore.getState())));
        if (alive) setState({ status: 'ready', preview });
      } catch (e) {
        if (alive) setState({ status: 'error', message: errorMessage(e) });
      }
    })();
    return () => {
      alive = false;
    };
  }, []);
  return <GalvoPreviewBody state={state} />;
}

/**
 * Aviso del diálogo de inicio con un galvo: es experimental, el haz es invisible y el láser DISPARARÁ;
 * no hay pausa (solo detener); y el resumen del marcado dividido o repetido, ANTES de enviar.
 */
export function GalvoStartNotice({ device }: { device: Pick<DeviceProfile, 'kind' | 'galvo'> }) {
  if (device.kind !== 'galvo') return null;
  const nm = SOURCE_WAVELENGTH_NM[device.galvo.source];
  return (
    <section className="mc-galvo-start" role="note" aria-label={t('galvo.start.title')}>
      <div className="warn-line">
        <TriangleAlert size={14} aria-hidden="true" />
        <div>
          <p><strong>{t('galvo.start.title')}.</strong> {t('galvo.experimental.note')}</p>
          <p className="warn-line__key">{t('galvo.start.invisible', { nm, watts: fmt(device.galvo.watts, 0) })}</p>
          <p className="warn-line__key">{t('galvo.start.noPause')}</p>
        </div>
      </div>
      <GalvoStartBody />
    </section>
  );
}

// ---------------------------------------------------------------------------
// Fuente que puede seguir encendida
// ---------------------------------------------------------------------------

/**
 * Aviso destacado y PERSISTENTE cuando la tarjeta no confirmó apagar la fuente: la única salida segura
 * es cortar la energía. No se va solo: solo lo cierra el usuario al confirmar que ya la cortó.
 */
export function GalvoStopNoticeView({ message, onDismiss }: { message: string | null; onDismiss: () => void }) {
  if (message === null) return null;
  return (
    <div className="mc-joberror mc-galvo-stop" role="alert" aria-live="assertive">
      <TriangleAlert size={20} aria-hidden="true" />
      <div>
        <p className="mc-joberror__title">{t('galvo.stop.title')}</p>
        <p>{t('galvo.stop.text')}</p>
        <p className="hint num">{message}</p>
        <button type="button" className="btn btn--danger" onClick={onDismiss}>{t('galvo.stop.dismiss')}</button>
      </div>
    </div>
  );
}

/** El aviso de cortar la energía; se ve aunque la conexión ya se haya cerrado. */
export function GalvoStopNotice() {
  const message = useStore((s) => s.galvoLaserWarning);
  const dismiss = useStore((s) => s.dismissGalvoLaserWarning);
  return <GalvoStopNoticeView message={message} onDismiss={dismiss} />;
}

/** Los pasos para cortar la energía (sin el marco del diálogo). */
export function GalvoStopSteps({ message }: { message: string }) {
  return (
    <>
      <p><strong>{t('galvo.stop.text')}</strong></p>
      <ul className="mc-list">
        {(['galvo.stop.step1', 'galvo.stop.step2', 'galvo.stop.step3'] as const).map((k) => (
          <li key={k}>{t(k)}</li>
        ))}
      </ul>
      <p className="hint num">{message}</p>
    </>
  );
}

/** Ventana que aparece en cuanto llega el aviso (el aviso del panel sigue ahí hasta confirmarlo). */
export function GalvoStopModal() {
  const message = useStore((s) => s.galvoLaserWarning);
  const dismiss = useStore((s) => s.dismissGalvoLaserWarning);
  const [seen, setSeen] = useState<string | null>(null);
  if (message === null || seen === message) return null;
  return (
    <Modal
      title={t('galvo.stop.title')}
      tone="danger"
      onClose={() => setSeen(message)}
      footer={
        <>
          <button type="button" className="btn" onClick={() => setSeen(message)}>{t('galvo.stop.later')}</button>
          <button type="button" className="btn btn--danger" data-autofocus onClick={dismiss}>{t('galvo.stop.dismiss')}</button>
        </>
      }
    >
      <GalvoStopSteps message={message} />
    </Modal>
  );
}

// ---------------------------------------------------------------------------
// Mover el material entre pasos
// ---------------------------------------------------------------------------

/** El texto de la espera para mover el material (sin el marco del diálogo). */
export function GalvoMoveBody({ info }: { info: GalvoStepInfo }) {
  return (
    <>
      <p>{t('galvo.move.text')}</p>
      <p className="mc-code num" role="status">{moveText(info)}</p>
      {info.pause_ms > 0 && <p className="hint num">{t('galvo.move.pause', { ms: fmt(info.pause_ms, 0) })}</p>}
      <p className="warn-line">
        <TriangleAlert size={14} aria-hidden="true" />
        <span>{t('galvo.move.safety')}</span>
      </p>
      <p className="hint">{t('galvo.move.noAxes')}</p>
    </>
  );
}

/** El trabajo espera que muevas el material (o gires el rotativo) y confirmes. Cerrar equivale a Detener. */
export function GalvoMoveDialogView({ info, onContinue, onStop }: { info: GalvoStepInfo; onContinue: () => void; onStop: () => void }) {
  return (
    <Modal
      title={t('galvo.move.title')}
      subtitle={stepLabel(info)}
      tone="warn"
      onClose={onStop}
      footer={
        <>
          <button type="button" className="btn btn--danger" onClick={onStop}>{t('galvo.move.stop')}</button>
          <span className="topbar__spacer" />
          <button type="button" className="btn btn--accent" data-autofocus onClick={onContinue}>{t('galvo.move.continue')}</button>
        </>
      }
    >
      <GalvoMoveBody info={info} />
    </Modal>
  );
}

export function GalvoMoveDialog() {
  const info = useStore((s) => s.galvoWait);
  const confirm = useStore((s) => s.confirmGalvoMove);
  const cancel = useStore((s) => s.cancelJob);
  if (!info) return null;
  return <GalvoMoveDialogView info={info} onContinue={() => void confirm()} onStop={() => void cancel()} />;
}
