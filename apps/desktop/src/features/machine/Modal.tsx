import { X } from 'lucide-react';
import { useEffect, useId, useRef, type KeyboardEvent, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { t } from '@/i18n';

interface Props {
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  /** `lg` para el gestor de dispositivos. */
  size?: 'sm' | 'md' | 'lg';
  /** Color de la franja superior: peligro, aviso o neutro. */
  tone?: 'danger' | 'warn' | 'accent';
  /** Subtítulo corto bajo el título. */
  subtitle?: string;
}

const FOCUSABLE = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * Diálogo modal accesible: foco atrapado, Esc cierra y los atajos de la aplicación no llegan a la
 * ventana mientras está abierto (así una `F` o `Mod+Enter` no mueve la máquina por detrás).
 * El foco inicial va al elemento con `data-autofocus` (en los diálogos de riesgo, "Cancelar").
 */
export function Modal({ title, subtitle, onClose, children, footer, size = 'md', tone }: Props) {
  const titleId = useId();
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const el = box.current;
    const first = el?.querySelector<HTMLElement>('[data-autofocus]') ?? el?.querySelector<HTMLElement>(FOCUSABLE);
    (first ?? el)?.focus();
    return () => previous?.focus?.();
  }, []);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    // Ningún atajo de la ventana debe ver estas teclas, salvo Detener (Mod+.): un paro no se bloquea.
    if (!((e.metaKey || e.ctrlKey) && e.key === '.')) e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
      return;
    }
    if (e.key !== 'Tab' || !box.current) return;
    const items = [...box.current.querySelectorAll<HTMLElement>(FOCUSABLE)];
    if (items.length === 0) return;
    const first = items[0]!;
    const last = items[items.length - 1]!;
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  };

  return createPortal(
    <div className="mc-backdrop" onKeyDown={onKeyDown} onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div ref={box} className="mc-modal" data-size={size} data-tone={tone} role="dialog" aria-modal="true" aria-labelledby={titleId} tabIndex={-1}>
        <header className="mc-modal__head">
          <div>
            <h2 id={titleId} className="mc-modal__title">{title}</h2>
            {subtitle && <p className="hint">{subtitle}</p>}
          </div>
          <button type="button" className="icon-btn" title={t('machine.dlg.close')} aria-label={t('machine.dlg.close')} onClick={onClose}>
            <X size={16} aria-hidden="true" />
          </button>
        </header>
        <div className="mc-modal__body">{children}</div>
        {footer && <footer className="mc-modal__foot">{footer}</footer>}
      </div>
    </div>,
    document.body,
  );
}
