import { getApi } from '@/api';
import type { Document } from '@/api/types';
import { ensureSaved } from '@/features/history/files';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { useStore } from '@/store';

/**
 * Plantillas de proyecto: el diseño, las capas y los ajustes del documento (y sus notas y guías) se
 * guardan en la carpeta de configuración y sirven para empezar otro proyecto. No incluyen el
 * historial, la selección ni el archivo de origen.
 */

const state = () => useStore.getState();

/** Lo que se guarda en la plantilla: el documento actual tal cual. */
export const templateDoc = (doc: Document): Document => doc;

/** El documento nuevo parte de la plantilla pero conserva la máquina activa si la plantilla no trae una. */
export function documentFromTemplate(template: Document, currentDeviceId: string | null): Document {
  return { ...template, device_id: template.device_id ?? currentDeviceId };
}

export async function saveAsTemplate(name: string): Promise<boolean> {
  try {
    const info = await getApi().saveTemplate({ name, doc: templateDoc(state().doc) });
    state().notify('success', t('canvas.templates.saved', { name: info.name }));
    return true;
  } catch (e) {
    state().notify('error', t('canvas.templates.saveFailed', { name, error: errorMessage(e) }));
    return false;
  }
}

/** Reemplaza el documento por la plantilla `name` (pregunta antes si hay cambios sin guardar). */
export async function newFromTemplate(name: string): Promise<boolean> {
  if (!(await ensureSaved('new'))) return false;
  try {
    const doc = await getApi().loadTemplate({ name });
    state().replaceDocument(documentFromTemplate(doc, state().doc.device_id), null);
    state().fitBed();
    void state().computePreview();
    state().notify('success', t('canvas.templates.created', { name }));
    return true;
  } catch (e) {
    state().notify('error', t('canvas.templates.loadFailed', { name, error: errorMessage(e) }));
    return false;
  }
}

export async function deleteTemplate(name: string): Promise<boolean> {
  try {
    await getApi().deleteTemplate({ name });
    return true;
  } catch (e) {
    state().notify('error', t('canvas.templates.deleteFailed', { name, error: errorMessage(e) }));
    return false;
  }
}
