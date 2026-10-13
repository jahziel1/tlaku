import type { ReactNode } from 'react';
import { Modal as BaseModal } from '@/ui/Modal';
import './history.css';

interface Props {
  /** Id del elemento que titula el diálogo (`aria-labelledby`). */
  labelId: string;
  onClose: () => void;
  wide?: boolean;
  /** Selector del elemento que recibe el foco al abrir; por defecto, el primero enfocable (aquí no se usa `data-autofocus`). */
  initialFocus?: string;
  children: ReactNode;
}

/** Diálogo de preguntas y atajos: el comportamiento accesible vive en `@/ui/Modal`. */
export function Modal({ labelId, onClose, wide, initialFocus, children }: Props) {
  return (
    <BaseModal
      labelId={labelId}
      onClose={onClose}
      backdropClassName="modal"
      cardClassName={`modal__card${wide ? ' modal__card--wide' : ''}`}
      initialFocus={initialFocus}
      guardOutside
    >
      {children}
    </BaseModal>
  );
}
