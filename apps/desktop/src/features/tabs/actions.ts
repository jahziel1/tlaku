import { askChoice } from '@/features/history/prompt';
import { askUnsaved, saveTab } from '@/features/history/files';
import { guardUnsaved, type GuardIo } from '@/features/history/unsaved';
import { t } from '@/i18n';
import { tabLabel, tabViews, useStore, type TabView } from '@/store';

const state = () => useStore.getState();

/** Nombre de una pestaña tal como la ve el usuario. */
export function nameOf(view: Pick<TabView, 'filePath' | 'untitled'>): string {
  return tabLabel(view, state().tabs.length > 1);
}

const viewOf = (id: string): TabView | undefined => tabViews(state()).find((v) => v.id === id);
export const dirtyTabs = (): TabView[] => tabViews(state()).filter((v) => v.dirty);

/** Mientras se pregunta por pestañas, otra petición de cerrar se ignora (el diálogo es modal, pero un atajo podría repetirse). */
let closing = false;

function tabIo(id: string, intent: 'tab' | 'close'): GuardIo {
  return {
    isDirty: () => viewOf(id)?.dirty ?? false,
    ask: () => askUnsaved(intent, viewOf(id) ? nameOf(viewOf(id)!) : ''),
    save: () => saveTab(id),
  };
}

type ManyIntent = 'window' | 'others';
export type ManyChoice = 'save' | 'discard' | 'cancel';

/** Diálogo con la lista de pestañas con cambios: guardar todas, descartar todas o cancelar. */
export async function askMany(views: TabView[], intent: ManyIntent): Promise<ManyChoice> {
  const id = await askChoice({
    title: t('tabs.closeMany.title', { n: views.length }),
    message: t(`tabs.closeMany.ask.${intent}`, { n: views.length }),
    items: views.map(nameOf),
    choices: [
      { id: 'save', label: t('tabs.closeMany.saveAll'), tone: 'accent' },
      { id: 'discard', label: t('tabs.closeMany.discardAll'), tone: 'danger' },
      { id: 'cancel', label: t('tabs.closeMany.cancel') },
    ],
    cancelId: 'cancel',
  });
  return id === 'save' || id === 'discard' ? id : 'cancel';
}

/** Guarda una por una las pestañas indicadas; se detiene en la primera que no se guarda (cancelada o con error). */
export async function saveTabs(ids: string[]): Promise<boolean> {
  for (const id of ids) {
    if (viewOf(id)?.dirty && !(await saveTab(id))) return false;
  }
  return true;
}

/** Guarda todas las pestañas con cambios. */
export function saveAllTabs(): Promise<boolean> {
  return saveTabs(dirtyTabs().map((v) => v.id));
}

/**
 * Pregunta por las pestañas `views` con cambios. Una sola: el diálogo de siempre (guardar / no guardar / cancelar),
 * mostrando esa pestaña. Varias: la lista con «Guardar todas» / «Descartar todas» / cancelar. `true` si se puede seguir.
 */
async function settle(views: TabView[], intent: ManyIntent | 'window1'): Promise<boolean> {
  if (views.length === 0) return true;
  if (views.length === 1) {
    const only = views[0]!;
    if (!only.active) state().activateTab(only.id);
    const kind = intent === 'window1' ? 'close' : 'tab';
    return guardUnsaved(kind, tabIo(only.id, kind));
  }
  const choice = await askMany(views, intent === 'window1' ? 'window' : intent);
  if (choice === 'discard') return true;
  if (choice === 'cancel') return false;
  return saveTabs(views.map((v) => v.id));
}

/** Cierra la pestaña `id`; con cambios sin guardar pregunta antes. `false` si el usuario canceló o no se pudo guardar. */
export async function closeTab(id: string): Promise<boolean> {
  if (closing) return false;
  closing = true;
  try {
    const view = viewOf(id);
    if (!view) return true;
    if (view.dirty && !(await settle([view], 'others'))) return false;
    state().removeTab(id);
    return true;
  } finally {
    closing = false;
  }
}

/** Cierra todas menos `keepId`. Con varias con cambios, una sola pregunta con la lista. */
export async function closeOtherTabs(keepId: string): Promise<boolean> {
  if (closing) return false;
  closing = true;
  try {
    const others = tabViews(state()).filter((v) => v.id !== keepId);
    if (!(await settle(others.filter((v) => v.dirty), 'others'))) return false;
    if (state().activeTabId !== keepId) state().activateTab(keepId);
    for (const v of others) state().removeTab(v.id);
    return true;
  } finally {
    closing = false;
  }
}

/** Antes de cerrar la ventana: `true` si no queda nada sin guardar (o se guardó o descartó). */
export async function confirmCloseWindow(): Promise<boolean> {
  if (closing) return false;
  closing = true;
  try {
    return await settle(dirtyTabs(), 'window1');
  } finally {
    closing = false;
  }
}

export function openNewTab(): void {
  state().newTab();
  void state().computePreview();
}
