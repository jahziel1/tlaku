import { RotateCcw } from 'lucide-react';
import { Modal } from '@/features/machine/Modal';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { NumberField } from '@/ui/NumberField';
import { OPTIMIZATION_GROUPS, OVERLAP_TOLERANCE_RANGE, sameOptimization, DEFAULT_OPTIMIZATION } from './optimization';

/**
 * "Ajustes de optimización": todas las opciones con las que el motor ordena y recorre el trabajo,
 * cada una con su explicación. Los cambios se aplican al instante (la vista previa se recalcula
 * sola) y se recuerdan para la próxima sesión; viajan en `JobOptions` a la vista previa, al
 * encuadre, al envío y a la exportación.
 */
export function OptimizationDialog() {
  const close = useStore((s) => s.closeDialog);
  const opt = useStore((s) => s.jobOptions.optimization);
  const set = useStore((s) => s.setOptimization);
  const reset = useStore((s) => s.resetOptimization);
  const isDefault = sameOptimization(opt, DEFAULT_OPTIMIZATION);

  return (
    <Modal
      title={t('preview.opt.title')}
      subtitle={t('preview.opt.subtitle')}
      size="md"
      onClose={close}
      footer={
        <>
          <button type="button" className="btn" disabled={isDefault} title={isDefault ? t('preview.opt.resetDisabled') : t('preview.opt.resetHint')} onClick={reset}>
            <RotateCcw size={14} aria-hidden="true" />{t('preview.opt.reset')}
          </button>
          <button type="button" className="btn btn--accent" data-autofocus onClick={close}>{t('preview.opt.done')}</button>
        </>
      }
    >
      {OPTIMIZATION_GROUPS.map((g) => (
        <fieldset key={g.title} className="mc-optgroup">
          <legend className="mc-sub">{t(g.title)}</legend>
          {g.flags.map((f) => (
            <label key={f.key} className="mc-opt">
              <input type="checkbox" checked={opt[f.key]} onChange={(e) => set({ [f.key]: e.target.checked })} />
              <span className="mc-opt__text">
                <span className="mc-opt__label">{t(f.label)}</span>
                <span className="hint">{t(f.help)}</span>
              </span>
            </label>
          ))}
          {g.flags.some((f) => f.key === 'remove_overlaps') && (
            <div className="mc-opt__tol">
              <NumberField
                label={t('preview.opt.tolerance')}
                unit="mm"
                value={opt.overlap_tolerance}
                min={OVERLAP_TOLERANCE_RANGE.min}
                max={OVERLAP_TOLERANCE_RANGE.max}
                step={0.01}
                disabled={!opt.remove_overlaps}
                onCommit={(overlap_tolerance) => set({ overlap_tolerance })}
              />
              <p className="hint">{opt.remove_overlaps ? t('preview.opt.tolerance.help') : t('preview.opt.tolerance.off')}</p>
            </div>
          )}
        </fieldset>
      ))}
    </Modal>
  );
}
