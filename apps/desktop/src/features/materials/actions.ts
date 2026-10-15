import { getApi, getApiKind } from '@/api';
import type { DeviceProfile, Document, LayerSettings, MachineClass, MaterialLibrary, MaterialPreset } from '@/api/types';
import { baseName, withExtension } from '@/features/history/paths';
import { askChoice } from '@/features/history/prompt';
import { t, tn, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { selectActiveDevice, useStore, type AppState } from '@/store';
import { DEFAULT_EXPORT_NAME, downloadText, pickLibraryToOpen, pickLibraryToSave } from './files';
import { materialName, siteOf, titleName } from './labels';
import { resolveMachineClass, storeClassFor } from './machineClass';
import {
  applyPresetChange,
  copyFactoryPreset,
  duplicatePreset,
  formExtras,
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

/**
 * Lee la biblioteca del usuario y la de fábrica (una vez). La de fábrica es de solo lectura: si
 * falla se avisa, pero no impide usar ni guardar la del usuario. Si la del usuario falla se
 * bloquea el guardado hasta reintentar.
 */
export function loadMaterials(force = false): Promise<void> {
  if (ui().loaded && !force) return Promise.resolve();
  loading ??= Promise.all([
    getApi()
      .loadFactoryMaterials()
      .then((factory) => ui().setFactory(factory))
      .catch((e: unknown) => app().notify('error', t('materials.err.loadFactory', { error: errorMessage(e) }))),
    getApi()
      .loadMaterialLibrary()
      .then((library) => {
        ui().setLibrary(library);
        ui().setLoaded(true);
      })
      .catch((e: unknown) => {
        const error = errorMessage(e);
        ui().setLoaded(false, error);
        app().notify('error', t('materials.err.load', { error }));
      }),
  ])
    .then(() => undefined)
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

const presetName = (p: Pick<MaterialPreset, 'material' | 'title' | 'material_key' | 'title_key'>) => `${materialName(p)} · ${titleName(p)}`;

/** Agrega un ajuste ya armado a la biblioteca del usuario y la guarda (lo usa el navegador de la comunidad). */
export function addPreset(preset: MaterialPreset): void {
  mutate((lib) => ({ ...lib, presets: [...lib.presets, preset] }));
}

// ---------------------------------------------------------------------------
// Clase de la máquina y enlaces de fuente
// ---------------------------------------------------------------------------

/** Toma la clase de la máquina activa: la que el usuario eligió para ella o la que se deduce del perfil. */
export function syncMachineClass(device: Pick<DeviceProfile, 'id' | 'kind' | 'galvo'>): void {
  ui().setMachineClass(resolveMachineClass(device).machineClass);
}

/** El usuario elige (o borra) la clase de la máquina activa; se recuerda para esa máquina en este equipo. */
export function chooseMachineClass(deviceId: string, machineClass: MachineClass | null): void {
  storeClassFor(deviceId, machineClass);
  ui().setMachineClass(machineClass);
}

/** Copia al portapapeles la dirección de la fuente de un ajuste. */
export async function copySourceLink(preset: Pick<MaterialPreset, 'source'>): Promise<boolean> {
  const url = preset.source;
  if (!url) return false;
  try {
    await navigator.clipboard.writeText(url);
    app().notify('success', t('materials.source.copied', { url: siteOf(url) }));
    return true;
  } catch {
    app().notify('warning', t('materials.source.copyFailed', { url }));
    return false;
  }
}

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
      const { role: _r, machine_class: _c, ...kept } = p;
      const edited = { ...kept, material: form.material.trim(), thickness: form.thickness, title: form.title.trim(), notes: form.notes.trim(), device_id: form.device_id, ...formExtras(form) };
      return layer ? { ...edited, settings: presetFromLayer(layer, form, id).settings } : edited;
    }),
  }));
  const p = ui().library.presets.find((x) => x.id === id);
  if (p) app().notify('success', t('materials.notice.updated', { name: presetName(p) }));
}

/**
 * Duplica un ajuste de Mis materiales (con el sufijo "copia") o uno de fábrica (que pasa a Mis
 * materiales ya traducido, para poder editarlo).
 */
export function duplicateMaterial(id: string): void {
  const own = ui().library.presets.find((p) => p.id === id);
  const factory = own ? undefined : ui().factory.presets.find((p) => p.id === id);
  const taken = idsOf(ui().library.presets);
  const copy = own ? duplicatePreset(own, uniqueId(taken), t('materials.copySuffix')) : factory ? copyFactoryPreset(factory, uniqueId(taken)) : null;
  if (!copy) return;
  mutate((lib) => ({ ...lib, presets: [...lib.presets, copy] }));
  app().notify('success', t(factory ? 'materials.notice.duplicatedFromFactory' : 'materials.notice.duplicated', { name: presetName(copy) }));
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
      app().notify('success', tn('materials.notice.exported', library.presets.length, { path }));
    } else {
      // En el navegador no hay disco: se descarga el mismo JSON que escribiría el motor.
      downloadText(DEFAULT_EXPORT_NAME, JSON.stringify(library, null, 2));
      app().notify('success', tn('materials.notice.exported', library.presets.length, { path: DEFAULT_EXPORT_NAME }));
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
  const s = app();
  s.openInTab({ ...doc, device_id: s.doc.device_id }, null);
  s.fitContent();
  void s.computePreview();
  app().notify('success', t('materials.test.done', { cells: form.rows.count * form.cols.count }));
  return true;
}
