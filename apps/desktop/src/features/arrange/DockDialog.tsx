import { useMemo } from 'react';
import { t } from '@/i18n';
import { LengthField } from '@/ui/LengthField';
import { useStore } from '@/store';
import { Check, Choice, Dialog, Footer } from './Dialog';
import { boxOutline, DesignPreview } from './DesignPreview';
import { designReason } from './designOps';
import { useDesignUi } from './designUi';
import { dockDoc, planDock, type DockDirection, type DockMode } from './dock';
import { unitLabel } from './unitLabel';
import { selectionUnits } from './units';

/**
 * Acoplar y mover juntos: una forma de referencia se queda quieta y las demás se deslizan hasta
 * tocarla (en una dirección, o en línea recta hacia ella), dejando el hueco que se pida.
 */
export function DockDialog() {
  const doc = useStore((s) => s.doc);
  const selectedIds = useStore((s) => s.selectedIds);
  const params = useDesignUi((s) => s.dock);
  const patch = useDesignUi((s) => s.patchDock);
  const refId = useDesignUi((s) => s.dockRefId);
  const setRef = useDesignUi((s) => s.setDockRef);
  const close = useDesignUi((s) => s.close);

  const units = useMemo(() => selectionUnits(doc, selectedIds), [doc, selectedIds]);
  const reference = units.find((u) => u.id === refId) ?? units[0];
  const movers = useMemo(() => units.filter((u) => u !== reference), [units, reference]);
  const plan = useMemo(() => (reference ? planDock(reference, movers, params) : null), [reference, movers, params]);

  const reason = designReason({ doc, selectedIds }, 2);
  const nothing = plan !== null && plan.moves.length === 0;
  const issue = reason !== true ? reason : nothing ? 'design.dock.nothing' : null;

  const apply = () => {
    if (issue !== null || !plan) return;
    useStore.getState().applyDocChange('design.history.dock', (d) => dockDoc(d, plan.moves));
    close();
  };

  const after = useMemo(() => {
    const by = new Map(plan?.moves.flatMap((m) => m.ids.map((id) => [id, m] as const)) ?? []);
    return movers.map((u) => {
      const m = by.get(u.id);
      const [dx, dy] = m ? [m.dx, m.dy] : [0, 0];
      return boxOutline([[u.bounds[0][0] + dx, u.bounds[0][1] + dy], [u.bounds[1][0] + dx, u.bounds[1][1] + dy]]);
    });
  }, [movers, plan]);

  return (
    <Dialog
      title={t('design.dock.title')}
      onClose={close}
      wide
      initialFocus="[aria-checked='true']"
      footer={<Footer onClose={close} onApply={apply} disabled={issue !== null} reason={issue ? t(issue) : null} applyLabel={t('design.dock.apply')} />}
    >
      <div className="ar-cols">
        <div className="ar-form">
          <div className="ar-list" role="radiogroup" aria-label={t('design.dock.refPick')}>
            {units.map((u) => (
              <button key={u.id} type="button" role="radio" aria-checked={u === reference} className="ar-item" onClick={() => setRef(u.id)}>
                <span className="ar-item__role" aria-hidden="true">{u === reference ? t('design.dock.roleRef') : t('design.dock.roleMove')}</span>
                <span className="ar-item__name">{unitLabel(u)}</span>
              </button>
            ))}
          </div>
          <Choice<DockMode>
            legend={t('design.dock.mode')}
            value={params.mode}
            options={[
              { value: 'dock', label: t('design.dock.mode.dock') },
              { value: 'together', label: t('design.dock.mode.together') },
            ]}
            onChange={(mode) => patch({ mode })}
          />
          {params.mode === 'dock' && (
            <Choice<DockDirection>
              legend={t('design.dock.direction')}
              value={params.direction}
              options={(['left', 'right', 'up', 'down'] as const).map((d) => ({ value: d, label: t(`design.dock.dir.${d}`) }))}
              onChange={(direction) => patch({ direction })}
            />
          )}
          <LengthField label={t('design.dock.padding')} value={params.padding} min={-1000} max={1000} step={0.5} onCommit={(padding) => patch({ padding })} />
          <Check label={t('design.dock.asGroup')} checked={params.asGroup} onChange={(asGroup) => patch({ asGroup })} />
          <p className="hint">{t(params.mode === 'dock' ? 'design.dock.hintDock' : 'design.dock.hintTogether')}</p>
        </div>
        <div className="ar-preview-box">
          <span className="field__label">{t('design.dock.preview')}</span>
          <DesignPreview
            label={t('design.dock.previewLabel')}
            layers={[
              { outlines: movers.map((u) => boxOutline(u.bounds)), tone: 'original' },
              { outlines: reference ? [boxOutline(reference.bounds)] : [], tone: 'accent' },
              { outlines: after, tone: 'result' },
            ]}
          />
          {plan && plan.skipped > 0 && <p className="hint" role="status">{t('design.dock.skipped', { n: plan.skipped })}</p>}
        </div>
      </div>
    </Dialog>
  );
}
