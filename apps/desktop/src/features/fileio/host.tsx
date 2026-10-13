import type { ReactNode } from 'react';
import { createRoot } from 'react-dom/client';

/**
 * Abre un diálogo propio de esta área sin pasar por el árbol del shell: crea su raíz de React en
 * `document.body`, la quita al cerrarse y resuelve con lo que el diálogo entregue a `done`
 * (`null` si se cancela). Los estilos y temas son globales, así que se ve igual que el resto.
 */
export function openDialog<T>(render: (done: (value: T) => void) => ReactNode): Promise<T> {
  return new Promise<T>((resolve) => {
    const host = document.createElement('div');
    document.body.appendChild(host);
    const root = createRoot(host);
    let closed = false;
    const done = (value: T) => {
      if (closed) return;
      closed = true;
      // Se desmonta fuera del manejador del evento que cerró el diálogo.
      queueMicrotask(() => {
        root.unmount();
        host.remove();
      });
      resolve(value);
    };
    root.render(render(done));
  });
}
