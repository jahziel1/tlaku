import { TriangleAlert } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { getApi } from '@/api';
import type { DesignObject, Point } from '@/api/types';
import { t } from '@/i18n';
import { flattenBez } from '@/lib/bezpath';
import { errorMessage } from '@/lib/errors';
import { objectPolylines, pointsBounds } from '@/lib/geometry';
import { NumberField } from '@/ui/NumberField';
import { useStore } from '@/store';
import { Check, Choice, Dialog, Footer } from './Dialog';
import { geometryReason, operandOf, runGeometry } from './geomOps';
import { useArrangeUi, type JoinStyle } from './uiStore';
import { selectionUnits } from './units';

type Outline = { points: Point[]; closed: boolean };
type Preview = { status: 'computing' } | { status: 'error'; msg: string } | { status: 'ready'; outlines: Outline[] };

const PREVIEW_DELAY_MS = 150;

function resultOutlines(result: DesignObject[]): Outline[] {
  return result.flatMap((o) => (o.kind.type === 'vector' ? o.kind.paths.flatMap((p) => flattenBez(p)) : []));
}

const pathData = (list: Outline[]): string =>
  list.map((p) => `M${p.points.map(([x, y]) => `${x.toFixed(3)} ${y.toFixed(3)}`).join('L')}${p.closed ? 'Z' : ''}`).join('');

/** Offset de la selección: distancia, esquinas, conservar el original y vista previa del contorno. */
export function OffsetDialog() {
  const doc = useStore((s) => s.doc);
  const selectedIds = useStore((s) => s.selectedIds);
  const params = useArrangeUi((s) => s.offset);
  const patch = useArrangeUi((s) => s.patchOffset);
  const busy = useArrangeUi((s) => s.busy);
  const close = useArrangeUi((s) => s.close);
  const [preview, setPreview] = useState<Preview>({ status: 'computing' });

  const units = useMemo(() => selectionUnits(doc, selectedIds), [doc, selectedIds]);
  const original = useMemo(() => units.flatMap((u) => u.objects.flatMap((o) => objectPolylines(o))), [units]);
  const reason = geometryReason({ doc, selectedIds }, 'offset');

  // Vista previa con retardo: cada cambio cancela el cálculo anterior (solo vale el último).
  useEffect(() => {
    if (reason !== true) return;
    let current = true;
    setPreview({ status: 'computing' });
    const timer = setTimeout(() => {
      getApi()
        .geometryOp({ op: 'offset', objects: units.map(operandOf), distance: params.distance, join: params.join })
        .then((r) => current && setPreview({ status: 'ready', outlines: resultOutlines(r) }))
        .catch((e: unknown) => current && setPreview({ status: 'error', msg: errorMessage(e) }));
    }, PREVIEW_DELAY_MS);
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [units, params.distance, params.join, reason]);

  const empty = preview.status === 'ready' && preview.outlines.length === 0;
  const blocked = reason !== true || busy || preview.status !== 'ready' || empty;
  const why = reason !== true ? t(reason) : preview.status === 'error' ? t('arrange.offset.error', { msg: preview.msg }) : empty ? t('arrange.offset.empty') : null;

  const apply = async () => {
    if (await runGeometry(useStore.getState(), { op: 'offset', offset: params })) close();
  };

  const result = preview.status === 'ready' ? preview.outlines : [];
  const box = pointsBounds([...original, ...result].flatMap((p) => p.points));
  const pad = box ? Math.max(box[1][0] - box[0][0], box[1][1] - box[0][1], 1) * 0.08 + Math.abs(params.distance) * 0.25 : 1;

  return (
    <Dialog
      title={t('arrange.offset.title')}
      onClose={close}
      initialFocus=".field__input"
      footer={<Footer onClose={close} onApply={() => void apply()} disabled={blocked} reason={why} applyLabel={busy ? t('arrange.dlg.working') : undefined} />}
    >
      <NumberField label={t('arrange.offset.distance')} value={params.distance} min={-500} max={500} step={0.5} unit={t('arrange.dlg.mm')} onCommit={(distance) => patch({ distance })} />
      <p className="hint">{t('arrange.offset.distanceHint')}</p>
      <Choice<JoinStyle>
        legend={t('arrange.offset.corners')}
        value={params.join}
        options={[
          { value: 'round', label: t('arrange.offset.join.round') },
          { value: 'miter', label: t('arrange.offset.join.miter') },
          { value: 'bevel', label: t('arrange.offset.join.bevel') },
        ]}
        onChange={(join) => patch({ join })}
      />
      <Check label={t('arrange.offset.keep')} checked={params.keepOriginal} onChange={(keepOriginal) => patch({ keepOriginal })} />

      <div className="ar-preview-box">
        <span className="field__label">{t('arrange.offset.preview')}</span>
        {box ? (
          <svg
            className="ar-preview"
            role="img"
            aria-label={t('arrange.offset.previewLabel')}
            viewBox={`${box[0][0] - pad} ${-(box[1][1] + pad)} ${box[1][0] - box[0][0] + 2 * pad} ${box[1][1] - box[0][1] + 2 * pad}`}
            preserveAspectRatio="xMidYMid meet"
          >
            {/* Y hacia arriba en el documento: se voltea para dibujar. */}
            <g transform="scale(1 -1)">
              <path className="ar-preview__original" d={pathData(original)} />
              <path className="ar-preview__result" d={pathData(result)} />
            </g>
          </svg>
        ) : (
          <div className="ar-preview ar-preview--empty" />
        )}
        <div className="ar-legend">
          <span><i className="ar-legend__line ar-legend__line--original" aria-hidden="true" />{t('arrange.offset.legendOriginal')}</span>
          <span><i className="ar-legend__line ar-legend__line--result" aria-hidden="true" />{t('arrange.offset.legendResult')}</span>
          {preview.status === 'computing' && reason === true && <span className="hint" role="status">{t('arrange.dlg.working')}</span>}
        </div>
        {why && reason === true && (
          <p className="warn-line" role="alert">
            <TriangleAlert size={14} aria-hidden="true" />
            <span>{why}</span>
          </p>
        )}
      </div>
    </Dialog>
  );
}
