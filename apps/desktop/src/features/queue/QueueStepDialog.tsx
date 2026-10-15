import { Crosshair, Pause, SkipForward, Square } from 'lucide-react';
import { useState } from 'react';
import { CHECKLIST_ITEMS, visibleChecklist, type ChecklistItem } from '@/features/machine/checklist';
import { Modal } from '@/features/machine/Modal';
import { useRemote } from '@/features/remote/remote';
import { t } from '@/i18n';
import { fmtClock } from '@/lib/platform';
import { usePref } from '@/store';
import { currentCopy, currentItem, totals } from './queueModel';
import { confirmStep, estimateOne, frameStep, pauseQueue, skipCurrent, stopQueue } from './queue';
import { useQueue } from './queueStore';

/** Casillas del paso: lo propio de la cola más las de la lista de comprobación del diálogo de inicio. */
type StepCheck = 'material' | 'area' | ChecklistItem['id'];

/**
 * «Siguiente trabajo de la cola»: la parada obligatoria ENTRE trabajos. La cola nunca envía el siguiente por sí
 * sola: aquí una persona confirma que cambió el material o la pieza y que la máquina está lista (más la lista
 * de comprobación configurada), puede encuadrar, y solo entonces se abre el diálogo de inicio de siempre (con su
 * preflight y su lista de comprobación). Esc o cerrar no avanzan nada: la cola sigue esperando.
 */
function StepBody() {
  const q = useQueue((s) => s.q);
  const close = () => useQueue.getState().setStepOpen(false);
  const mask = usePref('jobChecklistItems');
  const remoteOn = useRemote((r) => r.status?.running === true);
  const item = currentItem(q);
  const [ticked, setTicked] = useState<ReadonlySet<StepCheck>>(new Set());
  const [busy, setBusy] = useState(false);
  if (!item || q.run !== 'awaiting') return null;

  const checks: { id: StepCheck; label: string }[] = [
    { id: 'material', label: t('queue.step.material') },
    { id: 'area', label: t('queue.step.area') },
    ...visibleChecklist(mask).map((c) => ({ id: c.id, label: t(CHECKLIST_ITEMS.find((x) => x.id === c.id)!.label) })),
  ];
  const all = checks.every((c) => ticked.has(c.id));
  const pending = checks.filter((c) => !ticked.has(c.id)).length;
  const toggle = (id: StepCheck, on: boolean) =>
    setTicked((prev) => {
      const next = new Set(prev);
      if (on) next.add(id);
      else next.delete(id);
      return next;
    });
  const { done, total } = totals(q);
  const one = estimateOne(item);

  return (
    <Modal
      title={t('queue.step.title')}
      subtitle={t('queue.step.subtitle', { n: done + 1, total, name: item.name, copy: currentCopy(q), copies: item.copies })}
      tone="warn"
      size="md"
      onClose={close}
      footer={
        <>
          <button type="button" className="btn" data-autofocus onClick={close}>{t('queue.step.later')}</button>
          <button
            type="button"
            className="btn btn--accent"
            disabled={!all || busy}
            title={all ? undefined : t('queue.step.blocked', { n: pending })}
            onClick={() => {
              setBusy(true);
              void confirmStep().finally(() => setBusy(false));
            }}
          >
            {t('queue.step.continue')}
          </button>
        </>
      }
    >
      {item.note && (
        <p className="queue__stepnote" role="note">
          <strong>{t('queue.step.note')}</strong> {item.note}
        </p>
      )}
      <p className="hint">{one === null ? t('queue.item.noEstimate') : t('queue.item.estimate', { time: fmtClock(one) })}</p>

      <section className="mc-checklist" aria-labelledby="queue-step-checks">
        <h3 id="queue-step-checks" className="mc-sub">{t('queue.step.checks')}</h3>
        <div className="mc-checklist__items">
          {checks.map((c) => (
            <label key={c.id} className="mc-check">
              <input type="checkbox" checked={ticked.has(c.id)} onChange={(e) => toggle(c.id, e.target.checked)} />
              {c.label}
            </label>
          ))}
        </div>
        <p className={all ? 'hint' : 'hint hint--danger'} role="status">
          {all ? t('queue.step.ready') : t('queue.step.blocked', { n: pending })}
        </p>
      </section>

      <div className="queue__stepactions" role="group" aria-label={t('queue.step.options')}>
        <button type="button" className="btn" disabled={busy} title={t('queue.step.frameHint')} onClick={() => void frameStep()}>
          <Crosshair size={14} aria-hidden="true" />
          {t('queue.step.frame')}
        </button>
        <button type="button" className="btn" onClick={skipCurrent}>
          <SkipForward size={14} aria-hidden="true" />
          {t('queue.step.skip')}
        </button>
        <button type="button" className="btn" onClick={pauseQueue}>
          <Pause size={14} aria-hidden="true" />
          {t('queue.step.pause')}
        </button>
        <button type="button" className="btn" onClick={stopQueue}>
          <Square size={14} aria-hidden="true" />
          {t('queue.step.stop')}
        </button>
      </div>

      <p className="hint">{t('queue.step.afterContinue')}</p>
      {remoteOn && <p className="hint">{t('queue.step.remote')}</p>}
    </Modal>
  );
}

export function QueueStepDialog() {
  const q = useQueue((s) => s.q);
  const item = currentItem(q);
  if (!item || q.run !== 'awaiting') return null;
  // Una pregunta distinta (otro trabajo u otra copia) empieza con las casillas sin marcar.
  return <StepBody key={`${item.id}-${item.done}`} />;
}
