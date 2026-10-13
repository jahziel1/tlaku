import { Crosshair } from 'lucide-react';
import { t } from '@/i18n';
import { useStore } from '@/store';

/** Aviso fijo sobre el lienzo mientras está activo el modo "Posicionar el láser". */
export function PositionIndicator() {
  const on = useStore((s) => s.positioningLaser);
  if (!on) return null;
  return (
    <div className="canvas-mode" role="status">
      <Crosshair size={14} aria-hidden="true" />
      <span>{t('canvas.position.indicator')}</span>
    </div>
  );
}
