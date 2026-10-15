import { ArrowDown, ArrowUp, FileUp, Layers, Pause, Play, RotateCcw, SkipForward, Square, Trash2, X } from 'lucide-react';
import { useId } from 'react';
import { useNow } from '@/features/machine/hooks';
import { jobFraction } from '@/features/live/progress';
import { t, type I18nKey } from '@/i18n';
import { fmtClock } from '@/lib/platform';
import { useStore } from '@/store';
import { Modal } from '@/ui/Modal';
import {
  MAX_COPIES,
  currentCopy,
  currentItem,
  remainingEstimate,
  totals,
  type QueueItem,
  type QueueRun,
  type QueueState,
} from './queueModel';
import {
  addAllTabs,
  addFilesFromDialog,
  addTab,
  clearFinishedFromQueue,
  estimateOne,
  isMissing,
  moveInQueue,
  pauseQueue,
  removeFromQueue,
  restoreInQueue,
  resumeQueue,
  setItemCopies,
  setItemNote,
  skipInQueue,
  startQueue,
  stopQueue,
} from './queue';
import { useQueue } from './queueStore';
import './queue.css';

/** Texto corto del estado de la cola. */
export function runLabel(run: QueueRun): string {
  return t(`queue.run.${run}` as I18nKey);
}

/** Avance de la cola entera: copias terminadas más lo que lleva el trabajo en curso. */
export function overallFraction(q: QueueState, currentJobFraction: number): number {
  const { done, total } = totals(q);
  if (total === 0) return 0;
  return Math.min(1, (done + (q.run === 'running' ? currentJobFraction : 0)) / total);
}

function ItemRow({ item, index, last, q, editable }: { item: QueueItem; index: number; last: number; q: QueueState; editable: boolean }) {
  const running = q.run === 'running' && q.currentId === item.id;
  const waiting = q.run === 'awaiting' && q.currentId === item.id;
  const missing = isMissing(item);
  const one = estimateOne(item);
  const status = running ? t('queue.item.running') : waiting ? t('queue.item.waiting') : t(`queue.item.${item.status}` as I18nKey);
  return (
    <li className="queue__item" data-status={item.status} data-current={running || waiting ? 'true' : undefined}>
      <span className="queue__index num" aria-hidden="true">{index + 1}</span>
      <div className="queue__main">
        <div className="queue__name">
          <strong>{item.name}</strong>
          <span className="queue__src">{item.source === 'tab' ? t('queue.item.tab') : t('queue.item.file')}</span>
          {missing && <span className="queue__bad">{t('queue.item.missing')}</span>}
        </div>
        <div className="queue__meta">
          <span className={`queue__status queue__status--${running ? 'running' : waiting ? 'waiting' : item.status}`}>{status}</span>
          <span className="num">{t('queue.item.progress', { done: item.done, copies: item.copies })}</span>
          <span className="num">{one === null ? t('queue.item.noEstimate') : t('queue.item.estimate', { time: fmtClock(one) })}</span>
        </div>
        <input
          type="text"
          className="input queue__note"
          value={item.note}
          maxLength={200}
          placeholder={t('queue.item.notePlaceholder')}
          aria-label={t('queue.item.note', { name: item.name })}
          onChange={(e) => setItemNote(item.id, e.target.value)}
        />
      </div>
      <label className="queue__copies">
        <span>{t('queue.item.copies')}</span>
        <input
          type="number"
          className="input num"
          min={1}
          max={MAX_COPIES}
          value={item.copies}
          disabled={running}
          aria-label={t('queue.item.copiesOf', { name: item.name })}
          onChange={(e) => setItemCopies(item.id, Number(e.target.value))}
        />
      </label>
      <div className="queue__actions" role="group" aria-label={t('queue.item.actions', { name: item.name })}>
        <button type="button" className="icon-btn" disabled={index === 0 || !editable} title={t('queue.item.up')} aria-label={t('queue.item.up')} onClick={() => moveInQueue(item.id, index - 1)}>
          <ArrowUp size={14} aria-hidden="true" />
        </button>
        <button type="button" className="icon-btn" disabled={index === last || !editable} title={t('queue.item.down')} aria-label={t('queue.item.down')} onClick={() => moveInQueue(item.id, index + 1)}>
          <ArrowDown size={14} aria-hidden="true" />
        </button>
        {item.status === 'skipped' || item.status === 'failed' ? (
          <button type="button" className="icon-btn" title={t('queue.item.restore')} aria-label={t('queue.item.restore')} onClick={() => restoreInQueue(item.id)}>
            <RotateCcw size={14} aria-hidden="true" />
          </button>
        ) : (
          <button type="button" className="icon-btn" disabled={running || item.status === 'done'} title={t('queue.item.skip')} aria-label={t('queue.item.skip')} onClick={() => skipInQueue(item.id)}>
            <SkipForward size={14} aria-hidden="true" />
          </button>
        )}
        <button type="button" className="icon-btn" disabled={running} title={t('queue.item.remove')} aria-label={t('queue.item.remove')} onClick={() => removeFromQueue(item.id)}>
          <Trash2 size={14} aria-hidden="true" />
        </button>
      </div>
    </li>
  );
}

/** Contenido de la gestión de la cola: lista ordenable, copias, avisos por paso, progreso total y controles. */
export function QueueBody() {
  const q = useQueue((s) => s.q);
  const tabs = useStore((s) => s.tabs);
  // Para que cambien los estimados al calcularse una vista previa y el avance mientras corre un trabajo.
  useStore((s) => s.preview);
  const now = useNow(q.run === 'running');
  const jobFrac = useStore((s) => (q.run === 'running' ? jobFraction(s, now) : 0));
  const { done, total } = totals(q);
  const est = remainingEstimate(q, estimateOne);
  const fraction = overallFraction(q, jobFrac);
  const percent = Math.round(fraction * 100);
  const active = q.run === 'awaiting' || q.run === 'running';
  const hasPending = q.items.some((i) => i.status === 'pending' && i.done < i.copies);
  const canStart = !active && q.run !== 'paused' && hasPending;
  const cur = currentItem(q);
  const addable = tabs.filter((tab) => !q.items.some((i) => i.tabId === tab.id)).length;

  return (
    <>
      <p className="hint queue__safety" role="note">{t('queue.safety')}</p>

      <div className="queue__toolbar" role="group" aria-label={t('queue.add.label')}>
        <button type="button" className="btn" disabled={addable === 0} onClick={() => addTab(useStore.getState().activeTabId)}>
          <Layers size={14} aria-hidden="true" />
          {t('queue.add.active')}
        </button>
        <button type="button" className="btn" disabled={addable === 0} onClick={() => addAllTabs()}>
          {t('queue.add.allTabs', { n: addable })}
        </button>
        <button type="button" className="btn" onClick={() => void addFilesFromDialog()}>
          <FileUp size={14} aria-hidden="true" />
          {t('queue.add.files')}
        </button>
      </div>

      {q.items.length === 0 ? (
        <p className="hint" role="status">{t('queue.empty')}</p>
      ) : (
        <ol className="queue__list" aria-label={t('queue.list')}>
          {q.items.map((item, i) => (
            <ItemRow key={item.id} item={item} index={i} last={q.items.length - 1} q={q} editable />
          ))}
        </ol>
      )}

      <section className="queue__summary" aria-labelledby="queue-summary">
        <h3 id="queue-summary" className="mc-sub">{t('queue.summary')}</h3>
        <div className="queue__bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent} aria-label={t('queue.progress')}>
          <i style={{ width: `${percent}%` }} />
        </div>
        <p className="num queue__line">
          {t('queue.summary.line', { done, total, percent })}
          {' · '}
          {est.seconds > 0 || est.unknown === 0
            ? t(est.unknown > 0 ? 'queue.summary.etaPartial' : 'queue.summary.eta', { time: fmtClock(est.seconds), n: est.unknown })
            : t('queue.summary.etaUnknown')}
        </p>
        <p className="queue__state" role="status">
          <strong>{runLabel(q.run)}</strong>
          {cur && (q.run === 'awaiting' || q.run === 'running' || q.run === 'paused') && <> · {t('queue.summary.current', { name: cur.name, copy: currentCopy(q), copies: cur.copies })}</>}
          {q.run === 'stopped' && q.stopMessage && <> · {q.stopMessage}</>}
          {q.run === 'stopped' && !q.stopMessage && q.stopReason && <> · {t(`queue.stop.${q.stopReason}` as I18nKey)}</>}
          {q.pauseAfter && <> · {t('queue.pauseAfter')}</>}
        </p>
      </section>

      <div className="queue__controls" role="group" aria-label={t('queue.controls')}>
        {canStart && (
          <button type="button" className="btn btn--accent" onClick={startQueue}>
            <Play size={14} aria-hidden="true" />
            {t(q.run === 'stopped' ? 'queue.restart' : 'queue.start')}
          </button>
        )}
        {q.run === 'awaiting' && (
          <button type="button" className="btn btn--accent" onClick={() => useQueue.getState().setStepOpen(true)}>
            {t('queue.reopenStep')}
          </button>
        )}
        {q.run === 'paused' ? (
          <button type="button" className="btn" onClick={resumeQueue}>
            <Play size={14} aria-hidden="true" />
            {t('queue.resume')}
          </button>
        ) : (
          (q.run === 'awaiting' || q.run === 'running') && (
            <button type="button" className="btn" disabled={q.pauseAfter} onClick={pauseQueue}>
              <Pause size={14} aria-hidden="true" />
              {q.run === 'running' ? t('queue.pauseAfterBtn') : t('queue.pause')}
            </button>
          )
        )}
        {(active || q.run === 'paused') && (
          <button type="button" className="btn" onClick={stopQueue}>
            <Square size={14} aria-hidden="true" />
            {t('queue.stop')}
          </button>
        )}
      </div>
    </>
  );
}

/** Diálogo de gestión de la cola guiada (se abre con el comando `queue.open`). */
export function QueueDialog() {
  const titleId = useId();
  const close = () => useQueue.getState().setDialogOpen(false);
  const finished = useQueue((s) => s.q.items.some((i) => i.status === 'done' || i.status === 'skipped'));
  return (
    <Modal labelId={titleId} onClose={close} portal closeOnBackdrop backdropClassName="mc-backdrop" cardClassName="mc-modal queue-modal" initialFocus="[data-autofocus]" passKey={(e) => (e.metaKey || e.ctrlKey) && e.key === '.'}>
      <header className="mc-modal__head">
        <div>
          <h2 id={titleId} className="mc-modal__title">{t('queue.title')}</h2>
          <p className="hint">{t('queue.subtitle')}</p>
        </div>
        <button type="button" className="icon-btn" title={t('queue.close')} aria-label={t('queue.close')} onClick={close}>
          <X size={16} aria-hidden="true" />
        </button>
      </header>
      <div className="mc-modal__body">
        <QueueBody />
      </div>
      <footer className="mc-modal__foot">
        <button type="button" className="btn" disabled={!finished} onClick={clearFinishedFromQueue}>
          {t('queue.clearFinished')}
        </button>
        <span className="queue__spacer" />
        <button type="button" className="btn" data-autofocus onClick={close}>{t('queue.close')}</button>
      </footer>
    </Modal>
  );
}
