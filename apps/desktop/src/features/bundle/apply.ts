import { commands } from '@/commands';
import { loadMaterials, persistMaterials } from '@/features/materials/actions';
import { useMaterials } from '@/features/materials/state';
import { t, type I18nKey } from '@/i18n';
import { isJobBusy, useStore, type AppState } from '@/store';
import { type CurrentState, type ImportPlan, type SectionChoice } from './model';

/** Por qué no se puede importar ahora (un trabajo en curso), o `null`. */
export function importBlockedReason(s: Pick<AppState, 'jobState'>): I18nKey | null {
  return isJobBusy(s) ? 'bundle.blocked.job' : null;
}

/** Estado actual contra el que se calcula la vista previa. Carga la biblioteca de materiales si hace falta. */
export async function currentState(): Promise<CurrentState> {
  await loadMaterials();
  const s = useStore.getState();
  return {
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
  prefs: { prefs: number; shortcuts: number } | null;
}

/**
 * Aplica las secciones elegidas de `plan`. Nada se escribe antes de llamarla (la vista previa solo
 * lee). Los perfiles se guardan con `saveDevice` del store; la biblioteca de materiales ya viene
 * mezclada en el plan (no se pierde nada local); las preferencias reemplazan lo que el paquete trae.
 * Una sección que falla no impide aplicar las demás.
 */
export async function applyImport(plan: ImportPlan, choice: SectionChoice, name = ''): Promise<ApplyResult> {
  const result: ApplyResult = { blocked: false, devices: null, materials: null, prefs: null };
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
    prefs: result.prefs?.prefs ?? 0,
  });
  if (failed > 0) s.notify('warning', text, [t('bundle.notice.devicesFailed', { n: failed })]);
  else s.notify('success', text);
  return result;
}
