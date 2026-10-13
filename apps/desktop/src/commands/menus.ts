import type { I18nKey } from '@/i18n';

/** Estructura de los menús. Solo ids de comando: etiquetas y atajos salen del registro. */
export interface MenuDef {
  id: string;
  label: I18nKey;
  items: (string | '-')[];
}

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
      'panel.layers', 'panel.machine', 'panel.console', '-',
      'view.theme',
    ],
  },
  {
    id: 'machine',
    label: 'menu.machine',
    items: ['machine.connect', '-', 'machine.frame', 'machine.start', 'machine.pause', 'machine.stop'],
  },
];
