import { Redo2, Trash2, Undo2 } from 'lucide-react';
import { useEffect, useMemo, useRef, type KeyboardEvent } from 'react';
import { useCommand } from '@/commands/useCommand';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { HISTORY_LIMIT } from '@/store/document';
import './history.css';
import { askChoice } from './prompt';
import { buildTimeline, formatTime } from './timeline';

/** Pestaña "Historial" del panel acoplado: lista de pasos con etiqueta y hora; un clic vuelve a ese punto. */
export function HistoryPanel() {
  const past = useStore((s) => s.past);
  const future = useStore((s) => s.future);
  const jumpTo = useStore((s) => s.jumpTo);
  const undo = useCommand('edit.undo');
  const redo = useCommand('edit.redo');
  const listRef = useRef<HTMLUListElement>(null);

  const rows = useMemo(() => buildTimeline(past, future).reverse(), [past, future]);
  const total = past.length + future.length;

  // El paso actual siempre queda a la vista.
  useEffect(() => {
    listRef.current?.querySelector('[aria-current="step"]')?.scrollIntoView({ block: 'nearest' });
  }, [past.length]);

  const onKey = (e: KeyboardEvent<HTMLUListElement>) => {
    const buttons = [...e.currentTarget.querySelectorAll<HTMLButtonElement>('button[data-row]')];
    const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const to = e.key === 'ArrowDown' ? i + 1 : e.key === 'ArrowUp' ? i - 1 : e.key === 'Home' ? 0 : e.key === 'End' ? buttons.length - 1 : null;
    if (to === null || i < 0) return;
    e.preventDefault();
    buttons[Math.max(0, Math.min(buttons.length - 1, to))]?.focus();
  };

  const clear = async () => {
    const answer = await askChoice({
      title: t('history.panel.clearTitle'),
      message: t('history.panel.clearMessage'),
      choices: [
        { id: 'clear', label: t('history.panel.clearConfirm'), tone: 'danger' },
        { id: 'cancel', label: t('history.panel.cancel') },
      ],
      cancelId: 'cancel',
    });
    if (answer === 'clear') useStore.getState().clearHistory();
  };

  return (
    <section className="histpanel" aria-label={t('history.panel.label')}>
      <div className="histpanel__bar">
        <button type="button" className="icon-btn icon-btn--boxed" title={undo.tooltip} aria-label={undo.label} aria-disabled={!undo.enabled} onClick={() => undo.enabled && undo.run()}>
          <Undo2 size={15} aria-hidden="true" />
        </button>
        <button type="button" className="icon-btn icon-btn--boxed" title={redo.tooltip} aria-label={redo.label} aria-disabled={!redo.enabled} onClick={() => redo.enabled && redo.run()}>
          <Redo2 size={15} aria-hidden="true" />
        </button>
        <span className="histpanel__count num">{t('history.panel.steps', { n: total, max: HISTORY_LIMIT })}</span>
        <div className="grow" />
        <button type="button" className="icon-btn" title={t('history.panel.clear')} aria-label={t('history.panel.clear')} aria-disabled={total === 0} onClick={() => total > 0 && void clear()}>
          <Trash2 size={15} aria-hidden="true" />
        </button>
      </div>
      <p className="hint histpanel__hint">{t('history.panel.hint')}</p>
      {total === 0 && <p className="panel-empty">{t('history.panel.empty')}</p>}
      <ul ref={listRef} className="histpanel__list" onKeyDown={onKey}>
        {rows.map((row) => {
          const label = t(row.label);
          return (
            <li key={row.position}>
              <button
                type="button"
                data-row=""
                className="histrow"
                data-status={row.status}
                aria-current={row.status === 'current' ? 'step' : undefined}
                tabIndex={row.status === 'current' ? 0 : -1}
                title={t('history.panel.jump', { label })}
                onClick={() => jumpTo(row.position)}
              >
                <span className="histrow__mark" aria-hidden="true" />
                <span className="histrow__label">{label}</span>
                {row.status === 'undone' && <span className="histrow__tag">{t('history.panel.undone')}</span>}
                {row.status === 'current' && <span className="sr-only">{t('history.panel.current')}</span>}
                {row.at !== null && <time className="histrow__time num" dateTime={new Date(row.at).toISOString()}>{formatTime(row.at)}</time>}
              </button>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
