import { X } from 'lucide-react';
import { useMemo } from 'react';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { ExprField } from './ExprField';
import { editShape, effectiveShape, type ShapeParams, type ShapeType } from './shapeParams';

const TITLES: Record<ShapeType, Parameters<typeof t>[0]> = {
  rect: 'canvas.shape.rect',
  ellipse: 'canvas.shape.ellipse',
  polygon: 'canvas.shape.polygon',
  star: 'canvas.shape.star',
};

interface ViewProps {
  params: ShapeParams;
  /** Hay seleccionados objetos que no son formas editables. */
  mixed: boolean;
  onApply: (patch: Partial<ShapeParams>) => void;
  onClose: () => void;
  onLock: () => void;
}

/** Contenido de la ventana: los campos de cada tipo de forma. */
export function ShapePropsView({ params: first, mixed, onApply, onClose, onLock }: ViewProps) {
  const field = (key: string, label: string, value: number, unit?: string, hintKey?: Parameters<typeof t>[0]) => (
    <ExprField
      key={key}
      label={label}
      unit={unit}
      value={value}
      title={hintKey ? t(hintKey) : undefined}
      onApply={(v) => onApply({ [key]: key === 'ratio' ? v / 100 : v } as Partial<ShapeParams>)}
    />
  );
  return (
    <section className="shape-props" aria-label={t('canvas.shape.title')}>
      <header className="shape-props__head">
        <h2 className="shape-props__title">{t(TITLES[first.type])}</h2>
        <button type="button" className="icon-btn" title={t('canvas.shape.close')} aria-label={t('canvas.shape.close')} onClick={onClose}>
          <X size={14} aria-hidden="true" />
        </button>
      </header>
      <div className="shape-props__grid">
        {first.type === 'rect' && (
          <>
            {field('w', t('canvas.bar.w'), first.w, 'mm')}
            {field('h', t('canvas.bar.h'), first.h, 'mm')}
            {field('radius', t('canvas.bar.radius'), first.radius, 'mm', 'canvas.shape.radiusHint')}
          </>
        )}
        {first.type === 'ellipse' && (
          <>
            {field('w', t('canvas.bar.w'), first.w, 'mm')}
            {field('h', t('canvas.bar.h'), first.h, 'mm')}
          </>
        )}
        {first.type === 'polygon' && (
          <>
            {field('sides', t('canvas.bar.sides'), first.sides, undefined, 'canvas.bar.sidesHint')}
            {field('radius', t('canvas.shape.radius'), first.radius, 'mm', 'canvas.shape.radiusPolyHint')}
            {field('rounding', t('canvas.bar.rounding'), first.rounding, 'mm', 'canvas.bar.roundingHint')}
          </>
        )}
        {first.type === 'star' && (
          <>
            {field('points', t('canvas.bar.points'), first.points, undefined, 'canvas.bar.pointsHint')}
            {field('outer', t('canvas.shape.outer'), first.outer, 'mm')}
            {field('ratio', t('canvas.bar.inner'), Math.round(first.ratio * 1000) / 10, '%', 'canvas.bar.innerHint')}
            {field('rounding', t('canvas.bar.rounding'), first.rounding, 'mm', 'canvas.bar.roundingHint')}
          </>
        )}
      </div>
      <label className="shape-props__check">
        <input type="checkbox" checked={false} onChange={onLock} />
        <span>{t('canvas.shape.locked')}</span>
      </label>
      {mixed && <p className="shape-props__note">{t('canvas.shape.mixed')}</p>}
      <p className="shape-props__note">{t('canvas.shape.note')}</p>
    </section>
  );
}

/**
 * Propiedades de forma: edita los parámetros de rectángulos, elipses, polígonos y estrellas
 * mientras sigan siendo formas (no se les hayan editado los nodos). Con varias formas del mismo
 * tipo el cambio se aplica a todas; con tipos distintos no hay campos comunes y no se muestra.
 */
export function ShapeProps() {
  const open = useStore((s) => s.shapePropsOpen);
  const ids = useStore((s) => s.selectedIds);
  const objects = useStore((s) => s.doc.objects);

  const picked = useMemo(() => {
    const set = new Set(ids);
    return objects.filter((o) => set.has(o.id));
  }, [ids, objects]);
  const editable = useMemo(
    () => picked.flatMap((o) => {
      const params = effectiveShape(o);
      return params ? [{ obj: o, params }] : [];
    }),
    [picked],
  );

  if (!open || picked.length === 0 || editable.length === 0) return null;
  const type = editable[0]!.params.type;
  if (editable.some((x) => x.params.type !== type)) return null;

  const apply = (patch: Partial<ShapeParams>) => {
    const byId = new Map(editable.map((x) => [x.obj.id, editShape(x.obj, patch)]));
    useStore.getState().applyDocChange('canvas.history.shapeProps', (doc) => ({
      ...doc,
      objects: doc.objects.map((o) => byId.get(o.id) ?? o),
    }));
  };
  const lock = () => {
    const set = new Set(ids);
    useStore.getState().applyDocChange('arrange.history.lock', (doc) => ({ ...doc, objects: doc.objects.map((o) => (set.has(o.id) ? { ...o, locked: true } : o)) }));
  };
  return (
    <ShapePropsView
      params={editable[0]!.params}
      mixed={editable.length !== picked.length}
      onApply={apply}
      onClose={() => useStore.getState().setShapePropsOpen(false)}
      onLock={lock}
    />
  );
}
