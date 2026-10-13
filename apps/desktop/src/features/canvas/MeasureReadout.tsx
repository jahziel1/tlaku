import { useMemo } from 'react';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { formatLength, MM_PER_INCH, type LengthUnit } from '@/lib/units';
import { usePref, useStore } from '@/store';
import { useInteraction, type Measurement } from './interaction';
import { measureBetween, measureShape, type ShapeInfo } from './measure';

/** Longitud en la unidad elegida en preferencias (3 decimales en mm, 4 en pulgadas). */
const len = (v: number, unit: LengthUnit) => formatLength(v, unit, { digits: unit === 'in' ? 4 : 3 });
const area = (v: number, unit: LengthUnit) => (unit === 'in' ? `${fmt(v / (MM_PER_INCH * MM_PER_INCH), 4)} in²` : `${fmt(v, 3)} mm²`);

/** Lo que muestra el panel: la medición y, si se hizo clic en una forma, sus datos. */
export function MeasureView({ measure, info, unit = 'mm' }: { measure: Measurement | null; info: ShapeInfo | null; unit?: LengthUnit }) {
  if (!measure) {
    return (
      <div className="measure-readout" role="status">
        <p className="measure-readout__title">{t('canvas.measure.title')}</p>
        <span style={{ gridColumn: '1 / -1' }}>{t('canvas.measure.hint')}</span>
      </div>
    );
  }
  const rows: [string, string][] = [];
  let title = t('canvas.measure.title');
  if (measure.objectId) {
    if (!info) return null;
    title = t('canvas.measure.shape');
    rows.push([t('canvas.measure.nodes'), String(info.nodes)]);
    rows.push([t('canvas.measure.segments'), t('canvas.measure.segmentsValue', { lines: info.lines, curves: info.curves })]);
    rows.push([t('canvas.measure.size'), `${len(info.size[0], unit)} × ${len(info.size[1], unit)}`]);
    rows.push([t('canvas.measure.perimeter'), len(info.perimeter, unit)]);
    if (info.closed) rows.push([t('canvas.measure.area'), area(info.area, unit)]);
    rows.push([t('canvas.measure.state'), t(info.closed ? 'canvas.measure.closed' : info.open && info.perimeter > 0 ? 'canvas.measure.open' : 'canvas.measure.mixed')]);
    if (info.circle) {
      rows.push([t('canvas.measure.diameter'), len(info.circle.diameter, unit)]);
      rows.push([t('canvas.measure.center'), `${len(info.circle.center[0], unit)}, ${len(info.circle.center[1], unit)}`]);
    }
  } else {
    const m = measureBetween(measure.a, measure.b);
    rows.push([t('canvas.measure.distance'), len(m.distance, unit)]);
    rows.push([t('canvas.measure.angle'), `${fmt(m.angle, 2)}°`]);
    rows.push(['Δx', len(m.dx, unit)]);
    rows.push(['Δy', len(m.dy, unit)]);
  }
  return (
    <dl className="measure-readout" aria-live="polite" aria-label={title}>
      <p className="measure-readout__title">{title}</p>
      {rows.map(([k, v]) => (
        <div key={k} className="measure-readout__row">
          <dt>{k}</dt>
          <dd className="num">{v}</dd>
        </div>
      ))}
    </dl>
  );
}

/** Panel de la herramienta Medir: distancia, ángulo y Δx/Δy de la línea, o los datos de una forma. */
export function MeasureReadout() {
  const tool = useStore((s) => s.activeTool);
  const unit = usePref('units');
  const measure = useInteraction((s) => s.measure);
  const objects = useStore((s) => s.doc.objects);
  const obj = useMemo(() => (measure?.objectId ? objects.find((o) => o.id === measure.objectId) : undefined), [measure, objects]);
  const info = useMemo(() => (obj ? measureShape(obj) : null), [obj]);
  if (tool !== 'measure') return null;
  return <MeasureView measure={measure} info={info} unit={unit} />;
}
