import { Maximize, Minus, Plus } from 'lucide-react';
import { useCommand } from '@/commands/useCommand';
import { t } from '@/i18n';
import { zoomPercent } from '@/lib/viewport';
import { useStore } from '@/store';

export function ZoomControls() {
  const zoomIn = useCommand('view.zoomIn');
  const zoomOut = useCommand('view.zoomOut');
  const fit = useCommand('view.fitBed');
  const pct = useStore((s) => zoomPercent(s.zoom));
  return (
    <div className="zoom-controls" role="group" aria-label={t('canvas.zoom')}>
      <button type="button" className="icon-btn" title={zoomOut.tooltip} aria-label={zoomOut.label} onClick={zoomOut.run}>
        <Minus size={15} aria-hidden="true" />
      </button>
      <span className="zoom-controls__value num" aria-live="polite">{pct} %</span>
      <button type="button" className="icon-btn" title={zoomIn.tooltip} aria-label={zoomIn.label} onClick={zoomIn.run}>
        <Plus size={15} aria-hidden="true" />
      </button>
      <button type="button" className="icon-btn" title={fit.tooltip} aria-label={fit.label} onClick={fit.run}>
        <Maximize size={15} aria-hidden="true" />
      </button>
    </div>
  );
}
