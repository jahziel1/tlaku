import { BookmarkPlus, Copy, Download, ExternalLink, FlaskConical, Globe, Link2, Pencil, Share2, Trash2, TriangleAlert, Upload } from 'lucide-react';
import { useEffect, useId, useMemo } from 'react';
import type { DeviceProfile, LayerSettings, MachineClass, MaterialLibrary, MaterialPreset } from '@/api/types';
import { useCommand, type CommandView } from '@/commands/useCommand';
import { t, tn } from '@/i18n';
import { useHelpUi } from '@/features/help/helpStore';
import { colorName } from '@/lib/palette';
import { selectActiveDevice, useStore } from '@/store';
import {
  activeLayerOf,
  applyMaterial,
  applyReasonFor,
  chooseMachineClass,
  copySourceLink,
  deleteMaterial,
  duplicateMaterial,
  exportMaterials,
  importMaterials,
  loadMaterials,
  syncMachineClass,
} from './actions';
import { openCommunity } from './communityActions';
import { classLabel, confidenceLabel, confidenceWhy, materialKeyName, materialName, roleLabel, siteOf, titleName, warningText } from './labels';
import { filterPresets, groupPresets, matchingLayers, settingsSummary, thicknessLabel } from './library';
import { classKind, laserKindOf, MACHINE_CLASSES } from './machineClass';
import { effectiveRole } from './role';
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
  /** Los ajustes de fábrica son de solo lectura: se duplican para editarlos. */
  factory: boolean;
  deviceName: string | null;
  activeLayer: LayerSettings | null;
  inUse: LayerSettings[];
  /** Velocidad máxima de la máquina activa (mm/min), para avisar de ajustes más rápidos. */
  maxSpeed: number;
}

/** Fuente de un ajuste: enlace y botón para copiarlo, con el nivel de confianza. */
function SourceLine({ preset, name }: { preset: MaterialPreset; name: string }) {
  const url = preset.source;
  if (!url) return null;
  const site = siteOf(url);
  const level = preset.confidence;
  return (
    <span className="mat-source">
      {level && (
        <span className="mat-tag" data-tone={level === 'official' ? 'ok' : level === 'forum' ? 'warn' : undefined} title={confidenceWhy(level)}>
          {t('materials.confidence.label', { level: confidenceLabel(level) })}
        </span>
      )}
      <a className="mat-source__link" href={url} target="_blank" rel="noopener noreferrer" aria-label={t('materials.source.openFor', { name, site })} title={url}>
        <ExternalLink size={12} aria-hidden="true" />
        {t('materials.source.open', { site })}
      </a>
      <button type="button" className="icon-btn" title={t('materials.source.copy')} aria-label={t('materials.source.copyFor', { name })} onClick={() => void copySourceLink(preset)}>
        <Link2 size={12} aria-hidden="true" />
      </button>
    </span>
  );
}

function PresetRow({ preset, factory, deviceName, activeLayer, inUse, maxSpeed }: RowProps) {
  const applyKey = applyReasonFor(activeLayer, preset);
  const applyWhy = applyKey ? t(applyKey) : null;
  const name = `${materialName(preset)} · ${titleName(preset)}`;
  const inUseNames = inUse.map((l) => l.name || colorName(l.color)).join(', ');
  const warnings = [...(preset.warnings ?? [])];
  const tooFast = preset.settings.speed > maxSpeed;
  return (
    <li className="mat-item" data-in-use={inUse.length > 0 || undefined}>
      <div className="mat-item__main">
        <span className="mat-item__title">{titleName(preset)}</span>
        <span className="mat-item__sum num">{settingsSummary(preset.settings)}</span>
        <span className="mat-item__tags">
          <span className="mat-tag">{t(`materials.mode.${preset.settings.mode}`)}</span>
          {(preset.role || factory) && <span className="mat-tag">{roleLabel(effectiveRole(preset))}</span>}
          {preset.machine_class && <span className="mat-tag" data-tone="class">{classLabel(preset.machine_class)}</span>}
          {deviceName && <span className="mat-tag" title={t('materials.tag.deviceWhy', { name: deviceName })}>{deviceName}</span>}
          {inUse.length > 0 && (
            <span className="mat-tag" data-tone="ok" title={t('materials.tag.inUseWhy', { layers: inUseNames })}>
              {t('materials.tag.inUse', { n: inUse.length })}
            </span>
          )}
        </span>
        {preset.notes && <span className="hint mat-item__notes">{preset.notes}</span>}
        {(warnings.length > 0 || tooFast) && (
          <ul className="mat-warns">
            {warnings.map((w) => (
              <li key={w} className="mat-warn">
                <TriangleAlert size={12} aria-hidden="true" />
                <span>{warningText(w)}</span>
              </li>
            ))}
            {tooFast && (
              <li className="mat-warn">
                <TriangleAlert size={12} aria-hidden="true" />
                <span>{t('materials.warn.fast', { speed: Math.round(preset.settings.speed), max: Math.round(maxSpeed) })}</span>
              </li>
            )}
          </ul>
        )}
        <SourceLine preset={preset} name={name} />
      </div>
      <div className="mat-item__actions">
        <ReasonButton className="btn btn--accent mat-apply" reason={applyWhy} onRun={() => applyMaterial(preset)} label={t('materials.apply')}>
          {t('materials.apply')}
        </ReasonButton>
        {!factory && (
          <button
            type="button"
            className="icon-btn"
            title={t('materials.edit')}
            aria-label={t('materials.editFor', { name })}
            onClick={() => useMaterials.getState().openEditor({ kind: 'edit', id: preset.id })}
          >
            <Pencil size={14} aria-hidden="true" />
          </button>
        )}
        <button
          type="button"
          className="icon-btn"
          title={factory ? t('materials.duplicateToEdit') : t('materials.duplicate')}
          aria-label={factory ? t('materials.duplicateToEditFor', { name }) : t('materials.duplicateFor', { name })}
          onClick={() => duplicateMaterial(preset.id)}
        >
          <Copy size={14} aria-hidden="true" />
        </button>
        {!factory && (
          <button
            type="button"
            className="icon-btn"
            title={t('help.materials.share')}
            aria-label={t('help.materials.shareFor', { name })}
            onClick={() => useHelpUi.getState().openShareMaterial(preset)}
          >
            <Share2 size={14} aria-hidden="true" />
          </button>
        )}
        {!factory && (
          <button type="button" className="icon-btn" title={t('materials.delete')} aria-label={t('materials.deleteFor', { name })} onClick={() => void deleteMaterial(preset.id)}>
            <Trash2 size={14} aria-hidden="true" />
          </button>
        )}
      </div>
    </li>
  );
}

const EMPTY_FACTORY: MaterialLibrary = { version: 1, presets: [] };

export interface PanelViewProps {
  library: MaterialLibrary;
  loaded: boolean;
  loadError: string | null;
  search: string;
  onlyDevice: boolean;
  /** Biblioteca de fábrica (solo lectura). */
  factory?: MaterialLibrary;
  /** Clase elegida para la máquina activa y si se filtra por ella. */
  machineClass?: MachineClass | null;
  onlyClass?: boolean;
  devices: DeviceProfile[];
  device: DeviceProfile;
  layers: LayerSettings[];
  activeLayer: LayerSettings | null;
  /** Comando "Prueba de materiales…" (botón con su atajo y motivo). */
  test: Pick<CommandView, 'enabled' | 'tooltip' | 'label' | 'run'>;
}

/** Una lista de ajustes agrupada por material y grosor. */
function PresetGroups({ groups, factory, deviceName, activeLayer, layers, maxSpeed }: {
  groups: ReturnType<typeof groupPresets>;
  factory: boolean;
  deviceName: (id: string | null) => string | null;
  activeLayer: LayerSettings | null;
  layers: LayerSettings[];
  maxSpeed: number;
}) {
  return (
    <>
      {groups.map((g) => (
        <div key={g.material} className="mat-group" role="group" aria-label={g.material}>
          <h4 className="mat-group__title">{g.material}</h4>
          {g.thicknesses.map((th) => (
            <div key={th.thickness ?? 'none'} className="mat-thickness">
              <h5 className="mat-thickness__title num">{th.thickness === null ? t('materials.noThickness') : thicknessLabel(th.thickness)}</h5>
              <ul className="mat-list">
                {th.presets.map((p) => (
                  <PresetRow key={p.id} preset={p} factory={factory} deviceName={deviceName(p.device_id)} activeLayer={activeLayer} inUse={matchingLayers(layers, p)} maxSpeed={maxSpeed} />
                ))}
              </ul>
            </div>
          ))}
        </div>
      ))}
    </>
  );
}

/** Contenido del panel a partir de datos: lo pinta `MaterialsPanel` y lo prueban los tests. */
export function MaterialsPanelView({
  library, factory = EMPTY_FACTORY, machineClass = null, onlyClass = false, loaded, loadError, search, onlyDevice, devices, device, layers, activeLayer, test,
}: PanelViewProps) {
  const searchId = useId();
  const classId = useId();
  const mine = useMemo(
    () => filterPresets(library.presets, { search, deviceId: onlyDevice ? device.id : null }),
    [library.presets, search, onlyDevice, device.id],
  );
  // Los aportes de la comunidad (confianza «foro») van en su propia sección, aparte de los de fábrica.
  const factoryShown = useMemo(
    () => filterPresets(factory.presets.filter((p) => p.confidence !== 'forum'), { search, deviceId: null, machineClass: onlyClass ? machineClass : null }),
    [factory.presets, search, onlyClass, machineClass],
  );
  const communityShown = useMemo(
    () => filterPresets(factory.presets.filter((p) => p.confidence === 'forum'), { search, deviceId: null, machineClass: onlyClass ? machineClass : null }),
    [factory.presets, search, onlyClass, machineClass],
  );
  const communityTotal = useMemo(() => factory.presets.filter((p) => p.confidence === 'forum').length, [factory.presets]);
  const mineGroups = useMemo(() => groupPresets(mine), [mine]);
  const factoryGroups = useMemo(() => groupPresets(factoryShown), [factoryShown]);
  const communityGroups = useMemo(() => groupPresets(communityShown), [communityShown]);
  const deviceName = (id: string | null) => (id === null ? null : (devices.find((d) => d.id === id)?.name ?? id));
  const kind = laserKindOf(device);
  const restrictions = (factory.restrictions ?? []).filter((r) => r.laser === kind);
  const maxSpeed = Math.max(device.max_rate[0], 1);

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
        <button type="button" className="btn" title={t('materials.community.open.why')} onClick={() => void openCommunity()}>
          <Globe size={14} aria-hidden="true" />
          {t('materials.community.open')}
        </button>
        {saveWhy && <p className="hint">{t(saveWhy)}</p>}
      </section>

      <p className="mat-notice" role="note">
        <TriangleAlert size={14} aria-hidden="true" />
        <span>{t('materials.notice.startingPoints')}</span>
      </p>

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
        <div className="field">
          <label className="field__label" htmlFor={classId} title={t('materials.machineClass.why')}>{t('materials.machineClass')}</label>
          <select
            id={classId}
            className="select"
            value={machineClass ?? ''}
            title={t('materials.machineClass.why')}
            onChange={(e) => chooseMachineClass(device.id, (e.target.value || null) as MachineClass | null)}
          >
            <option value="">{t('materials.class.none')}</option>
            {MACHINE_CLASSES.filter((c) => kind === null || classKind(c) === kind).map((c) => <option key={c} value={c}>{classLabel(c)}</option>)}
          </select>
        </div>
        <label className="mat-check" title={t('materials.onlyDevice.why')}>
          <input type="checkbox" checked={onlyDevice} onChange={(e) => useMaterials.getState().setOnlyDevice(e.target.checked)} />
          {t('materials.onlyDevice', { name: device.name })}
        </label>
        {machineClass && (
          <label className="mat-check" title={t('materials.onlyClass.why')}>
            <input type="checkbox" checked={onlyClass} onChange={(e) => useMaterials.getState().setOnlyClass(e.target.checked)} />
            {t('materials.onlyClass', { class: classLabel(machineClass) })}
          </label>
        )}
        {restrictions.map((r) => (
          <p key={`${r.material}-${r.role ?? 'any'}`} className="warn-line" role="note">
            <TriangleAlert size={14} aria-hidden="true" />
            <span>{t('materials.restriction', { material: materialKeyName(r.material), text: warningText(r.warning) })}</span>
          </p>
        ))}
      </section>

      <section className="section mat-section" aria-label={t('materials.section.mine')}>
        <h3 className="mat-section__title">{t('materials.section.mine')}</h3>
        {!loaded && !loadError && <p className="hint" role="status">{t('materials.loading')}</p>}
        {loaded && empty && <p className="panel-empty">{t('materials.panel.empty')}</p>}
        {loaded && !empty && mineGroups.length === 0 && <p className="hint" role="status">{t('materials.noMatches')}</p>}
        <PresetGroups groups={mineGroups} factory={false} deviceName={deviceName} activeLayer={activeLayer} layers={layers} maxSpeed={maxSpeed} />
      </section>

      <section className="section mat-section" aria-label={t('materials.section.factory')}>
        <h3 className="mat-section__title">
          {t('materials.section.factory')}
          <span className="mat-section__count num">{tn('materials.count', factoryShown.length)}</span>
        </h3>
        <p className="hint">{t('materials.section.factory.hint')}</p>
        {factory.presets.length > 0 && factoryGroups.length === 0 && <p className="hint" role="status">{t('materials.noMatchesIn')}</p>}
        <PresetGroups groups={factoryGroups} factory deviceName={deviceName} activeLayer={activeLayer} layers={layers} maxSpeed={maxSpeed} />
      </section>

      <section className="section mat-section" aria-label={t('materials.section.community')}>
        <h3 className="mat-section__title">
          {t('materials.section.community')}
          <span className="mat-section__count num">{tn('materials.count', communityShown.length)}</span>
        </h3>
        <p className="hint">{t('materials.section.community.hint')}</p>
        {communityTotal === 0 && factory.presets.length > 0 && <p className="hint">{t('materials.section.community.empty')}</p>}
        {communityTotal > 0 && communityGroups.length === 0 && <p className="hint" role="status">{t('materials.noMatchesIn')}</p>}
        <PresetGroups groups={communityGroups} factory deviceName={deviceName} activeLayer={activeLayer} layers={layers} maxSpeed={maxSpeed} />
      </section>
    </div>
  );
}

/** Pestaña Materiales: biblioteca de ajustes agrupada por material y grosor. */
export function MaterialsPanel() {
  const library = useMaterials((s) => s.library);
  const loaded = useMaterials((s) => s.loaded);
  const loadError = useMaterials((s) => s.loadError);
  const factory = useMaterials((s) => s.factory);
  const machineClass = useMaterials((s) => s.machineClass);
  const onlyClass = useMaterials((s) => s.onlyClass);
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

  // Cada máquina recuerda su clase; al cambiar de máquina se toma la suya.
  useEffect(() => {
    syncMachineClass(device);
  }, [device]);

  return <MaterialsPanelView {...{ library, factory, machineClass, onlyClass, loaded, loadError, search, onlyDevice, devices, device, layers, activeLayer, test }} />;
}
