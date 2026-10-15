import type { versions as Source } from '../es/versions';

/** English for the `versions` area (project version history). Missing keys fall back to Spanish. */
export const versions: { [K in keyof typeof Source]?: string } = {
  // ---- commands ----
  'versions.cmd.open': 'Version history…',
  'versions.cmd.save': 'Save version…',
  'versions.reason.noFile': 'Save the project to a file to get a version history.',
  'versions.history.restore': 'Restore version',

  // ---- main dialog ----
  'versions.title': 'Version history',
  'versions.subtitle': 'Project: {name}',
  'versions.close': 'Close',
  'versions.noFile.title': 'This document has no file yet',
  'versions.noFile.body':
    'No file, no history: in the meantime the recovery copy protects your work. Save the project to start keeping versions, or open the history of another project.',
  'versions.toolbar.save': 'Save version…',
  'versions.toolbar.openOther': 'History of another file…',
  'versions.projects.label': 'Other projects with history',
  'versions.projects.choose': 'Pick a project…',
  'versions.projects.option': '{name} · {n} versions',
  'versions.projects.missing': '(file no longer exists)',
  'versions.loading': 'Loading versions…',
  'versions.loadFailed': 'Could not read the history: {error}',

  // ---- list ----
  'versions.list.label': 'Saved versions',
  'versions.list.empty':
    'This project has no versions yet. They are created when you save (what the file held before is kept), every few minutes while you edit, and when you use "Save version…".',
  'versions.kind.saved': 'Saved',
  'versions.kind.auto': 'Automatic',
  'versions.kind.manual': 'Named',
  'versions.kind.restore': 'Before restoring',
  'versions.row.objects.one': '{n} object',
  'versions.row.objects.other': '{n} objects',
  'versions.row.layers.one': '{n} layer',
  'versions.row.layers.other': '{n} layers',
  'versions.row.thumbAlt': 'Version thumbnail',

  // ---- detail ----
  'versions.detail.empty': 'Pick a version from the list to view, compare or restore it.',
  'versions.detail.preview': 'Preview',
  'versions.detail.noThumb': 'No preview (the design is empty or too dense).',
  'versions.detail.size': '{size} document, {stored} on disk',
  'versions.detail.restoreNew': 'Restore as a new tab',
  'versions.detail.restoreHere': 'Replace the open document…',
  'versions.detail.highlight': 'Highlight what changes on the canvas',
  'versions.detail.delete': 'Delete this version',
  'versions.reason.notOpen': 'The file is not open in any tab: open it to replace its document.',
  'versions.reason.nothingToHighlight': 'There are no objects in the open document to highlight.',

  // ---- comparison ----
  'versions.compare.title': 'What restoring it would change',
  'versions.compare.loading': 'Comparing with the open document…',
  'versions.compare.failed': 'Could not compare: {error}',
  'versions.compare.needOpen': 'Open the file in a tab to compare this version with the current document.',
  'versions.compare.same': 'It matches the open document: restoring it would change nothing.',
  'versions.compare.lost.one': '{n} object added since would be lost.',
  'versions.compare.lost.other': '{n} objects added since would be lost.',
  'versions.compare.back.one': '{n} object deleted since would come back.',
  'versions.compare.back.other': '{n} objects deleted since would come back.',
  'versions.compare.reverted.one': '{n} object would go back to how it was.',
  'versions.compare.reverted.other': '{n} objects would go back to how they were.',
  'versions.compare.layersLost': 'These layers would be removed: {names}.',
  'versions.compare.layersBack': 'These layers would come back: {names}.',
  'versions.compare.layersChanged': 'Layer settings would change for: {names}.',
  'versions.compare.notes': 'The project notes would change.',
  'versions.compare.guides': 'The guides would change.',
  'versions.compare.device': 'The project device would change.',

  // ---- delete ----
  'versions.delete.title': 'Delete version',
  'versions.delete.message': '"{name}" will be removed from the history. This cannot be undone.',
  'versions.delete.confirm': 'Delete',
  'versions.delete.cancel': 'Keep it',

  // ---- replace the document ----
  'versions.replace.title': 'Replace the document',
  'versions.replace.message':
    'The open document of "{name}" will be replaced by the version "{when}". What you have now is saved first as a version ("Before restoring"), and you can also use Undo.',
  'versions.replace.confirm': 'Restore',
  'versions.replace.cancel': 'Cancel',

  // ---- footer and settings ----
  'versions.footer.summary.one': '{n} version · {size}',
  'versions.footer.summary.other': '{n} versions · {size}',
  'versions.footer.dir': 'Folder: {dir}',
  'versions.settings.title': 'History settings',
  'versions.settings.enabled': 'Keep the previous version on every save and take automatic snapshots',
  'versions.settings.location': 'Where to store versions',
  'versions.settings.location.app': 'In the app data folder (recommended)',
  'versions.settings.location.beside': 'Next to the file, in the hidden ".tlaku-versions" folder',
  'versions.settings.locationHint':
    'Next to the file it travels with the project (and with any cloud that syncs the folder); in the app folder it keeps your folders clean. When you change it, the earlier history stays visible.',
  'versions.settings.autoMinutes': 'Automatic snapshot every (minutes; 0 = never)',
  'versions.settings.recentHours': 'Keep every version from the last (hours)',
  'versions.settings.dailyDays': 'Then keep one per day up to (days ago)',
  'versions.settings.weeklyWeeks': 'Then keep one per week up to (weeks ago)',
  'versions.settings.maxMb': 'Size cap per project (MB)',
  'versions.settings.maxVersions': 'Version cap per project',
  'versions.settings.help':
    'Cleanup removes the oldest first and never touches the newest version or named versions. Past the last tier, every automatic version is removed.',
  'versions.settings.apply': 'Apply',
  'versions.settings.applied': 'History settings saved.',
  'versions.settings.failed': 'Could not save the history settings: {error}',
  'versions.settings.clearAll': 'Delete this project’s whole history…',
  'versions.clear.title': 'Delete the history',
  'versions.clear.message': 'All {n} versions of "{name}" will be deleted, including the named ones. This cannot be undone.',
  'versions.clear.confirm': 'Delete everything',

  // ---- "Save version…" ----
  'versions.name.title': 'Save version',
  'versions.name.label': 'Version name',
  'versions.name.placeholder': 'For example: before changing the cut',
  'versions.name.hint':
    'Stores a copy of the document exactly as it is now, including changes you have not saved yet. Named versions are never deleted automatically.',
  'versions.name.save': 'Save version',
  'versions.name.cancel': 'Cancel',

  // ---- notices ----
  'versions.notice.captureFailed':
    'Could not store the previous version in the history: {error}. The project is saved anyway.',
  'versions.notice.autoFailed': 'Could not take the automatic history snapshot: {error}',
  'versions.notice.needFile': 'Save the project to a file first: versions are kept per file.',
  'versions.notice.saved': 'Version "{name}" stored in the history.',
  'versions.notice.saveFailed': 'Could not save the version: {error}',
  'versions.notice.openedCopy': 'Version "{when}" of "{name}" opened in a new tab. Save it under another name to keep it.',
  'versions.notice.restored': 'Version "{when}" restored. You can undo it; the previous state is in the history.',
  'versions.notice.restoreFailed': 'Could not restore the version: {error}',
  'versions.notice.notOpen': 'That file is not open in any tab: restore it as a new tab instead.',
  'versions.notice.deleted.one': '{n} version deleted.',
  'versions.notice.deleted.other': '{n} versions deleted.',
  'versions.notice.deleteFailed': 'Could not delete: {error}',
};
