import { useStore } from '@/store';
import { editCommands } from './definitions/edit';
import { fileCommands } from './definitions/file';
import { layerCommands } from './definitions/layers';
import { machineCommands } from './definitions/machine';
import { panelCommands } from './definitions/panels';
import { previewCommands } from './definitions/preview';
import { toolCommands } from './definitions/tools';
import { viewCommands } from './definitions/view';
import { CommandRegistry } from './registry';
import type { CommandDef } from './types';

export * from './registry';
export * from './shortcuts';
export * from './menus';
export type { CommandDef, CommandCategory } from './types';

/**
 * Para agregar un comando: añadirlo en el archivo de `definitions/` de su categoría.
 * El registro rechaza ids o atajos repetidos al arrancar.
 */
export const ALL_COMMANDS: CommandDef[] = [
  ...fileCommands,
  ...editCommands,
  ...toolCommands,
  ...viewCommands,
  ...machineCommands,
  ...panelCommands,
  ...layerCommands,
  ...previewCommands,
];

export const commands = new CommandRegistry(ALL_COMMANDS, () => useStore.getState());
