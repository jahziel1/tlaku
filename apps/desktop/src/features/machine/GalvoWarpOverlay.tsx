import { useEffect, useMemo, useState } from 'react';
import { createPortal } from 'react-dom';
import { t } from '@/i18n';
import { worldToScreen } from '@/lib/viewport';
import { selectActiveDevice, useStore } from '@/store';
import { warpActive, warpGrid, type Pt } from './galvoWarp';
import { useWarpUi } from './galvoWarpState';

/** Contenedor del lienzo del diseño (lo monta el área `canvas`). Se busca hasta que aparezca. */
function useCanvasArea(): HTMLElement | null {
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

/**
 * Capa sobre el lienzo con la rejilla deformada por el cilindro o el cono del perfil galvo (M46): muestra cómo se
 * moverían los puntos del diseño en el campo plano. Los tramos que pasan del horizonte del cilindro (no los ve el
 * espejo) van en rojo y el campo de la lente en línea punteada. Se monta con un portal para no tocar el código
 * del lienzo; no recibe eventos.
 */
export function GalvoWarpOverlay() {
  const area = useCanvasArea();
  const show = useWarpUi((s) => s.show);
  const device = useStore((s) => selectActiveDevice(s));
  const zoom = useStore((s) => s.zoom);
  const panX = useStore((s) => s.panX);
  const panY = useStore((s) => s.panY);
  const g = device.galvo;
  const on = show && device.kind === 'galvo' && warpActive(g);
  const bed = device.bed;
  const lines = useMemo(() => (on ? warpGrid(g, [bed[0], bed[1]]) : []), [on, g, bed]);
  if (!area || !on) return null;
  const view = { zoom, panX, panY };
  const path = (pts: Pt[]) => pts.map((p, i) => `${i === 0 ? 'M' : 'L'}${worldToScreen(view, p).map((n) => n.toFixed(1)).join(' ')}`).join('');
  const half = g.field_mm / 2;
  const [cx, cy] = [bed[0] / 2, bed[1] / 2];
  const field: Pt[] = [[cx - half, cy - half], [cx + half, cy - half], [cx + half, cy + half], [cx - half, cy + half], [cx - half, cy - half]];
  return createPortal(
    <svg className="galvo-warp" data-testid="galvo-warp" style={{ position: 'absolute', inset: 0, width: '100%', height: '100%', pointerEvents: 'none', zIndex: 3 }} role="img" aria-label={t('galvo.warp.aria')}>
      <path d={path(field)} fill="none" stroke="var(--border-strong)" strokeWidth={1} strokeDasharray="6 4" />
      {lines.map((l, i) => (
        <path key={i} d={path(l.points)} fill="none" stroke={l.visible ? 'var(--accent)' : 'var(--danger)'} strokeWidth={l.visible ? 1 : 1.5} strokeOpacity={l.visible ? 0.8 : 0.9} />
      ))}
    </svg>,
    area,
  );
}
