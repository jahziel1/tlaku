import { useMaterials } from '@/features/materials/state';
import type { CommandDef } from '../types';

/**
 * Comandos de materiales. La biblioteca vive en la pestaña Materiales (`panel.materials`); aquí
 * solo está la prueba de materiales, que aparece sola en el menú Herramientas (categoría `tools`):
 * genera un documento, igual que Vectorizar imagen.
 */
export const materialCommands: CommandDef[] = [
  {
    id: 'tools.materialTest',
    label: 'materials.cmd.test',
    category: 'tools',
    run: () => useMaterials.getState().openTest(),
  },
];
