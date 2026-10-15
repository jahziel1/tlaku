import { ListOrdered } from 'lucide-react';
import { useEffect } from 'react';
import { t } from '@/i18n';
import { QueueDialog, runLabel } from './QueueDialog';
import { QueueStepDialog } from './QueueStepDialog';
import { startQueuePersistence, startQueueWatcher } from './queue';
import { currentCopy, currentItem, totals } from './queueModel';
import { useQueue } from './queueStore';
import './queue.css';

/** Píldora flotante con el estado de la cola; un clic abre el paso pendiente o la gestión. */
function QueueChip() {
  const q = useQueue((s) => s.q);
  const { done, total } = totals(q);
  if (q.items.length === 0 || q.run === 'idle' || q.run === 'done') return null;
  const cur = currentItem(q);
  const label = t('queue.chip', { done, total, state: runLabel(q.run) });
  return (
    <button
      type="button"
      className="queue__chip"
      data-run={q.run}
      title={cur ? t('queue.summary.current', { name: cur.name, copy: currentCopy(q), copies: cur.copies }) : label}
      onClick={() => {
        const ui = useQueue.getState();
        if (q.run === 'awaiting') ui.setStepOpen(true);
        else ui.setDialogOpen(true);
      }}
    >
      <ListOrdered size={14} aria-hidden="true" />
      <span>{label}</span>
    </button>
  );
}

/** Una línea con el paso de la cola, para la franja de la vista en vivo (nada si no hay cola en marcha). */
export function QueueLiveLine() {
  const q = useQueue((s) => s.q);
  if (q.items.length === 0 || (q.run !== 'running' && q.run !== 'awaiting' && q.run !== 'paused')) return null;
  const cur = currentItem(q);
  const { done, total } = totals(q);
  return (
    <p className="queue__liveline" data-testid="queue-live-line">
      {t('queue.live', { done: Math.min(done + 1, total), total, name: cur?.name ?? '', copy: currentCopy(q), copies: cur?.copies ?? 1, state: runLabel(q.run) })}
    </p>
  );
}

/**
 * Punto de montaje de la cola guiada (lo monta el shell una vez): sigue los trabajos de la máquina, recuerda la
 * definición de la cola y muestra sus diálogos. Los diálogos de la cola nunca inician nada por sí mismos.
 */
export function QueueHost() {
  const dialogOpen = useQueue((s) => s.dialogOpen);
  const stepOpen = useQueue((s) => s.stepOpen);
  const run = useQueue((s) => s.q.run);
  useEffect(() => {
    const stopPersist = startQueuePersistence();
    const stopWatch = startQueueWatcher();
    return () => {
      stopWatch();
      stopPersist();
    };
  }, []);
  return (
    <>
      <QueueChip />
      {dialogOpen && <QueueDialog />}
      {stepOpen && run === 'awaiting' && <QueueStepDialog />}
    </>
  );
}
