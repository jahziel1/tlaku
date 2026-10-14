import { useEffect } from 'react';

/**
 * Para paneles que no son modales (sin trampa de foco): al cerrarse, el foco vuelve al control que
 * los abrió, siempre que el foco no esté ya en otro lugar útil. No mueve el foco al abrir.
 */
export function useReturnFocus(): void {
  useEffect(() => {
    const opener = document.activeElement as HTMLElement | null;
    return () => {
      const now = document.activeElement;
      const lost = !now || now === document.body;
      if (lost && opener && opener !== document.body && opener.isConnected) opener.focus?.();
    };
  }, []);
}
