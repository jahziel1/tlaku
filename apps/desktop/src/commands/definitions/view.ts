import type { CommandDef } from '../types';

/**
 * Notas de uso:
 *  - S `view.snap`: enciende o apaga el ajuste magnético (cuadrícula visible, bordes y centros de
 *    los demás objetos y de la cama; tolerancia de 8 px de pantalla). Mantener Alt mientras se
 *    arrastra lo suspende solo en ese gesto (al escalar, Alt es "desde el centro": ahí se
 *    suspende con Ctrl/Cmd).
 *  - `view.snapGrid` / `view.snapObjects`: eligen a qué se pega. Sin atajo propio; están en la
 *    barra de ajuste del lienzo.
 */
export const viewCommands: CommandDef[] = [
  { id: 'view.zoomIn', label: 'cmd.view.zoomIn', category: 'view', shortcuts: ['Mod+=', 'Mod+Shift+='], repeatable: true, run: (s) => s.zoomAt(1.25) },
  { id: 'view.zoomOut', label: 'cmd.view.zoomOut', category: 'view', shortcuts: ['Mod+-'], repeatable: true, run: (s) => s.zoomAt(1 / 1.25) },
  { id: 'view.fitBed', label: 'cmd.view.fitBed', category: 'view', shortcuts: ['Mod+0'], run: (s) => s.fitBed() },
  { id: 'view.fitContent', label: 'cmd.view.fitContent', category: 'view', shortcuts: ['Mod+Shift+0'], run: (s) => s.fitContent() },
  { id: 'view.wireframe', label: 'cmd.view.wireframe', category: 'view', shortcuts: ['W'], checked: (s) => s.wireframe, run: (s) => s.toggleWireframe() },
  { id: 'view.theme', label: 'cmd.view.theme', category: 'view', run: (s) => s.toggleTheme() },
  { id: 'view.snap', label: 'canvas.cmd.snap', category: 'view', shortcuts: ['S'], checked: (s) => s.snapEnabled, run: (s) => s.toggleSnap() },
  {
    id: 'view.snapGrid',
    label: 'canvas.cmd.snapGrid',
    category: 'view',
    checked: (s) => s.snapEnabled && s.snapGrid,
    enabled: (s) => (s.snapEnabled ? true : 'canvas.reason.snapOff'),
    run: (s) => s.toggleSnapGrid(),
  },
  {
    id: 'view.snapObjects',
    label: 'canvas.cmd.snapObjects',
    category: 'view',
    checked: (s) => s.snapEnabled && s.snapObjects,
    enabled: (s) => (s.snapEnabled ? true : 'canvas.reason.snapOff'),
    run: (s) => s.toggleSnapObjects(),
  },
];
