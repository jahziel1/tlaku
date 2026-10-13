import { create } from 'zustand';

export interface PromptChoice {
  id: string;
  label: string;
  tone?: 'accent' | 'danger' | 'neutral';
}

export interface PromptRequest {
  id: number;
  title: string;
  message: string;
  choices: PromptChoice[];
  /** Opción que se toma con Esc o al cerrar el diálogo. */
  cancelId: string;
}

interface PromptState {
  current: PromptRequest | null;
  queue: PromptRequest[];
  /** Resuelve la pregunta actual y pasa a la siguiente. */
  answer: (choiceId: string) => void;
}

const resolvers = new Map<number, (choiceId: string) => void>();
let nextId = 1;

/** Cola de preguntas modales de la app (se pintan en `PromptHost`). Funciona igual en Tauri y en el navegador. */
export const usePrompts = create<PromptState>()((set, get) => ({
  current: null,
  queue: [],
  answer: (choiceId) => {
    const { current, queue } = get();
    if (!current) return;
    resolvers.get(current.id)?.(choiceId);
    resolvers.delete(current.id);
    set({ current: queue[0] ?? null, queue: queue.slice(1) });
  },
}));

/** Muestra una pregunta y resuelve con el id de la opción elegida. */
export function askChoice(req: Omit<PromptRequest, 'id'>): Promise<string> {
  return new Promise((resolve) => {
    const full: PromptRequest = { ...req, id: nextId++ };
    resolvers.set(full.id, resolve);
    usePrompts.setState((s) => (s.current ? { queue: [...s.queue, full] } : { current: full }));
  });
}
