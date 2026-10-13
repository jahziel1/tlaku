import type { LensCalibration } from '@/api/types';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { useStore } from '@/store';
import { emptyCameraSetup, type CameraAlignment, type CameraPrefs } from '@/store/camera';
import { getSetupBackend } from './setup';

/** Carga la configuración guardada (una sola vez). Si falla, sigue con una vacía y avisa. */
export async function loadCameraSetup(): Promise<void> {
  const s = useStore.getState();
  if (s.cameraSetupLoaded) return;
  try {
    s.setCameraSetup(await getSetupBackend().load());
  } catch (e) {
    s.setCameraSetup(emptyCameraSetup());
    s.notify('error', t('camera.err.load', { error: errorMessage(e) }));
  }
}

let saveQueue: Promise<void> = Promise.resolve();

/** Guarda la configuración actual. Las escrituras se encadenan para que la última gane. */
export function persistCameraSetup(): Promise<void> {
  saveQueue = saveQueue.then(async () => {
    const s = useStore.getState();
    try {
      const saved = await getSetupBackend().save(s.cameraSetup);
      // El motor recalcula la homografía; se adopta lo guardado sin pisar cambios posteriores.
      if (JSON.stringify(saved.alignment) !== JSON.stringify(s.cameraSetup.alignment)) {
        useStore.getState().setCameraSetup({ ...useStore.getState().cameraSetup, alignment: saved.alignment });
      }
    } catch (e) {
      s.notify('error', t('camera.err.save', { error: errorMessage(e) }));
    }
  });
  return saveQueue;
}

let prefsTimer: ReturnType<typeof setTimeout> | undefined;

/** Cambia preferencias; con `debounce` espera a que el usuario suelte el control (opacidad). */
export function updateCameraPrefs(prefs: Partial<CameraPrefs>, debounce = 0): void {
  useStore.getState().setCameraPrefs(prefs);
  clearTimeout(prefsTimer);
  if (debounce > 0) prefsTimer = setTimeout(() => void persistCameraSetup(), debounce);
  else void persistCameraSetup();
}

/**
 * Guarda una calibración nueva. La alineación depende de la imagen corregida, así que una
 * calibración distinta la invalida: se borra y se avisa.
 */
export function applyLens(calibration: LensCalibration): void {
  const s = useStore.getState();
  const hadAlignment = s.cameraSetup.alignment !== null;
  s.setCameraSetup({ ...s.cameraSetup, calibration, alignment: null });
  if (hadAlignment) s.notify('warning', t('camera.lens.alignmentReset'));
  void persistCameraSetup();
}

export function clearLens(): void {
  const s = useStore.getState();
  s.setCameraSetup({ ...s.cameraSetup, calibration: null, alignment: null });
  void persistCameraSetup();
}

export function applyAlignment(alignment: CameraAlignment | null): void {
  const s = useStore.getState();
  s.setCameraSetup({ ...s.cameraSetup, alignment });
  void persistCameraSetup();
}
