import { commands } from '@/commands';
import { loadUserArt } from '@/features/art/actions';
import { useArtPanel } from '@/features/art/artStore';
import { loadUserComponents } from '@/features/art/componentActions';
import { loadCameraSetup, persistCameraSetup } from '@/features/camera/actions';
import { useCostStore } from '@/features/costs/costStore';
import { writeCosts } from '@/features/costs/costProfiles';
import { loadMaterials, persistMaterials } from '@/features/materials/actions';
import { useMaterials } from '@/features/materials/state';
import { loadRemnants, useRemnants } from '@/features/nest/remnantStore';
import { getApi, type UserBundle } from '@/api';
import { t, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { isJobBusy, useStore, type AppState } from '@/store';
import { defaultPolicies, resolveCosts, resolveRemnants, templatesToWrite, type PolicyChoice } from './extras';
import { resolveDevices, resolveMaterials, sectionChanges, type CurrentState, type ImportPlan, type SectionChoice } from './model';
import { buildCurrentBundle, currentCosts, loadHistoryEntries, loadTemplateItems } from './sources';

/** Por qué no se puede importar ahora (un trabajo en curso), o `null`. */
export function importBlockedReason(s: Pick<AppState, 'jobState'>): I18nKey | null {
  return isJobBusy(s) ? 'bundle.blocked.job' : null;
}

/**
 * Estado actual contra el que se calcula la vista previa. Carga lo que haga falta (materiales, cámara,
 * arte...). Con el paquete a la vista, solo se leen las plantillas y el historial que él también trae.
 */
export async function currentState(bundle?: UserBundle): Promise<CurrentState> {
  await loadMaterials();
  await loadCameraSetup();
  await loadUserArt();
  await loadRemnants();
  const s = useStore.getState();
  const bundleTemplates = bundle?.templates?.items?.map((i) => String(i?.name ?? '').toLowerCase()) ?? [];
  const templates = bundle?.templates ? await loadTemplateItems(new Set(bundleTemplates)) : { items: [], names: [] };
  const historyIds = bundle?.history ? new Set((await loadHistoryEntries()).map((e) => String(e.id))) : new Set<string>();
  return {
    cameras: s.cameraSetup,
    userArt: useArtPanel.getState().user,
    userComponents: useArtPanel.getState().components,
    devices: s.devices,
    activeDeviceId: s.activeDeviceId,
    library: useMaterials.getState().library,
    prefs: s.prefs,
    shortcuts: s.shortcutOverrides,
    costs: currentCosts(),
    remnants: useRemnants.getState().library,
    templates: templates.items,
    templateNames: templates.names,
    historyIds,
    knownCommands: new Set(commands.list().map((c) => c.id)),
  };
}

export interface ApplyResult {
  /** No se aplicó nada porque hay un trabajo en curso o porque no se pudo guardar el respaldo previo. */
  blocked: boolean;
  devices: { saved: number; failed: number; activated: boolean } | null;
  materials: { added: number } | null;
  /** Piezas de cámara que cambiaron y configuraciones nuevas. */
  cameras: { parts: number; presets: number } | null;
  art: { added: number; failed: number } | null;
  prefs: { prefs: number; shortcuts: number } | null;
  costs: { changes: number } | null;
  remnants: { saved: number; dropped: number } | null;
  templates: { saved: number; failed: number } | null;
  history: { added: number } | null;
}

/**
 * Guarda un respaldo local del estado actual (todo menos el historial de trabajos) antes de aplicar
 * una importación, para poder volver atrás. Devuelve `false` si no se pudo: no se aplica nada sin él.
 */
export async function saveSafetyBackup(): Promise<boolean> {
  try {
    const bundle = await buildCurrentBundle({ devices: true, materials: true, costs: true, remnants: true, templates: true, cameras: true, art: true, prefs: true });
    await getApi().backupSafety({ bundle });
    return true;
  } catch (e) {
    useStore.getState().notify('error', t('bundle.err.safetyFailed', { error: errorMessage(e) }));
    return false;
  }
}

/**
 * Aplica las secciones elegidas de `plan`. Nada se escribe antes de llamarla (la vista previa solo
 * lee); antes de escribir se guarda un respaldo del estado actual. Los conflictos por id se resuelven
 * con `policies`. Una sección que falla no impide aplicar las demás.
 */
export async function applyImport(plan: ImportPlan, choice: SectionChoice, name = '', policies: PolicyChoice = defaultPolicies()): Promise<ApplyResult> {
  const result: ApplyResult = { blocked: false, devices: null, materials: null, cameras: null, art: null, prefs: null, costs: null, remnants: null, templates: null, history: null };
  const blocked = importBlockedReason(useStore.getState());
  if (blocked) {
    useStore.getState().notify('warning', t(blocked));
    return { ...result, blocked: true };
  }
  if (!(await saveSafetyBackup())) return { ...result, blocked: true };

  if (choice.devices && plan.devices) {
    const { save, idMap } = resolveDevices(plan.devices, useStore.getState().devices, policies.devices);
    let saved = 0;
    for (const device of save) {
      if (await useStore.getState().saveDevice(device)) saved++;
    }
    let activated = false;
    const wanted = plan.devices.activeDevice ? (idMap.get(plan.devices.activeDevice) ?? plan.devices.activeDevice) : null;
    if (wanted && useStore.getState().devices.some((d) => d.id === wanted)) {
      useStore.getState().setActiveDevice(wanted);
      activated = true;
    }
    result.devices = { saved, failed: save.length - saved, activated };
  }

  if (choice.materials && plan.materials) {
    await loadMaterials();
    const ui = useMaterials.getState();
    if (!ui.loaded || ui.loadError) {
      useStore.getState().notify('error', t('bundle.err.materialsUnavailable'));
    } else {
      ui.setLibrary(resolveMaterials(plan.materials, policies.materials));
      await persistMaterials();
      result.materials = { added: sectionChanges(plan, policies).materials };
    }
  }

  if (choice.costs && plan.costs) {
    try {
      const merged = resolveCosts(plan.costs, currentCosts(), policies.costs);
      writeCosts(merged);
      useCostStore.setState({ machines: merged.machines, sales: merged.sales, materials: merged.materials });
      result.costs = { changes: sectionChanges(plan, policies).costs };
    } catch (e) {
      useStore.getState().notify('error', t('bundle.err.costsFailed', { error: errorMessage(e) }));
    }
  }

  if (choice.remnants && plan.remnants) {
    try {
      await loadRemnants(true);
      const { library, dropped } = resolveRemnants(plan.remnants, useRemnants.getState().library, policies.remnants);
      await getApi().saveRemnants({ library });
      await loadRemnants(true);
      result.remnants = { saved: sectionChanges(plan, policies).remnants - dropped, dropped };
    } catch (e) {
      useStore.getState().notify('error', t('bundle.err.remnantsFailed', { error: errorMessage(e) }));
    }
  }

  if (choice.templates && plan.templates) {
    const write = templatesToWrite(plan.templates, plan.templates.localNames, policies.templates);
    let saved = 0;
    for (const item of write) {
      try {
        await getApi().saveTemplate({ name: item.name, doc: item.doc });
        saved++;
      } catch {
        // Se cuenta abajo: una plantilla que falla no detiene las demás.
      }
    }
    result.templates = { saved, failed: write.length - saved };
  }

  if (choice.cameras && plan.cameras) {
    const { setup } = plan.cameras;
    const s = useStore.getState();
    // Solo cambian la calibración, la alineación, el cabezal y las configuraciones: lo demás (fuente, opacidad...) se queda.
    s.setCameraSetup({ ...s.cameraSetup, calibration: setup.calibration, alignment: setup.alignment, head: setup.head, presets: setup.presets });
    await persistCameraSetup();
    const parts = [plan.cameras.calibration, plan.cameras.alignment, plan.cameras.head].filter((x) => x === 'new' || x === 'replace').length;
    result.cameras = { parts, presets: plan.cameras.presets?.added.length ?? 0 };
  }

  if (choice.art && plan.art) {
    try {
      const saved = plan.art.items.length > 0 ? await getApi().artSaveUser({ items: plan.art.items }) : { added: [], errors: [] };
      // Los componentes viajan con la biblioteca de arte y entran igual: nunca pisan uno local.
      const comps = plan.art.components.length > 0 ? await getApi().componentSaveUser({ items: plan.art.components }) : { added: [], errors: [] };
      result.art = { added: saved.added.length + comps.added.length, failed: saved.errors.length + comps.errors.length };
      await loadUserArt();
      await loadUserComponents();
    } catch (e) {
      useStore.getState().notify('error', t('bundle.err.artFailed', { error: errorMessage(e) }));
    }
  }

  if (choice.history && plan.history) {
    try {
      const added = plan.history.added.length > 0 ? await getApi().jobHistoryImport({ entries: plan.history.added, max: useStore.getState().prefs.jobHistoryMax }) : 0;
      result.history = { added };
    } catch (e) {
      useStore.getState().notify('error', t('bundle.err.historyFailed', { error: errorMessage(e) }));
    }
  }

  if (choice.prefs && plan.prefs) {
    const { change } = plan.prefs;
    useStore.getState().replacePrefs({ prefs: change.prefs, shortcuts: change.shortcuts }, false);
    result.prefs = { prefs: change.appliedPrefs, shortcuts: change.appliedShortcuts };
  }

  const s = useStore.getState();
  const failed = result.devices?.failed ?? 0;
  const text = t('bundle.notice.imported', {
    name,
    devices: result.devices?.saved ?? 0,
    materials: result.materials?.added ?? 0,
    cameras: (result.cameras?.parts ?? 0) + (result.cameras?.presets ?? 0),
    art: result.art?.added ?? 0,
    prefs: result.prefs?.prefs ?? 0,
  });
  const extras = [
    result.costs && t('bundle.notice.costs', { n: result.costs.changes }),
    result.remnants && t('bundle.notice.remnants', { n: result.remnants.saved }),
    result.templates && t('bundle.notice.templates', { n: result.templates.saved }),
    result.history && t('bundle.notice.history', { n: result.history.added }),
  ].filter((x): x is string => !!x);
  const details: string[] = [];
  if (failed > 0) details.push(t('bundle.notice.devicesFailed', { n: failed }));
  if ((result.art?.failed ?? 0) > 0) details.push(t('bundle.notice.artFailed', { n: result.art!.failed }));
  if ((result.templates?.failed ?? 0) > 0) details.push(t('bundle.notice.templatesFailed', { n: result.templates!.failed }));
  if ((result.remnants?.dropped ?? 0) > 0) details.push(t('bundle.notice.remnantsDropped', { n: result.remnants!.dropped }));
  const full = extras.length > 0 ? `${text} ${extras.join(' ')}` : text;
  if (details.length > 0) s.notify('warning', full, details);
  else s.notify('success', full);
  return result;
}
