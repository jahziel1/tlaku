import { Repeat, X } from 'lucide-react';
import { createRoot, type Root } from 'react-dom/client';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { useReturnFocus } from '@/ui/useReturnFocus';
import { isRepeating, resetRepeat, startRepeat, stopRepeat, useRepeat } from './repeat';
import './feeder.css';

/** Progreso de la tanda de repeticiones: qué repetición va, el avance del material y cómo detener. */
export function RepeatPanel({ onClose }: { onClose: () => void }) {
  const { phase, index, done, total, message, config } = useRepeat();
  const running = phase === 'sending' || phase === 'feeding';
  useReturnFocus();
  const line =
    phase === 'sending'
      ? t('tiling.repeat.sending', { i: index + 1, n: total })
      : phase === 'feeding'
        ? t('tiling.repeat.feeding', { mm: fmt(config.feedMm, 1), i: index + 1, n: total })
        : phase === 'finished'
          ? t('tiling.repeat.finished', { n: total })
          : phase === 'cancelled'
            ? t('tiling.repeat.cancelled', { done, n: total })
            : phase === 'failed'
              ? t('tiling.repeat.failed', { i: index + 1, why: message ?? t('tiling.err.job') })
              : '';
  return (
    <section
      className="rep-panel"
      role="dialog"
      aria-label={t('tiling.repeat.panel')}
      data-phase={phase}
      onKeyDown={(e) => {
        if (e.key === 'Escape' && !running) onClose();
      }}
    >
      <header className="rep-panel__head">
        <h2 className="panel-title"><Repeat size={15} aria-hidden="true" />{t('tiling.repeat.panel')}</h2>
        <button type="button" className="icon-btn" disabled={running} aria-label={t('tiling.repeat.close')} title={running ? t('tiling.repeat.closeBusy') : t('tiling.repeat.close')} onClick={onClose}>
          <X size={16} aria-hidden="true" />
        </button>
      </header>
      <progress className="rep-panel__bar" max={total} value={done} aria-label={t('tiling.repeat.progress', { done, n: total })} />
      <p className="num" role="status">{t('tiling.repeat.progress', { done, n: total })}</p>
      <p className={phase === 'failed' ? 'warn-line' : 'hint'} role={phase === 'failed' ? 'alert' : 'status'}>{line}</p>
      <div className="rep-panel__actions">
        {running ? (
          <button type="button" className="btn" onClick={() => void stopRepeat()}>{t('tiling.repeat.stop')}</button>
        ) : (
          <button type="button" className="btn btn--accent" onClick={onClose}>{t('tiling.repeat.close')}</button>
        )}
      </div>
    </section>
  );
}

let root: Root | null = null;
let host: HTMLElement | null = null;

/** Muestra el panel de avance en su propia raíz de React (el diálogo de inicio ya se cerró). */
export function showRepeatPanel(): void {
  if (root || typeof document === 'undefined') return;
  host = document.createElement('div');
  document.body.appendChild(host);
  root = createRoot(host);
  root.render(<RepeatPanel onClose={hideRepeatPanel} />);
}

export function hideRepeatPanel(): void {
  if (isRepeating()) return;
  resetRepeat();
  const r = root;
  const h = host;
  root = null;
  host = null;
  // Se desmonta fuera del manejador del evento que lo cerró.
  queueMicrotask(() => {
    r?.unmount();
    h?.remove();
  });
}

/** Empieza la tanda de repeticiones desde el diálogo de inicio y abre el panel de avance. */
export function launchRepeat(): Promise<void> {
  return startRepeat(showRepeatPanel);
}
