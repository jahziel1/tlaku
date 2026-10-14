/**
 * «Continuar un trabajo interrumpido» en el store: qué punto de control hay y cuándo avisar.
 *
 * El motor guarda el punto de control mientras envía y lo borra al terminar bien o cancelar a propósito;
 * aquí solo se pregunta (`interruptedJob`) al abrir la app, al perderse la conexión a mitad de un trabajo
 * y al terminar un trabajo con error. Con un trabajo en marcha en esta ventana no se pregunta: su punto de
 * control es el suyo, no uno interrumpido.
 */
import { getApi } from '@/api';
import type { InterruptedJob } from '@/api/types';
import { askChoice } from '@/features/history/prompt';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { isJobBusy } from '../selectors';
import type { AppState } from '../types';

type Set = (partial: Partial<AppState> | ((s: AppState) => Partial<AppState>)) => void;
type Get = () => AppState;

export interface ResumeSlice {
  /** El trabajo interrumpido del que queda punto de control (null = ninguno o aún no se ha preguntado). */
  interrupted: InterruptedJob | null;
  /**
   * Pregunta al motor. Con `open`, muestra el aviso si hay algo que ofrecer. Devuelve lo que haya.
   * No hace nada (y devuelve null) mientras un trabajo esté en marcha.
   */
  checkInterrupted: (opts?: { open?: boolean }) => Promise<InterruptedJob | null>;
  /** Pide confirmación y borra el punto de control. Devuelve si se descartó. */
  discardInterrupted: () => Promise<boolean>;
  /** Abre el aviso «se interrumpió al N %». */
  openResumeNotice: () => void;
  /** Abre el diálogo de reanudación; `choose` pone el control del punto de reinicio a la vista. */
  openResume: (choose: boolean) => void;
}

export function createResumeActions(set: Set, get: Get): ResumeSlice {
  return {
    interrupted: null,
    checkInterrupted: async (opts) => {
      if (isJobBusy(get())) return null;
      // Sin punto de control legible no hay nada que ofrecer; no se molesta al usuario.
      const job: InterruptedJob | null = await getApi()
        .interruptedJob()
        .catch(() => null);
      // Mientras se preguntaba pudo empezar un trabajo.
      if (isJobBusy(get())) return null;
      set({ interrupted: job });
      if (job && opts?.open) get().openResumeNotice();
      return job;
    },
    discardInterrupted: async () => {
      const job = get().interrupted;
      if (!job) return false;
      const choice = await askChoice({
        title: t('machine.resume.discard.title'),
        message: t('machine.resume.discard.message', { percent: Math.round(job.percent) }),
        choices: [
          { id: 'ok', label: t('machine.resume.discard.confirm'), tone: 'danger' },
          { id: 'cancel', label: t('machine.resume.discard.cancel') },
        ],
        cancelId: 'cancel',
      });
      if (choice !== 'ok') return false;
      try {
        await getApi().discardInterruptedJob();
      } catch (e) {
        get().notify('error', errorMessage(e));
        return false;
      }
      set({ interrupted: null });
      if (get().dialog?.kind === 'resumeNotice' || get().dialog?.kind === 'resume') get().closeDialog();
      return true;
    },
    openResumeNotice: () => get().openDialog({ kind: 'resumeNotice' }),
    openResume: (choose) => get().openDialog({ kind: 'resume', choose }),
  };
}
