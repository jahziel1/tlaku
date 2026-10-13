import type { history as Source } from '../es/history';

/** English for the `history` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const history: { [K in keyof typeof Source]?: string } = {
  // ---- history steps (HistoryEntry labels) --------------------------------
  'history.cut': 'Cut',
  'history.paste': 'Paste',
  'history.initial': 'Initial state',
  'history.group': 'Group',
  'history.ungroup': 'Ungroup',

  // ---- commands -----------------------------------------------------------
  'history.cmd.pasteInPlace': 'Paste in Place',
  'history.cmd.selectInvert': 'Invert Selection',
  'history.cmd.shortcuts': 'Keyboard Shortcuts…',
  'history.cmd.panelHistory': 'Show History',

  // ---- why a command is unavailable ---------------------------------------
  'history.reason.clipboardEmpty': 'The clipboard is empty: copy or cut some objects first.',
  'history.reason.emptyDocument': 'There are no objects in the design.',
  'history.reason.groupNeedsTwo': 'Select two or more unlocked objects to group them.',
  'history.reason.noGroup': 'None of the selected objects is in a group.',

  // ---- menus --------------------------------------------------------------
  'history.menu.help': 'Help',
  'history.menu.recent': 'Open Recent',
  'history.menu.noRecent': 'No recent files yet',
  'history.menu.clearRecent': 'Clear the recent list',

  // ---- History panel ------------------------------------------------------
  'history.dock.title': 'History',
  'history.panel.label': 'Change history',
  'history.panel.hint': 'Click a step to go back to that point. Undone steps can be recovered until you do something else.',
  'history.panel.empty': 'No changes yet. Every action you take will appear here.',
  'history.panel.jump': 'Go back to this point: {label}',
  'history.panel.current': 'Current state',
  'history.panel.undone': 'Undone',
  'history.panel.steps': '{n} of {max} steps',
  'history.panel.clear': 'Clear history',
  'history.panel.clearTitle': 'Clear the history',
  'history.panel.clearMessage': 'All undo and redo steps will be forgotten. The design does not change.',
  'history.panel.clearConfirm': 'Clear',
  'history.panel.cancel': 'Cancel',

  // ---- unsaved changes ----------------------------------------------------
  'history.unsaved.title': 'Unsaved changes',
  'history.unsaved.ask.new': '“{name}” has unsaved changes. Do you want to save them before starting a new project?',
  'history.unsaved.ask.open': '“{name}” has unsaved changes. Do you want to save them before opening another project?',
  'history.unsaved.ask.close': '“{name}” has unsaved changes. Do you want to save them before closing Tlaku?',
  'history.unsaved.save': 'Save',
  'history.unsaved.discard': "Don't Save",
  'history.unsaved.cancel': 'Cancel',

  // ---- file notices -------------------------------------------------------
  'history.notice.opened': 'Project opened: {name}.',
  'history.notice.openFailed': 'Could not open {name}: {error}',
  'history.notice.recentMissing': 'Could not open {name}; it was removed from the recent list. {error}',
  'history.notice.saveFailed': 'Could not save {name}: {error}',
  'history.notice.importedFile': 'Imported {n} objects from {file}.',
  'history.notice.importedWithWarnings':
    'Imported {n} objects from {file}, but the importer reported {w} problems. Check that the design is complete.',
  'history.notice.importedNothing': '{file} has no objects to import.',
  'history.notice.importFailed': 'Could not import {file}: {error}',
  'history.notice.unsupported': '{file}: unsupported format. Supported: SVG, DXF, PDF, AI, HPGL, G-code and images (PNG, JPG, BMP, WebP, TIFF, GIF and TGA).',
  'history.notice.exportFailed': 'Could not export the G-code: {error}',

  // ---- drag and drop ------------------------------------------------------
  'history.drop.title': 'Drop to import',
  'history.drop.hint': 'SVG, DXF, PNG, JPG or BMP',

  // ---- notifications (toasts) ---------------------------------------------
  'history.toast.more': 'Show {n} more',
  'history.toast.less': 'Show less',

  // ---- title bar and status bar -------------------------------------------
  'history.doc.unsaved': 'Unsaved',
  'history.status.objects': '{n} objects',
  'history.status.objectOne': '1 object',
  'history.status.size': 'Selection size: {size}',
  'history.status.saved': 'All saved',
  'history.status.shortcuts': 'Keyboard shortcuts',

  // ---- shortcuts dialog ---------------------------------------------------
  'history.shortcuts.title': 'Keyboard Shortcuts',
  'history.shortcuts.search': 'Search for a command or shortcut',
  'history.shortcuts.hint': 'Opens with ? or {combo}. Esc closes it.',
  'history.shortcuts.none': 'No shortcut matches “{q}”.',
  'history.shortcuts.close': 'Close',
  'history.shortcuts.cat.file': 'File',
  'history.shortcuts.cat.edit': 'Edit',
  'history.shortcuts.cat.tools': 'Tools',
  'history.shortcuts.cat.view': 'View',
  'history.shortcuts.cat.machine': 'Machine',
  'history.shortcuts.cat.arrange': 'Arrange',
  'history.shortcuts.cat.camera': 'Camera',
  'history.shortcuts.cat.panels': 'Panels and help',
  'history.shortcuts.cat.canvas': 'Canvas (gestures)',
  'history.shortcuts.step.normal': '1 mm',
  'history.shortcuts.step.big': '10 mm',
  'history.shortcuts.step.fine': '0.1 mm',
  'history.shortcuts.canvas.wheel': 'Zoom at the cursor',
  'history.shortcuts.canvas.wheelKeys': 'Mouse wheel',
  'history.shortcuts.canvas.pinch': 'Zoom with the trackpad',
  'history.shortcuts.canvas.pinchKeys': 'Pinch',
  'history.shortcuts.canvas.pan': 'Pan the view',
  'history.shortcuts.canvas.panKeys': 'Middle button, Space + drag, or two fingers',
};
