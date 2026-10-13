import { useMemo } from 'react';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { Dialog, Footer } from './Dialog';
import { geometryReason, runGeometry } from './geomOps';
import { useArrangeUi, type BooleanDialogOp } from './uiStore';
import { selectionUnits, unitSize, type Unit } from './units';

const unitName = (u: Unit): string => (u.objects.length > 1 ? t('arrange.bool.group', { n: u.objects.length }) + ` · ${u.id}` : u.id);

/**
 * Elige la forma principal (A) de una booleana: A se conserva y el resto (B) actúa sobre ella.
 * Así el resultado no depende de un orden de selección que nadie ve.
 */
export function BooleanDialog({ op }: { op: BooleanDialogOp }) {
  const doc = useStore((s) => s.doc);
  const selectedIds = useStore((s) => s.selectedIds);
  const busy = useArrangeUi((s) => s.busy);
  const primaryId = useArrangeUi((s) => s.primaryId);
  const setPrimary = useArrangeUi((s) => s.setPrimary);
  const close = useArrangeUi((s) => s.close);

  const units = useMemo(() => selectionUnits(doc, selectedIds), [doc, selectedIds]);
  const primary = units.find((u) => u.id === primaryId) ?? units[0];
  const reason = geometryReason({ doc, selectedIds }, op);
  const blocked = reason !== true || busy || !primary;
  const rest = units.filter((u) => u !== primary);

  const apply = async () => {
    if (await runGeometry(useStore.getState(), { op, primaryId: primary?.id })) close();
  };

  return (
    <Dialog
      title={t(`arrange.bool.title.${op}`)}
      onClose={close}
      initialFocus="[aria-checked='true']"
      footer={<Footer onClose={close} onApply={() => void apply()} disabled={blocked} reason={reason === true ? null : t(reason)} applyLabel={busy ? t('arrange.dlg.working') : t('arrange.bool.apply')} />}
    >
      <p className="hint">{t(`arrange.bool.hint.${op}`)}</p>
      <div className="ar-list" role="radiogroup" aria-label={t('arrange.bool.primary')}>
        {units.map((u) => {
          const isA = u === primary;
          const [w, h] = unitSize(u);
          return (
            <button key={u.id} type="button" role="radio" aria-checked={isA} className="ar-item" onClick={() => setPrimary(u.id)}>
              <span className="ar-item__role" aria-hidden="true">{isA ? t('arrange.bool.roleA') : t('arrange.bool.roleB')}</span>
              <span className="ar-swatch" style={{ background: u.objects[0]!.layer }} aria-hidden="true" />
              <span className="ar-item__name">{unitName(u)}</span>
              <span className="num ar-item__meta">{t('arrange.bool.size', { w: fmt(w), h: fmt(h) })}</span>
            </button>
          );
        })}
      </div>
      {primary && <p className="hint ar-summary">{t('arrange.bool.summary', { a: unitName(primary), rest: rest.map(unitName).join(', ') })}</p>}
    </Dialog>
  );
}
