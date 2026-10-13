import { useBundleUi } from '@/features/bundle/uiStore';
import type { CommandDef } from '../types';

/**
 * Paquetes de configuración `.tlakubundle` (máquinas, materiales, preferencias y atajos en un solo
 * archivo). Abren su diálogo; ahí se eligen las secciones y, al importar, se ve la vista previa.
 */
export const bundleCommands: CommandDef[] = [
  { id: 'bundle.export', label: 'bundle.cmd.export', category: 'file', run: () => useBundleUi.getState().openBundle('export') },
  { id: 'bundle.import', label: 'bundle.cmd.import', category: 'file', run: () => useBundleUi.getState().openBundle('import') },
];
