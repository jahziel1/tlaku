/** Etiqueta de la ventana aparte de la vista en vivo; debe coincidir con `live_window.rs::LABEL` y `capabilities/live.json`. */
export const LIVE_WINDOW_LABEL = 'live';

/**
 * ¿Esta ventana es la ventana aparte de la vista en vivo? En la app (Tauri) lo dice la etiqueta de la
 * ventana; en el navegador (modo simulado) la pestaña que abre `?liveWindow=1`.
 */
export function isLiveWindow(): boolean {
  if (typeof window === 'undefined') return false;
  try {
    const meta = (window as unknown as { __TAURI_INTERNALS__?: { metadata?: { currentWindow?: { label?: string } } } }).__TAURI_INTERNALS__?.metadata;
    if (meta?.currentWindow?.label === LIVE_WINDOW_LABEL) return true;
    return new URLSearchParams(window.location.search).has('liveWindow');
  } catch {
    return false;
  }
}
