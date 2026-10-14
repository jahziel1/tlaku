import type { FilletKind } from '@/api/types';
import { useShapeOps } from '@/features/arrange/shapeOpsUi';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { LengthField } from '@/ui/LengthField';
import { setFilletRevert, useFilletTool } from './filletTool';
import './tool-bars.css';

const STYLES: readonly FilletKind[] = ['round', 'chamfer', 'bite'];

/**
 * Opciones de la herramienta Redondear esquinas con clic: tipo de esquina, radio (el mismo del
 * diálogo «Redondear esquinas») y el modo, redondear o revertir.
 */
export function FilletBar() {
  const active = useStore((s) => s.activeTool === 'fillet');
  const { radius, style } = useShapeOps((s) => s.fillet);
  const patch = useShapeOps((s) => s.patchFillet);
  const revert = useFilletTool((s) => s.revert);
  if (!active) return null;
  return (
    <div className="tool-bar" role="group" aria-label={t('canvas.fillet.bar')} data-testid="fillet-bar">
      <div className="tool-bar__chips" role="group" aria-label={t('canvas.fillet.mode')}>
        <button type="button" className="chip" aria-pressed={!revert} onClick={() => setFilletRevert(false)}>{t('canvas.fillet.modeRound')}</button>
        <button type="button" className="chip" aria-pressed={revert} onClick={() => setFilletRevert(true)}>{t('canvas.fillet.modeRevert')}</button>
      </div>
      {!revert && (
        <>
          <div className="tool-bar__chips" role="group" aria-label={t('canvas.fillet.style')}>
            {STYLES.map((k) => (
              <button key={k} type="button" className="chip" aria-pressed={style === k} onClick={() => patch({ style: k })}>
                {t(`canvas.fillet.${k}`)}
              </button>
            ))}
          </div>
          <LengthField label={style === 'round' ? t('canvas.fillet.radius') : t('canvas.fillet.distance')} value={radius} min={0.01} step={0.5} onCommit={(v) => v > 0 && patch({ radius: v })} />
        </>
      )}
    </div>
  );
}
