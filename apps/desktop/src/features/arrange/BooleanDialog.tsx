import { useMemo } from 'react';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { Dialog, Footer } from './Dialog';
import { geometryReason, runGeometry } from './geomOps';
import { useArrangeUi, type BooleanDialogOp } from './uiStore';
import { selectionUnits, unitSize, type Unit } from './units';
import { unitLabel, unitThumbnail } from './unitLabel';

/** Miniatura del contorno en el color de la capa; si la forma no es simple (imagen, texto, muy detallada), solo la muestra de color. */
function UnitThumb({ unit }: { unit: Unit }) {
  const color = unit.objects[0]!.layer;
  const thumb = useMemo(() => unitThumbnail(unit), [unit]);
  if (!thumb) return <span className="ar-swatch" style={{ background: color }} aria-hidden="true" />;
  return (
    <svg className="ar-thumb" viewBox={`0 0 ${thumb.width} ${thumb.height}`} preserveAspectRatio="xMidYMid meet" aria-hidden="true">
      <path d={thumb.d} fill="none" stroke={color} strokeWidth="6" strokeLinejoin="round" strokeLinecap="round" vectorEffect="non-scaling-stroke" />
    </svg>
  );
}

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
              <UnitThumb unit={u} />
              <span className="ar-item__name">{unitLabel(u)}</span>
              <span className="num ar-item__meta">{t('arrange.bool.size', { w: fmt(w), h: fmt(h) })}</span>
            </button>
          );
        })}
      </div>
      {primary && <p className="hint ar-summary">{t('arrange.bool.summary', { a: unitLabel(primary), rest: rest.map(unitLabel).join(', ') })}</p>}
    </Dialog>
  );
}
