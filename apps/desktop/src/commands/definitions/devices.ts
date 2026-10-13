import { useDevicesWizard } from '@/features/devices/wizardStore';
import { isJobBusy } from '@/store';
import type { CommandDef } from '../types';

/**
 * Comandos del catálogo de máquinas. "Agregar máquina…" aparece solo, al final del menú Máquina
 * (categoría `machine`): abre el asistente de detección y catálogo.
 */
export const deviceCommands: CommandDef[] = [
  {
    id: 'machine.addDevice',
    label: 'catalog.cmd.add',
    category: 'machine',
    enabled: (s) => (isJobBusy(s) ? 'catalog.reason.jobRunning' : true),
    run: () => useDevicesWizard.getState().openWizard(),
  },
];
