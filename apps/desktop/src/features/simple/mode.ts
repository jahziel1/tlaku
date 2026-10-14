import { loadMaterials } from '@/features/materials/actions';
import { useStore, type AppState } from '@/store';
import type { UiMode } from '@/store/prefs';
import { stepName, useSimple } from './store';

/** ¿Está activo el modo sencillo? (para fuera de React) */
export const isSimpleMode = (): boolean => useStore.getState().prefs.uiMode === 'simple';

/** ¿Hay que preguntar "¿Es tu primera vez con un láser?"? Solo cuando nunca se eligió y ya se leyeron las preferencias del disco. */
export const needsFirstRun = (s: Pick<AppState, 'prefs' | 'prefsLoaded'>): boolean => s.prefs.uiMode === 'ask' && s.prefsLoaded;

/** Modo de la interfaz como estado de React. */
export function useUiMode(): UiMode {
  return useStore((s) => s.prefs.uiMode);
}

/**
 * Cambia de modo. El documento, las capas y el historial son los mismos en los dos: solo cambia lo
 * que se muestra. Al entrar al modo sencillo se deja la herramienta de seleccionar (para arrastrar el
 * diseño) y la vista previa como la necesita el paso en que se quedó.
 */
export function setUiMode(mode: 'simple' | 'full'): void {
  const s = useStore.getState();
  s.setPref('uiMode', mode);
  if (mode === 'simple') {
    // Los materiales (del usuario y de fábrica) se leen aunque nunca se haya abierto el panel de materiales.
    void loadMaterials();
    s.setTool('select');
    const name = stepName(useSimple.getState().step);
    s.setPreviewVisible(name === 'review' || name === 'start');
  }
}

export const toggleUiMode = (): void => setUiMode(isSimpleMode() ? 'full' : 'simple');
