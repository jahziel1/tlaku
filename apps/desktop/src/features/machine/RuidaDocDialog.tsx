import { useState } from 'react';
import { getApi } from '@/api';
import type { RuidaFile } from '@/api/types';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { getPref, usePref } from '@/store';
import { checklistPending, checklistSatisfied, visibleChecklist, type ChecklistItem } from './checklist';
import { Modal } from './Modal';
import { WarnLine } from './RuidaDiff';

export type RuidaDocAction = 'delete' | 'run';

/**
 * Borrar o ejecutar un documento de la memoria de la controladora Ruida (M24). **EXPERIMENTAL: sin
 * probar con una controladora real.** Pide una casilla explícita; ejecutar, además, pasa por la
 * misma lista de comprobación de seguridad que Iniciar (con las casillas y la obligatoriedad que
 * la persona configuró). El motor vuelve a leer el nombre del documento antes de actuar.
 */
export function RuidaDocDialog({ action, file, onClose, onDone }: { action: RuidaDocAction; file: RuidaFile; onClose: () => void; onDone: (message: string) => void }) {
  const name = file.name ?? '';
  const isRun = action === 'run';
  const writesOn = usePref('ruidaExperimentalWrites');
  const mask = usePref('jobChecklistItems');
  const required = usePref('jobChecklistRequired');
  const [ticked, setTicked] = useState<ReadonlySet<ChecklistItem['id']>>(new Set());
  const [confirmed, setConfirmed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const checklistOk = !isRun || checklistSatisfied(mask, required, ticked);
  const pending = checklistPending(mask, ticked);
  const toggle = (id: ChecklistItem['id'], on: boolean) =>
    setTicked((prev) => {
      const next = new Set(prev);
      if (on) next.add(id);
      else next.delete(id);
      return next;
    });

  const go = async () => {
    setBusy(true);
    setError(null);
    try {
      const args = { index: file.index, name, confirmed: true, experimental_writes: getPref('ruidaExperimentalWrites') };
      const r = isRun ? await getApi().ruidaFileRun(args) : await getApi().ruidaFileDelete(args);
      onDone(
        isRun
          ? t(r.confirmed ? 'ruida.doc.run.ok' : 'ruida.doc.run.notStarted', { name })
          : t(r.confirmed ? 'ruida.doc.delete.ok' : 'ruida.doc.delete.notVerified', { name }),
      );
      onClose();
    } catch (e) {
      setError(t('ruida.doc.failed', { error: errorMessage(e) }));
      setBusy(false);
    }
  };

  const params = { index: file.index, name };
  return (
    <Modal
      title={t(isRun ? 'ruida.doc.run.title' : 'ruida.doc.delete.title')}
      subtitle={t('ruida.set.experimental')}
      size="md"
      tone={isRun ? 'warn' : 'danger'}
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn" data-autofocus onClick={onClose}>{t('ruida.doc.cancel')}</button>
          <button type="button" className="btn btn--accent" disabled={!writesOn || !confirmed || !checklistOk || busy} title={!writesOn ? t('ruida.exp.off') : !checklistOk ? t('machine.checklist.blocked', { n: pending.length }) : undefined} onClick={() => void go()}>
            {t(isRun ? 'ruida.doc.run.action' : 'ruida.doc.delete.action')}
          </button>
        </>
      }
    >
      <div className="rset">
        <WarnLine role="alert">{t(isRun ? 'ruida.doc.run.body' : 'ruida.doc.delete.body', params)}</WarnLine>
        <p className="hint">{t('ruida.files.note2')}</p>
        {isRun && (
          <section className="mc-checklist" aria-labelledby="ruida-run-checklist">
            <h3 id="ruida-run-checklist" className="mc-sub">{t('machine.checklist.title')}</h3>
            {visibleChecklist(mask).length === 0 ? (
              <p className="hint">{t('machine.checklist.empty')}</p>
            ) : (
              <>
                <div className="mc-checklist__items">
                  {visibleChecklist(mask).map((i) => (
                    <label key={i.id} className="mc-check">
                      <input type="checkbox" checked={ticked.has(i.id)} onChange={(e) => toggle(i.id, e.target.checked)} />
                      {t(i.label)}
                    </label>
                  ))}
                </div>
                <p className={required && !checklistOk ? 'hint hint--danger' : 'hint'} role="status">
                  {required ? (checklistOk ? t('machine.checklist.ready') : t('machine.checklist.blocked', { n: pending.length })) : t('machine.checklist.optional')}
                </p>
              </>
            )}
          </section>
        )}
        <label className="rset-confirm">
          <input type="checkbox" checked={confirmed} disabled={!writesOn} onChange={(e) => setConfirmed(e.target.checked)} />
          {t(isRun ? 'ruida.doc.run.confirm' : 'ruida.doc.delete.confirm', params)}
        </label>
        {error && <p className="hint hint--danger" role="alert">{error}</p>}
      </div>
    </Modal>
  );
}
