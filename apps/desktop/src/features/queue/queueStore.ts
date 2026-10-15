import { create } from 'zustand';
import { EMPTY_QUEUE, type QueueState } from './queueModel';

/**
 * Estado de la cola guiada en la ventana principal. Solo guarda el modelo (`queueModel.ts`, puro) y dos
 * banderas de interfaz; no importa el store de la app para que `remote/snapshot.ts` y la vista en vivo
 * puedan leerlo sin ciclos. La orquestación (abrir documentos, pedir el inicio) está en `queue.ts`.
 */
interface QueueStore {
  q: QueueState;
  /** Diálogo de gestión de la cola abierto. */
  dialogOpen: boolean;
  /** Diálogo de confirmación del siguiente trabajo abierto (solo se muestra con la cola en `awaiting`). */
  stepOpen: boolean;
  /** Cambia el modelo con una función pura. */
  update: (fn: (q: QueueState) => QueueState) => void;
  setDialogOpen: (open: boolean) => void;
  setStepOpen: (open: boolean) => void;
}

export const useQueue = create<QueueStore>()((set) => ({
  q: EMPTY_QUEUE,
  dialogOpen: false,
  stepOpen: false,
  update: (fn) => set((s) => ({ q: fn(s.q) })),
  setDialogOpen: (dialogOpen) => set({ dialogOpen }),
  setStepOpen: (stepOpen) => set({ stepOpen }),
}));

/** Para las pruebas: vuelve a una cola vacía. */
export function resetQueue(): void {
  useQueue.setState({ q: EMPTY_QUEUE, dialogOpen: false, stepOpen: false });
}
