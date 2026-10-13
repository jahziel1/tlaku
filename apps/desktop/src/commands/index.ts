import { useStore } from '@/store';
import { arrangeCommands } from './definitions/arrange';
import { cameraCommands } from './definitions/camera';
import { codesCommands } from './definitions/codes';
import { deviceCommands } from './definitions/devices';
import { editCommands } from './definitions/edit';
import { fileCommands } from './definitions/file';
import { fileioCommands } from './definitions/fileio';
import { helpCommands } from './definitions/help';
import { layerCommands } from './definitions/layers';
import { machineCommands } from './definitions/machine';
import { materialCommands } from './definitions/materials';
import { panelCommands } from './definitions/panels';
import { prefsCommands } from './definitions/prefs';
import { previewCommands } from './definitions/preview';
import { toolCommands } from './definitions/tools';
import { traceCommands } from './definitions/trace';
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
  ...fileioCommands,
  ...editCommands,
  ...toolCommands,
  ...viewCommands,
  ...machineCommands,
  ...deviceCommands,
  ...panelCommands,
  ...layerCommands,
  ...previewCommands,
  ...arrangeCommands,
  ...traceCommands,
  ...codesCommands,
  ...cameraCommands,
  ...materialCommands,
  ...prefsCommands,
  ...helpCommands,
];

export const commands = new CommandRegistry(ALL_COMMANDS, () => useStore.getState());
