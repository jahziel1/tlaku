import { AddMachineWizard } from './AddMachineWizard';
import { useDevicesWizard } from './wizardStore';

export { useDevicesWizard } from './wizardStore';
export { CATALOG, findEntry, type CatalogEntry } from './catalog';

/** Punto de montaje del asistente "Agregar máquina" (lo monta el shell una sola vez). */
export function DevicesDialogHost() {
  const open = useDevicesWizard((s) => s.open);
  return open ? <AddMachineWizard /> : null;
}
