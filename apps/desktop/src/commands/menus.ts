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
    items: ['file.new', 'file.open', '-', 'file.save', 'file.saveAs', '-', 'file.import', 'file.exportGcode'],
  },
  {
    id: 'edit',
    label: 'menu.edit',
    items: [
      'edit.undo', 'edit.redo', '-',
      'edit.cut', 'edit.copy', 'edit.paste', 'edit.duplicate', '-',
      'edit.selectAll', 'edit.delete', '-',
      'edit.group', 'edit.ungroup',
    ],
  },
  {
    id: 'tools',
    label: 'menu.tools',
    items: ['tool.select', 'tool.nodes', '-', 'tool.line', 'tool.rect', 'tool.ellipse', 'tool.text', '-', 'tool.pan', 'tool.measure'],
  },
  {
    id: 'view',
    label: 'menu.view',
    items: [
      'view.zoomIn', 'view.zoomOut', 'view.fitBed', 'view.fitContent', '-',
      'view.wireframe', 'view.preview', '-',
      'view.snap', 'view.snapGrid', 'view.snapObjects', '-',
      'panel.layers', 'panel.machine', 'panel.console', '-',
      'view.theme',
    ],
  },
  {
    id: 'machine',
    label: 'menu.machine',
    items: ['machine.connect', '-', 'machine.frame', 'machine.start', 'machine.pause', 'machine.stop'],
  },
  {
    id: 'arrange',
    label: 'menu.arrange',
    items: [
      'arrange.union', 'arrange.difference', 'arrange.intersection', 'arrange.xor', 'arrange.weld', '-',
      'arrange.offset', '-',
      'arrange.join', 'arrange.close', 'arrange.dedupe', '-',
      'arrange.align.left', 'arrange.align.hcenter', 'arrange.align.right',
      'arrange.align.top', 'arrange.align.vcenter', 'arrange.align.bottom', '-',
      'arrange.distribute.h', 'arrange.distribute.v', 'arrange.alignDialog', '-',
      'arrange.alignTo.selection', 'arrange.alignTo.object', 'arrange.alignTo.bed', 'arrange.centerOnBed', '-',
      'arrange.grid', 'arrange.circular',
    ],
  },
];
