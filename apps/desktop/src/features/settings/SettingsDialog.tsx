import { X } from 'lucide-react';
import { useId, type KeyboardEvent, type ReactNode } from 'react';
import { askChoice } from '@/features/history/prompt';
import { useRecents } from '@/features/history/recent';
import '@/features/history/history.css';
import { t, type I18nKey } from '@/i18n';
import { formatLength } from '@/lib/units';
import { usePref, useStore } from '@/store';
import { Modal } from '@/ui/Modal';
import { AutomationPanel } from '@/features/automation';
import { RemotePanel } from '@/features/remote';
import { PrefCheck, PrefLength, PrefNumber, PrefSelect } from './fields';
import { exportPrefs, importPrefs } from './files';
import { ShortcutsPanel } from './ShortcutsPanel';
import { SETTINGS_SECTIONS, useSettingsUi, type SettingsSection } from './uiStore';
import './settings.css';

/** Medidas fijas que ofrece la lista de rejilla (mm); 0 = automática. Un valor importado fuera de la lista se agrega solo. */
const GRID_PRESETS = [0, 0.5, 1, 2, 5, 10, 25];

function Group({ title, children }: { title?: I18nKey; children: ReactNode }) {
  return (
    <fieldset className="set-group">
      {title && <legend className="panel-title">{t(title)}</legend>}
      {children}
    </fieldset>
  );
}

function GeneralPanel() {
  return (
    <Group>
      <PrefSelect
        k="language"
        label="prefs.language"
        hint={t('prefs.language.hint')}
        options={[
          { value: 'es', label: t('prefs.language.es') },
          { value: 'en', label: t('prefs.language.en') },
        ]}
      />
    </Group>
  );
}

function EditingPanel() {
  return (
    <>
      <Group>
        <PrefSelect
          k="units"
          label="prefs.units"
          hint={t('prefs.units.hint')}
          options={[
            { value: 'mm', label: t('prefs.units.mm') },
            { value: 'in', label: t('prefs.units.in') },
          ]}
        />
      </Group>
      <Group title="prefs.nudge.title">
        <div className="set-grid">
          <PrefLength k="nudgeStep" label="prefs.nudgeStep" />
          <PrefLength k="nudgeStepBig" label="prefs.nudgeStepBig" />
          <PrefLength k="nudgeStepFine" label="prefs.nudgeStepFine" />
        </div>
        <p className="hint">{t('prefs.length.hint')}</p>
      </Group>
      <Group>
        <div className="set-grid">
          <PrefNumber k="rotateStep" label="prefs.rotateStep" unit="prefs.unit.deg" />
          <PrefNumber k="snapPx" label="prefs.snapPx" unit="prefs.unit.px" />
          <PrefLength k="duplicateOffset" label="prefs.duplicateOffset" />
        </div>
      </Group>
    </>
  );
}

function AppearancePanel() {
  const units = usePref('units');
  const grid = usePref('gridMm');
  const presets = GRID_PRESETS.includes(grid) ? GRID_PRESETS : [...GRID_PRESETS, grid].sort((a, b) => a - b);
  const gridOptions = presets.map((mm) => ({ value: String(mm), label: mm === 0 ? t('prefs.grid.auto') : formatLength(mm, units, { trim: true }) }));
  const setPref = useStore((s) => s.setPref);
  return (
    <>
      <Group>
        <PrefSelect
          k="theme"
          label="prefs.theme"
          options={[
            { value: 'dark', label: t('prefs.theme.dark') },
            { value: 'light', label: t('prefs.theme.light') },
          ]}
        />
      </Group>
      <Group>
        <div className="field">
          <GridSelect value={String(grid)} options={gridOptions} onChange={(v) => setPref('gridMm', Number(v))} />
          <p className="hint">{t('prefs.grid.hint')}</p>
        </div>
      </Group>
    </>
  );
}

function GridSelect({ value, options, onChange }: { value: string; options: { value: string; label: string }[]; onChange: (v: string) => void }) {
  const id = useId();
  return (
    <>
      <label className="field__label" htmlFor={id}>{t('prefs.grid')}</label>
      <select id={id} className="select" value={value} onChange={(e) => onChange(e.target.value)}>
        {options.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
      </select>
    </>
  );
}

function FilesPanel() {
  const autosave = usePref('autosave');
  const clear = useRecents((s) => s.clear);
  const recent = useRecents((s) => s.paths.length);
  return (
    <>
      <Group>
        <PrefCheck k="autosave" label="prefs.autosave" />
        <div className="set-grid">
          {autosave ? <PrefNumber k="autosaveSecs" label="prefs.autosaveSecs" unit="prefs.unit.s" /> : null}
        </div>
        <p className="hint">{t('prefs.autosave.hint')}</p>
      </Group>
      <Group>
        <div className="set-grid">
          <PrefNumber k="recentMax" label="prefs.recentMax" unit="prefs.unit.count" />
        </div>
        <div>
          <button type="button" className="btn" disabled={recent === 0} onClick={clear}>{t('prefs.recent.clear')}</button>
        </div>
      </Group>
    </>
  );
}

const PANELS: Record<Exclude<SettingsSection, 'shortcuts'>, () => ReactNode> = {
  general: () => <GeneralPanel />,
  editing: () => <EditingPanel />,
  appearance: () => <AppearancePanel />,
  files: () => <FilesPanel />,
  automation: () => <AutomationPanel />,
  remote: () => <RemotePanel />,
};

async function resetPrefs(): Promise<void> {
  const choice = await askChoice({
    title: t('prefs.reset.title'),
    message: t('prefs.reset.message'),
    choices: [
      { id: 'ok', label: t('prefs.reset.confirm'), tone: 'danger' },
      { id: 'cancel', label: t('prefs.reset.cancel') },
    ],
    cancelId: 'cancel',
  });
  if (choice === 'ok') useStore.getState().resetPrefs();
}

/** Ventana de Preferencias: General, Edición, Apariencia, Archivos y el editor de atajos. */
export function SettingsDialog() {
  // Al cambiar el idioma este diálogo sigue abierto: suscribirse hace que se pinte de nuevo ya traducido.
  usePref('language');
  const titleId = useId();
  const section = useSettingsUi((s) => s.section);
  const setSection = useSettingsUi((s) => s.setSection);
  const close = useSettingsUi((s) => s.closeSettings);

  // Flechas arriba y abajo recorren las secciones, como en cualquier lista de pestañas.
  const onNavKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.key === 'ArrowDown' ? 1 : e.key === 'ArrowUp' ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = SETTINGS_SECTIONS[(SETTINGS_SECTIONS.indexOf(section) + step + SETTINGS_SECTIONS.length) % SETTINGS_SECTIONS.length]!;
    setSection(next);
    e.currentTarget.querySelector<HTMLElement>(`[data-section="${next}"]`)?.focus();
  };

  return (
    <Modal labelId={titleId} onClose={close} backdropClassName="modal" cardClassName="modal__card set-card" initialFocus="[role='tab'][aria-selected='true']" guardOutside>
      <div className="modal__head">
        <h2 id={titleId} className="modal__title">{t('prefs.title')}</h2>
        <button type="button" className="icon-btn" aria-label={t('prefs.close')} title={t('prefs.close')} onClick={close}>
          <X size={16} aria-hidden="true" />
        </button>
      </div>
      <div className="set-body">
        <div role="tablist" aria-orientation="vertical" aria-label={t('prefs.nav')} className="set-nav" onKeyDown={onNavKey}>
          {SETTINGS_SECTIONS.map((id) => (
            <button
              key={id}
              type="button"
              role="tab"
              id={`set-tab-${id}`}
              data-section={id}
              aria-selected={section === id}
              aria-controls="set-panel"
              tabIndex={section === id ? 0 : -1}
              className="set-nav__item"
              onClick={() => setSection(id)}
            >
              {t(`prefs.section.${id}` as I18nKey)}
            </button>
          ))}
        </div>
        <div role="tabpanel" id="set-panel" aria-labelledby={`set-tab-${section}`} className="set-panel">
          {section === 'shortcuts' ? <ShortcutsPanel /> : PANELS[section]()}
        </div>
      </div>
      <div className="modal__foot set-foot">
        <span className="hint set-foot__hint">{t('prefs.footer.hint')}</span>
        <button type="button" className="btn" onClick={() => void importPrefs('all')}>{t('prefs.import')}</button>
        <button type="button" className="btn" onClick={() => void exportPrefs('all')}>{t('prefs.export')}</button>
        <button type="button" className="btn" onClick={() => void resetPrefs()}>{t('prefs.reset')}</button>
        <button type="button" className="btn btn--accent" onClick={close}>{t('prefs.close')}</button>
      </div>
    </Modal>
  );
}
