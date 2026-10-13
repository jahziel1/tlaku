import type { CommandDef } from '../types';

export const viewCommands: CommandDef[] = [
  { id: 'view.zoomIn', label: 'cmd.view.zoomIn', category: 'view', shortcuts: ['Mod+=', 'Mod+Shift+='], repeatable: true, run: (s) => s.zoomAt(1.25) },
  { id: 'view.zoomOut', label: 'cmd.view.zoomOut', category: 'view', shortcuts: ['Mod+-'], repeatable: true, run: (s) => s.zoomAt(1 / 1.25) },
  { id: 'view.fitBed', label: 'cmd.view.fitBed', category: 'view', shortcuts: ['Mod+0'], run: (s) => s.fitBed() },
  { id: 'view.fitContent', label: 'cmd.view.fitContent', category: 'view', shortcuts: ['Mod+Shift+0'], run: (s) => s.fitContent() },
  { id: 'view.wireframe', label: 'cmd.view.wireframe', category: 'view', shortcuts: ['W'], checked: (s) => s.wireframe, run: (s) => s.toggleWireframe() },
  { id: 'view.theme', label: 'cmd.view.theme', category: 'view', run: (s) => s.toggleTheme() },
];
