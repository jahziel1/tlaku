/**
 * "Actualizar al terminar un trabajo": cuando un trabajo termina se renueva el fondo de la cámara.
 * Con la cámara fija y el cuadro congelado se toma un cuadro nuevo; con la cámara en el cabezal se
 * repite el recorrido (el usuario lo autorizó al activar la opción, por eso no se vuelve a pedir
 * confirmación).
 */
import { t } from '@/i18n';
import { useStore } from '@/store';
import { updateHeadOverlay } from './headScan';
import { refreshSnapshot } from './session';

/** Espera tras terminar el trabajo: que la controladora regrese a reposo antes de mover el cabezal. */
export const AFTER_JOB_DELAY_MS = 1500;

/** Renueva el fondo ahora. Devuelve si hizo algo. */
export async function refreshOverlayNow(): Promise<boolean> {
  const s = useStore.getState();
  if (s.cameraSetup.head?.enabled) return updateHeadOverlay();
  if (s.cameraFrozen) return refreshSnapshot();
  return false;
}

let running = false;

/** Se suscribe al fin de trabajo; devuelve la función que cancela la suscripción. */
export function startAutoRefresh(delayMs = AFTER_JOB_DELAY_MS): () => void {
  return useStore.subscribe((s, prev) => {
    if (s.jobState !== 'finished' || prev.jobState === 'finished') return;
    const { prefs, head } = s.cameraSetup;
    if (!prefs.update_on_job_finish || !prefs.visible || running) return;
    if (!head?.enabled && !s.cameraFrozen) return;
    if (s.cameraStatus !== 'live' && !s.cameraFrozen) return;
    running = true;
    setTimeout(() => {
      void refreshOverlayNow()
        .then((did) => {
          if (did) useStore.getState().notify('info', t('camera.overlay.refreshed'));
        })
        .finally(() => {
          running = false;
        });
    }, delayMs);
  });
}
