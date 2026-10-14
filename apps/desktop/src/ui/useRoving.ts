import { useEffect, type RefObject } from 'react';

interface Options {
  /** `vertical` usa ↑/↓; `horizontal` usa ←/→. */
  orientation?: 'horizontal' | 'vertical';
  /** Qué elementos del contenedor forman parte de la barra. */
  selector?: string;
}

const ITEMS = 'button:not([disabled]), [role="button"]:not([disabled])';

/**
 * Teclado de una barra de herramientas (`role="toolbar"`, patrón WAI-ARIA): una sola parada de Tab
 * (el último elemento usado), flechas para recorrer, Inicio y Fin para los extremos. Actúa sobre el
 * DOM (`tabIndex`) para no tocar los botones, que pueden vivir en componentes ajenos.
 */
export function useRovingToolbar(ref: RefObject<HTMLElement | null>, { orientation = 'horizontal', selector = ITEMS }: Options = {}): void {
  // Sin lista de dependencias: tras cada render del contenedor se reaplica (botones que aparecen o se quitan).
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const items = () => [...el.querySelectorAll<HTMLElement>(selector)];
    const current = (list: HTMLElement[]) => list.find((i) => i.dataset.rovingStop === 'true') ?? list[0];
    const apply = (stop?: HTMLElement) => {
      const list = items();
      const keep = stop ?? current(list);
      for (const i of list) {
        i.tabIndex = i === keep ? 0 : -1;
        i.dataset.rovingStop = i === keep ? 'true' : 'false';
      }
    };
    apply();
    const onFocusIn = (e: FocusEvent) => {
      const target = (e.target as Element).closest<HTMLElement>(selector);
      if (target && el.contains(target)) apply(target);
    };
    const prev = orientation === 'vertical' ? 'ArrowUp' : 'ArrowLeft';
    const next = orientation === 'vertical' ? 'ArrowDown' : 'ArrowRight';
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
      if (e.key !== prev && e.key !== next && e.key !== 'Home' && e.key !== 'End') return;
      const list = items();
      const i = list.indexOf((e.target as Element).closest<HTMLElement>(selector) as HTMLElement);
      if (i < 0) return;
      e.preventDefault();
      const n = e.key === 'Home' ? 0 : e.key === 'End' ? list.length - 1 : (i + (e.key === next ? 1 : -1) + list.length) % list.length;
      list[n]?.focus();
    };
    el.addEventListener('focusin', onFocusIn);
    el.addEventListener('keydown', onKeyDown);
    return () => {
      el.removeEventListener('focusin', onFocusIn);
      el.removeEventListener('keydown', onKeyDown);
    };
  });
}
