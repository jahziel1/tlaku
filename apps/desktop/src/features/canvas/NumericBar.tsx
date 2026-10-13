import { useMemo } from 'react';
import { Lock, LockOpen } from 'lucide-react';
import { t, type I18nKey } from '@/i18n';
import { useStore } from '@/store';
import { ExprField } from './ExprField';
import { useInteraction } from './interaction';
import {
  canRotate, boundsOf, frameValues, keepsAxes, numericEdit, selectionRotation, type NumericField,
} from './transform';

const ANCHOR_KEYS = [0, 1, 2, 3, 4, 5, 6, 7, 8].map((i) => `canvas.bar.anchor.${i}` as I18nKey);

function normalizeDeg(d: number): number {
  const r = ((((d + 180) % 360) + 360) % 360) - 180;
  return r === -180 ? 180 : r;
}

/**
 * Barra de edición numérica sobre el lienzo: X, Y, ancho, alto y rotación de la selección, con
 * candado de proporción y un punto de referencia de 9 posiciones (origen de X/Y, ancla de la
 * escala y centro del giro). Los valores siguen en vivo los arrastres del ratón.
 */
export function NumericBar() {
  const ids = useStore((s) => s.selectedIds);
  const objects = useStore((s) => s.doc.objects);
  const anchor = useStore((s) => s.anchor);
  const lockRatio = useStore((s) => s.lockRatio);
  const tool = useStore((s) => s.activeTool);
  const cornerRadius = useStore((s) => s.cornerRadius);
  const live = useInteraction((s) => s.live);

  const selected = useMemo(() => {
    const set = new Set(ids);
    return objects.filter((o) => set.has(o.id) && !o.locked);
  }, [ids, objects]);
  const frame = useMemo(() => boundsOf(selected), [selected]);
  const shownFrame = useMemo(() => (live ? boundsOf(selected, live) : frame), [selected, frame, live]);

  const baseRot = selectionRotation(selected);
  const rotation = normalizeDeg(baseRot + (live && !keepsAxes(live) ? (Math.atan2(live[1], live[0]) * 180) / Math.PI : 0));
  const rotatable = canRotate(selected);
  const empty = selected.length === 0 || !shownFrame;
  const v = shownFrame ? frameValues(shownFrame, anchor) : { x: 0, y: 0, w: 0, h: 0 };
  const noSel = t('reason.noSelection');

  const apply = (field: NumericField, value: number) => {
    const s = useStore.getState();
    if (!frame) return;
    const m = numericEdit(field, value, { bounds: frame, anchor: s.anchor, lockRatio: s.lockRatio, rotation: baseRot });
    if (m) s.transformObjects(selected.map((o) => o.id), m, `numeric:${field}`);
  };

  return (
    <div className="numeric-bar" role="group" aria-label={t('canvas.bar.label')}>
      <div className="anchor-grid" role="radiogroup" aria-label={t('canvas.bar.anchor')}>
        {ANCHOR_KEYS.map((key, i) => (
          <button
            key={key}
            type="button"
            role="radio"
            aria-checked={anchor === i}
            aria-label={t(key)}
            title={t(key)}
            className="anchor-grid__cell"
            onClick={() => useStore.getState().setAnchor(i)}
          />
        ))}
      </div>
      <ExprField label="X" unit="mm" value={v.x} disabled={empty} title={empty ? noSel : t('canvas.bar.hint')} onApply={(x) => apply('x', x)} />
      <ExprField label="Y" unit="mm" value={v.y} disabled={empty} title={empty ? noSel : t('canvas.bar.hint')} onApply={(y) => apply('y', y)} />
      <ExprField label={t('canvas.bar.w')} unit="mm" value={v.w} disabled={empty} title={empty ? noSel : t('canvas.bar.hint')} onApply={(w) => apply('w', w)} />
      <button
        type="button"
        className="icon-btn nbar-lock"
        aria-pressed={lockRatio}
        data-checked={lockRatio}
        title={lockRatio ? t('canvas.bar.lock') : t('canvas.bar.unlock')}
        aria-label={t('canvas.bar.lockLabel')}
        onClick={() => useStore.getState().setLockRatio(!lockRatio)}
      >
        {lockRatio ? <Lock size={14} aria-hidden="true" /> : <LockOpen size={14} aria-hidden="true" />}
      </button>
      <ExprField label={t('canvas.bar.h')} unit="mm" value={v.h} disabled={empty} title={empty ? noSel : t('canvas.bar.hint')} onApply={(h) => apply('h', h)} />
      <ExprField
        label={t('canvas.bar.rot')}
        unit="°"
        mode="angle"
        value={rotation}
        disabled={empty || !rotatable}
        title={empty ? noSel : rotatable ? t('canvas.bar.rotHint') : t('canvas.bar.rotateImage')}
        onApply={(deg) => apply('rot', deg)}
      />
      {tool === 'rect' && (
        <ExprField
          label={t('canvas.bar.radius')}
          unit="mm"
          value={cornerRadius}
          title={t('canvas.bar.radiusHint')}
          onApply={(r) => useStore.getState().setCornerRadius(r)}
        />
      )}
    </div>
  );
}
