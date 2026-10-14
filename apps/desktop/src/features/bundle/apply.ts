import { commands } from '@/commands';
import { loadUserArt } from '@/features/art/actions';
import { useArtPanel } from '@/features/art/artStore';
import { loadCameraSetup, persistCameraSetup } from '@/features/camera/actions';
import { loadMaterials, persistMaterials } from '@/features/materials/actions';
import { useMaterials } from '@/features/materials/state';
import { getApi } from '@/api';
import { t, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { isJobBusy, useStore, type AppState } from '@/store';
import { type CurrentState, type ImportPlan, type SectionChoice } from './model';

/** Por qué no se puede importar ahora (un trabajo en curso), o `null`. */
export function importBlockedReason(s: Pick<AppState, 'jobState'>): I18nKey | null {
  return isJobBusy(s) ? 'bundle.blocked.job' : null;
}

/** Estado actual contra el que se calcula la vista previa. Carga lo que haga falta (materiales, cámara, arte). */
export async function currentState(): Promise<CurrentState> {
  await loadMaterials();
  await loadCameraSetup();
  await loadUserArt();
  const s = useStore.getState();
  return {
    cameras: s.cameraSetup,
    userArt: useArtPanel.getState().user,
    devices: s.devices,
    activeDeviceId: s.activeDeviceId,
    library: useMaterials.getState().library,
    prefs: s.prefs,
    shortcuts: s.shortcutOverrides,
    knownCommands: new Set(commands.list().map((c) => c.id)),
  };
}

export interface ApplyResult {
  /** No se aplicó nada porque hay un trabajo en curso. */
  blocked: boolean;
  devices: { saved: number; failed: number; activated: boolean } | null;
  materials: { added: number } | null;
  /** Piezas de cámara que cambiaron y configuraciones nuevas. */
  cameras: { parts: number; presets: number } | null;
  art: { added: number; failed: number } | null;
  prefs: { prefs: number; shortcuts: number } | null;
}

/**
 * Aplica las secciones elegidas de `plan`. Nada se escribe antes de llamarla (la vista previa solo
 * lee). Los perfiles se guardan con `saveDevice` del store; la biblioteca de materiales ya viene
 * mezclada en el plan (no se pierde nada local); las preferencias reemplazan lo que el paquete trae.
 * Una sección que falla no impide aplicar las demás.
 */
export async function applyImport(plan: ImportPlan, choice: SectionChoice, name = ''): Promise<ApplyResult> {
  const result: ApplyResult = { blocked: false, devices: null, materials: null, cameras: null, art: null, prefs: null };
  const blocked = importBlockedReason(useStore.getState());
  if (blocked) {
    useStore.getState().notify('warning', t(blocked));
    return { ...result, blocked: true };
  }

  if (choice.devices && plan.devices) {
    const todo = [...plan.devices.added, ...plan.devices.replaced];
    let saved = 0;
    for (const device of todo) {
      if (await useStore.getState().saveDevice(device)) saved++;
    }
    let activated = false;
    const wanted = plan.devices.activeDevice;
    if (wanted && useStore.getState().devices.some((d) => d.id === wanted)) {
      useStore.getState().setActiveDevice(wanted);
      activated = true;
    }
    result.devices = { saved, failed: todo.length - saved, activated };
  }

  if (choice.materials && plan.materials) {
    await loadMaterials();
    const ui = useMaterials.getState();
    if (!ui.loaded || ui.loadError) {
      useStore.getState().notify('error', t('bundle.err.materialsUnavailable'));
    } else {
      ui.setLibrary(plan.materials.library);
      await persistMaterials();
      result.materials = { added: plan.materials.added };
    }
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
      result.art = { added: saved.added.length, failed: saved.errors.length };
      await loadUserArt();
    } catch (e) {
      useStore.getState().notify('error', t('bundle.err.artFailed', { error: errorMessage(e) }));
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
  const details: string[] = [];
  if (failed > 0) details.push(t('bundle.notice.devicesFailed', { n: failed }));
  if ((result.art?.failed ?? 0) > 0) details.push(t('bundle.notice.artFailed', { n: result.art!.failed }));
  if (details.length > 0) s.notify('warning', text, details);
  else s.notify('success', text);
  return result;
}
