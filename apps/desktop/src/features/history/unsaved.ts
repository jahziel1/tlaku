/**
 * Flujo de "cambios sin guardar": antes de nuevo, abrir o cerrar la ventana con cambios se pregunta
 * guardar, descartar o cancelar. La lógica es una máquina de estados pura (`guardReducer`) y
 * `guardUnsaved` la maneja con las operaciones reales inyectadas, así se prueba sin interfaz.
 */

export type GuardIntent = 'new' | 'open' | 'close' | 'tab';
export type GuardChoice = 'save' | 'discard' | 'cancel';

export type GuardState =
  | { phase: 'idle' }
  | { phase: 'asking'; intent: GuardIntent }
  | { phase: 'saving'; intent: GuardIntent }
  /** Terminal: se puede seguir con la acción. */
  | { phase: 'proceed'; intent: GuardIntent }
  /** Terminal: el usuario canceló o el guardado no se completó. */
  | { phase: 'cancelled'; intent: GuardIntent };

export type GuardEvent =
  | { type: 'request'; intent: GuardIntent; dirty: boolean }
  | { type: 'choose'; choice: GuardChoice }
  | { type: 'saved' }
  | { type: 'saveAborted' };

export function guardReducer(state: GuardState, ev: GuardEvent): GuardState {
  switch (ev.type) {
    case 'request':
      // Una petición nueva solo arranca desde un estado de reposo o terminal.
      if (state.phase === 'asking' || state.phase === 'saving') return state;
      return ev.dirty ? { phase: 'asking', intent: ev.intent } : { phase: 'proceed', intent: ev.intent };
    case 'choose':
      if (state.phase !== 'asking') return state;
      if (ev.choice === 'save') return { phase: 'saving', intent: state.intent };
      if (ev.choice === 'discard') return { phase: 'proceed', intent: state.intent };
      return { phase: 'cancelled', intent: state.intent };
    case 'saved':
      return state.phase === 'saving' ? { phase: 'proceed', intent: state.intent } : state;
    case 'saveAborted':
      return state.phase === 'saving' ? { phase: 'cancelled', intent: state.intent } : state;
  }
}

export interface GuardIo {
  isDirty: () => boolean;
  /** Pregunta al usuario; debe resolver siempre (cerrar el diálogo = cancelar). */
  ask: (intent: GuardIntent) => Promise<GuardChoice>;
  /** Guarda el proyecto; `false` si no se guardó (diálogo cancelado o error). */
  save: () => Promise<boolean>;
}

let busy = false;

/** `true` si la acción protegida puede continuar. Mientras haya una pregunta abierta, otras se rechazan. */
export async function guardUnsaved(intent: GuardIntent, io: GuardIo): Promise<boolean> {
  if (busy) return false;
  busy = true;
  try {
    let state = guardReducer({ phase: 'idle' }, { type: 'request', intent, dirty: io.isDirty() });
    if (state.phase === 'asking') {
      state = guardReducer(state, { type: 'choose', choice: await io.ask(intent) });
      if (state.phase === 'saving') {
        state = guardReducer(state, (await io.save()) ? { type: 'saved' } : { type: 'saveAborted' });
      }
    }
    return state.phase === 'proceed';
  } finally {
    busy = false;
  }
}
