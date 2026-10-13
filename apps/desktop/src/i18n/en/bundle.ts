import type { bundle as es } from '../es/bundle';

/** English texts for the `.tlakubundle` settings bundles. Every key starts with `bundle.` (checked by i18n.test.ts). */
export const bundle: Record<keyof typeof es, string> = {
  // ---- commands ---------------------------------------------------------------
  'bundle.cmd.export': 'Export settings bundle…',
  'bundle.cmd.import': 'Import settings bundle…',

  // ---- dialog -----------------------------------------------------------------
  'bundle.close': 'Close',
  'bundle.cancel': 'Cancel',
  'bundle.title.export': 'Export settings bundle',
  'bundle.title.import': 'Import settings bundle',
  'bundle.section.devices': 'Machines',
  'bundle.section.materials': 'Materials',
  'bundle.section.prefs': 'Preferences and shortcuts',

  // ---- export -----------------------------------------------------------------
  'bundle.export.hint': 'Saves whatever you pick in a single file, to move it to another computer or share it.',
  'bundle.export.devices': '{n} machine profiles, with their saved macros and positions, and which machine is active',
  'bundle.export.materials': '{n} material presets from the library',
  'bundle.export.prefs': 'Your preferences and custom keyboard shortcuts',
  'bundle.export.none': 'Pick at least one section.',
  'bundle.export.action': 'Export…',

  // ---- import -----------------------------------------------------------------
  'bundle.import.hint': 'Pick a bundle, review what would change and tick the sections to apply. Nothing is written until you confirm.',
  'bundle.import.pick': 'Choose file…',
  'bundle.import.another': 'Choose another file…',
  'bundle.import.meta': '{name} · created on {date} with Tlaku {app}',
  'bundle.import.metaNoApp': '{name} · created on {date}',
  'bundle.import.preview': 'Preview',
  'bundle.import.absent': 'The bundle does not include this section.',
  'bundle.import.nothing': 'No changes: everything is already the same.',
  'bundle.import.empty': 'The bundle has no sections.',
  'bundle.import.apply': 'Apply',
  'bundle.import.applying': 'Applying…',
  'bundle.import.noneChecked': 'Tick at least one section that has changes.',

  // ---- section summaries ----------------------------------------------------------
  'bundle.devices.summary': '{added} new · {replaced} replace one with the same id · {unchanged} unchanged',
  'bundle.devices.replaced': 'Will be replaced: {names}',
  'bundle.devices.active': 'Active machine: {name}',
  'bundle.devices.invalid': '{n} profiles skipped because they are not valid',
  'bundle.materials.summary': '{added} new · {skipped} already there · what you have is kept',
  'bundle.materials.renamed': '{n} clashed with a different material of the same id and are added as copies with a new id',
  'bundle.materials.invalid': '{n} presets skipped because they are not valid',
  'bundle.prefs.summary': '{prefs} preferences and {shortcuts} commands with custom shortcuts',
  'bundle.prefs.ignored': '{n} preferences are not recognised and will be ignored',
  'bundle.prefs.unknown': '{n} shortcuts belong to commands this version does not have (they are kept)',
  'bundle.prefs.replaceShortcuts': 'If the bundle has shortcuts, they replace the current ones.',

  // ---- reasons for skipping --------------------------------------------------------
  'bundle.invalid.item': '"{id}": {reason}',
  'bundle.reason.notObject': 'not a profile',
  'bundle.reason.noId': 'has no id',
  'bundle.reason.noName': 'has no name',
  'bundle.reason.invalid': 'invalid data ({fields})',
  'bundle.reason.duplicate': 'id repeated in the bundle',

  // ---- notices and errors ----------------------------------------------------------
  'bundle.file.filter': 'Tlaku settings bundle',
  'bundle.file.exportTitle': 'Export settings bundle',
  'bundle.file.importTitle': 'Import settings bundle',
  'bundle.notice.exported': 'Bundle saved to {path}.',
  'bundle.notice.exportFailed': 'Could not export the bundle: {error}',
  'bundle.notice.imported': 'Bundle "{name}" applied: {devices} machines, {materials} new materials and {prefs} preferences.',
  'bundle.notice.devicesFailed': '{n} machines could not be saved.',
  'bundle.notice.importFailed': 'Could not read the bundle "{name}": {error}',
  'bundle.blocked.job': 'A bundle cannot be imported while a job is running.',
  'bundle.err.notBundle': 'The file is not a Tlaku settings bundle.',
  'bundle.err.materialsUnavailable': 'The material library could not be read, so it was left untouched.',
};
