import { useStore } from '@/store';
import { arrangeCommands } from './definitions/arrange';
import { designAdvancedCommands } from './definitions/designAdvanced';
import { bundleCommands } from './definitions/bundle';
import { cameraCommands } from './definitions/camera';
import { canvasToolCommands } from './definitions/canvasTools';
import { codesCommands } from './definitions/codes';
import { costsCommands } from './definitions/costs';
import { designCommands } from './definitions/design';
import { deviceCommands } from './definitions/devices';
import { editCommands } from './definitions/edit';
import { fileCommands } from './definitions/file';
import { fileioCommands } from './definitions/fileio';
import { helpCommands } from './definitions/help';
import { layerCommands } from './definitions/layers';
import { machineCommands } from './definitions/machine';
import { machineExtraCommands } from './definitions/machineExtras';
import { materialCommands } from './definitions/materials';
import { nestCommands } from './definitions/nest';
import { panelCommands } from './definitions/panels';
import { prefsCommands } from './definitions/prefs';
import { printcutCommands } from './definitions/printcut';
import { reviewCommands } from './definitions/review';
import { shapeCommands } from './definitions/shapes';
import { simpleCommands } from './definitions/simple';
import { previewCommands } from './definitions/preview';
import { tilingCommands } from './definitions/tiling';
import { toolCommands } from './definitions/tools';
import { traceCommands } from './definitions/trace';
import { variablesCommands } from './definitions/variables';
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
  ...canvasToolCommands,
  ...viewCommands,
  ...designCommands,
  ...machineCommands,
  ...machineExtraCommands,
  ...tilingCommands,
  ...reviewCommands,
  ...deviceCommands,
  ...panelCommands,
  ...layerCommands,
  ...previewCommands,
  ...costsCommands,
  ...arrangeCommands,
  ...designAdvancedCommands,
  ...shapeCommands,
  ...nestCommands,
  ...printcutCommands,
  ...traceCommands,
  ...codesCommands,
  ...variablesCommands,
  ...cameraCommands,
  ...materialCommands,
  ...prefsCommands,
  ...bundleCommands,
  ...helpCommands,
  ...simpleCommands,
];

export const commands = new CommandRegistry(ALL_COMMANDS, () => useStore.getState());
