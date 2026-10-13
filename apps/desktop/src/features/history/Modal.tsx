import { useEffect, useRef, type KeyboardEvent, type ReactNode } from 'react';
import './history.css';

const FOCUSABLE = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

interface Props {
  /** Id del elemento que titula el diálogo (`aria-labelledby`). */
  labelId: string;
  onClose: () => void;
  wide?: boolean;
  /** Selector del elemento que recibe el foco al abrir; por defecto, el primero enfocable. */
  initialFocus?: string;
  children: ReactNode;
}

/**
 * Diálogo modal accesible: atrapa el foco, Esc cierra, devuelve el foco a quien lo abrió y
 * mientras está abierto ningún atajo global actúa sobre el diseño de fondo.
 */
export function Modal({ labelId, onClose, wide, initialFocus, children }: Props) {
  const card = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;

  useEffect(() => {
    const el = card.current;
    if (!el) return;
    const previous = document.activeElement as HTMLElement | null;
    const target = (initialFocus && el.querySelector<HTMLElement>(initialFocus)) || el.querySelector<HTMLElement>(FOCUSABLE) || el;
    target.focus();
    // Si el foco se pierde fuera del diálogo (clic en el fondo), las teclas no llegan a los atajos globales.
    const guard = (e: globalThis.KeyboardEvent) => {
      if (el.contains(e.target as Node)) return;
      e.stopPropagation();
      e.preventDefault();
      if (e.key === 'Escape') closeRef.current();
      else el.focus();
    };
    window.addEventListener('keydown', guard, true);
    return () => {
      window.removeEventListener('keydown', guard, true);
      previous?.focus?.();
    };
  }, [initialFocus]);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      closeRef.current();
    } else if (e.key === 'Tab') {
      const items = [...e.currentTarget.querySelectorAll<HTMLElement>(FOCUSABLE)];
      const first = items[0];
      const last = items[items.length - 1];
      if (!first || !last) return e.preventDefault();
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
    // Lo que se teclea dentro del diálogo no debe disparar atajos de fondo (Supr, flechas...).
    e.stopPropagation();
  };

  return (
    <div className="modal" onMouseDown={(e) => e.target === e.currentTarget && card.current?.focus()}>
      <div
        ref={card}
        className={`modal__card${wide ? ' modal__card--wide' : ''}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby={labelId}
        tabIndex={-1}
        onKeyDown={onKeyDown}
      >
        {children}
      </div>
    </div>
  );
}
