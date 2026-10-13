import { useEffect } from 'react';
import { useStore } from '@/store';
import { loadCameraSetup } from './actions';
import { CalibrationDialog } from './CalibrationDialog';
import { MarksDialog } from './MarksDialog';
import { useMarks } from './marksState';
import { startAutoRefresh } from './refresh';
import { startCamera } from './session';

export { CameraPanel } from './CameraPanel';

/**
 * Punto de montaje del área `camera` (lo monta el shell una sola vez): carga la configuración
 * guardada, reabre la cámara si el fondo estaba visible al cerrar y aloja el asistente de lente.
 */
export function CameraDialogHost() {
  const open = useStore((s) => s.cameraCalibrationOpen);
  const close = useStore((s) => s.setCameraCalibrationOpen);
  const marksOpen = useMarks((s) => s.open);
  const closeMarks = useMarks((s) => s.closeDialog);

  // "Actualizar al terminar un trabajo": mientras la app está abierta.
  useEffect(() => startAutoRefresh(), []);

  useEffect(() => {
    void loadCameraSetup().then(() => {
      const { cameraSetup, cameraStatus } = useStore.getState();
      // Se recupera el fondo tras reiniciar sin que el usuario repita nada.
      if (cameraSetup.prefs.visible && (cameraSetup.alignment || cameraSetup.head?.calibration) && cameraStatus === 'off') void startCamera();
    });
  }, []);

  return (
    <>
      {open && <CalibrationDialog onClose={() => close(false)} />}
      {marksOpen && <MarksDialog onClose={closeMarks} />}
    </>
  );
}
