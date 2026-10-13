import { Trash2 } from 'lucide-react';
import { useEffect, useId, useState } from 'react';
import { getApi, type TemplateInfo } from '@/api';
import { askChoice } from '@/features/history/prompt';
import { documentName } from '@/features/history/files';
import { locale, t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { useStore } from '@/store';
import { Modal } from '@/ui/Modal';
import { deleteTemplate, newFromTemplate, saveAsTemplate } from './templates';
import { useTemplatesUi } from './templatesUi';

type Loaded = { status: 'loading' } | { status: 'error'; message: string } | { status: 'ready'; list: TemplateInfo[] };

/** Plantillas guardadas; se vuelve a pedir la lista al borrar o guardar. */
function useTemplateList(): { loaded: Loaded; reload: () => void } {
  const [loaded, setLoaded] = useState<Loaded>({ status: 'loading' });
  const [tick, setTick] = useState(0);
  useEffect(() => {
    let current = true;
    getApi()
      .listTemplates()
      .then((list) => current && setLoaded({ status: 'ready', list }))
      .catch((e: unknown) => current && setLoaded({ status: 'error', message: errorMessage(e) }));
    return () => {
      current = false;
    };
  }, [tick]);
  return { loaded, reload: () => setTick((n) => n + 1) };
}

/** Fecha y hora cortas de la última modificación, en el idioma de la interfaz. */
const formatDate = (ms: number): string => new Intl.DateTimeFormat(locale(), { dateStyle: 'medium', timeStyle: 'short' }).format(ms);

const stem = (name: string) => name.replace(/\.[^.]+$/, '');

function List({ loaded, selected, onSelect, onDelete }: { loaded: Loaded; selected: string; onSelect: (name: string) => void; onDelete: (name: string) => void }) {
  if (loaded.status === 'loading') return <p className="hint" role="status">{t('canvas.templates.loading')}</p>;
  if (loaded.status === 'error') return <p className="hint hint--danger" role="alert">{t('canvas.templates.listFailed', { error: loaded.message })}</p>;
  if (loaded.list.length === 0) return <p className="hint">{t('canvas.templates.empty')}</p>;
  return (
    <ul className="tpl-list" role="listbox" aria-label={t('canvas.templates.list')}>
      {loaded.list.map((tpl) => (
        <li key={tpl.name} className="tpl-list__row">
          <button type="button" role="option" aria-selected={selected === tpl.name} className="tpl-list__item" onClick={() => onSelect(tpl.name)}>
            <span className="tpl-list__name">{tpl.name}</span>
            <span className="tpl-list__meta num">{formatDate(tpl.saved_at_ms)}</span>
          </button>
          <button type="button" className="icon-btn" title={t('canvas.templates.delete')} aria-label={`${t('canvas.templates.delete')}: ${tpl.name}`} onClick={() => onDelete(tpl.name)}>
            <Trash2 size={14} aria-hidden="true" />
          </button>
        </li>
      ))}
    </ul>
  );
}

function TemplatesDialog({ kind }: { kind: 'save' | 'new' }) {
  const close = useTemplatesUi((s) => s.close);
  const filePath = useStore((s) => s.filePath);
  const labelId = useId();
  const nameId = useId();
  const { loaded, reload } = useTemplateList();
  const [name, setName] = useState(() => (filePath ? stem(documentName(filePath)) : ''));
  const [picked, setPicked] = useState('');
  const [busy, setBusy] = useState(false);

  const existing = loaded.status === 'ready' && loaded.list.some((x) => x.name.toLowerCase() === name.trim().toLowerCase());

  const remove = async (target: string) => {
    const answer = await askChoice({
      title: t('canvas.templates.deleteTitle'),
      message: t('canvas.templates.deleteMessage', { name: target }),
      choices: [
        { id: 'delete', label: t('canvas.templates.delete'), tone: 'danger' },
        { id: 'cancel', label: t('canvas.templates.cancel') },
      ],
      cancelId: 'cancel',
    });
    if (answer === 'delete' && (await deleteTemplate(target))) {
      if (picked === target) setPicked('');
      reload();
    }
  };

  const save = async () => {
    const clean = name.trim();
    if (!clean || busy) return;
    if (existing) {
      const answer = await askChoice({
        title: t('canvas.templates.replaceTitle'),
        message: t('canvas.templates.replaceMessage', { name: clean }),
        choices: [
          { id: 'replace', label: t('canvas.templates.replace'), tone: 'accent' },
          { id: 'cancel', label: t('canvas.templates.cancel') },
        ],
        cancelId: 'cancel',
      });
      if (answer !== 'replace') return;
    }
    setBusy(true);
    const ok = await saveAsTemplate(clean);
    setBusy(false);
    if (ok) close();
  };

  const create = async () => {
    if (!picked || busy) return;
    setBusy(true);
    close();
    await newFromTemplate(picked);
  };

  return (
    <Modal labelId={labelId} onClose={close} backdropClassName="modal" cardClassName="modal__card" portal guardOutside initialFocus={kind === 'save' ? `#${CSS.escape(nameId)}` : '[data-autofocus]'}>
      <div className="modal__head">
        <h2 id={labelId} className="modal__title">{t(kind === 'save' ? 'canvas.templates.saveTitle' : 'canvas.templates.newTitle')}</h2>
      </div>
      <div className="modal__body tpl-dlg">
        <p className="hint">{t(kind === 'save' ? 'canvas.templates.saveHint' : 'canvas.templates.newHint')}</p>
        {kind === 'save' && (
          <div className="field">
            <label htmlFor={nameId} className="field__label">{t('canvas.templates.name')}</label>
            <input
              id={nameId}
              className="field__input"
              value={name}
              maxLength={80}
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') void save();
              }}
            />
          </div>
        )}
        <List loaded={loaded} selected={kind === 'save' ? name.trim() : picked} onSelect={(n) => (kind === 'save' ? setName(n) : setPicked(n))} onDelete={(n) => void remove(n)} />
        {kind === 'save' && existing && <p className="hint">{t('canvas.templates.willReplace')}</p>}
      </div>
      <div className="modal__foot">
        <button type="button" className="btn" onClick={close}>{t('canvas.templates.cancel')}</button>
        {kind === 'save' ? (
          <button type="button" className="btn btn--accent" disabled={!name.trim() || busy} onClick={() => void save()}>
            {t('canvas.templates.save')}
          </button>
        ) : (
          <button type="button" className="btn btn--accent" data-autofocus disabled={!picked || busy} onClick={() => void create()}>
            {t('canvas.templates.create')}
          </button>
        )}
      </div>
    </Modal>
  );
}

/** Monta el diálogo de plantillas que esté abierto (Archivo > Guardar como plantilla / Nuevo desde plantilla). */
export function TemplatesHost() {
  const dialog = useTemplatesUi((s) => s.dialog);
  return dialog ? <TemplatesDialog key={dialog} kind={dialog} /> : null;
}
