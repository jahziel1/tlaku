import { useEffect, useRef, type KeyboardEvent, type ReactNode } from 'react';
import { createPortal } from 'react-dom';

const FOCUSABLE = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export interface ModalProps {
  /** Id del elemento que titula el diálogo (`aria-labelledby`). */
  labelId: string;
  onClose: () => void;
  /** Clases del fondo y de la tarjeta: cada área aporta su estilo, el comportamiento es común. */
  backdropClassName: string;
  cardClassName: string;
  /** Atributos `data-*` de la tarjeta (tamaño, tono). */
  cardData?: Record<`data-${string}`, string | undefined>;
  /**
   * Selector del elemento que recibe el foco al abrir. Por defecto el primero con `data-autofocus`
   * (en los diálogos de riesgo, "Cancelar").
   */
  initialFocus?: string;
  /** Si no hay foco inicial: `first` = el primer elemento enfocable, `card` = la tarjeta. */
  fallbackFocus?: 'first' | 'card';
  /** Se monta en `document.body` (los diálogos que se abren desde paneles con `overflow`). */
  portal?: boolean;
  /** Un clic en el fondo cierra; si no, solo devuelve el foco a la tarjeta. */
  closeOnBackdrop?: boolean;
  /** Si el foco se pierde fuera del diálogo, sus teclas tampoco llegan a los atajos globales. */
  guardOutside?: boolean;
  /** Teclas que siguen llegando a la ventana aunque el diálogo esté abierto (ej. Detener, `Mod+.`). */
  passKey?: (e: { key: string; metaKey: boolean; ctrlKey: boolean }) => boolean;
  children: ReactNode;
}

/**
 * Diálogo modal accesible y común a todas las áreas: atrapa el foco con Tab, Esc cierra, el foco
 * inicial es configurable, al cerrar el foco vuelve a quien lo abrió y mientras está abierto ningún
 * atajo global actúa sobre el diseño de fondo.
 */
export function Modal({
  labelId, onClose, backdropClassName, cardClassName, cardData, initialFocus = '[data-autofocus]',
  fallbackFocus = 'first', portal = false, closeOnBackdrop = false, guardOutside = false, passKey, children,
}: ModalProps) {
  const card = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  const passRef = useRef(passKey);
  passRef.current = passKey;

  useEffect(() => {
    const el = card.current;
    if (!el) return;
    const previous = document.activeElement as HTMLElement | null;
    const first = fallbackFocus === 'first' ? el.querySelector<HTMLElement>(FOCUSABLE) : null;
    (el.querySelector<HTMLElement>(initialFocus) ?? first ?? el).focus();
    // Si el foco se pierde fuera del diálogo (clic en el fondo), las teclas no llegan a los atajos globales.
    const guard = (e: globalThis.KeyboardEvent) => {
      if (el.contains(e.target as Node) || passRef.current?.(e)) return;
      e.stopPropagation();
      e.preventDefault();
      if (e.key === 'Escape') closeRef.current();
      else el.focus();
    };
    if (guardOutside) window.addEventListener('keydown', guard, true);
    return () => {
      if (guardOutside) window.removeEventListener('keydown', guard, true);
      previous?.focus?.();
    };
    // Solo al montar: el foco inicial no se reasigna mientras el diálogo está abierto.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    // Lo que se teclea dentro del diálogo no debe disparar atajos de fondo (Supr, flechas...).
    if (!passRef.current?.(e)) e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      closeRef.current();
      return;
    }
    if (e.key !== 'Tab' || !card.current) return;
    const items = [...card.current.querySelectorAll<HTMLElement>(FOCUSABLE)];
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
  };

  const tree = (
    <div
      className={backdropClassName}
      onKeyDown={onKeyDown}
      onMouseDown={(e) => {
        if (e.target !== e.currentTarget) return;
        if (closeOnBackdrop) closeRef.current();
        else card.current?.focus();
      }}
    >
      <div ref={card} className={cardClassName} {...cardData} role="dialog" aria-modal="true" aria-labelledby={labelId} tabIndex={-1}>
        {children}
      </div>
    </div>
  );
  return portal ? createPortal(tree, document.body) : tree;
}
