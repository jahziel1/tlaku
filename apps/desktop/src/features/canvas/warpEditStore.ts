import { create } from 'zustand';
import type { I18nKey } from '@/i18n';
import type { WarpMode, WarpSession } from './warpEdit';

/**
 * Estado del modo de edición de Deformar en el lienzo: la sesión (malla de control) y lo que la
 * barra de opciones necesita para pintarse. No es parte del documento: Aplicar escribe el resultado
 * UNA vez y Esc o cambiar de herramienta la descartan sin dejar rastro.
 */
export interface WarpEditState {
  /** Malla en edición; `null` si la selección no se puede deformar (ver `reason`). */
  session: WarpSession | null;
  /** Por qué no hay sesión, o `null`. */
  reason: I18nKey | null;
  /** Manija bajo el puntero o arrastrándose; -1 si ninguna. */
  hot: number;
  /** El motor está calculando el resultado de Aplicar. */
  busy: boolean;
}

const INITIAL: WarpEditState = { session: null, reason: null, hot: -1, busy: false };

export const useWarpEdit = create<WarpEditState>()(() => ({ ...INITIAL }));

export const patchWarpEdit = (patch: Partial<WarpEditState>): void => useWarpEdit.setState(patch);
export const resetWarpEdit = (): void => useWarpEdit.setState({ ...INITIAL });

// ---- acciones (las registra el hook del lienzo; las usan la barra y los comandos) ------------------------------

export interface WarpActions {
  apply: () => Promise<boolean>;
  reset: () => void;
  setMode: (mode: WarpMode) => void;
  canApply: () => boolean;
}

let actions: WarpActions | null = null;

export function registerWarpActions(a: WarpActions): () => void {
  actions = a;
  return () => {
    if (actions === a) actions = null;
  };
}

export const applyWarp = (): Promise<boolean> => actions?.apply() ?? Promise.resolve(false);
export const resetWarp = (): void => actions?.reset();
export const setWarpMode = (mode: WarpMode): void => actions?.setMode(mode);
export const canApplyWarp = (): boolean => actions?.canApply() ?? false;
