import { useEffect, useState } from 'react';
import { loadThumb } from './useVersions';

interface Props {
  path: string;
  id: string;
  /** La versión dice que tiene miniatura; si no, ni se pide. */
  has: boolean;
  large?: boolean;
  /** Texto alternativo (la vista previa grande lo lleva; la miniatura de la lista es decorativa). */
  alt?: string;
}

/** Miniatura SVG de una versión. Se muestra como imagen (`<img>`): un SVG así nunca ejecuta nada. */
export function Thumb({ path, id, has, large, alt = '' }: Props) {
  const [svg, setSvg] = useState<string | null | undefined>(has ? undefined : null);
  useEffect(() => {
    let alive = true;
    if (!has) {
      setSvg(null);
      return;
    }
    setSvg(undefined);
    loadThumb(path, id)
      .then((s) => alive && setSvg(s))
      .catch(() => alive && setSvg(null));
    return () => {
      alive = false;
    };
  }, [path, id, has]);

  const cls = `ver-thumb${large ? ' ver-thumb--large' : ''}`;
  if (!svg) return <div className={`${cls} ver-thumb--empty`} data-state={svg === undefined ? 'loading' : 'none'} aria-hidden="true" />;
  return <img className={cls} alt={alt} src={`data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`} />;
}
