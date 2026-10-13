import { useEffect } from 'react';
import { useStore } from '@/store';
import { variableTexts } from './advance';
import { useVariablesUi } from './context';
import { startCsvSync } from './csvSync';
import { variablesOf } from './syntax';

/** Cada cuánto se revisa si el reloj del lienzo debe cambiar (fecha y hora de los textos). */
const CLOCK_CHECK_MS = 15_000;

/** ¿Algún texto muestra fecha u hora? Entonces el lienzo sigue al reloj. */
function usesClock(): boolean {
  return variableTexts(useStore.getState().doc).some((o) => o.kind.type === 'text' && variablesOf(o.kind.text).some((v) => v.name === 'date' || v.name === 'time'));
}

/**
 * Mantiene al día lo que el lienzo necesita para ver los textos variables: el CSV del documento
 * (se vuelve a leer al cambiar de archivo) y el reloj (por minuto, solo si hay fecha u hora).
 */
export function useVariablesSync(): void {
  useEffect(() => {
    const stopCsv = startCsvSync();
    const timer = setInterval(() => {
      const { clock, setClock } = useVariablesUi.getState();
      const now = Date.now();
      if (Math.floor(now / 60_000) !== Math.floor(clock / 60_000) && usesClock()) setClock(now);
    }, CLOCK_CHECK_MS);
    return () => {
      stopCsv();
      clearInterval(timer);
    };
  }, []);
}
