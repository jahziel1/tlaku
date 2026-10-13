import { BookmarkPlus, Copy, Download, FlaskConical, Pencil, Trash2, TriangleAlert, Upload } from 'lucide-react';
import { useEffect, useId, useMemo } from 'react';
import type { DeviceProfile, LayerSettings, MaterialLibrary, MaterialPreset } from '@/api/types';
import { useCommand, type CommandView } from '@/commands/useCommand';
import { t } from '@/i18n';
import { colorName } from '@/lib/palette';
import { selectActiveDevice, useStore } from '@/store';
import {
  activeLayerOf,
  applyMaterial,
  applyReasonFor,
  deleteMaterial,
  duplicateMaterial,
  exportMaterials,
  importMaterials,
  loadMaterials,
} from './actions';
import { filterPresets, groupPresets, matchingLayers, settingsSummary, thicknessLabel } from './library';
import { useMaterials } from './state';
import './materials.css';

/** Botón que se queda enfocable aunque no se pueda usar y explica por qué (patrón de la app). */
function ReasonButton({
  reason, onRun, className, children, label,
}: { reason: string | null; onRun: () => void; className: string; children: React.ReactNode; label?: string }) {
  return (
    <button
      type="button"
      className={className}
      aria-disabled={reason !== null}
      aria-label={label}
      title={reason ? `${label ?? ''}${label ? '. ' : ''}${reason}` : label}
      onClick={reason ? undefined : onRun}
    >
      {children}
    </button>
  );
}

interface RowProps {
  preset: MaterialPreset;
  deviceName: string | null;
  activeLayer: LayerSettings | null;
  inUse: LayerSettings[];
}

function PresetRow({ preset, deviceName, activeLayer, inUse }: RowProps) {
  const applyKey = applyReasonFor(activeLayer, preset);
  const applyWhy = applyKey ? t(applyKey) : null;
  const name = `${preset.material} · ${preset.title}`;
  const inUseNames = inUse.map((l) => l.name || colorName(l.color)).join(', ');
  return (
    <li className="mat-item" data-in-use={inUse.length > 0 || undefined}>
      <div className="mat-item__main">
        <span className="mat-item__title">{preset.title}</span>
        <span className="mat-item__sum num">{settingsSummary(preset.settings)}</span>
        <span className="mat-item__tags">
          <span className="mat-tag">{t(`materials.mode.${preset.settings.mode}`)}</span>
          {deviceName && <span className="mat-tag" title={t('materials.tag.deviceWhy', { name: deviceName })}>{deviceName}</span>}
          {inUse.length > 0 && (
            <span className="mat-tag" data-tone="ok" title={t('materials.tag.inUseWhy', { layers: inUseNames })}>
              {t('materials.tag.inUse', { n: inUse.length })}
            </span>
          )}
        </span>
        {preset.notes && <span className="hint mat-item__notes">{preset.notes}</span>}
      </div>
      <div className="mat-item__actions">
        <ReasonButton className="btn btn--accent mat-apply" reason={applyWhy} onRun={() => applyMaterial(preset)} label={t('materials.apply')}>
          {t('materials.apply')}
        </ReasonButton>
        <button
          type="button"
          className="icon-btn"
          title={t('materials.edit')}
          aria-label={t('materials.editFor', { name })}
          onClick={() => useMaterials.getState().openEditor({ kind: 'edit', id: preset.id })}
        >
          <Pencil size={14} aria-hidden="true" />
        </button>
        <button type="button" className="icon-btn" title={t('materials.duplicate')} aria-label={t('materials.duplicateFor', { name })} onClick={() => duplicateMaterial(preset.id)}>
          <Copy size={14} aria-hidden="true" />
        </button>
        <button type="button" className="icon-btn" title={t('materials.delete')} aria-label={t('materials.deleteFor', { name })} onClick={() => void deleteMaterial(preset.id)}>
          <Trash2 size={14} aria-hidden="true" />
        </button>
      </div>
    </li>
  );
}

export interface PanelViewProps {
  library: MaterialLibrary;
  loaded: boolean;
  loadError: string | null;
  search: string;
  onlyDevice: boolean;
  devices: DeviceProfile[];
  device: DeviceProfile;
  layers: LayerSettings[];
  activeLayer: LayerSettings | null;
  /** Comando "Prueba de materiales…" (botón con su atajo y motivo). */
  test: Pick<CommandView, 'enabled' | 'tooltip' | 'label' | 'run'>;
}

/** Contenido del panel a partir de datos: lo pinta `MaterialsPanel` y lo prueban los tests. */
export function MaterialsPanelView({ library, loaded, loadError, search, onlyDevice, devices, device, layers, activeLayer, test }: PanelViewProps) {
  const searchId = useId();
  const shown = useMemo(
    () => filterPresets(library.presets, { search, deviceId: onlyDevice ? device.id : null }),
    [library.presets, search, onlyDevice, device.id],
  );
  const groups = useMemo(() => groupPresets(shown), [shown]);
  const deviceName = (id: string | null) => (id === null ? null : (devices.find((d) => d.id === id)?.name ?? id));

  const saveWhy = activeLayer ? null : ('materials.reason.noLayer' as const);
  const exportWhy = library.presets.length === 0 ? t('materials.reason.emptyLibrary') : null;
  const empty = library.presets.length === 0;

  return (
    <div className="panel-scroll mat-panel">
      <section className="section" aria-label={t('materials.actions.title')}>
        <div className="mat-bar">
          <ReasonButton
            className="btn btn--accent grow"
            reason={saveWhy ? t(saveWhy) : null}
            onRun={() => useMaterials.getState().openEditor({ kind: 'save' })}
            label={t('materials.saveLayer')}
          >
            <BookmarkPlus size={14} aria-hidden="true" />
            {t('materials.saveLayer')}
          </ReasonButton>
        </div>
        <div className="mat-bar">
          <button type="button" className="btn grow" onClick={() => void importMaterials()}>
            <Upload size={14} aria-hidden="true" />
            {t('materials.import')}
          </button>
          <ReasonButton className="btn grow" reason={exportWhy} onRun={() => void exportMaterials()} label={t('materials.export')}>
            <Download size={14} aria-hidden="true" />
            {t('materials.export')}
          </ReasonButton>
        </div>
        <button type="button" className="btn" aria-disabled={!test.enabled} title={test.tooltip} onClick={test.enabled ? test.run : undefined}>
          <FlaskConical size={14} aria-hidden="true" />
          {test.label}
        </button>
        {saveWhy && <p className="hint">{t(saveWhy)}</p>}
      </section>

      {loadError && (
        <section className="section">
          <p className="warn-line" role="alert">
            <TriangleAlert size={14} aria-hidden="true" />
            <span>{t('materials.err.loadBlocked', { error: loadError })}</span>
          </p>
          <button type="button" className="btn" onClick={() => void loadMaterials(true)}>{t('materials.retry')}</button>
        </section>
      )}

      <section className="section" aria-label={t('materials.list.title')}>
        <div className="field">
          <label className="field__label" htmlFor={searchId}>{t('materials.search')}</label>
          <input
            id={searchId}
            type="search"
            className="input"
            value={search}
            placeholder={t('materials.search.placeholder')}
            onChange={(e) => useMaterials.getState().setSearch(e.target.value)}
          />
        </div>
        <label className="mat-check" title={t('materials.onlyDevice.why')}>
          <input type="checkbox" checked={onlyDevice} onChange={(e) => useMaterials.getState().setOnlyDevice(e.target.checked)} />
          {t('materials.onlyDevice', { name: device.name })}
        </label>

        {!loaded && !loadError && <p className="hint" role="status">{t('materials.loading')}</p>}
        {loaded && empty && <p className="panel-empty">{t('materials.panel.empty')}</p>}
        {loaded && !empty && groups.length === 0 && <p className="hint" role="status">{t('materials.noMatches')}</p>}

        {groups.map((g) => (
          <div key={g.material} className="mat-group" role="group" aria-label={g.material}>
            <h3 className="mat-group__title">{g.material}</h3>
            {g.thicknesses.map((th) => (
              <div key={th.thickness ?? 'none'} className="mat-thickness">
                <h4 className="mat-thickness__title num">{th.thickness === null ? t('materials.noThickness') : thicknessLabel(th.thickness)}</h4>
                <ul className="mat-list">
                  {th.presets.map((p) => (
                    <PresetRow key={p.id} preset={p} deviceName={deviceName(p.device_id)} activeLayer={activeLayer} inUse={matchingLayers(layers, p)} />
                  ))}
                </ul>
              </div>
            ))}
          </div>
        ))}
      </section>
    </div>
  );
}

/** Pestaña Materiales: biblioteca de ajustes agrupada por material y grosor. */
export function MaterialsPanel() {
  const library = useMaterials((s) => s.library);
  const loaded = useMaterials((s) => s.loaded);
  const loadError = useMaterials((s) => s.loadError);
  const search = useMaterials((s) => s.search);
  const onlyDevice = useMaterials((s) => s.onlyDevice);
  const devices = useStore((s) => s.devices);
  const device = useStore(selectActiveDevice);
  const layers = useStore((s) => s.doc.layers);
  const activeLayer = useStore((s) => activeLayerOf(s));
  const test = useCommand('tools.materialTest');

  useEffect(() => {
    void loadMaterials();
  }, []);

  return <MaterialsPanelView {...{ library, loaded, loadError, search, onlyDevice, devices, device, layers, activeLayer, test }} />;
}
