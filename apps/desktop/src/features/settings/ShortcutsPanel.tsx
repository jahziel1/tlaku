import { Eraser, Pencil, Plus, RotateCcw, Search } from 'lucide-react';
import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import { commands } from '@/commands';
import { eventToCombo, formatCombo, type KeyEventLike } from '@/commands/shortcuts';
import { askChoice } from '@/features/history/prompt';
import { t, type I18nKey } from '@/i18n';
import { isMac } from '@/lib/platform';
import { useStore } from '@/store';
import { exportPrefs, importPrefs } from './files';
import {
  assignShortcuts, buildEditorRows, clearShortcuts, commandLabel, filterEditorRows, findConflicts, groupEditorRows, restoreShortcuts, type EditorRow,
} from './shortcutsModel';

type CaptureMode = 'replace' | 'add';
interface Capture {
  id: string;
  mode: CaptureMode;
}
interface Pending extends Capture {
  combos: string[];
  conflicts: { combo: string; name: string }[];
}

/** Caja que espera una combinación de teclas. Esc cancela; perder el foco también. */
function CaptureBox({ name, onCapture, onCancel }: { name: string; onCapture: (combo: string) => void; onCancel: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => ref.current?.focus(), []);
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    // Nada de lo que se pulse aquí debe llegar a los atajos globales ni cerrar la ventana.
    e.preventDefault();
    e.stopPropagation();
    if (e.key === 'Escape' && !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey) return onCancel();
    const combo = eventToCombo(e.nativeEvent as KeyEventLike, isMac());
    if (combo) onCapture(combo);
  };
  return (
    <div ref={ref} tabIndex={0} role="textbox" aria-label={t('prefs.shortcuts.captureLabel', { name })} className="set-sc__capture" onKeyDown={onKeyDown} onBlur={onCancel}>
      {t('prefs.shortcuts.press')}
    </div>
  );
}

function IconBtn({ label, onClick, hidden, children }: { label: string; onClick: () => void; hidden?: boolean; children: ReactNode }) {
  // Oculto pero con su sitio reservado, para que los botones de todas las filas queden alineados.
  return (
    <button type="button" className="icon-btn" title={label} aria-label={label} onClick={onClick} style={hidden ? { visibility: 'hidden' } : undefined}>
      {children}
    </button>
  );
}

/** Editor de atajos: busca, reasigna capturando la tecla, avisa de conflictos y restaura. */
export function ShortcutsPanel() {
  const mac = isMac();
  const overrides = useStore((s) => s.shortcutOverrides);
  const setOverrides = useStore((s) => s.setShortcutOverrides);
  const [query, setQuery] = useState('');
  const [capture, setCapture] = useState<Capture | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);

  // `overrides` cambia el resultado de `commands` aunque no sea un argumento: por eso es dependencia.
  const rows = useMemo(() => buildEditorRows(commands, mac), [overrides, mac]); // eslint-disable-line react-hooks/exhaustive-deps
  const groups = useMemo(() => groupEditorRows(filterEditorRows(rows, query)), [rows, query]);
  const customCount = rows.filter((r) => r.customized).length;

  const apply = (id: string, combos: string[], steal: boolean) => {
    setOverrides(assignShortcuts(commands, overrides, id, combos, steal));
    setPending(null);
  };

  const onCaptured = (row: EditorRow, mode: CaptureMode, combo: string) => {
    setCapture(null);
    const combos = mode === 'add' ? [...new Set([...row.combos, combo])] : [combo];
    const found = findConflicts(commands, row.id, combos);
    if (found.length === 0) return apply(row.id, combos, false);
    setPending({
      id: row.id,
      mode,
      combos,
      conflicts: found.map((c) => ({ combo: formatCombo(c.combo, mac), name: commandLabel(c.owner) })),
    });
  };

  const restoreAll = async () => {
    const choice = await askChoice({
      title: t('prefs.shortcuts.restoreAll.title'),
      message: t('prefs.shortcuts.restoreAll.message'),
      choices: [
        { id: 'ok', label: t('prefs.shortcuts.restoreAll'), tone: 'danger' },
        { id: 'cancel', label: t('prefs.shortcuts.cancel') },
      ],
      cancelId: 'cancel',
    });
    if (choice === 'ok') setOverrides({});
  };

  return (
    <div className="set-sc">
      <div className="set-sc__bar">
        <div className="set-sc__search">
          <Search size={14} aria-hidden="true" />
          <input type="search" className="input" value={query} placeholder={t('prefs.shortcuts.search')} aria-label={t('prefs.shortcuts.search')} onChange={(e) => setQuery(e.target.value)} />
        </div>
        <span className="hint num">{t('prefs.shortcuts.count', { n: customCount })}</span>
      </div>

      <div className="set-sc__scroll">
        {groups.length === 0 && <p className="panel-empty" role="status">{t('prefs.shortcuts.none', { q: query })}</p>}
        {groups.map((g) => (
          <section key={g.category} className="set-sc__group" aria-labelledby={`set-sc-${g.category}`}>
            <h3 id={`set-sc-${g.category}`} className="panel-title">{t(`history.shortcuts.cat.${g.category}` as I18nKey)}</h3>
            <ul>
              {g.rows.map((r) => {
                const capturing = capture?.id === r.id ? capture : null;
                const conflict = pending?.id === r.id ? pending : null;
                return (
                  <li key={r.id} className="set-sc__item" data-custom={r.customized || undefined}>
                    <div className="set-sc__row">
                      <span className="set-sc__label">
                        {r.label}
                        {r.customized && <span className="set-sc__tag">{t('prefs.shortcuts.custom')}</span>}
                      </span>
                      <span className="set-sc__keys">
                        {capturing ? (
                          <CaptureBox name={r.label} onCapture={(c) => onCaptured(r, capturing.mode, c)} onCancel={() => setCapture(null)} />
                        ) : r.combos.length > 0 ? (
                          r.combos.map((c) => <kbd key={c} className="kbd">{formatCombo(c, mac)}</kbd>)
                        ) : (
                          <span className="hint">{t('prefs.shortcuts.unassigned')}</span>
                        )}
                      </span>
                      <span className="set-sc__actions">
                        <IconBtn label={t('prefs.shortcuts.change')} onClick={() => { setPending(null); setCapture({ id: r.id, mode: 'replace' }); }}>
                          <Pencil size={14} aria-hidden="true" />
                        </IconBtn>
                        <IconBtn label={t('prefs.shortcuts.add')} onClick={() => { setPending(null); setCapture({ id: r.id, mode: 'add' }); }}>
                          <Plus size={14} aria-hidden="true" />
                        </IconBtn>
                        <IconBtn label={t('prefs.shortcuts.clear')} onClick={() => setOverrides(clearShortcuts(commands, overrides, r.id))}>
                          <Eraser size={14} aria-hidden="true" />
                        </IconBtn>
                        <IconBtn label={t('prefs.shortcuts.restore')} hidden={!r.customized} onClick={() => setOverrides(restoreShortcuts(commands, overrides, r.id))}>
                          <RotateCcw size={14} aria-hidden="true" />
                        </IconBtn>
                      </span>
                    </div>
                    {conflict && (
                      <div className="set-sc__conflict" role="alert">
                        <span>{conflict.conflicts.map((c) => t('prefs.shortcuts.conflict', { key: c.combo, name: c.name })).join(' ')}</span>
                        <button type="button" className="btn btn--accent" onClick={() => apply(conflict.id, conflict.combos, true)}>{t('prefs.shortcuts.steal')}</button>
                        <button type="button" className="btn" onClick={() => setPending(null)}>{t('prefs.shortcuts.cancel')}</button>
                      </div>
                    )}
                  </li>
                );
              })}
            </ul>
          </section>
        ))}
      </div>

      <div className="set-sc__foot">
        <p className="hint">{t('prefs.shortcuts.hint')}</p>
        <div className="set-sc__foot-btns">
          <button type="button" className="btn" disabled={Object.keys(overrides).length === 0} onClick={() => void restoreAll()}>{t('prefs.shortcuts.restoreAll')}</button>
          <button type="button" className="btn" onClick={() => void importPrefs('shortcuts')}>{t('prefs.shortcuts.import')}</button>
          <button type="button" className="btn" onClick={() => void exportPrefs('shortcuts')}>{t('prefs.shortcuts.export')}</button>
        </div>
      </div>
    </div>
  );
}
