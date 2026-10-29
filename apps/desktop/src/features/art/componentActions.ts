import { getApi } from '@/api';
import type { Point } from '@/api/types';
import { askChoice } from '@/features/history/prompt';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { pinTab, selectActiveDevice, useStore } from '@/store';
import { viewCenter } from './actions';
import { useArtPanel } from './artStore';
import { buildComponent, planInsert, type Placement } from './components';
import type { UserComponent } from './types';

const state = () => useStore.getState();

/** Carga los componentes guardados desde la carpeta de configuración. */
export async function loadUserComponents(): Promise<void> {
  try {
    useArtPanel.getState().setComponents(await getApi().componentListUser());
    useArtPanel.getState().setError(null);
  } catch (e) {
    useArtPanel.getState().setError(t('art.component.loadFailed', { error: errorMessage(e) }));
  }
}

/** Abre el diálogo para guardar la selección actual como componente (vacío = avisa y no abre nada). */
export function requestSaveComponent(): boolean {
  const ids = state().selectedIds;
  if (ids.length === 0) {
    state().notify('info', t('art.component.needSelection'));
    return false;
  }
  useArtPanel.getState().setSaving([...ids]);
  return true;
}

/**
 * Guarda los objetos `ids` del documento actual, con sus capas y ajustes, como componente `name`.
 * Devuelve el nombre final (el motor añade `-2` si ya existía uno) o `null` si no se pudo.
 */
export async function saveSelectionAsComponent(name: string, ids: readonly string[]): Promise<string | null> {
  const wanted = new Set(ids);
  const { doc } = state();
  const data = buildComponent(doc.objects.filter((o) => wanted.has(o.id)), doc.layers);
  if (!data) {
    state().notify('error', t('art.component.nothingToSave'));
    return null;
  }
  try {
    const r = await getApi().componentSaveUser({ items: [{ name, data }] });
    const saved = r.added[0];
    if (!saved) {
      state().notify('error', t('art.component.saveFailed', { error: r.errors[0] ?? '' }));
      return null;
    }
    await loadUserComponents();
    state().notify('success', t('art.component.saved', { name: saved.name, n: data.objects.length }));
    return saved.name;
  } catch (e) {
    state().notify('error', t('art.component.saveFailed', { error: errorMessage(e) }));
    return null;
  }
}

export async function renameUserComponent(name: string, newName: string): Promise<void> {
  try {
    await getApi().componentRenameUser({ name, new_name: newName });
    await loadUserComponents();
  } catch (e) {
    useArtPanel.getState().setError(t('art.component.renameFailed', { error: errorMessage(e) }));
  }
}

/** Borra un componente de la biblioteca, tras confirmar. */
export async function removeUserComponent(name: string): Promise<void> {
  const answer = await askChoice({
    title: t('art.component.delete.title'),
    message: t('art.component.delete.ask', { name }),
    choices: [
      { id: 'delete', label: t('art.component.delete.confirm'), tone: 'danger' },
      { id: 'cancel', label: t('art.component.delete.cancel') },
    ],
    cancelId: 'cancel',
  });
  if (answer !== 'delete') return;
  try {
    await getApi().componentRemoveUser({ name });
    await loadUserComponents();
  } catch (e) {
    useArtPanel.getState().setError(t('art.component.removeFailed', { error: errorMessage(e) }));
  }
}

/**
 * Inserta el componente en el documento como un solo paso de Deshacer, con sus capas. Con `at` (al
 * soltarlo en el lienzo) se centra ahí; si no, según `placement` (por omisión el de la biblioteca:
 * la posición original si cabe en la cama, o el centro de la vista). Las capas que el documento ya
 * tenía con otros ajustes conservan los del documento, y se avisa. Deja lo insertado seleccionado.
 */
export async function insertComponent(c: UserComponent, opts: { at?: Point; placement?: Placement } = {}): Promise<boolean> {
  const inTab = pinTab(state());
  const placement: Placement = opts.at ? 'center' : (opts.placement ?? useArtPanel.getState().placement);
  const ids = inTab(() => {
    const s = state();
    const plan = planInsert(c.data, s.doc, { placement, center: opts.at ?? viewCenter(), bed: selectActiveDevice(s).bed });
    const made = s.applyImport(plan.result);
    if (made.length > 0) s.select(made);
    return { made, plan };
  });
  if (!ids || ids.made.length === 0) return false;
  const { plan } = ids;
  const s = state();
  s.notify('success', t(plan.placedAt === 'original' ? 'art.component.insertedOriginal' : 'art.component.insertedCenter', { name: c.name }));
  if (plan.outsideBed) s.notify('warning', t('art.component.outsideBed', { name: c.name }));
  if (plan.conflicts.length > 0) s.notify('warning', t('art.component.layerConflict', { n: plan.conflicts.length }), plan.conflicts);
  void s.computePreview();
  return true;
}
