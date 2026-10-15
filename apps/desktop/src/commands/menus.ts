import type { I18nKey } from '@/i18n';

/** Estructura de los menús. Solo ids de comando: etiquetas y atajos salen del registro. */
export interface MenuDef {
  id: string;
  label: I18nKey;
  items: (string | '-')[];
}

/**
 * Un comando que ningún menú menciona aparece solo al final del menú de su categoría
 * (features/shell/menuModel.ts), así que cada área puede agregar comandos sin editar este archivo.
 */
export const MENUS: MenuDef[] = [
  {
    id: 'file',
    label: 'menu.file',
    items: [
      'file.new', 'file.open', '-', 'tabs.new', 'tabs.close', 'tabs.closeOthers', 'tabs.duplicate', 'tabs.moveToWindow', '-', 'file.save', 'file.saveAs', 'tabs.saveAll', 'versions.saveNamed', 'versions.open', '-', 'file.import', 'fileio.importOptions', '-',
      'bundle.export', 'bundle.import', '-',
      'file.exportGcode', 'fileio.exportSvg', 'fileio.exportDxf', 'fileio.exportPng', 'fileio.exportPdf', 'fileio.exportAi', '-', 'fileio.print',
    ],
  },
  {
    id: 'edit',
    label: 'menu.edit',
    items: [
      'edit.undo', 'edit.redo', '-',
      'edit.cut', 'edit.copy', 'edit.copySvg', 'edit.paste', 'edit.duplicate', '-',
      'edit.selectAll', 'edit.delete', '-',
      'edit.selectLayer', 'edit.selectOpen', 'edit.selectOpenFill', 'edit.selectContained', 'edit.selectSmaller', 'edit.selectCircles', '-',
      'edit.group', 'edit.ungroup', '-',
      'prefs.open', 'prefs.shortcuts',
    ],
  },
  {
    id: 'tools',
    label: 'menu.tools',
    items: ['tool.select', 'tool.nodes', '-', 'tool.line', 'tool.rect', 'tool.ellipse', 'tool.polygon', 'tool.star', 'tool.text', '-', 'tool.pan', 'tool.measure', '-', 'parts.open', 'parts.regenerate'],
  },
  {
    id: 'view',
    label: 'menu.view',
    items: [
      'tabs.next', 'tabs.prev', '-',
      'view.zoomIn', 'view.zoomOut', 'view.fitBed', 'view.fitContent', '-',
      'view.wireframe', 'view.preview', 'view.priorityBadges', '-',
      'view.snap', 'view.snapGrid', 'view.snapObjects', '-',
      'panel.layers', 'panel.machine', 'panel.console', '-',
      'view.theme',
    ],
  },
  {
    id: 'machine',
    label: 'menu.machine',
    items: ['machine.connect', '-', 'machine.frame', 'machine.frameHull', 'review.open', 'machine.start', 'machine.pause', 'machine.stop', '-', 'machine.positionLaser', 'machine.probeCenter', 'preview.optimization', '-', 'printcut.open', 'printcut.reset', 'machine.tiling'],
  },
  {
    id: 'arrange',
    label: 'menu.arrange',
    items: [
      'arrange.flip.h', 'arrange.flip.v', 'arrange.mirrorLine', '-',
      'arrange.rotate.ccw90', 'arrange.rotate.cw90', 'arrange.rotate.ccwStep', 'arrange.rotate.cwStep',
      'arrange.rotate.ccw5', 'arrange.rotate.cw5', 'arrange.twoPoint', '-',
      'arrange.order.front', 'arrange.order.forward', 'arrange.order.backward', 'arrange.order.back', '-',
      'arrange.priority.earlier', 'arrange.priority.later', '-',
      'arrange.lock', 'arrange.unlockAll', '-',
      'arrange.same.width', 'arrange.same.height', 'arrange.same.widthKeep', 'arrange.same.heightKeep', '-',
      'arrange.xform.move', 'arrange.xform.scale', 'arrange.xform.rotate', 'arrange.xform.center', 'arrange.xform.ratio', '-',
      'arrange.union', 'arrange.difference', 'arrange.intersection', 'arrange.xor', 'arrange.weld', '-',
      'arrange.offset', '-',
      'arrange.join', 'arrange.close', 'arrange.dedupe', '-',
      'arrange.trim', 'arrange.breakApart', 'arrange.optimize', 'arrange.fillet', 'arrange.cutShapes', 'arrange.resizeSlots', '-',
      'arrange.align.left', 'arrange.align.hcenter', 'arrange.align.right',
      'arrange.align.top', 'arrange.align.vcenter', 'arrange.align.bottom', '-',
      'arrange.distribute.h', 'arrange.distribute.v', 'arrange.alignDialog', '-',
      'arrange.alignTo.selection', 'arrange.alignTo.object', 'arrange.alignTo.bed', 'arrange.centerOnBed', '-',
      'arrange.grid', 'arrange.circular', 'arrange.nest',
    ],
  },
];
