import { useArrangeUi } from '@/features/arrange/uiStore';
import {
  circleDiameter, selectableObjects, selectContained, selectInLayer, selectOpen, selectOpenInFill, selectSmaller,
} from '@/features/arrange/special';
import { nodeNudge, nodeTargets, nudgeNodes, runNodeOp } from '@/features/canvas/nodeActions';
import { copyAsSvg, pasteFromSystem, systemClipboardAvailable } from '@/features/canvas/systemClipboard';
import { HISTORY_TAB } from '@/features/shell/dockTabs';
import { useShellUi } from '@/features/shell/uiStore';
import type { I18nKey } from '@/i18n';
import { objectBounds, unionBounds } from '@/lib/geometry';
import { countUnits, groupDoc, hasGroup, ungroupDoc } from '@/lib/groups';
import { t } from '@/i18n';
import { selectSelectedObjects } from '@/store';
import type { AppState } from '@/store';
import type { CommandDef } from '../types';

const needSelection = (s: AppState): true | I18nKey => (s.selectedIds.length > 0 ? true : 'reason.noSelection');

/** Hay algo seleccionado que se pueda cortar (no bloqueado). */
const needMovable = (s: AppState): true | I18nKey => {
  const picked = selectSelectedObjects(s);
  if (picked.length === 0) return 'reason.noSelection';
  return picked.some((o) => !o.locked) ? true : 'reason.locked';
};

/** Selecciona `ids` o avisa que no hay nada. */
function pick(s: AppState, ids: string[], none: I18nKey): void {
  if (ids.length === 0) s.notifyKey('info', none);
  else s.select(ids);
}

/** Capa "actual": la activa en el panel de capas o, si no, la del primer objeto seleccionado. */
function currentLayer(s: AppState): string | null {
  return s.activeLayer ?? selectSelectedObjects(s)[0]?.layer ?? null;
}

const needClipboard = (s: AppState): true | I18nKey =>
  s.clipboard && s.clipboard.objects.length > 0 ? true : 'history.reason.clipboardEmpty';

/** Copia al portapapeles del sistema como SVG (mejor esfuerzo: si el sistema lo impide, queda el interno). */
const writeSystem = (s: AppState, ids: readonly string[]): void => {
  const picked = new Set(ids);
  void copyAsSvg(s.doc.objects.filter((o) => picked.has(o.id)), s.doc.layers);
};

/** Ids de todo lo que se puede seleccionar con el área (visible y sin bloquear). */
const selectable = (s: AppState): string[] => s.doc.objects.filter((o) => !o.hidden && !o.locked).map((o) => o.id);

/**
 * Con la herramienta de nodos activa y nodos elegidos, Supr y las flechas actúan sobre los nodos
 * en lugar de sobre el objeto entero (no hay otro oyente de teclado: lo resuelve el comando).
 */
const onNodes = (s: AppState): boolean => s.activeTool === 'nodes' && nodeTargets(s).length > 0;

/** Pasos de las flechas en mm: normal, grande (Mayús) y fino (Alt). Salen de las preferencias. */
export const nudgeSteps = (): { normal: number; big: number; fine: number } => nodeNudge();

const DIRS = {
  left: [-1, 0],
  right: [1, 0],
  up: [0, 1], // Y hacia arriba
  down: [0, -1],
} as const;

const SIZES = [
  { id: 'normal', prefix: '', suffix: '' },
  { id: 'big', prefix: 'Shift+', suffix: '.big' },
  { id: 'fine', prefix: 'Alt+', suffix: '.fine' },
] as const;

const ARROW: Record<keyof typeof DIRS, string> = { left: 'ArrowLeft', right: 'ArrowRight', up: 'ArrowUp', down: 'ArrowDown' };

const nudges: CommandDef[] = Object.entries(DIRS).flatMap(([dir, [ux, uy]]) =>
  SIZES.map<CommandDef>((size) => ({
    id: `edit.nudge.${dir}${size.suffix}`,
    label: `cmd.edit.nudge.${dir}` as I18nKey,
    category: 'edit',
    hidden: true,
    repeatable: true,
    shortcuts: [`${size.prefix}Arrow${dir[0]!.toUpperCase()}${dir.slice(1)}`],
    enabled: (s) => {
      if (onNodes(s)) return true;
      const picked = selectSelectedObjects(s);
      if (picked.length === 0) return 'reason.noSelection';
      return picked.some((o) => !o.locked) ? true : 'reason.locked';
    },
    run: (s) => {
      if (onNodes(s)) {
        const step = nodeNudge()[size.id];
        const ids = nodeTargets(s).map((tg) => `${tg.obj.id}:${tg.sel.join('.')}`).join(',');
        nudgeNodes(s, [ux * step, uy * step], `nodenudge:${ARROW[dir as keyof typeof DIRS]}:${size.id}:${ids}`);
        return;
      }
      const step = nudgeSteps()[size.id];
      // `key` funde la ráfaga (dirección, tamaño y selección iguales) en un solo paso del historial.
      s.moveObjects(s.selectedIds, ux * step, uy * step, `nudge:${dir}:${size.id}:${s.selectedIds.join(',')}`);
    },
  })),
);

export const editCommands: CommandDef[] = [
  {
    id: 'edit.undo',
    label: 'cmd.edit.undo',
    category: 'edit',
    shortcuts: ['Mod+Z'],
    enabled: (s) => (s.canUndo() ? true : 'reason.nothingToUndo'),
    run: (s) => s.undo(),
  },
  {
    id: 'edit.redo',
    label: 'cmd.edit.redo',
    category: 'edit',
    shortcuts: ['Mod+Shift+Z', 'Mod+Y'],
    enabled: (s) => (s.canRedo() ? true : 'reason.nothingToRedo'),
    run: (s) => s.redo(),
  },
  {
    id: 'edit.selectAll',
    label: 'cmd.edit.selectAll',
    category: 'edit',
    shortcuts: ['Mod+A'],
    enabled: (s) => (selectable(s).length > 0 ? true : 'history.reason.emptyDocument'),
    run: (s) => s.selectAll(),
  },
  {
    id: 'edit.selectInvert',
    label: 'history.cmd.selectInvert',
    category: 'edit',
    shortcuts: ['Mod+Shift+I'],
    enabled: (s) => (selectable(s).length > 0 ? true : 'history.reason.emptyDocument'),
    run: (s) => {
      const picked = new Set(s.selectedIds);
      s.select(selectable(s).filter((id) => !picked.has(id)));
    },
  },
  {
    id: 'edit.duplicate',
    label: 'cmd.edit.duplicate',
    category: 'edit',
    shortcuts: ['Mod+D'],
    enabled: needSelection,
    run: (s) => s.select(s.duplicateObjects(s.selectedIds)),
  },
  {
    id: 'edit.delete',
    label: 'cmd.edit.delete',
    category: 'edit',
    shortcuts: ['Delete', 'Backspace'],
    enabled: (s) => (onNodes(s) ? true : needSelection(s)),
    run: (s) => {
      if (onNodes(s)) runNodeOp(s, 'delete');
      else s.deleteObjects(s.selectedIds);
    },
  },
  {
    id: 'edit.copy',
    label: 'cmd.edit.copy',
    category: 'edit',
    shortcuts: ['Mod+C'],
    enabled: needSelection,
    run: (s) => {
      if (s.copyObjects(s.selectedIds) > 0) writeSystem(s, s.selectedIds);
    },
  },
  {
    id: 'edit.cut',
    label: 'cmd.edit.cut',
    category: 'edit',
    shortcuts: ['Mod+X'],
    enabled: needMovable,
    run: (s) => {
      const ids = s.selectedIds;
      if (s.cutObjects(ids) > 0) writeSystem(s, ids);
    },
  },
  {
    // El SVG lo ven otras aplicaciones; Copiar ya lo escribe también, esto avisa que se hizo.
    id: 'edit.copySvg',
    label: 'cmd.edit.copySvg',
    category: 'edit',
    shortcuts: ['Mod+Alt+C'],
    enabled: needSelection,
    run: async (s) => {
      s.copyObjects(s.selectedIds);
      const picked = new Set(s.selectedIds);
      const ok = await copyAsSvg(s.doc.objects.filter((o) => picked.has(o.id)), s.doc.layers);
      s.notify(ok ? 'success' : 'warning', t(ok ? 'canvas.clip.copied' : 'canvas.clip.copyFailed'));
    },
  },
  {
    // Pega lo que haya en el portapapeles del sistema (un SVG o un texto de otra aplicación) o, si
    // no hay nada ajeno, lo copiado dentro de la app.
    id: 'edit.paste',
    label: 'cmd.edit.paste',
    category: 'edit',
    shortcuts: ['Mod+V'],
    // Con el portapapeles del sistema a mano puede haber algo ajeno que pegar; sin él, solo el interno.
    enabled: (s) => (systemClipboardAvailable() ? true : needClipboard(s)),
    run: async (s) => {
      if ((await pasteFromSystem(s)) === 'none') s.notifyKey('info', 'history.reason.clipboardEmpty');
    },
  },
  {
    id: 'edit.pasteInPlace',
    label: 'history.cmd.pasteInPlace',
    category: 'edit',
    shortcuts: ['Mod+Shift+V'],
    enabled: needClipboard,
    run: (s) => s.select(s.pasteObjects({ inPlace: true })),
  },
  {
    id: 'edit.group',
    label: 'cmd.edit.group',
    category: 'edit',
    shortcuts: ['Mod+G'],
    // Un grupo completo cuenta como un solo objeto: se necesitan dos "unidades" (anida grupos).
    enabled: (s) => (countUnits(s.doc.objects, s.selectedIds) >= 2 ? true : 'history.reason.groupNeedsTwo'),
    run: (s) => s.applyDocChange('history.group', (doc) => groupDoc(doc, s.selectedIds)),
  },
  {
    id: 'edit.ungroup',
    label: 'cmd.edit.ungroup',
    category: 'edit',
    shortcuts: ['Mod+Shift+G'],
    enabled: (s) => (hasGroup(s.doc.objects, s.selectedIds) ? true : 'history.reason.noGroup'),
    run: (s) => s.applyDocChange('history.ungroup', (doc) => ungroupDoc(doc, s.selectedIds)),
  },
  // Selecciones especiales
  {
    id: 'edit.selectOpen',
    label: 'cmd.edit.selectOpen',
    category: 'edit',
    enabled: (s) => (selectable(s).length > 0 ? true : 'history.reason.emptyDocument'),
    run: (s) => pick(s, selectOpen(selectableObjects(s.doc.objects, s.doc.layers)), 'canvas.select.none.open'),
  },
  {
    id: 'edit.selectOpenFill',
    label: 'cmd.edit.selectOpenFill',
    category: 'edit',
    enabled: (s) => (selectable(s).length > 0 ? true : 'history.reason.emptyDocument'),
    run: (s) => pick(s, selectOpenInFill(selectableObjects(s.doc.objects, s.doc.layers), s.doc.layers), 'canvas.select.none.openFill'),
  },
  {
    id: 'edit.selectLayer',
    label: 'cmd.edit.selectLayer',
    category: 'edit',
    enabled: (s) => (currentLayer(s) ? true : 'canvas.select.reason.noLayer'),
    run: (s) => {
      const color = currentLayer(s);
      if (color) pick(s, selectInLayer(selectableObjects(s.doc.objects, s.doc.layers), color), 'canvas.select.none.layer');
    },
  },
  {
    id: 'edit.selectContained',
    label: 'cmd.edit.selectContained',
    category: 'edit',
    enabled: needSelection,
    run: (s) => {
      const containers = selectSelectedObjects(s).filter((o) => o.kind.type === 'vector');
      const found = selectContained(selectableObjects(s.doc.objects, s.doc.layers), containers);
      if (found.length <= containers.length) s.notifyKey('info', 'canvas.select.none.contained');
      else s.select(found);
    },
  },
  {
    id: 'edit.selectSmaller',
    label: 'cmd.edit.selectSmaller',
    category: 'edit',
    enabled: needSelection,
    run: (s) => {
      const picked = selectSelectedObjects(s);
      const box = unionBounds(picked.map(objectBounds));
      if (!box) return;
      const limit = (box[1][0] - box[0][0]) * (box[1][1] - box[0][1]);
      const exclude = new Set(s.selectedIds);
      pick(s, selectSmaller(selectableObjects(s.doc.objects, s.doc.layers), limit, exclude), 'canvas.select.none.smaller');
    },
  },
  {
    // Para agujeros de tornillo y similares: círculos de un diámetro exacto o dentro de un rango.
    id: 'edit.selectCircles',
    label: 'cmd.edit.selectCircles',
    category: 'edit',
    enabled: (s) => (selectable(s).length > 0 ? true : 'history.reason.emptyDocument'),
    run: (s) => {
      const d = selectSelectedObjects(s).length === 1 ? circleDiameter(selectSelectedObjects(s)[0]!) : null;
      useArrangeUi.getState().patchCircles(d ? { min: d, max: d } : {});
      useArrangeUi.getState().open({ kind: 'circles' });
    },
  },
  // Paneles y ayuda. `?` (en cualquier distribución de teclado) lo atiende el shell; aquí quedan
  // los atajos que el registro puede resolver por tecla física.
  {
    id: 'panel.history',
    label: 'history.cmd.panelHistory',
    category: 'panels',
    shortcuts: ['Mod+4'],
    // El historial es una pestaña más del panel acoplado.
    checked: (s) => s.dockTab === HISTORY_TAB,
    run: (s) => s.setDockTab(HISTORY_TAB),
  },
  {
    id: 'help.shortcuts',
    label: 'history.cmd.shortcuts',
    category: 'panels',
    shortcuts: ['Mod+/', 'F1', 'Shift+/'],
    run: () => useShellUi.getState().openShortcuts(),
  },
  ...nudges,
];
