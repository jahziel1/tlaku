import { Grid3x3, Magnet, SquareDashed } from 'lucide-react';
import { useCommand } from '@/commands/useCommand';
import { t } from '@/i18n';

/** Interruptores visibles del ajuste magnético: general, a cuadrícula y a objetos/cama. */
export function SnapBar() {
  const snap = useCommand('view.snap');
  const grid = useCommand('view.snapGrid');
  const objects = useCommand('view.snapObjects');
  return (
    <div className="snap-bar" role="group" aria-label={t('canvas.snap.group')}>
      <button type="button" className="icon-btn" aria-pressed={snap.checked} data-checked={snap.checked} title={snap.tooltip} aria-label={snap.label} onClick={snap.run}>
        <Magnet size={15} aria-hidden="true" />
      </button>
      <button type="button" className="icon-btn" aria-pressed={grid.checked} data-checked={grid.checked} title={grid.tooltip} aria-label={grid.label} onClick={grid.run}>
        <Grid3x3 size={15} aria-hidden="true" />
      </button>
      <button type="button" className="icon-btn" aria-pressed={objects.checked} data-checked={objects.checked} title={objects.tooltip} aria-label={objects.label} onClick={objects.run}>
        <SquareDashed size={15} aria-hidden="true" />
      </button>
    </div>
  );
}
