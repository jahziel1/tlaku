import { useCallback, useEffect, useRef, type KeyboardEvent, type MouseEvent, type PointerEvent } from 'react';
import { useStore } from '@/store';
import { dirFromKey, HOLD_MS, modifiedStep, type JogDir } from './jogMath';

/**
 * Pulsación de un botón de jog: un toque corto mueve un paso; mantener pulsado más de `HOLD_MS`
 * empieza un jog continuo que se cancela (jog cancel) al soltar, al perder el foco la ventana,
 * al cancelarse el puntero o al desmontar. Un jog que arranca siempre tiene quien lo detenga.
 */
export function useHoldJog() {
  const active = useRef<{ timer: ReturnType<typeof setTimeout>; dir: JogDir; continuous: boolean } | null>(null);

  const finish = useCallback((cancelled: boolean) => {
    const a = active.current;
    if (!a) return;
    clearTimeout(a.timer);
    active.current = null;
    const s = useStore.getState();
    if (a.continuous) void s.jogStop();
    else if (!cancelled) void s.jogDir(a.dir);
  }, []);

  useEffect(() => {
    const onBlur = () => finish(true);
    window.addEventListener('blur', onBlur);
    return () => {
      window.removeEventListener('blur', onBlur);
      finish(true);
    };
  }, [finish]);

  /** Propiedades de eventos para el botón de una dirección. */
  return useCallback(
    (dir: JogDir) => ({
      onPointerDown: (e: PointerEvent<HTMLButtonElement>) => {
        if (e.button !== 0 || e.currentTarget.disabled) return;
        e.currentTarget.setPointerCapture(e.pointerId);
        finish(true);
        const timer = setTimeout(() => {
          const a = active.current;
          if (!a) return;
          a.continuous = true;
          void useStore.getState().jogContinuousStart(a.dir);
        }, HOLD_MS);
        active.current = { timer, dir, continuous: false };
      },
      onPointerUp: () => finish(false),
      onPointerCancel: () => finish(true),
      onLostPointerCapture: () => finish(true),
      // El clic con teclado (Intro o Espacio) llega con detail 0: un paso. El del ratón ya se atendió arriba.
      onClick: (e: MouseEvent<HTMLButtonElement>) => {
        if (e.detail === 0) void useStore.getState().jogDir(dir);
      },
    }),
    [finish],
  );
}

/** Flechas con el foco en el pad: un paso (Mayús x10, Alt /10). No llegan al editor. */
export function onPadKeyDown(e: KeyboardEvent<HTMLElement>) {
  const dir = dirFromKey(e.key);
  if (!dir || e.metaKey || e.ctrlKey) return;
  // Se consume siempre (también al repetirse la tecla) para que las flechas no muevan objetos por detrás.
  e.preventDefault();
  e.stopPropagation();
  if (e.repeat) return;
  const s = useStore.getState();
  void s.jogDir(dir, modifiedStep(s.jogStep, { shift: e.shiftKey, alt: e.altKey }));
}
