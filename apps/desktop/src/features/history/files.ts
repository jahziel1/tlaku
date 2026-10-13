import { getApi } from '@/api';
import { t } from '@/i18n';
import { fmtClock } from '@/lib/platform';
import { buildJobArgs, useStore } from '@/store';
import { confirmDiscard, pickPath } from './dialogs';

const state = () => useStore.getState();

async function discardOk(): Promise<boolean> {
  return !state().dirty || (await confirmDiscard());
}

export async function newProject(): Promise<void> {
  if (!(await discardOk())) return;
  state().newDocument();
  void state().computePreview();
}

export async function openProject(): Promise<void> {
  if (!(await discardOk())) return;
  const path = await pickPath('openProject');
  if (!path) return;
  const doc = await getApi().loadProject({ path });
  state().replaceDocument(doc, path);
  state().fitContent();
  void state().computePreview();
}

/** Guarda en la ruta actual, o pregunta una si no hay o se pidió "Guardar como". */
export async function saveProject(asNew: boolean): Promise<void> {
  let path = state().filePath;
  if (asNew || !path) {
    path = await pickPath('saveProject', path ?? undefined);
    if (!path) return;
  }
  await getApi().saveProject({ path, doc: state().doc });
  state().markSaved(path);
  state().notify('success', t('notice.saved', { path }));
}

export async function importDesign(): Promise<void> {
  const path = await pickPath('import');
  if (!path) return;
  const result = await getApi().importFile({ path });
  state().applyImport(result);
  state().select(result.objects.map((o) => o.id));
  const n = result.objects.length;
  if (result.warnings.length > 0) {
    state().notify('warning', t('notice.importedWithWarnings', { n, w: result.warnings.length }), result.warnings);
  } else {
    state().notify('success', t('notice.imported', { n }));
  }
}

export async function exportGcodeFile(): Promise<void> {
  const path = await pickPath('exportGcode');
  if (!path) return;
  const r = await getApi().exportGcode({ ...buildJobArgs(state()), path });
  const text = t('notice.exported', { lines: r.lines, time: fmtClock(r.estimate_s), path });
  if (r.warnings.length > 0) state().notify('warning', text, r.warnings);
  else state().notify('success', text);
}
