import { getApi } from '@/api';
import { loadUserArt } from '@/features/art/actions';
import { useArtPanel } from '@/features/art/artStore';
import { loadCameraSetup } from '@/features/camera/actions';
import { readCosts } from '@/features/costs/costProfiles';
import { loadMaterials } from '@/features/materials/actions';
import { useMaterials } from '@/features/materials/state';
import { loadRemnants, useRemnants } from '@/features/nest/remnantStore';
import { getLanguage } from '@/i18n';
import { useStore } from '@/store';
import { buildUserBundle, type BundleSource, type SectionChoice } from './model';
import type { TemplateItem } from './extras';

/**
 * De dónde sale cada sección de un paquete: lee del disco (o de su store) lo que haga falta y arma la
 * fuente que entiende `buildUserBundle`. La usan exportar, el respaldo a carpeta, el respaldo previo a
 * importar y la vista previa de importación, así todas ven el mismo estado.
 */

const systemTag = () => (typeof navigator === 'undefined' ? null : navigator.language);

/** Perfiles de costos guardados en esta computadora. */
export const currentCosts = () => readCosts(getLanguage(), systemTag());

/**
 * Plantillas locales con su documento. Con `only`, solo se leen los documentos de esos nombres
 * (para comparar contra un paquete sin cargar todas); los nombres de todas salen siempre.
 */
export async function loadTemplateItems(only?: ReadonlySet<string>): Promise<{ items: TemplateItem[]; names: string[] }> {
  const api = getApi();
  const infos = await api.listTemplates();
  const names = infos.map((i) => i.name);
  const wanted = infos.filter((i) => !only || only.has(i.name.toLowerCase()));
  const items: TemplateItem[] = [];
  for (const info of wanted) {
    try {
      items.push({ name: info.name, doc: await api.loadTemplate({ name: info.name }) });
    } catch {
      // Una plantilla ilegible se omite: el respaldo no debe fallar por una sola.
    }
  }
  return { items, names };
}

/** Entradas del historial de trabajos (tantas como guarda la aplicación). */
export async function loadHistoryEntries(): Promise<Record<string, unknown>[]> {
  return getApi().jobHistoryLoad({ max: useStore.getState().prefs.jobHistoryMax });
}

/** Lee del disco solo lo que `sections` pide y lo junta con el estado de la aplicación. */
export async function collectSource(sections: Partial<SectionChoice>): Promise<BundleSource> {
  if (sections.materials) await loadMaterials();
  if (sections.cameras) await loadCameraSetup();
  if (sections.art) await loadUserArt();
  if (sections.remnants) await loadRemnants();
  const templates = sections.templates ? (await loadTemplateItems()).items : [];
  const history = sections.history ? await loadHistoryEntries() : [];
  const s = useStore.getState();
  return {
    devices: s.devices,
    activeDeviceId: s.activeDeviceId,
    library: useMaterials.getState().library,
    prefs: s.prefs,
    shortcuts: s.shortcutOverrides,
    cameras: s.cameraSetup,
    userArt: useArtPanel.getState().user,
    costs: sections.costs ? currentCosts() : undefined,
    remnants: useRemnants.getState().library,
    templates,
    history,
  };
}

/** Arma el paquete con el estado actual de las secciones elegidas. */
export async function buildCurrentBundle(sections: Partial<SectionChoice>) {
  const source = await collectSource(sections);
  const app = await getApi().appVersion().catch(() => '');
  return buildUserBundle(source, { sections, app });
}
