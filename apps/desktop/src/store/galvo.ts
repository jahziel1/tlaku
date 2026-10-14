import { getApi } from '@/api';
import type { ConnectionEvent, GalvoEvent, GalvoSafetyAck, GalvoStepInfo, JobEvent } from '@/api/types';
import { isUnconfirmedStop } from '@/features/machine/galvoPorts';
import { resolveJobArgs } from '@/features/variables/jobResolve';
import { errorMessage } from '@/lib/errors';
import { buildJobArgs } from './selectors';
import type { SliceCreator } from './types';

/**
 * Estado propio del galvo (EXPERIMENTAL): el paso del marcado dividido o repetido, la espera a que
 * muevas el material, las vueltas del encuadre, el reconocimiento de seguridad de la conexión y el
 * aviso de «la fuente puede seguir encendida», que NO se va solo: queda hasta que el usuario confirme
 * que cortó la energía.
 */
export interface GalvoSlice {
  /** Paso que se está marcando. */
  galvoStep: GalvoStepInfo | null;
  /** El trabajo espera que muevas el material (o gires el rotativo) y confirmes. */
  galvoWait: GalvoStepInfo | null;
  /** Vueltas del encuadre con el puntero rojo. */
  galvoLoops: number;
  /** El puntero rojo está encendido en su sitio (lo que pidió la interfaz). */
  galvoRedOn: boolean;
  /** Texto del aviso de cortar la energía; `null` = no hay aviso. Solo lo borra `dismissGalvoLaserWarning`. */
  galvoLaserWarning: string | null;
  /** Hay un diálogo de seguridad esperando respuesta (la conexión USB no sigue hasta que se conteste). */
  galvoAckPending: boolean;
  /** Cuenta las peticiones: cada diálogo nuevo parte con las casillas desmarcadas. */
  galvoAckId: number;

  /** Abre el diálogo de seguridad; devuelve el reconocimiento, o `null` si se cancela. */
  requestGalvoAck: () => Promise<GalvoSafetyAck | null>;
  /** Respuesta del diálogo: solo un reconocimiento con las tres confirmaciones se entrega. */
  answerGalvoAck: (ack: GalvoSafetyAck | null) => void;
  confirmGalvoMove: () => Promise<void>;
  setGalvoRedLight: (on: boolean) => Promise<void>;
  /** Recorre el trabajo con el láser deshabilitado y el puntero rojo. */
  galvoRedRun: () => Promise<void>;
  dismissGalvoLaserWarning: () => void;

  // Entrada de eventos (los conectan `bindMachineEvents` y la rama de eventos comunes de `machine`).
  onGalvo: (ev: GalvoEvent) => void;
  onGalvoJob: (job: JobEvent) => void;
  onGalvoConnection: (c: ConnectionEvent) => void;
}

/** El único resolutor pendiente del diálogo de seguridad (no se guarda en el estado: no es serializable). */
let pendingAck: ((ack: GalvoSafetyAck | null) => void) | null = null;

const complete = (a: GalvoSafetyAck | null): GalvoSafetyAck | null => (a && a.eyewear && a.enclosure && a.emergency_stop ? { ...a } : null);

export const createGalvoSlice: SliceCreator<GalvoSlice> = (set, get) => {
  const warn = (message: string | null | undefined) => {
    if (message && isUnconfirmedStop(message)) set({ galvoLaserWarning: message });
  };
  return {
    galvoStep: null,
    galvoWait: null,
    galvoLoops: 0,
    galvoRedOn: false,
    galvoLaserWarning: null,
    galvoAckPending: false,
    galvoAckId: 0,

    requestGalvoAck: () => {
      // Una petición anterior sin contestar se cancela: nunca hay dos diálogos.
      pendingAck?.(null);
      return new Promise<GalvoSafetyAck | null>((resolve) => {
        pendingAck = resolve;
        set((st) => ({ galvoAckPending: true, galvoAckId: st.galvoAckId + 1 }));
      });
    },
    answerGalvoAck: (ack) => {
      const resolve = pendingAck;
      pendingAck = null;
      set({ galvoAckPending: false });
      resolve?.(complete(ack));
    },
    confirmGalvoMove: async () => {
      try {
        await getApi().galvoConfirmMove();
        set({ galvoWait: null });
      } catch (e) {
        get().notify('error', errorMessage(e));
      }
    },
    setGalvoRedLight: async (on) => {
      try {
        await getApi().galvoRedLight({ on });
        set({ galvoRedOn: on });
      } catch (e) {
        const message = errorMessage(e);
        warn(message);
        get().notify('error', message);
      }
    },
    galvoRedRun: async () => {
      const s = get();
      set({ jobError: null, positioningLaser: false });
      try {
        await getApi().galvoRedRun(await resolveJobArgs(buildJobArgs(s)));
      } catch (e) {
        const message = errorMessage(e);
        set({ jobError: message });
        get().notify('error', message);
      }
    },
    dismissGalvoLaserWarning: () => set({ galvoLaserWarning: null }),

    onGalvo: (ev) => {
      switch (ev.kind) {
        case 'step':
          set({ galvoStep: ev.info, galvoWait: null });
          return;
        case 'wait_move':
          set({ galvoWait: ev.info });
          return;
        case 'light':
          set({ galvoLoops: ev.loops });
          return;
        case 'laser_may_be_on':
          set({ galvoLaserWarning: ev.message });
          return;
      }
    },
    onGalvoJob: (job) => {
      warn(job.message);
      if (job.state !== 'started') set({ galvoStep: null, galvoWait: null, galvoRedOn: false });
    },
    onGalvoConnection: (c) => {
      warn(c.error);
      if (!c.connected) set({ galvoStep: null, galvoWait: null, galvoLoops: 0, galvoRedOn: false });
    },
  };
};
