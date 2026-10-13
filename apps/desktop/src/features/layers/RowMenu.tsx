import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';

export interface MenuItem {
  id: string;
  label: string;
  icon?: ReactNode;
  /** Si no se puede usar: el motivo, visible en el propio elemento. */
  disabledReason?: string;
  /** Texto secundario (atajo). */
  hint?: string;
  onSelect: () => void;
}

interface Props {
  items: MenuItem[];
  /** Posición del botón o del clic que lo abrió. */
  anchor: DOMRect;
  label: string;
  onClose: (restoreFocus: boolean) => void;
}

const WIDTH = 236;

/** Menú emergente accesible: flechas, Inicio/Fin, Enter y Esc; cierra al hacer clic fuera. */
export function RowMenu({ items, anchor, label, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: 0, top: 0 });

  // Se coloca bajo el ancla; si no cabe, encima o pegado al borde.
  useLayoutEffect(() => {
    const h = ref.current?.offsetHeight ?? 0;
    const left = Math.max(4, Math.min(anchor.right - WIDTH, window.innerWidth - WIDTH - 4));
    const below = anchor.bottom + 4;
    const top = below + h > window.innerHeight - 4 ? Math.max(4, anchor.top - h - 4) : below;
    setPos({ left, top });
  }, [anchor]);

  // `onClose` suele ser una función nueva en cada render: se guarda en una referencia para no reenfocar.
  const closeRef = useRef(onClose);
  useEffect(() => {
    closeRef.current = onClose;
  });
  useEffect(() => {
    ref.current?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
    const down = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) closeRef.current(false);
    };
    document.addEventListener('pointerdown', down, true);
    return () => document.removeEventListener('pointerdown', down, true);
  }, []);

  const onKeyDown = (e: KeyboardEvent) => {
    const els = [...(ref.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])];
    const i = els.indexOf(document.activeElement as HTMLElement);
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose(true);
    } else if (e.key === 'Tab') {
      onClose(false);
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      const n = e.key === 'Home' ? 0 : e.key === 'End' ? els.length - 1 : (i + (e.key === 'ArrowDown' ? 1 : -1) + els.length) % els.length;
      els[n]?.focus();
    }
  };

  return (
    <div ref={ref} className="lyr-menu" role="menu" aria-label={label} style={{ left: pos.left, top: pos.top, width: WIDTH }} onKeyDown={onKeyDown}>
      {items.map((it) => (
        <button
          key={it.id}
          type="button"
          role="menuitem"
          className="lyr-menu__item"
          aria-disabled={it.disabledReason ? true : undefined}
          onClick={() => {
            if (it.disabledReason) return;
            onClose(true);
            it.onSelect();
          }}
        >
          <span className="lyr-menu__icon" aria-hidden="true">{it.icon}</span>
          <span className="lyr-menu__text">
            {it.label}
            {it.disabledReason && <span className="lyr-menu__reason">{it.disabledReason}</span>}
          </span>
          {it.hint && !it.disabledReason && <span className="kbd">{it.hint}</span>}
        </button>
      ))}
    </div>
  );
}
