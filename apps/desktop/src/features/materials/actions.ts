import { getApi, getApiKind } from '@/api';
import type { Document, LayerSettings, MaterialLibrary, MaterialPreset } from '@/api/types';
import { baseName, withExtension } from '@/features/history/paths';
import { ensureSaved } from '@/features/history/files';
import { askChoice } from '@/features/history/prompt';
import { t, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { selectActiveDevice, useStore, type AppState } from '@/store';
import { DEFAULT_EXPORT_NAME, downloadText, pickLibraryToOpen, pickLibraryToSave } from './files';
import {
  applyPresetChange,
  duplicatePreset,
  idsOf,
  layerMatchesPreset,
  mergeLibraries,
  presetFromLayer,
  presetPatch,
  uniqueId,
  type PresetForm,
} from './library';
import { useMaterials } from './state';
import { buildParams, type TestForm } from './testPlan';

const ui = () => useMaterials.getState();
const app = () => useStore.getState();

/** Capa activa del documento, si existe. */
export function activeLayerOf(s: Pick<AppState, 'doc' | 'activeLayer'>): LayerSettings | null {
  const color = s.activeLayer?.toLowerCase();
  return (color && s.doc.layers.find((l) => l.color.toLowerCase() === color)) || null;
}

/** Por qué no se puede usar la capa activa (guardarla o aplicarle un material), o `null` si se puede. */
export function noLayerReason(s: Pick<AppState, 'doc' | 'activeLayer'>): I18nKey | null {
  return activeLayerOf(s) ? null : 'materials.reason.noLayer';
}

/** Por qué no se puede aplicar `preset` a `layer` (la capa activa), o `null` si se puede. */
export function applyReasonFor(layer: LayerSettings | null, preset: MaterialPreset): I18nKey | null {
  if (!layer) return 'materials.reason.noLayer';
  return layerMatchesPreset(layer, preset) ? 'materials.reason.alreadyApplied' : null;
}

// ---------------------------------------------------------------------------
// Carga y guardado
// ---------------------------------------------------------------------------

let loading: Promise<void> | null = null;

/** Lee la biblioteca (una vez). Si falla avisa y bloquea el guardado hasta reintentar. */
export function loadMaterials(force = false): Promise<void> {
  if (ui().loaded && !force) return Promise.resolve();
  loading ??= getApi()
    .loadMaterialLibrary()
    .then((library) => {
      ui().setLibrary(library);
      ui().setLoaded(true);
    })
    .catch((e: unknown) => {
      const error = errorMessage(e);
      ui().setLoaded(false, error);
      app().notify('error', t('materials.err.load', { error }));
    })
    .finally(() => {
      loading = null;
    });
  return loading;
}

let saveQueue: Promise<void> = Promise.resolve();

/** Guarda la biblioteca actual. Las escrituras se encadenan para que la última gane. */
export function persistMaterials(): Promise<void> {
  saveQueue = saveQueue.then(async () => {
    if (ui().loadError) return; // no se pisa un archivo que no se pudo leer
    try {
      await getApi().saveMaterialLibrary({ library: ui().library });
    } catch (e) {
      app().notify('error', t('materials.err.save', { error: errorMessage(e) }));
    }
  });
  return saveQueue;
}

/** Cambia la biblioteca y la guarda. */
function mutate(change: (library: MaterialLibrary) => MaterialLibrary): void {
  ui().setLibrary(change(ui().library));
  void persistMaterials();
}

const presetName = (p: Pick<MaterialPreset, 'material' | 'title'>) => `${p.material} · ${p.title}`;

// ---------------------------------------------------------------------------
// Operaciones sobre los ajustes
// ---------------------------------------------------------------------------

/** Guarda la capa activa como material nuevo. Devuelve el ajuste creado, o `null` si no hay capa. */
export function savePresetFromActiveLayer(form: PresetForm): MaterialPreset | null {
  const layer = activeLayerOf(app());
  if (!layer) return null;
  const preset = presetFromLayer(layer, form, uniqueId(idsOf(ui().library.presets)));
  mutate((lib) => ({ ...lib, presets: [...lib.presets, preset] }));
  app().notify('success', t('materials.notice.saved', { name: presetName(preset) }));
  return preset;
}

/** Edita los datos de un ajuste; con `refreshFromLayer` también toma los ajustes de la capa activa. */
export function updatePreset(id: string, form: PresetForm, refreshFromLayer: boolean): void {
  const layer = refreshFromLayer ? activeLayerOf(app()) : null;
  mutate((lib) => ({
    ...lib,
    presets: lib.presets.map((p) => {
      if (p.id !== id) return p;
      const edited = { ...p, material: form.material.trim(), thickness: form.thickness, title: form.title.trim(), notes: form.notes.trim(), device_id: form.device_id };
      return layer ? { ...edited, settings: presetFromLayer(layer, form, id).settings } : edited;
    }),
  }));
  const p = ui().library.presets.find((x) => x.id === id);
  if (p) app().notify('success', t('materials.notice.updated', { name: presetName(p) }));
}

export function duplicateMaterial(id: string): void {
  const source = ui().library.presets.find((p) => p.id === id);
  if (!source) return;
  const copy = duplicatePreset(source, uniqueId(idsOf(ui().library.presets)), t('materials.copySuffix'));
  mutate((lib) => ({ ...lib, presets: [...lib.presets, copy] }));
  app().notify('success', t('materials.notice.duplicated', { name: presetName(copy) }));
}

/** Pide confirmación y borra. Devuelve si se borró. */
export async function deleteMaterial(id: string): Promise<boolean> {
  const preset = ui().library.presets.find((p) => p.id === id);
  if (!preset) return false;
  const answer = await askChoice({
    title: t('materials.delete.title'),
    message: t('materials.delete.ask', { name: presetName(preset) }),
    choices: [
      { id: 'delete', label: t('materials.delete.confirm'), tone: 'danger' },
      { id: 'cancel', label: t('materials.delete.cancel') },
    ],
    cancelId: 'cancel',
  });
  if (answer !== 'delete') return false;
  mutate((lib) => ({ ...lib, presets: lib.presets.filter((p) => p.id !== id) }));
  app().notify('success', t('materials.notice.deleted', { name: presetName(preset) }));
  return true;
}

/** Copia los ajustes del material a la capa activa, con un paso de deshacer. */
export function applyMaterial(preset: MaterialPreset): boolean {
  const s = app();
  const layer = activeLayerOf(s);
  const reason = applyReasonFor(layer, preset);
  if (reason || !layer) {
    s.notify('warning', t(reason ?? 'materials.reason.noLayer'));
    return false;
  }
  const color = layer.color;
  s.applyDocChange('materials.history.apply', (doc) => applyPresetChange(doc, color, preset));
  s.notify('success', t('materials.notice.applied', { name: presetName(preset), layer: layer.name || color }));
  // Aviso útil: el ajuste trae Z y la máquina activa no tiene eje Z (el trabajo lo ignoraría).
  const p = presetPatch(preset);
  if ((p.z_offset !== 0 || p.z_step !== 0) && !selectActiveDevice(s).z_axis) {
    s.notify('warning', t('materials.notice.zIgnored'));
  }
  return true;
}

// ---------------------------------------------------------------------------
// Importar y exportar
// ---------------------------------------------------------------------------

/** Elige un `.tlakumat` y lo mezcla con la biblioteca. Devuelve si se importó algo. */
export async function importMaterials(): Promise<boolean> {
  const path = await pickLibraryToOpen();
  if (!path) return false;
  const file = baseName(path);
  try {
    const incoming = await getApi().importMaterialLibrary({ path });
    const merged = mergeLibraries(ui().library, incoming);
    if (merged.added > 0) mutate(() => merged.library);
    const text = t('materials.notice.imported', { file, added: merged.added, renamed: merged.renamed, skipped: merged.skipped });
    app().notify(merged.added > 0 ? 'success' : 'info', text);
    return merged.added > 0;
  } catch (e) {
    app().notify('error', t('materials.err.import', { file, error: errorMessage(e) }));
    return false;
  }
}

/** Exporta toda la biblioteca a un `.tlakumat`. Devuelve si se escribió. */
export async function exportMaterials(): Promise<boolean> {
  const library = ui().library;
  const picked = await pickLibraryToSave();
  if (!picked) return false;
  try {
    if (getApiKind() === 'tauri') {
      const path = withExtension(picked, 'tlakumat');
      await getApi().exportMaterialLibrary({ path, library });
      app().notify('success', t('materials.notice.exported', { n: library.presets.length, path }));
    } else {
      // En el navegador no hay disco: se descarga el mismo JSON que escribiría el motor.
      downloadText(DEFAULT_EXPORT_NAME, JSON.stringify(library, null, 2));
      app().notify('success', t('materials.notice.exported', { n: library.presets.length, path: DEFAULT_EXPORT_NAME }));
    }
    return true;
  } catch (e) {
    app().notify('error', t('materials.err.export', { error: errorMessage(e) }));
    return false;
  }
}

// ---------------------------------------------------------------------------
// Prueba de materiales
// ---------------------------------------------------------------------------

/** Genera la cuadrícula con el motor. */
export function generateTest(form: TestForm): Promise<Document> {
  return getApi().materialTest({ params: buildParams(form, activeLayerOf(app())) });
}

/**
 * Abre la cuadrícula como documento nuevo, con el aviso de cambios sin guardar de "Nuevo". Se
 * genera antes de preguntar, así un error del motor no cuesta una pregunta ni el documento actual.
 * Devuelve si se abrió.
 */
export async function openTestAsNewDocument(form: TestForm): Promise<boolean> {
  let doc: Document;
  try {
    doc = await generateTest(form);
  } catch (e) {
    app().notify('error', t('materials.test.err.generate', { error: errorMessage(e) }));
    return false;
  }
  if (!(await ensureSaved('new'))) return false;
  const s = app();
  s.replaceDocument({ ...doc, device_id: s.doc.device_id }, null);
  s.fitContent();
  void s.computePreview();
  app().notify('success', t('materials.test.done', { cells: form.rows.count * form.cols.count }));
  return true;
}
