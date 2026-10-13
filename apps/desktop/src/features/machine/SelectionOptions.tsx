import { useStore, selectionBlockedReason } from '@/store';
import { t } from '@/i18n';

/**
 * "Solo la selección" y "Usar el origen de la selección": se muestran en el diálogo de inicio, en
 * el panel del trabajo (encuadre) y en la vista previa, y son la misma opción del store. Sin nada
 * seleccionado se deshabilitan y dicen por qué; el segundo además necesita al primero.
 */
export function SelectionOptions({ compact = false }: { compact?: boolean }) {
  const only = useStore((s) => s.cutSelectionOnly);
  const useOrigin = useStore((s) => s.jobOptions.use_selection_origin);
  const count = useStore((s) => s.selectedIds.length);
  const reasonKey = useStore(selectionBlockedReason);
  const setOnly = useStore((s) => s.setCutSelectionOnly);
  const setOrigin = useStore((s) => s.setUseSelectionOrigin);

  const noSelection = reasonKey !== null;
  const onlyOn = only && !noSelection;
  const originReason = noSelection ? t(reasonKey) : !onlyOn ? t('machine.selection.reason.needOnly') : undefined;

  return (
    <div className={compact ? 'mc-selopts mc-selopts--compact' : 'mc-selopts'}>
      <label className="mc-check" title={noSelection ? t(reasonKey) : t('machine.selection.only.help')}>
        <input type="checkbox" checked={onlyOn} disabled={noSelection} onChange={(e) => setOnly(e.target.checked)} />
        {t('machine.selection.only')}
        {onlyOn && <span className="hint num">({t('machine.selection.count', { n: count })})</span>}
      </label>
      <label className="mc-check" title={originReason ?? t('machine.selection.origin.help')}>
        <input type="checkbox" checked={onlyOn && useOrigin} disabled={originReason !== undefined} onChange={(e) => setOrigin(e.target.checked)} />
        {t('machine.selection.origin')}
      </label>
      {noSelection && <p className="hint">{t(reasonKey)}</p>}
      {!compact && !noSelection && !onlyOn && <p className="hint">{t('machine.selection.only.help')}</p>}
      {!compact && onlyOn && <p className="hint">{useOrigin ? t('machine.selection.origin.on') : t('machine.selection.origin.off')}</p>}
    </div>
  );
}
