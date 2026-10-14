import { useRef, type ReactNode } from 'react';
import { useRovingToolbar } from './useRoving';

interface Props {
  label: string;
  className?: string;
  orientation?: 'horizontal' | 'vertical';
  children: ReactNode;
}

/** Barra de herramientas con teclado de una sola parada de Tab y flechas (ver `useRovingToolbar`). */
export function Toolbar({ label, className, orientation = 'horizontal', children }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  useRovingToolbar(ref, { orientation });
  return (
    <div ref={ref} className={className} role="toolbar" aria-orientation={orientation} aria-label={label}>
      {children}
    </div>
  );
}
