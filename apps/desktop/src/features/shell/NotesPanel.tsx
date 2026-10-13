import { useId } from 'react';
import { t } from '@/i18n';
import { useStore } from '@/store';

/** Pestaña «Notas»: texto libre del proyecto que se guarda con el documento (no se corta ni se envía). */
export function NotesPanel() {
  const notes = useStore((s) => s.doc.notes) ?? '';
  const setNotes = useStore((s) => s.setNotes);
  const id = useId();
  return (
    <section className="notes-panel" aria-label={t('canvas.notes.label')}>
      <label htmlFor={id} className="notes-panel__label">{t('canvas.notes.title')}</label>
      <textarea
        id={id}
        className="notes-panel__text"
        value={notes}
        placeholder={t('canvas.notes.placeholder')}
        spellCheck
        onChange={(e) => setNotes(e.target.value)}
      />
      <p className="hint">{t('canvas.notes.hint', { n: notes.length })}</p>
    </section>
  );
}
