import { Modal } from './Modal';
import { usePrompts } from './prompt';

/** Pinta la pregunta modal actual (cambios sin guardar, vaciar historial...). */
export function PromptHost() {
  const current = usePrompts((s) => s.current);
  const answer = usePrompts((s) => s.answer);
  if (!current) return null;
  const labelId = `prompt-title-${current.id}`;
  const safe = current.choices.find((c) => c.tone === 'accent')?.id ?? current.cancelId;
  return (
    <Modal key={current.id} labelId={labelId} initialFocus={`[data-choice="${safe}"]`} onClose={() => answer(current.cancelId)}>
      <div className="modal__head">
        <h2 id={labelId} className="modal__title">{current.title}</h2>
      </div>
      <p className="modal__body">{current.message}</p>
      {current.items && current.items.length > 0 && (
        <ul className="modal__list" aria-label={current.title}>
          {current.items.map((item, i) => (
            <li key={`${i}-${item}`}>{item}</li>
          ))}
        </ul>
      )}
      <div className="modal__foot">
        {current.choices.map((c) => (
          <button
            key={c.id}
            type="button"
            data-choice={c.id}
            className={`btn${c.tone === 'accent' ? ' btn--accent' : c.tone === 'danger' ? ' btn--danger' : ''}`}
            onClick={() => answer(c.id)}
          >
            {c.label}
          </button>
        ))}
      </div>
    </Modal>
  );
}
