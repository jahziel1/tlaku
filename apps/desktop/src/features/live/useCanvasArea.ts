import { useEffect, useState } from 'react';

/** Contenedor del lienzo del diseño (lo monta el área `canvas`). Se busca hasta que aparezca. */
export function useCanvasArea(): HTMLElement | null {
  const [area, setArea] = useState<HTMLElement | null>(null);
  useEffect(() => {
    let tries = 0;
    let timer = 0;
    const look = () => {
      const el = document.querySelector<HTMLElement>('.canvas-area');
      if (el) setArea(el);
      else if (tries++ < 50) timer = window.setTimeout(look, 100);
    };
    look();
    return () => window.clearTimeout(timer);
  }, []);
  return area;
}
