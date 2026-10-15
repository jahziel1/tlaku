import { useId } from 'react';
import { askChoice } from '@/features/history/prompt';
import { Modal } from '@/features/history/Modal';
import { baseName } from '@/features/history/paths';
import { t } from '@/i18n';
import { formatBytes, formatCopyDate } from './format';
import { discardAll, discardCopy, recoverAll, recoverCopy, useRecovery } from './recover';
import './recovery.css';

async function confirmDiscardAll(n: number): Promise<void> {
  if (n > 1) {
    const choice = await askChoice({
      title: t('prefs.recovery.discardAll.title'),
      message: t('prefs.recovery.discardAll.message', { n }),
      choices: [
        { id: 'ok', label: t('prefs.recovery.discardAll.confirm'), tone: 'danger' },
        { id: 'cancel', label: t('prefs.recovery.discardAll.cancel') },
      ],
      cancelId: 'cancel',
    });
    if (choice !== 'ok') return;
  }
  await discardAll();
}

/** Diálogo de recuperación: una fila por copia, con nombre, fecha y las opciones recuperar o descartar. */
export function RecoveryDialog() {
  const titleId = useId();
  const items = useRecovery((s) => s.items);
  const busy = useRecovery((s) => s.busy);
  const close = useRecovery((s) => s.close);

  return (
    <Modal labelId={titleId} wide onClose={close} initialFocus="[data-recover]">
      <div className="modal__head">
        <h2 id={titleId} className="modal__title">{t('prefs.recovery.title')}</h2>
      </div>
      <p className="modal__body">{t('prefs.recovery.intro')}</p>
      <ul className="rec-list" aria-label={t('prefs.recovery.list')}>
        {items.map((it, i) => (
          <li key={it.id} className="rec-item">
            <div className="rec-item__info">
              <span className="rec-item__name">{it.name}</span>
              <span className="hint num">
                {t('prefs.recovery.when', { date: formatCopyDate(it.saved_at_ms) })} · {formatBytes(it.size_bytes)}
              </span>
              <span className="hint rec-item__path" title={it.path ?? undefined}>
                {it.path ? t('prefs.recovery.from', { path: baseName(it.path) }) : t('prefs.recovery.new')}
              </span>
            </div>
            <div className="rec-item__btns">
              <button type="button" className="btn btn--accent" data-recover={i === 0 ? '' : undefined} disabled={busy !== null} onClick={() => void recoverCopy(it)}>
                {t('prefs.recovery.recover')}
              </button>
              <button type="button" className="btn btn--danger" disabled={busy !== null} onClick={() => void discardCopy(it.id)}>
                {t('prefs.recovery.discard')}
              </button>
            </div>
          </li>
        ))}
      </ul>
      <div className="modal__foot">
        {items.length > 1 && (
          <button type="button" className="btn btn--accent" disabled={busy !== null} onClick={() => void recoverAll()}>
            {t('prefs.recovery.recoverAll')}
          </button>
        )}
        {items.length > 1 && (
          <button type="button" className="btn btn--danger" disabled={busy !== null} onClick={() => void confirmDiscardAll(items.length)}>
            {t('prefs.recovery.discardAll')}
          </button>
        )}
        <button type="button" className="btn" onClick={close}>{t('prefs.recovery.later')}</button>
      </div>
    </Modal>
  );
}
