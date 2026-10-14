import { useEffect, useId } from 'react';
import { OptimizationDialog } from '@/features/preview/OptimizationDialog';
import { useStore, type AppState } from '@/store';
import { AlarmActionDialog } from './AlarmActionDialog';
import { DeviceManager } from './DeviceManager';
import { GalvoMoveDialog, GalvoStopModal } from './GalvoNotice';
import { GalvoSafetyDialog } from './GalvoSafetyDialog';
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

  if (!isPrimary) return null;
  // Galvo: el reconocimiento de seguridad de la conexión, mover el material entre pasos y el aviso de
  // cortar la energía salen sobre cualquier pestaña, estén o no abiertos otros diálogos.
  return (
    <>
      <GalvoSafetyDialog />
      <GalvoMoveDialog />
      <GalvoStopModal />
      {dialog && <MachineDialog dialog={dialog} />}
    </>
  );
}

function MachineDialog({ dialog }: { dialog: NonNullable<AppState['dialog']> }) {
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
