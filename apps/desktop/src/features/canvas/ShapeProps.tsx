import { X } from 'lucide-react';
import { useMemo } from 'react';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { objectsByIds } from '@/store/objectIndex';
import { ExprField } from './ExprField';
import { commonValue, setCutProps, type CutPatch } from './cutProps';
import { OptionalField } from './OptionalField';
import { editShape, effectiveShape, type ShapeSpec, type ShapeType } from '@/lib/paramShape';

const TITLES: Record<ShapeType, Parameters<typeof t>[0]> = {
  rect: 'canvas.shape.rect',
  ellipse: 'canvas.shape.ellipse',
  polygon: 'canvas.shape.polygon',
  star: 'canvas.shape.star',
};

/** Prioridad y escala de potencia de la selección (valor común o `'mixed'`). */
export interface CutValues {
  priority: number | null | 'mixed';
  power: number | null | 'mixed';
}

interface ViewProps {
  /** Parámetros de la forma seleccionada, o `null` si la selección no son formas del mismo tipo. */
  params: ShapeSpec | null;
  /** Hay seleccionados objetos que no son formas editables. */
  mixed: boolean;
  /** Propiedades de corte de la selección. */
  cut?: CutValues;
  onApply: (patch: Partial<ShapeSpec>) => void;
  onCut?: (patch: CutPatch) => void;
  onClose: () => void;
  onLock: () => void;
}

/** Contenido de la ventana: los campos de cada tipo de forma. */
export function ShapePropsView({ params: first, mixed, cut, onApply, onCut, onClose, onLock }: ViewProps) {
  const field = (key: string, label: string, value: number, unit?: string, hintKey?: Parameters<typeof t>[0]) => (
    <ExprField
      key={key}
      label={label}
      unit={unit}
      value={value}
      title={hintKey ? t(hintKey) : undefined}
      onApply={(v) => onApply({ [key]: key === 'ratio' ? v / 100 : v } as Partial<ShapeSpec>)}
    />
  );
  return (
    <section className="shape-props" aria-label={first ? t('canvas.shape.title') : t('canvas.obj.title')}>
      <header className="shape-props__head">
        <h2 className="shape-props__title">{first ? t(TITLES[first.type]) : t('canvas.obj.title')}</h2>
        <button type="button" className="icon-btn" title={t('canvas.shape.close')} aria-label={t('canvas.shape.close')} onClick={onClose}>
          <X size={14} aria-hidden="true" />
        </button>
      </header>
      {first && (
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
      )}
      {cut && onCut && (
        <>
          <h3 className="shape-props__sub">{t('canvas.obj.cut')}</h3>
          <div className="shape-props__grid">
            <OptionalField
              label={t('canvas.obj.priority')}
              value={cut.priority}
              placeholder={t('canvas.obj.auto')}
              title={t('canvas.obj.priorityHint')}
              onApply={(v) => onCut({ priority: v })}
            />
            <OptionalField
              label={t('canvas.obj.powerScale')}
              value={cut.power}
              placeholder="100"
              unit="%"
              title={t('canvas.obj.powerScaleHint')}
              onApply={(v) => onCut({ power_scale: v })}
            />
          </div>
          <p className="shape-props__note">{t('canvas.obj.help')}</p>
        </>
      )}
      <label className="shape-props__check">
        <input type="checkbox" checked={false} onChange={onLock} />
        <span>{t('canvas.shape.locked')}</span>
      </label>
      {first && mixed && <p className="shape-props__note">{t('canvas.shape.mixed')}</p>}
      {first && <p className="shape-props__note">{t('canvas.shape.note')}</p>}
    </section>
  );
}

/**
 * Propiedades del objeto: parámetros de rectángulos, elipses, polígonos y estrellas mientras sigan
 * siendo formas (no se les hayan editado los nodos), y la prioridad de corte y la escala de
 * potencia de cualquier selección. Con varias formas del mismo tipo el cambio se aplica a todas;
 * con tipos distintos no hay campos de forma comunes y solo se muestran los de corte.
 */
export function ShapeProps() {
  const open = useStore((s) => s.shapePropsOpen);
  const ids = useStore((s) => s.selectedIds);
  const objects = useStore((s) => s.doc.objects);

  const picked = useMemo(() => {
    return objectsByIds(objects, ids);
  }, [ids, objects]);
  const editable = useMemo(
    () => picked.flatMap((o) => {
      const params = effectiveShape(o);
      return params ? [{ obj: o, params }] : [];
    }),
    [picked],
  );
  const cut = useMemo<CutValues>(
    () => ({ priority: commonValue(picked, (o) => o.priority), power: commonValue(picked, (o) => o.power_scale) }),
    [picked],
  );

  if (!open || picked.length === 0) return null;
  const type = editable[0]?.params.type;
  const sameType = editable.length > 0 && editable.every((x) => x.params.type === type);
  const shapeParams = sameType ? editable[0]!.params : null;

  const apply = (patch: Partial<ShapeSpec>) => {
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
      params={shapeParams}
      mixed={editable.length !== picked.length}
      cut={cut}
      onApply={apply}
      onCut={(patch) => setCutProps(useStore.getState(), ids, patch)}
      onClose={() => useStore.getState().setShapePropsOpen(false)}
      onLock={lock}
    />
  );
}
