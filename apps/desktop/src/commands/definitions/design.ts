import { useGuideUi } from '@/features/canvas/guides';
import { anyStart, anyTabs, clearStart, clearTabs, reverseDirection } from '@/features/canvas/marksOps';
import { useTemplatesUi } from '@/features/shell/templatesUi';
import { getApi } from '@/api';
import { errorMessage } from '@/lib/errors';
import { t, type I18nKey } from '@/i18n';
import { selectSelectedObjects } from '@/store';
import type { AppState } from '@/store';
import type { CommandDef } from '../types';

/**
 * Comandos de la ola 3 de diseño: guías, puentes manuales, punto de inicio y sentido, notas,
 * plantillas y nueva ventana. Las herramientas `tool.tabs` y `tool.start` están en `tools.ts`.
 *
 *  - `view.guides`: muestra u oculta las guías del documento. Se crean arrastrando desde las
 *    reglas, se mueven arrastrándolas (Alt suspende el ajuste) y se borran soltándolas sobre su
 *    regla o fuera del lienzo; `view.guidesDialog` da la posición exacta de cada una.
 *  - `arrange.reverseDirection`: invierte el sentido de recorrido de los contornos seleccionados
 *    (si todos ya van invertidos, lo deshace). El planificador respeta el sentido fijado.
 */

/** Objetos de la selección que tienen un trazo cuyo inicio y sentido se pueden fijar. */
function strokeSelection(s: AppState): true | I18nKey {
  const picked = selectSelectedObjects(s);
  if (picked.length === 0) return 'reason.noSelection';
  return picked.some((o) => !o.locked && o.kind.type !== 'image') ? true : 'canvas.reason.noStroke';
}

export const designCommands: CommandDef[] = [
  { id: 'view.guides', label: 'canvas.cmd.guides', category: 'view', checked: (s) => s.showGuides, run: (s) => s.toggleGuides() },
  { id: 'view.guidesDialog', label: 'canvas.cmd.guidesDialog', category: 'view', run: () => useGuideUi.getState().show() },
  { id: 'view.startMarks', label: 'canvas.cmd.startMarks', category: 'view', checked: (s) => s.showStartMarks, run: (s) => s.toggleStartMarks() },

  { id: 'panel.notes', label: 'canvas.cmd.panelNotes', category: 'panels', shortcuts: ['Mod+7'], checked: (s) => s.dockTab === 'notes', run: (s) => s.setDockTab('notes') },

  {
    id: 'arrange.reverseDirection',
    label: 'canvas.cmd.reverse',
    category: 'arrange',
    enabled: strokeSelection,
    run: (s) => s.applyDocChange('canvas.history.reverse', (doc) => reverseDirection(doc, s.selectedIds)),
  },
  {
    id: 'arrange.clearStart',
    label: 'canvas.cmd.clearStart',
    category: 'arrange',
    enabled: (s) => (anyStart(selectSelectedObjects(s)) ? true : 'canvas.reason.noStart'),
    run: (s) => s.applyDocChange('canvas.history.clearStart', (doc) => clearStart(doc, s.selectedIds)),
  },
  {
    id: 'arrange.clearTabs',
    label: 'canvas.cmd.clearTabs',
    category: 'arrange',
    enabled: (s) => (anyTabs(selectSelectedObjects(s)) ? true : 'canvas.reason.noTabs'),
    run: (s) => s.applyDocChange('canvas.history.clearTabs', (doc) => clearTabs(doc, s.selectedIds)),
  },

  { id: 'file.saveTemplate', label: 'canvas.cmd.saveTemplate', category: 'file', run: () => useTemplatesUi.getState().open('save') },
  { id: 'file.newFromTemplate', label: 'canvas.cmd.newFromTemplate', category: 'file', run: () => useTemplatesUi.getState().open('new') },
  {
    id: 'file.newWindow',
    label: 'canvas.cmd.newWindow',
    category: 'file',
    shortcuts: ['Mod+Shift+N'],
    run: async (s) => {
      try {
        await getApi().newWindow();
      } catch (e) {
        s.notify('error', t('canvas.window.failed', { error: errorMessage(e) }));
      }
    },
  },
];
