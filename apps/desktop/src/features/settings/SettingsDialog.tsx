import { X } from 'lucide-react';
import { useId, type KeyboardEvent, type ReactNode } from 'react';
import { isUpdaterConfigured } from '@/features/help/updater';
import { askChoice } from '@/features/history/prompt';
import { useRecents } from '@/features/history/recent';
import '@/features/history/history.css';
import { t, type I18nKey } from '@/i18n';
import { DEFAULT_DOCK_LAYOUT, setDockSide, setToolsSide, type DockSide } from '@/lib/dockLayout';
import { formatLength } from '@/lib/units';
import { setUiMode } from '@/features/simple';
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

/**
 * Modo de la interfaz (D18). El modo sencillo es el modo para principiantes: pasos guiados, sin menús
 * ni herramientas avanzadas. Cambiarlo aquí hace lo mismo que el botón de la barra superior.
 */
function UiModeField() {
  const id = useId();
  const mode = usePref('uiMode');
  const setPref = useStore((s) => s.setPref);
  return (
    <div className="field">
      <label className="field__label" htmlFor={id}>{t('prefs.uiMode')}</label>
      <select
        id={id}
        className="select"
        value={mode}
        onChange={(e) => {
          const next = e.target.value;
          if (next === 'simple' || next === 'full') setUiMode(next);
          else setPref('uiMode', 'ask');
        }}
      >
        <option value="simple">{t('prefs.uiMode.simple')}</option>
        <option value="full">{t('prefs.uiMode.full')}</option>
        <option value="ask">{t('prefs.uiMode.ask')}</option>
      </select>
      <p className="hint">{t('prefs.uiMode.hint')}</p>
    </div>
  );
}

function GeneralPanel() {
  return (
    <>
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
      <UiModeField />
      <PrefCheck k="restoreTabs" label="prefs.restoreTabs" />
      <p className="hint">{t('prefs.restoreTabs.hint')}</p>
      <PrefCheck k="updateCheckOnStart" label="prefs.updateCheckOnStart" />
      <p className="hint">{t('prefs.updateCheckOnStart.hint')}</p>
      {isUpdaterConfigured() ? null : <p className="hint">{t('prefs.updateCheckOnStart.unconfigured')}</p>}
    </Group>
    <CameraGroup />
    </>
  );
}

/** Preferencias de la cámara (M51). Las de cada cámara (opacidad, dispositivo, ajuste) están en su panel. */
function CameraGroup() {
  return (
    <Group title="prefs.camera.title">
      <PrefCheck k="cameraReopen" label="prefs.cameraReopen" />
      <p className="hint">{t('prefs.cameraReopen.hint')}</p>
      <PrefNumber k="cameraFrameWidth" label="prefs.cameraFrameWidth" unit="prefs.unit.px" />
      <p className="hint">{t('prefs.cameraFrameWidth.hint')}</p>
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
      <Group>
        <PrefCheck k="returnToSelect" label="prefs.returnToSelect" />
        <p className="hint">{t('prefs.returnToSelect.hint')}</p>
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
        <PrefSelect
          k="gridContrast"
          label="prefs.gridContrast"
          hint={t('prefs.gridContrast.hint')}
          options={[
            { value: 'low', label: t('prefs.gridContrast.low') },
            { value: 'normal', label: t('prefs.gridContrast.normal') },
            { value: 'high', label: t('prefs.gridContrast.high') },
          ]}
        />
      </Group>
      <LayoutGroup />
    </>
  );
}

/** Borde del panel de pestañas y de las herramientas, y el botón para volver a la distribución de fábrica. */
function LayoutGroup() {
  const layout = usePref('dockLayout');
  const setPref = useStore((s) => s.setPref);
  const sides = [
    { value: 'left', label: t('prefs.dock.left') },
    { value: 'right', label: t('prefs.dock.right') },
  ];
  return (
    <Group title="prefs.dock.title">
      <div className="set-grid">
        <SideSelect label="prefs.dock.panelSide" value={layout.side} options={sides} onChange={(side) => setPref('dockLayout', setDockSide(layout, side))} />
        <SideSelect label="prefs.dock.toolsSide" value={layout.toolsSide} options={sides} onChange={(side) => setPref('dockLayout', setToolsSide(layout, side))} />
      </div>
      <p className="hint">{t('prefs.dock.hint')}</p>
      <div>
        <button type="button" className="btn" onClick={() => setPref('dockLayout', DEFAULT_DOCK_LAYOUT)}>{t('prefs.dock.resetLayout')}</button>
      </div>
    </Group>
  );
}

function SideSelect({ label, value, options, onChange }: { label: I18nKey; value: DockSide; options: { value: string; label: string }[]; onChange: (v: DockSide) => void }) {
  const id = useId();
  return (
    <div className="field">
      <label className="field__label" htmlFor={id}>{t(label)}</label>
      <select id={id} className="select" value={value} onChange={(e) => onChange(e.target.value === 'left' ? 'left' : 'right')}>
        {options.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
      </select>
    </div>
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
  const setPref = useStore((s) => s.setPref);
  const remembered = usePref('importOptions') !== null;
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
      <Group title="prefs.import.title">
        <div className="set-grid">
          <PrefNumber k="importDpi" label="prefs.importDpi" unit="prefs.unit.dpi" />
        </div>
        <p className="hint">{t('prefs.importDpi.hint')}</p>
        <PrefCheck k="importRemember" label="prefs.importRemember" />
        <p className="hint">{t('prefs.importRemember.hint')}</p>
        <div>
          <button type="button" className="btn" disabled={!remembered} onClick={() => setPref('importOptions', null)}>{t('prefs.importForget')}</button>
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
        <div role="tablist" aria-orientation="vertical" tabIndex={-1} aria-label={t('prefs.nav')} className="set-nav" onKeyDown={onNavKey}>
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
