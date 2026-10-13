import { useEffect, useId } from 'react';
import { OptimizationDialog } from '@/features/preview/OptimizationDialog';
import { useStore } from '@/store';
import { AlarmActionDialog } from './AlarmActionDialog';
import { DeviceManager } from './DeviceManager';
import { MacroConfirmDialog } from './MacroButtons';
import { MachineSettingsDialog } from './MachineSettingsDialog';
import { StartJobDialog } from './StartJobDialog';

/**
 * Dibuja el diálogo abierto en el slice `machine` (inicio, alarma, dispositivos, optimización). Lo monta una sola vez el
 * AppShell (así el diálogo se ve desde cualquier pestaña); si por error hubiera dos, solo la
 * primera instancia montada dibuja, así nunca hay dos diálogos iguales.
 */
export function MachineDialogHost() {
  const id = useId();
  const dialog = useStore((s) => s.dialog);
  const isPrimary = useStore((s) => s.dialogHosts[0] === id);
  const register = useStore((s) => s.registerDialogHost);

  useEffect(() => register(id), [register, id]);

  if (!dialog || !isPrimary) return null;
  switch (dialog.kind) {
    case 'start':
      return <StartJobDialog />;
    case 'alarm':
      return <AlarmActionDialog action={dialog.action} />;
    case 'devices':
      return <DeviceManager />;
    case 'optimization':
      return <OptimizationDialog />;
    case 'machineSettings':
      return <MachineSettingsDialog />;
    case 'macro':
      return <MacroConfirmDialog index={dialog.index} />;
  }
}
