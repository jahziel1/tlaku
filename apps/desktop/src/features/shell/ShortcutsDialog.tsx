import { Search, X } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { commands } from '@/commands';
import { Modal } from '@/features/history/Modal';
import { t, type I18nKey } from '@/i18n';
import { isMac } from '@/lib/platform';
import { isEditableTarget } from '@/commands/shortcuts';
import { buildShortcutRows, filterShortcutRows, groupShortcutRows } from './shortcutsModel';
import { useShellUi } from './uiStore';

/**
 * Atiende `?` y `Mod+/` sea cual sea la distribución del teclado (en teclado español `?` no es
 * Mayús+/). El registro ya resuelve las demás combinaciones; esto abre, nunca alterna, así que
 * si ambos reaccionan al mismo evento el resultado es el mismo.
 */
export function useShortcutsHotkey(): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || e.isComposing) return;
      const mod = isMac() ? e.metaKey : e.ctrlKey;
      const isQuestion = e.key === '?' && !e.altKey && !mod;
      const isModSlash = mod && !e.altKey && e.key === '/';
      if (!isQuestion && !isModSlash) return;
      if (isEditableTarget(e.target) && isQuestion) return; // en un campo de texto "?" es un carácter más
      e.preventDefault();
      useShellUi.getState().openShortcuts();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);
}

function Dialog({ onClose }: { onClose: () => void }) {
  const [query, setQuery] = useState('');
  const mac = isMac();
  const rows = useMemo(() => buildShortcutRows(commands, mac), [mac]);
  const groups = useMemo(() => groupShortcutRows(filterShortcutRows(rows, query)), [rows, query]);
  const combo = commands.shortcutLabel('help.shortcuts', mac);

  return (
    <Modal labelId="shortcuts-title" wide onClose={onClose} initialFocus="input">
      <div className="modal__head">
        <h2 id="shortcuts-title" className="modal__title">{t('history.shortcuts.title')}</h2>
        <button type="button" className="icon-btn" aria-label={t('history.shortcuts.close')} title={t('history.shortcuts.close')} onClick={onClose}>
          <X size={16} aria-hidden="true" />
        </button>
      </div>
      <div className="shortcuts__search">
        <Search size={14} aria-hidden="true" />
        <input
          type="search"
          className="input"
          value={query}
          placeholder={t('history.shortcuts.search')}
          aria-label={t('history.shortcuts.search')}
          onChange={(e) => setQuery(e.target.value)}
        />
      </div>
      <div className="shortcuts__scroll">
        {groups.length === 0 && <p className="panel-empty" role="status">{t('history.shortcuts.none', { q: query })}</p>}
        {groups.map((g) => (
          <section key={g.category} className="shortcuts__group" aria-labelledby={`sc-${g.category}`}>
            <h3 id={`sc-${g.category}`} className="panel-title">{t(`history.shortcuts.cat.${g.category}` as I18nKey)}</h3>
            <ul>
              {g.rows.map((r) => (
                <li key={r.id} className="shortcuts__row">
                  <span className="shortcuts__label">{r.label}</span>
                  <span className="shortcuts__keys">
                    {r.keys.map((k) => <kbd key={k} className="kbd">{k}</kbd>)}
                  </span>
                </li>
              ))}
            </ul>
          </section>
        ))}
      </div>
      <p className="hint shortcuts__hint">{t('history.shortcuts.hint', { combo })}</p>
    </Modal>
  );
}

export function ShortcutsDialog() {
  const open = useShellUi((s) => s.shortcutsOpen);
  const close = useShellUi((s) => s.closeShortcuts);
  return open ? <Dialog onClose={close} /> : null;
}
